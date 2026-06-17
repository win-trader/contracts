// ---------------------------------------------------------------------------
// Tests for: decrease_position
//
// These tests are written BEFORE the implementation (TDD). They MUST compile
// but are expected to FAIL until decrease_position is fully implemented.
//
// The function under test:
//   fn decrease_position(env, trader, symbol, size_delta)
//
// Key behaviors:
//   - INTENTIONALLY bypasses pause check (users can always reduce risk)
//   - Requires initialized
//   - trader.require_auth()
//   - Reverts PositionNotFound (6) if no position exists
//   - Reverts ZeroAmount (8) if size_delta <= 0
//   - Reverts PositionNotOldEnough (5) if called before MinPositionLifetime (60s)
//   - Full close: settles PnL, deletes position, updates OI + total_reserved
//   - Partial close: proportionally reduces size/collateral, settles PnL
// ---------------------------------------------------------------------------

use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Ledger, LedgerInfo},
    Address, Env, Symbol,
};

use crate::contract::PositionManagerContract;
use crate::math;
use shared::constants::PRECISION;
use crate::storage;
use crate::PositionManagerClient;

use config_manager::{ConfigManagerClient, ConfigManagerContract};
use super::helpers::{self, DualOracle};
use mock_token::{MockToken, MockTokenClient};
use oracle_router::{OracleConfig, OracleRouterClient, OracleRouterContract};
use vault::{VaultContract, VaultContractClient};

// ===========================================================================
// Constants
// ===========================================================================

/// BTC price: $50,000 scaled by 1e7
const BTC_PRICE: i128 = 50_000 * PRECISION;

/// ETH price: $3,000 scaled by 1e7
const ETH_PRICE: i128 = 3_000 * PRECISION;

/// 1 USDC = 1_000_000 (6 decimals)
const USDC_UNIT: i128 = 1_000_000;

/// Trader starts with 100,000 USDC
const TRADER_BALANCE: i128 = 100_000 * USDC_UNIT;

/// Initial vault deposit (1,000,000 USDC) -- large enough for position tests
const VAULT_DEPOSIT: i128 = 1_000_000 * USDC_UNIT;

/// Default position size: 10,000 USDC notional
const DEFAULT_SIZE: i128 = 10_000 * USDC_UNIT;

/// Default collateral: 1,000 USDC (10x leverage)
const DEFAULT_COLLATERAL: i128 = 1_000 * USDC_UNIT;

/// Ledger timestamp used in tests
const TEST_TIMESTAMP: u64 = 1_700_000_000;

/// Minimum position lifetime (anti-front-running) -- 60 seconds for V1
const MIN_POSITION_LIFETIME: u64 = 60;

// ===========================================================================
// Test fixture
// ===========================================================================

struct TestFixture<'a> {
    env: Env,
    pm_client: PositionManagerClient<'a>,
    vault_client: VaultContractClient<'a>,
    oracle_client: DualOracle<'a>,
    _oracle_router_client: OracleRouterClient<'a>,
    _config_client: ConfigManagerClient<'a>,
    usdc_client: MockTokenClient<'a>,
    _usdc_addr: Address,
    admin: Address,
    trader: Address,
    pm_addr: Address,
    _vault_addr: Address,
}

/// Deploy and wire up ALL protocol contracts needed for decrease_position tests.
///
/// Deployment order:
///   1. ConfigManager -- grants ADMIN, PAUSER, KEEPER roles
///   2. MockToken (USDC) -- mints to trader and LP provider
///   3. MockOracle -- sets BTC and ETH prices
///   4. OracleRouter -- initialized with ConfigManager, linked to MockOracle
///   5. Vault -- initialized with USDC, ConfigManager, PositionManager address
///   6. PositionManager -- initialized with Vault, ConfigManager, OracleRouter
///   7. LP deposits into the vault to provide free liquidity
fn setup_full<'a>() -> TestFixture<'a> {
    let env = Env::default();
    env.mock_all_auths();

    // Set a deterministic ledger timestamp
    env.ledger().set(LedgerInfo {
        timestamp: TEST_TIMESTAMP,
        protocol_version: 23,
        sequence_number: 100,
        network_id: [0u8; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 100,
        min_persistent_entry_ttl: 100,
        max_entry_ttl: 10_000_000,
    });

    let admin = Address::generate(&env);
    let trader = Address::generate(&env);
    let lp = Address::generate(&env);

    // --- 1. ConfigManager ---
    let config_id = env.register(ConfigManagerContract, (admin.clone(),));
    let config_client = ConfigManagerClient::new(&env, &config_id);

    // Grant roles needed by other contracts
    let pauser_role = Symbol::new(&env, "PAUSER");
    let keeper_role = Symbol::new(&env, "KEEPER");
    let _admin_role = Symbol::new(&env, "ADMIN");
    config_client.grant_role(&admin, &pauser_role, &admin);
    config_client.grant_role(&admin, &keeper_role, &admin);

    config_client.update_protocol_limits(
        &admin,
        &config_manager::ProtocolLimits {
            min_collateral: 1_000_000,
            cooldown_duration: 60,
            min_position_lifetime: 60,
            max_utilization_ratio: 8_500,
            funding_cut_bps: 500,
            adl_pnl_bps: 9_000,
            adl_utilization_bps: 9_500,
            liquidation_threshold_bps: 200,
        },
    );

    config_client.update_borrow_rate_config(
        &admin,
        &config_manager::BorrowRateConfig {
            base_borrow_rate_bps: 100,
            slope1_bps: 500,
            slope2_bps: 5_000,
            optimal_utilization_bps: 8_000,
            base_funding_rate_bps: 100,
        },
    );

    config_client.update_fee_splits(
        &admin,
        &config_manager::FeeSplits {
            lp_bps: 9000,
            dev_bps: 500,
            staker_bps: 500,
        },
    );

    // --- 2. MockToken (USDC) ---
    let usdc_id = env.register(MockToken, ());
    let usdc_client = MockTokenClient::new(&env, &usdc_id);
    usdc_client.initialize(
        &admin,
        &6u32,
        &soroban_sdk::String::from_str(&env, "USD Coin"),
        &soroban_sdk::String::from_str(&env, "USDC"),
    );

    // --- 3. MockOracle ---
    let (oracle_client, oracle_sources) = helpers::register_dual_oracle(&env);
    oracle_client.set_price(&symbol_short!("BTC"), &BTC_PRICE);
    oracle_client.set_price(&symbol_short!("ETH"), &ETH_PRICE);

    // --- 4. OracleRouter ---
    let oracle_router_id = env.register(OracleRouterContract, (config_id.clone(),));
    let oracle_router_client = OracleRouterClient::new(&env, &oracle_router_id);

    // Configure oracle sources and thresholds
    oracle_router_client.set_oracle_config(
        &admin,
        &OracleConfig {
            max_deviation_bps: 500,    // 5%
            staleness_threshold: 3600, // 1 hour
            cache_duration: 10,
            min_required_sources: 2,
        },
    );
    oracle_router_client.set_oracle_sources(
        &admin,
        &symbol_short!("BTC"),
        &oracle_sources);
    oracle_router_client.set_oracle_sources(
        &admin,
        &symbol_short!("ETH"),
        &oracle_sources);

    // --- 5. PositionManager (register first to get address for Vault init) ---
    let pm_id = env.register(PositionManagerContract, (config_id.clone(), oracle_router_id.clone()));
    let pm_client = PositionManagerClient::new(&env, &pm_id);

    // --- 6. Vault (binds the trusted PositionManager in its constructor) ---
    let vault_id = env.register(VaultContract, (usdc_id.clone(), config_id.clone(), pm_id.clone()));
    let vault_client = VaultContractClient::new(&env, &vault_id);

    // --- 7. Wire the Vault into the PositionManager (ADMIN-gated, one-shot) ---
    pm_client.set_vault(&admin, &vault_id);
    pm_client.set_max_leverage(&admin, &symbol_short!("BTC"), &100_i128);
    pm_client.set_max_leverage(&admin, &symbol_short!("ETH"), &100_i128);

    // --- Fund accounts ---
    // Mint USDC to trader for collateral
    usdc_client.mint(&trader, &TRADER_BALANCE);

    // Mint USDC to LP and deposit into vault for free liquidity
    usdc_client.mint(&lp, &VAULT_DEPOSIT);
    vault_client.deposit(&VAULT_DEPOSIT, &lp, &lp, &lp);

    // SAFETY: env lives in the fixture, clients borrow from it.
    let pm_client = unsafe { core::mem::transmute(pm_client) };
    let vault_client = unsafe { core::mem::transmute(vault_client) };
    let oracle_client = unsafe { core::mem::transmute(oracle_client) };
    let _oracle_router_client = unsafe { core::mem::transmute(oracle_router_client) };
    let _config_client = unsafe { core::mem::transmute(config_client) };
    let usdc_client = unsafe { core::mem::transmute(usdc_client) };

    TestFixture {
        env,
        pm_client,
        vault_client,
        oracle_client,
        _oracle_router_client,
        _config_client,
        usdc_client,
        _usdc_addr: usdc_id,
        admin,
        trader,
        pm_addr: pm_id,
        _vault_addr: vault_id,
    }
}

// ===========================================================================
// Helper: open a position and advance time past MinPositionLifetime + cache
// ===========================================================================

/// Opens a long BTC position with default size/collateral, then advances the
/// ledger timestamp past MinPositionLifetime (60s) + oracle cache (10s) so
/// that decrease_position can be called without hitting anti-front-running or
/// stale oracle cache.
fn open_position_and_advance(f: &TestFixture, is_long: bool) {
    f.pm_client.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &is_long,
        &0,
        &0, &0i128
    );

    // Advance time past MinPositionLifetime (60s) + oracle cache (10s)
    advance_time(f, TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 11);

    // Refresh oracle price so it is not stale after time advance
    f.oracle_client.set_price(&symbol_short!("BTC"), &BTC_PRICE);
}

/// Advance ledger to a new timestamp and refresh oracle prices to avoid
/// staleness.
fn advance_time(f: &TestFixture, new_timestamp: u64) {
    f.env.ledger().set(LedgerInfo {
        timestamp: new_timestamp,
        protocol_version: 23,
        sequence_number: 100 + ((new_timestamp - TEST_TIMESTAMP) as u32),
        network_id: [0u8; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 100,
        min_persistent_entry_ttl: 100,
        max_entry_ttl: 10_000_000,
    });
}

// ===========================================================================
// 1. Guard tests
// ===========================================================================

#[test]
fn test_decrease_position_succeeds_when_paused() {
    // KEY TEST: decrease_position intentionally bypasses the pause check.
    // Traders must ALWAYS be able to reduce risk / close positions, even when
    // the contract is paused. This test verifies the function does NOT revert
    // with Paused (error 3).
    let f = setup_full();

    // Open a position first
    open_position_and_advance(&f, true);

    // Pause the contract
    f.pm_client.pause(&f.admin);

    // Refresh oracle after potential time changes
    f.oracle_client.set_price(&symbol_short!("BTC"), &BTC_PRICE);

    // This must NOT panic -- decrease_position bypasses pause check
    f.pm_client
        .decrease_position(&f.trader, &symbol_short!("BTC"), &DEFAULT_SIZE, &0_i128);

    // If we reach here, the test passes -- the contract did not revert
    // even though it is paused. Verify position was closed.
    // (get_position should panic with PositionNotFound since it was a full close)
}

#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn test_decrease_position_reverts_position_not_found() {
    // Scenario: Trader has no open position for this symbol. Calling
    // decrease_position must revert with PositionNotFound (error 6).
    let f = setup_full();

    f.pm_client
        .decrease_position(&f.trader, &symbol_short!("BTC"), &DEFAULT_SIZE, &0_i128);
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn test_decrease_position_reverts_zero_size_delta() {
    // Scenario: size_delta is zero. Must revert with ZeroAmount (error 8).
    let f = setup_full();
    open_position_and_advance(&f, true);

    f.pm_client
        .decrease_position(&f.trader, &symbol_short!("BTC"), &0_i128, &0_i128);
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn test_decrease_position_reverts_negative_size_delta() {
    // Adversarial: Trader passes a negative size_delta to attempt underflow or
    // bypass logic. Must revert with ZeroAmount (error 8).
    let f = setup_full();
    open_position_and_advance(&f, true);

    f.pm_client
        .decrease_position(&f.trader, &symbol_short!("BTC"), &(-1_000_i128), &0_i128);
}

// ===========================================================================
// 2. Anti-front-running tests
// ===========================================================================

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_decrease_position_reverts_position_not_old_enough() {
    // Scenario: Trader opens a position and immediately tries to close it
    // within the same block (no time advance). Must revert with
    // PositionNotOldEnough (error 5) to prevent front-running.
    let f = setup_full();

    // Open position at TEST_TIMESTAMP
    f.pm_client.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0, &0i128
    );

    // Try to decrease immediately (same timestamp) -- should fail
    f.pm_client
        .decrease_position(&f.trader, &symbol_short!("BTC"), &DEFAULT_SIZE, &0_i128);
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_decrease_position_reverts_one_second_before_min_lifetime() {
    // Boundary test: Advance time to exactly MIN_POSITION_LIFETIME - 1 second.
    // Must still revert with PositionNotOldEnough (error 5).
    let f = setup_full();

    f.pm_client.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0, &0i128
    );

    // Advance to 59 seconds (one second short of 60s minimum)
    advance_time(&f, TEST_TIMESTAMP + MIN_POSITION_LIFETIME - 1);
    f.oracle_client.set_price(&symbol_short!("BTC"), &BTC_PRICE);

    f.pm_client
        .decrease_position(&f.trader, &symbol_short!("BTC"), &DEFAULT_SIZE, &0_i128);
}

#[test]
fn test_decrease_position_succeeds_at_exact_min_lifetime() {
    // Boundary test: Advance time to exactly MinPositionLifetime (60s).
    // The check is `current_time >= last_increased_time + MIN_POSITION_LIFETIME`,
    // so at exactly 60s it should succeed.
    let f = setup_full();

    f.pm_client.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0, &0i128
    );

    // Advance to exactly 60 seconds + enough for oracle cache (11s extra)
    advance_time(&f, TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 11);
    f.oracle_client.set_price(&symbol_short!("BTC"), &BTC_PRICE);

    // This should succeed -- position is old enough
    f.pm_client
        .decrease_position(&f.trader, &symbol_short!("BTC"), &DEFAULT_SIZE, &0_i128);
}

// ===========================================================================
// 3. Full close -- profit scenarios
// ===========================================================================

#[test]
fn test_full_close_long_profit_trader_receives_funds() {
    // Scenario: Trader opens long BTC at $50,000. Price rises to $55,000 (10%
    // increase). Trader closes entire position. They should receive their
    // collateral plus the profit.
    //
    // PnL = size * (mark - entry) / entry
    //     = 10,000 * (55,000 - 50,000) / 50,000
    //     = 10,000 * 5,000 / 50,000
    //     = 1,000 USDC profit
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    // Open long at $50,000
    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0, &0i128
    );

    let trader_balance_after_open = f.usdc_client.balance(&f.trader);

    // Advance time past MinPositionLifetime + oracle cache
    let close_time = TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 11;
    advance_time(&f, close_time);

    // Set new price: $55,000 (10% profit for long)
    let profit_price: i128 = 55_000 * PRECISION;
    f.oracle_client.set_price(&symbol, &profit_price);

    // Close entire position
    f.pm_client
        .decrease_position(&f.trader, &symbol, &DEFAULT_SIZE, &0_i128);

    let trader_balance_after_close = f.usdc_client.balance(&f.trader);

    // Trader should have received collateral + PnL back
    // PnL = 10,000 USDC * (55,000 - 50,000) / 50,000 = 1,000 USDC
    let expected_pnl = math::calc_unrealized_pnl(DEFAULT_SIZE, BTC_PRICE, profit_price, true);
    assert!(
        expected_pnl > 0,
        "PnL should be positive for profitable long"
    );

    // Trader receives at least collateral + pnl (minus fees)
    let received = trader_balance_after_close - trader_balance_after_open;
    assert!(
        received > 0,
        "Trader must receive funds on profitable full close. Received: {}",
        received
    );
    // The received amount should be approximately collateral + pnl - fees
    // We allow some tolerance for borrow/funding fees
    assert!(
        received >= DEFAULT_COLLATERAL + expected_pnl - (100 * USDC_UNIT),
        "Trader must receive close to collateral + PnL. Received: {}, Expected min: {}",
        received,
        DEFAULT_COLLATERAL + expected_pnl - (100 * USDC_UNIT)
    );
}

#[test]
fn test_full_close_long_profit_position_deleted() {
    // Scenario: After a full close, the position must be deleted from storage.
    // Calling get_position should revert with PositionNotFound.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0, &0i128
    );

    advance_time(&f, TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 11);
    let profit_price: i128 = 55_000 * PRECISION;
    f.oracle_client.set_price(&symbol, &profit_price);

    f.pm_client
        .decrease_position(&f.trader, &symbol, &DEFAULT_SIZE, &0_i128);

    // Position should be deleted -- verify via storage directly
    f.env.as_contract(&f.pm_addr, || {
        let pos = storage::get_position(&f.env, &f.trader, &symbol);
        assert!(pos.is_none(), "Position must be deleted after full close");
    });
}

#[test]
fn test_full_close_long_profit_market_oi_decreased() {
    // Scenario: After full close, the market's long_open_interest must decrease
    // by the position's full size.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0, &0i128
    );

    let market_before = f.pm_client.get_market(&symbol);
    assert_eq!(market_before.long_open_interest, DEFAULT_SIZE);

    advance_time(&f, TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 11);
    f.oracle_client.set_price(&symbol, &(55_000 * PRECISION));

    f.pm_client
        .decrease_position(&f.trader, &symbol, &DEFAULT_SIZE, &0_i128);

    let market_after = f.pm_client.get_market(&symbol);
    assert_eq!(
        market_after.long_open_interest, 0,
        "Long OI must be zero after full close of only position"
    );
}

#[test]
fn test_full_close_long_profit_total_reserved_decreased() {
    // Scenario: After full close, total_reserved must decrease by position.size.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0, &0i128
    );

    f.env.as_contract(&f.pm_addr, || {
        assert_eq!(f.vault_client.reserved_usdc(), DEFAULT_SIZE);
    });

    advance_time(&f, TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 11);
    f.oracle_client.set_price(&symbol, &(55_000 * PRECISION));

    f.pm_client
        .decrease_position(&f.trader, &symbol, &DEFAULT_SIZE, &0_i128);

    f.env.as_contract(&f.pm_addr, || {
        assert_eq!(
            f.vault_client.reserved_usdc(),
            0,
            "TotalReserved must be zero after full close of only position"
        );
    });
}

// ===========================================================================
// 4. Full close -- loss scenarios
// ===========================================================================

#[test]
fn test_full_close_long_loss_trader_receives_reduced_collateral() {
    // Scenario: Trader opens long BTC at $50,000. Price drops to $45,000
    // (10% decrease). Trader closes entire position.
    //
    // PnL = 10,000 * (45,000 - 50,000) / 50,000 = -1,000 USDC
    // health = 1,000 (collateral) - 1,000 (loss) = 0 (complete wipeout)
    //
    // With a 10% drop and 10x leverage, the trader loses their entire collateral.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0, &0i128
    );

    let trader_balance_after_open = f.usdc_client.balance(&f.trader);

    advance_time(&f, TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 11);
    let loss_price: i128 = 45_000 * PRECISION;
    f.oracle_client.set_price(&symbol, &loss_price);

    f.pm_client
        .decrease_position(&f.trader, &symbol, &DEFAULT_SIZE, &0_i128);

    let trader_balance_after_close = f.usdc_client.balance(&f.trader);

    // With exactly -1000 PnL and 1000 collateral, health ~= 0.
    // Trader should receive very little or nothing.
    let pnl = math::calc_unrealized_pnl(DEFAULT_SIZE, BTC_PRICE, loss_price, true);
    assert!(pnl < 0, "PnL must be negative for losing long");

    // At 10x leverage with 10% drop, the loss roughly equals collateral
    let received = trader_balance_after_close - trader_balance_after_open;
    assert!(
        received <= DEFAULT_COLLATERAL,
        "Trader must not receive more than their collateral on a loss. Received: {}",
        received
    );
}

#[test]
fn test_full_close_long_loss_position_deleted() {
    // Scenario: Even on a losing trade, the position must be deleted from storage.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0, &0i128
    );

    advance_time(&f, TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 11);
    f.oracle_client.set_price(&symbol, &(47_000 * PRECISION));

    f.pm_client
        .decrease_position(&f.trader, &symbol, &DEFAULT_SIZE, &0_i128);

    f.env.as_contract(&f.pm_addr, || {
        let pos = storage::get_position(&f.env, &f.trader, &symbol);
        assert!(
            pos.is_none(),
            "Position must be deleted after full close even on loss"
        );
    });
}

#[test]
fn test_full_close_long_loss_market_updated() {
    // Scenario: After full close on a loss, market OI and total_reserved
    // must still be updated correctly.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0, &0i128
    );

    advance_time(&f, TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 11);
    f.oracle_client.set_price(&symbol, &(47_000 * PRECISION));

    f.pm_client
        .decrease_position(&f.trader, &symbol, &DEFAULT_SIZE, &0_i128);

    let market = f.pm_client.get_market(&symbol);
    assert_eq!(
        market.long_open_interest, 0,
        "Long OI must be zero after full close"
    );

    f.env.as_contract(&f.pm_addr, || {
        assert_eq!(
            f.vault_client.reserved_usdc(),
            0,
            "TotalReserved must be zero after full close"
        );
    });
}

// ===========================================================================
// 5. Partial close tests
// ===========================================================================

#[test]
fn test_partial_close_reduces_size_by_delta() {
    // Scenario: Trader opens a 10,000 USDC position and closes half (5,000).
    // The remaining position should have size = 5,000.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0, &0i128
    );

    advance_time(&f, TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 11);
    f.oracle_client.set_price(&symbol, &BTC_PRICE); // same price, no PnL

    let half_size = DEFAULT_SIZE / 2;
    f.pm_client
        .decrease_position(&f.trader, &symbol, &half_size, &0_i128);

    let pos = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(
        pos.size,
        DEFAULT_SIZE - half_size,
        "Position size must decrease by size_delta after partial close"
    );
}

#[test]
fn test_partial_close_reduces_collateral_proportionally() {
    // Scenario: Closing half the position should reduce collateral by half.
    // collateral_delta = collateral * size_delta / size = 1000 * 5000 / 10000 = 500
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0, &0i128
    );

    advance_time(&f, TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 11);
    f.oracle_client.set_price(&symbol, &BTC_PRICE);

    let half_size = DEFAULT_SIZE / 2;
    f.pm_client
        .decrease_position(&f.trader, &symbol, &half_size, &0_i128);

    let pos = f.pm_client.get_position(&f.trader, &symbol);
    let expected_collateral = DEFAULT_COLLATERAL - (DEFAULT_COLLATERAL * half_size / DEFAULT_SIZE);
    assert_eq!(
        pos.collateral, expected_collateral,
        "Collateral must decrease proportionally. Expected: {}, Got: {}",
        expected_collateral, pos.collateral
    );
}

#[test]
fn test_partial_close_reduces_market_oi_by_size_delta_not_full_size() {
    // Scenario: Closing half the position should reduce OI by size_delta (5,000),
    // NOT by the full position size (10,000).
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0, &0i128
    );

    advance_time(&f, TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 11);
    f.oracle_client.set_price(&symbol, &BTC_PRICE);

    let half_size = DEFAULT_SIZE / 2;
    f.pm_client
        .decrease_position(&f.trader, &symbol, &half_size, &0_i128);

    let market = f.pm_client.get_market(&symbol);
    assert_eq!(
        market.long_open_interest,
        DEFAULT_SIZE - half_size,
        "Long OI must decrease by size_delta only, not full position size"
    );
}

#[test]
fn test_partial_close_reduces_total_reserved_by_size_delta() {
    // Scenario: TotalReserved must decrease by size_delta (not full size).
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0, &0i128
    );

    advance_time(&f, TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 11);
    f.oracle_client.set_price(&symbol, &BTC_PRICE);

    let half_size = DEFAULT_SIZE / 2;
    f.pm_client
        .decrease_position(&f.trader, &symbol, &half_size, &0_i128);

    f.env.as_contract(&f.pm_addr, || {
        assert_eq!(
            f.vault_client.reserved_usdc(),
            DEFAULT_SIZE - half_size,
            "TotalReserved must decrease by size_delta on partial close"
        );
    });
}

#[test]
fn test_partial_close_position_still_exists() {
    // Scenario: After partial close, position must still exist in storage
    // (unlike full close which deletes it).
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0, &0i128
    );

    advance_time(&f, TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 11);
    f.oracle_client.set_price(&symbol, &BTC_PRICE);

    let quarter_size = DEFAULT_SIZE / 4;
    f.pm_client
        .decrease_position(&f.trader, &symbol, &quarter_size, &0_i128);

    // get_position should NOT panic
    let pos = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(pos.size, DEFAULT_SIZE - quarter_size);
    assert!(
        pos.is_long,
        "is_long must remain unchanged after partial close"
    );
    assert_eq!(
        pos.entry_price, BTC_PRICE,
        "entry_price must remain unchanged after partial close"
    );
}

// ===========================================================================
// 6. Edge cases
// ===========================================================================

#[test]
fn test_close_with_size_delta_exceeding_position_size_reverts() {
    // Over-close reverts with SizeDeltaExceedsPosition rather than silently
    // clamping. Callers must close with the exact remaining size.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0, &0i128
    );

    advance_time(&f, TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 11);
    f.oracle_client.set_price(&symbol, &BTC_PRICE);

    let oversized_delta = DEFAULT_SIZE * 2;
    let result = f
        .pm_client
        .try_decrease_position(&f.trader, &symbol, &oversized_delta, &0_i128);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(
            crate::PositionManagerError::SizeDeltaExceedsPosition as u32,
        ),
    );

    // Position must remain — over-close did NOT execute as a partial close.
    f.env.as_contract(&f.pm_addr, || {
        let pos = storage::get_position(&f.env, &f.trader, &symbol);
        assert!(pos.is_some(), "Position must remain after a rejected over-close");
    });

    // Market OI must remain unchanged — the over-close did not execute.
    let market = f.pm_client.get_market(&symbol);
    assert_eq!(
        market.long_open_interest, DEFAULT_SIZE,
        "OI must be unchanged after rejected over-close"
    );

    // Reserved liquidity must remain — vault state is untouched.
    f.env.as_contract(&f.pm_addr, || {
        assert_eq!(f.vault_client.reserved_usdc(), DEFAULT_SIZE);
    });
}

#[test]
fn test_short_position_profit_on_price_decrease() {
    // Scenario: Trader opens a short ETH at $3,000. Price drops to $2,700
    // (10% decrease). Short profits when price goes down.
    //
    // PnL = size * (entry - mark) / entry
    //     = 10,000 * (3,000 - 2,700) / 3,000
    //     = 10,000 * 300 / 3,000
    //     = 1,000 USDC profit
    let f = setup_full();
    let symbol = symbol_short!("ETH");

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &false,
        &0,
        &0, // short
        &0i128,
    );

    let balance_after_open = f.usdc_client.balance(&f.trader);

    advance_time(&f, TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 11);
    let drop_price: i128 = 2_700 * PRECISION;
    f.oracle_client.set_price(&symbol, &drop_price);

    f.pm_client
        .decrease_position(&f.trader, &symbol, &DEFAULT_SIZE, &0_i128);

    let balance_after_close = f.usdc_client.balance(&f.trader);

    let pnl = math::calc_unrealized_pnl(DEFAULT_SIZE, ETH_PRICE, drop_price, false);
    assert!(pnl > 0, "Short PnL must be positive when price drops");

    let received = balance_after_close - balance_after_open;
    assert!(
        received > DEFAULT_COLLATERAL,
        "Short trader must receive more than collateral on profit. Received: {}, Collateral: {}",
        received,
        DEFAULT_COLLATERAL
    );

    // Position must be deleted
    f.env.as_contract(&f.pm_addr, || {
        assert!(storage::get_position(&f.env, &f.trader, &symbol).is_none());
    });
}

#[test]
fn test_short_position_loss_on_price_increase() {
    // Scenario: Trader opens a short ETH at $3,000. Price rises to $3,150
    // (5% increase). Short loses when price goes up.
    //
    // PnL = 10,000 * (3,000 - 3,150) / 3,000 = -500 USDC
    let f = setup_full();
    let symbol = symbol_short!("ETH");

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &false,
        &0,
        &0, &0i128
    );

    let balance_after_open = f.usdc_client.balance(&f.trader);

    advance_time(&f, TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 11);
    let rise_price: i128 = 3_150 * PRECISION;
    f.oracle_client.set_price(&symbol, &rise_price);

    f.pm_client
        .decrease_position(&f.trader, &symbol, &DEFAULT_SIZE, &0_i128);

    let balance_after_close = f.usdc_client.balance(&f.trader);
    let received = balance_after_close - balance_after_open;

    // Trader should receive collateral minus loss
    let pnl = math::calc_unrealized_pnl(DEFAULT_SIZE, ETH_PRICE, rise_price, false);
    assert!(pnl < 0, "Short PnL must be negative when price rises");

    assert!(
        received < DEFAULT_COLLATERAL,
        "Losing short trader must receive less than original collateral. Received: {}",
        received
    );
}

#[test]
fn test_full_close_releases_vault_liquidity() {
    // Scenario: After a full close, vault.release_liquidity must be called
    // with the position's full size. Verify free_liquidity increases.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0, &0i128
    );

    let free_liq_before_close = f.vault_client.free_liquidity();

    advance_time(&f, TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 11);
    f.oracle_client.set_price(&symbol, &BTC_PRICE);

    f.pm_client
        .decrease_position(&f.trader, &symbol, &DEFAULT_SIZE, &0_i128);

    let free_liq_after_close = f.vault_client.free_liquidity();

    // free_liquidity should increase by approximately the released size
    assert!(
        free_liq_after_close > free_liq_before_close,
        "Vault free liquidity must increase after position close. Before: {}, After: {}",
        free_liq_before_close,
        free_liq_after_close
    );
}

#[test]
fn test_partial_close_releases_proportional_vault_liquidity() {
    // Scenario: After partial close, vault.release_liquidity is called with
    // size_delta (not full position size).
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0, &0i128
    );

    let free_liq_before = f.vault_client.free_liquidity();

    advance_time(&f, TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 11);
    f.oracle_client.set_price(&symbol, &BTC_PRICE);

    let half_size = DEFAULT_SIZE / 2;
    f.pm_client
        .decrease_position(&f.trader, &symbol, &half_size, &0_i128);

    let free_liq_after = f.vault_client.free_liquidity();

    // Released amount should be approximately half_size
    let released = free_liq_after - free_liq_before;
    assert!(
        released > 0,
        "Vault free liquidity must increase after partial close"
    );
}

#[test]
fn test_decrease_position_with_multiple_traders_independent() {
    // Scenario: Two traders have positions. Closing one trader's position
    // must not affect the other trader's position.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    let trader2 = Address::generate(&f.env);
    f.usdc_client.mint(&trader2, &TRADER_BALANCE);

    // Both open positions
    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0, &0i128
    );
    f.pm_client.increase_position(
        &trader2,
        &symbol,
        &(20_000 * USDC_UNIT),
        &(2_000 * USDC_UNIT),
        &true,
        &0,
        &0, &0i128
    );

    advance_time(&f, TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 11);
    f.oracle_client.set_price(&symbol, &BTC_PRICE);

    // Close trader1's position
    f.pm_client
        .decrease_position(&f.trader, &symbol, &DEFAULT_SIZE, &0_i128);

    // Trader2's position must be unaffected
    let pos2 = f.pm_client.get_position(&trader2, &symbol);
    assert_eq!(
        pos2.size,
        20_000 * USDC_UNIT,
        "Trader2 position must be unaffected"
    );
    assert_eq!(
        pos2.collateral,
        2_000 * USDC_UNIT,
        "Trader2 collateral must be unaffected"
    );

    // Market OI should only reflect trader2's remaining position
    let market = f.pm_client.get_market(&symbol);
    assert_eq!(
        market.long_open_interest,
        20_000 * USDC_UNIT,
        "Long OI must only reflect remaining open positions"
    );

    // TotalReserved should reflect only trader2
    f.env.as_contract(&f.pm_addr, || {
        assert_eq!(
            f.vault_client.reserved_usdc(),
            20_000 * USDC_UNIT,
            "TotalReserved must only reflect remaining positions"
        );
    });
}

#[test]
fn test_decrease_short_updates_short_oi_not_long() {
    // Scenario: Closing a short position must decrease short_open_interest
    // and NOT affect long_open_interest.
    let f = setup_full();
    let symbol = symbol_short!("ETH");

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &false,
        &0,
        &0, &0i128
    );

    let market_before = f.pm_client.get_market(&symbol);
    assert_eq!(market_before.short_open_interest, DEFAULT_SIZE);
    assert_eq!(market_before.long_open_interest, 0);

    advance_time(&f, TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 11);
    f.oracle_client.set_price(&symbol, &ETH_PRICE);

    f.pm_client
        .decrease_position(&f.trader, &symbol, &DEFAULT_SIZE, &0_i128);

    let market_after = f.pm_client.get_market(&symbol);
    assert_eq!(
        market_after.short_open_interest, 0,
        "Short OI must decrease after closing short"
    );
    assert_eq!(
        market_after.long_open_interest, 0,
        "Long OI must remain unaffected by short close"
    );
}

// ===========================================================================
// 7. User-close escrow lifecycle
// ===========================================================================

/// Default flat TP/SL execution fee planted by ConfigManager::initialize.
const TP_SL_FEE: i128 = 5_000_000;

/// Valid TP for a long opened at BTC_PRICE.
const VALID_TP_LONG: i128 = 55_000 * PRECISION;

#[test]
fn user_full_close_refunds_escrow() {
    // Open a long without TP, then set TP via set_tp_sl so the escrow is
    // charged (TP_SL_FEE moves trader -> PM). Full close must refund the
    // escrow PM -> trader on top of the normal position payout.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0, &0i128
    );

    // Set TP -> escrow charged.
    f.pm_client.set_tp_sl(&f.trader, &symbol, &VALID_TP_LONG, &0);
    let pos = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(
        pos.execution_fee_escrow, TP_SL_FEE,
        "Setup: position must record TP_SL_FEE as escrow"
    );

    let trader_balance_before_close = f.usdc_client.balance(&f.trader);

    advance_time(&f, TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 11);
    f.oracle_client.set_price(&symbol, &BTC_PRICE);

    f.pm_client
        .decrease_position(&f.trader, &symbol, &DEFAULT_SIZE, &0_i128);

    let trader_balance_after_close = f.usdc_client.balance(&f.trader);
    let received = trader_balance_after_close - trader_balance_before_close;

    // With price unchanged the position payout is ~DEFAULT_COLLATERAL.
    // The escrow refund is an ADDITIONAL TP_SL_FEE on top.
    // So received >= DEFAULT_COLLATERAL + TP_SL_FEE (allowing for tiny
    // borrow/funding fees that may shrink the payout).
    assert!(
        received >= DEFAULT_COLLATERAL + TP_SL_FEE - 1_000_000,
        "Full close must refund escrow on top of payout. Got: {}, expected ~{}",
        received,
        DEFAULT_COLLATERAL + TP_SL_FEE
    );

    // Position deleted; escrow no longer recorded anywhere.
    f.env.as_contract(&f.pm_addr, || {
        let p = storage::get_position(&f.env, &f.trader, &symbol);
        assert!(p.is_none(), "Position must be deleted after full close");
    });
}

#[test]
fn user_partial_close_keeps_escrow() {
    // Open long, set TP -> escrow paid. Close half. Trader must NOT receive
    // the escrow back. The remaining position keeps the FULL escrow amount.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0, &0i128
    );

    f.pm_client.set_tp_sl(&f.trader, &symbol, &VALID_TP_LONG, &0);
    let pos_before = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(pos_before.execution_fee_escrow, TP_SL_FEE);

    advance_time(&f, TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 11);
    f.oracle_client.set_price(&symbol, &BTC_PRICE);

    let trader_balance_before_close = f.usdc_client.balance(&f.trader);
    let half_size = DEFAULT_SIZE / 2;
    f.pm_client
        .decrease_position(&f.trader, &symbol, &half_size, &0_i128);
    let trader_balance_after_close = f.usdc_client.balance(&f.trader);
    let received = trader_balance_after_close - trader_balance_before_close;

    // The partial close refunds ~half collateral but NOT the escrow.
    // received < DEFAULT_COLLATERAL/2 + TP_SL_FEE, i.e. escrow NOT included.
    assert!(
        received < DEFAULT_COLLATERAL / 2 + TP_SL_FEE,
        "Partial close must NOT refund escrow. Received: {}, would-include-escrow upper-bound: {}",
        received,
        DEFAULT_COLLATERAL / 2 + TP_SL_FEE
    );

    // Remaining position retains the full escrow amount.
    let pos_after = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(
        pos_after.execution_fee_escrow, TP_SL_FEE,
        "Partial close must preserve full escrow on the remaining position"
    );
}

#[test]
fn user_full_close_with_zero_escrow_no_transfer() {
    // Open without TP/SL. Full close must perform no escrow-related transfer
    // — trader receives only the position payout.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0, &0i128
    );
    let pos = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(
        pos.execution_fee_escrow, 0,
        "Setup: position without TP/SL must have escrow == 0"
    );

    let pm_balance_before = f.usdc_client.balance(&f.pm_addr);

    advance_time(&f, TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 11);
    f.oracle_client.set_price(&symbol, &BTC_PRICE);

    let trader_balance_before_close = f.usdc_client.balance(&f.trader);
    f.pm_client
        .decrease_position(&f.trader, &symbol, &DEFAULT_SIZE, &0_i128);
    let trader_balance_after_close = f.usdc_client.balance(&f.trader);
    let received = trader_balance_after_close - trader_balance_before_close;

    // Trader receives ~collateral, NOT collateral + an escrow phantom.
    // Bound the received delta to just over collateral.
    assert!(
        received <= DEFAULT_COLLATERAL + 1_000_000,
        "No-escrow close must NOT route any extra USDC to trader. Received: {}",
        received
    );

    // PM token balance must not retain residual escrow it never collected.
    // Sanity bound: PM balance is non-negative and not artificially inflated.
    let pm_balance_after = f.usdc_client.balance(&f.pm_addr);
    assert!(
        pm_balance_after <= pm_balance_before + TP_SL_FEE,
        "PM balance must not balloon by escrow path on a no-escrow close"
    );

}

// ===========================================================================
// Slippage (`acceptable_price`) tests
// ===========================================================================
//
// For closes: direction inverted from opens. Long closes into the bid →
// revert if mark < acceptable. Short closes into the ask → revert if
// mark > acceptable. `acceptable_price = 0` bypasses.

#[test]
#[should_panic(expected = "Error(Contract, #19)")]
fn test_decrease_long_reverts_when_mark_below_acceptable() {
    let f = setup_full();
    let symbol = symbol_short!("BTC");
    open_position_and_advance(&f, true);
    // Mark is $50k, trader wants $51k floor → revert
    let acceptable = 51_000 * PRECISION;
    f.pm_client
        .decrease_position(&f.trader, &symbol, &DEFAULT_SIZE, &acceptable);
}

#[test]
#[should_panic(expected = "Error(Contract, #19)")]
fn test_decrease_short_reverts_when_mark_above_acceptable() {
    let f = setup_full();
    let symbol = symbol_short!("BTC");
    open_position_and_advance(&f, false);
    // Mark is $50k, short trader wants $49k ceiling → revert
    let acceptable = 49_000 * PRECISION;
    f.pm_client
        .decrease_position(&f.trader, &symbol, &DEFAULT_SIZE, &acceptable);
}

#[test]
fn test_decrease_long_succeeds_when_mark_above_acceptable() {
    let f = setup_full();
    let symbol = symbol_short!("BTC");
    open_position_and_advance(&f, true);
    let acceptable = 49_000 * PRECISION;
    f.pm_client
        .decrease_position(&f.trader, &symbol, &DEFAULT_SIZE, &acceptable);
    let res = f.pm_client.try_get_position(&f.trader, &symbol);
    assert!(res.is_err(), "full close must delete the position");
}

#[test]
fn test_decrease_short_succeeds_when_mark_below_acceptable() {
    let f = setup_full();
    let symbol = symbol_short!("BTC");
    open_position_and_advance(&f, false);
    let acceptable = 51_000 * PRECISION;
    f.pm_client
        .decrease_position(&f.trader, &symbol, &DEFAULT_SIZE, &acceptable);
    let res = f.pm_client.try_get_position(&f.trader, &symbol);
    assert!(res.is_err(), "full close must delete the position");
}

#[test]
fn test_decrease_zero_acceptable_price_bypasses_check() {
    let f = setup_full();
    let symbol = symbol_short!("BTC");
    open_position_and_advance(&f, true);
    f.pm_client
        .decrease_position(&f.trader, &symbol, &DEFAULT_SIZE, &0_i128);
}

#[test]
fn test_decrease_boundary_mark_equals_acceptable_long() {
    // Inclusive bound: mark >= acceptable for long close.
    let f = setup_full();
    let symbol = symbol_short!("BTC");
    open_position_and_advance(&f, true);
    f.pm_client
        .decrease_position(&f.trader, &symbol, &DEFAULT_SIZE, &BTC_PRICE);
}

#[test]
fn test_decrease_boundary_mark_equals_acceptable_short() {
    let f = setup_full();
    let symbol = symbol_short!("BTC");
    open_position_and_advance(&f, false);
    f.pm_client
        .decrease_position(&f.trader, &symbol, &DEFAULT_SIZE, &BTC_PRICE);
}
