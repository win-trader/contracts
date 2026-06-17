// ---------------------------------------------------------------------------
// Tests for: liquidate_position
//
// These tests are written BEFORE the implementation (TDD). They MUST compile
// but are expected to FAIL until liquidate_position is fully implemented.
//
// The function under test:
//   fn liquidate_position(env, caller, trader, symbol)
//
// Behavior:
//   - KEEPER-only (caller.require_auth + require_keeper)
//   - Requires initialized + NOT paused
//   - Reverts PositionNotFound (error 6) if no position exists
//   - Fetches mark price from oracle
//   - Calculates health: collateral + unrealized_pnl - borrow_fee + funding_fee
//   - If health >= 0 => reverts HealthFactorOk (error 9)
//   - If health < 0 => liquidate:
//       Seize collateral, settle loss via vault, delete position,
//       decrease OI, decrease total_reserved, release vault liquidity,
//       accrue protocol fees via vault.
// ---------------------------------------------------------------------------

use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Ledger, LedgerInfo},
    Address, Env, Symbol,
};

use crate::contract::PositionManagerContract;
use shared::constants::{BPS, PRECISION};
use shared::FeeConfig;
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

/// 1 USDC = 1_000_000 (6 decimals)
const USDC_UNIT: i128 = 1_000_000;

/// Trader starts with 100,000 USDC
const TRADER_BALANCE: i128 = 100_000 * USDC_UNIT;

/// Initial vault deposit (1,000,000 USDC) -- large enough for tests
const VAULT_DEPOSIT: i128 = 1_000_000 * USDC_UNIT;

/// Default position size: 10,000 USDC notional
const DEFAULT_SIZE: i128 = 10_000 * USDC_UNIT;

/// Default collateral: 1,000 USDC (10x leverage)
const DEFAULT_COLLATERAL: i128 = 1_000 * USDC_UNIT;

/// Ledger timestamp used in tests
const TEST_TIMESTAMP: u64 = 1_700_000_000;

/// Min position lifetime (60s) + oracle cache (10s) + buffer
const TIME_ADVANCE: u64 = 75;

// ===========================================================================
// Test fixture
// ===========================================================================

struct TestFixture<'a> {
    env: Env,
    pm_client: PositionManagerClient<'a>,
    _vault_client: VaultContractClient<'a>,
    oracle_client: DualOracle<'a>,
    _oracle_router_client: OracleRouterClient<'a>,
    _config_client: ConfigManagerClient<'a>,
    usdc_client: MockTokenClient<'a>,
    _usdc_addr: Address,
    admin: Address,
    keeper: Address,
    trader: Address,
    pm_addr: Address,
    _vault_addr: Address,
}

/// Deploy and wire up ALL protocol contracts needed for liquidation tests.
///
/// Includes a dedicated `keeper` address with the KEEPER role, separate from admin.
fn setup_full<'a>() -> TestFixture<'a> {
    let env = Env::default();
    env.mock_all_auths();

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
    let keeper = Address::generate(&env);
    let trader = Address::generate(&env);
    let lp = Address::generate(&env);

    // --- 1. ConfigManager ---
    let config_id = env.register(ConfigManagerContract, (admin.clone(),));
    let config_client = ConfigManagerClient::new(&env, &config_id);

    let pauser_role = Symbol::new(&env, "PAUSER");
    let keeper_role = Symbol::new(&env, "KEEPER");
    let _admin_role = Symbol::new(&env, "ADMIN");
    config_client.grant_role(&admin, &pauser_role, &admin);
    config_client.grant_role(&admin, &keeper_role, &admin);
    // Grant KEEPER to the dedicated keeper address
    config_client.grant_role(&admin, &keeper_role, &keeper);

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

    // --- 4. OracleRouter ---
    let oracle_router_id = env.register(OracleRouterContract, (config_id.clone(),));
    let oracle_router_client = OracleRouterClient::new(&env, &oracle_router_id);
    oracle_router_client.set_oracle_config(
        &admin,
        &OracleConfig {
            max_deviation_bps: 500,
            staleness_threshold: 3600,
            cache_duration: 10,
            min_required_sources: 2,},
    );
    oracle_router_client.set_oracle_sources(
        &admin,
        &symbol_short!("BTC"),
        &oracle_sources);

    // --- 5. PositionManager ---
    let pm_id = env.register(PositionManagerContract, (config_id.clone(), oracle_router_id.clone()));
    let pm_client = PositionManagerClient::new(&env, &pm_id);

    // --- 6. Vault (binds the trusted PositionManager in its constructor) ---
    let vault_id = env.register(VaultContract, (usdc_id.clone(), config_id.clone(), pm_id.clone()));
    let vault_client = VaultContractClient::new(&env, &vault_id);

    // --- 7. Wire the Vault into the PositionManager (ADMIN-gated, one-shot) ---
    pm_client.set_vault(&admin, &vault_id);
    pm_client.set_max_leverage(&admin, &symbol_short!("BTC"), &100_i128);

    // --- Fund accounts ---
    usdc_client.mint(&trader, &TRADER_BALANCE);
    usdc_client.mint(&lp, &VAULT_DEPOSIT);
    vault_client.deposit(&VAULT_DEPOSIT, &lp, &lp, &lp);

    // SAFETY: env lives in the fixture; clients borrow from it.
    let pm_client = unsafe { core::mem::transmute(pm_client) };
    let _vault_client = unsafe { core::mem::transmute(vault_client) };
    let oracle_client = unsafe { core::mem::transmute(oracle_client) };
    let _oracle_router_client = unsafe { core::mem::transmute(oracle_router_client) };
    let _config_client = unsafe { core::mem::transmute(config_client) };
    let usdc_client = unsafe { core::mem::transmute(usdc_client) };

    TestFixture {
        env,
        pm_client,
        _vault_client,
        oracle_client,
        _oracle_router_client,
        _config_client,
        usdc_client,
        _usdc_addr: usdc_id,
        admin,
        keeper,
        trader,
        pm_addr: pm_id,
        _vault_addr: vault_id,
    }
}

// ===========================================================================
// Helpers
// ===========================================================================

/// Open a long BTC position for the fixture's trader with given size and collateral.
fn open_long_position(f: &TestFixture, size: i128, collateral: i128) {
    f.pm_client.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &size,
        &collateral,
        &true,
        &0,
        &0, &0i128
    );
}

/// Open a short BTC position for the fixture's trader.
fn open_short_position(f: &TestFixture, size: i128, collateral: i128) {
    f.pm_client.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &size,
        &collateral,
        &false,
        &0,
        &0, &0i128
    );
}

/// Advance time and refresh oracle price. This ensures the oracle cache is
/// invalidated so the new price is fetched on the next call.
fn advance_time_and_set_price(f: &TestFixture, new_ts: u64, new_price: i128) {
    f.env.ledger().set(LedgerInfo {
        timestamp: new_ts,
        protocol_version: 23,
        sequence_number: 100 + ((new_ts - TEST_TIMESTAMP) as u32),
        network_id: [0u8; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 100,
        min_persistent_entry_ttl: 100,
        max_entry_ttl: 10_000_000,
    });
    f.oracle_client.set_price(&symbol_short!("BTC"), &new_price);
}

/// Seed the market with non-zero borrow index to simulate accumulated fees.
/// Must be called BEFORE opening a position so the position snapshots the index.
fn _seed_market_borrow_index(f: &TestFixture, borrow_index: i128) {
    let symbol = symbol_short!("BTC");
    f.env.as_contract(&f.pm_addr, || {
        let mut market = storage::get_market(&f.env, &symbol);
        market.acc_borrow_index = borrow_index;
        market.last_index_update = f.env.ledger().timestamp();
        storage::set_market(&f.env, &symbol, &market);
    });
}

// ===========================================================================
// 1. Guard tests
// ===========================================================================

#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn test_liquidate_reverts_position_not_found() {
    // Scenario: Keeper calls liquidate on a trader who has no open position.
    // Must revert with PositionNotFound (error 6).
    let f = setup_full();
    let nonexistent_trader = Address::generate(&f.env);

    f.pm_client
        .liquidate_position(&f.keeper, &nonexistent_trader, &symbol_short!("BTC"));
}

// ===========================================================================
// 2. Health check -- position is still healthy
// ===========================================================================

#[test]
#[should_panic(expected = "Error(Contract, #9)")]
fn test_liquidate_reverts_health_factor_ok_price_unchanged() {
    // Scenario: Price has not moved. The position has full collateral and zero PnL,
    // so health = collateral > 0. Must revert HealthFactorOk (error 9).
    let f = setup_full();
    open_long_position(&f, DEFAULT_SIZE, DEFAULT_COLLATERAL);

    // Advance time but keep the same price
    advance_time_and_set_price(&f, TEST_TIMESTAMP + TIME_ADVANCE, BTC_PRICE);

    f.pm_client
        .liquidate_position(&f.keeper, &f.trader, &symbol_short!("BTC"));
}

#[test]
#[should_panic(expected = "Error(Contract, #9)")]
fn test_liquidate_reverts_health_factor_ok_price_up_long() {
    // Scenario: Long position with price rising. The position is profitable,
    // so health = collateral + positive_pnl > 0. Must revert HealthFactorOk (error 9).
    let f = setup_full();
    open_long_position(&f, DEFAULT_SIZE, DEFAULT_COLLATERAL);

    // Price goes up from $50,000 to $55,000
    let higher_price: i128 = 55_000 * PRECISION;
    advance_time_and_set_price(&f, TEST_TIMESTAMP + TIME_ADVANCE, higher_price);

    f.pm_client
        .liquidate_position(&f.keeper, &f.trader, &symbol_short!("BTC"));
}

// ===========================================================================
// 3. Successful liquidation
// ===========================================================================

#[test]
fn test_liquidate_long_position_price_drops_significantly() {
    // Scenario: Long BTC position opened at $50,000 with 1,000 USDC collateral
    // and 10,000 USDC size (10x leverage).
    //
    // Price drops to $44,000:
    //   unrealized_pnl = 10,000 * (44,000 - 50,000) / 50,000 = -1,200 USDC
    //   health = 1,000 + (-1,200) - borrow_fee + funding_fee
    //   With fresh indices (borrow_fee ~ 0, funding_fee ~ 0):
    //   health = 1,000 - 1,200 = -200 (underwater)
    //
    // Expected: position deleted, OI decreased, total_reserved decreased.
    let f = setup_full();
    let symbol = symbol_short!("BTC");
    open_long_position(&f, DEFAULT_SIZE, DEFAULT_COLLATERAL);

    // Verify position exists
    let pos_before = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(
        pos_before.size, DEFAULT_SIZE,
        "Position must exist before liquidation"
    );

    // Crash price
    let crash_price: i128 = 44_000 * PRECISION;
    advance_time_and_set_price(&f, TEST_TIMESTAMP + TIME_ADVANCE, crash_price);

    // Record state before liquidation
    let market_before = f.pm_client.get_market(&symbol);
    let total_reserved_before = f
        .env
        .as_contract(&f.pm_addr, || f._vault_client.reserved_usdc());

    // Execute liquidation
    f.pm_client
        .liquidate_position(&f.keeper, &f.trader, &symbol);

    // Position must be deleted
    // Attempting to get_position should panic with PositionNotFound.
    // We verify by checking storage directly instead.
    f.env.as_contract(&f.pm_addr, || {
        let pos = storage::get_position(&f.env, &f.trader, &symbol);
        assert!(pos.is_none(), "Position must be deleted after liquidation");
    });

    // Market OI must decrease
    let market_after = f.pm_client.get_market(&symbol);
    assert_eq!(
        market_after.long_open_interest,
        market_before.long_open_interest - DEFAULT_SIZE,
        "Long OI must decrease by position size after liquidation"
    );

    // Total reserved must decrease
    let total_reserved_after = f
        .env
        .as_contract(&f.pm_addr, || f._vault_client.reserved_usdc());
    assert_eq!(
        total_reserved_after,
        total_reserved_before - DEFAULT_SIZE,
        "Total reserved must decrease by position size after liquidation"
    );
}

#[test]
fn test_liquidate_short_position_price_rises_significantly() {
    // Scenario: Short BTC position opened at $50,000 with 1,000 USDC collateral
    // and 10,000 USDC size.
    //
    // Price rises to $56,000:
    //   unrealized_pnl = 10,000 * (50,000 - 56,000) / 50,000 = -1,200 USDC
    //   health = 1,000 - 1,200 = -200 (underwater)
    let f = setup_full();
    let symbol = symbol_short!("BTC");
    open_short_position(&f, DEFAULT_SIZE, DEFAULT_COLLATERAL);

    // Price rises sharply
    let spike_price: i128 = 56_000 * PRECISION;
    advance_time_and_set_price(&f, TEST_TIMESTAMP + TIME_ADVANCE, spike_price);

    // Execute liquidation
    f.pm_client
        .liquidate_position(&f.keeper, &f.trader, &symbol);

    // Position must be deleted
    f.env.as_contract(&f.pm_addr, || {
        let pos = storage::get_position(&f.env, &f.trader, &symbol);
        assert!(
            pos.is_none(),
            "Short position must be deleted after liquidation"
        );
    });

    // Short OI must decrease
    let market_after = f.pm_client.get_market(&symbol);
    assert_eq!(
        market_after.short_open_interest, 0,
        "Short OI must be zero after liquidating the only short position"
    );
}

#[test]
fn test_liquidate_trader_does_not_receive_funds() {
    // Scenario: When a position is liquidated, all remaining collateral is seized.
    // The trader's USDC balance should NOT increase after liquidation.
    let f = setup_full();
    let symbol = symbol_short!("BTC");
    open_long_position(&f, DEFAULT_SIZE, DEFAULT_COLLATERAL);

    let trader_balance_before = f.usdc_client.balance(&f.trader);

    // Crash price to make position liquidatable
    let crash_price: i128 = 44_000 * PRECISION;
    advance_time_and_set_price(&f, TEST_TIMESTAMP + TIME_ADVANCE, crash_price);

    f.pm_client
        .liquidate_position(&f.keeper, &f.trader, &symbol);

    let trader_balance_after = f.usdc_client.balance(&f.trader);

    // Trader should NOT receive any funds from liquidation
    assert!(
        trader_balance_after <= trader_balance_before,
        "Trader balance must not increase after liquidation. Before: {}, After: {}",
        trader_balance_before,
        trader_balance_after
    );
}

// ===========================================================================
// 4. Edge cases
// ===========================================================================

#[test]
fn test_liquidate_barely_underwater_position() {
    // Scenario: Position is just barely underwater (health = -1 or very small negative).
    // The liquidation should still succeed.
    //
    // Long at $50,000, size=10,000, collateral=1,000.
    // We need pnl = -(collateral + 1) = -1,001 USDC (approximately).
    // pnl = size * (mark - entry) / entry
    // -1,001 = 10,000 * (mark - 50,000) / 50,000
    // mark - 50,000 = -1,001 * 50,000 / 10,000 = -5,005
    // mark = 44,995 (in USDC terms, but with 1e7 precision)
    //
    // With collateral = 1,000 USDC, a price of ~$44,990 gives:
    //   pnl = 10,000 * (44,990 - 50,000) / 50,000 = -1,002 USDC
    //   health = 1,000 - 1,002 = -2 (just barely underwater)
    let f = setup_full();
    let symbol = symbol_short!("BTC");
    open_long_position(&f, DEFAULT_SIZE, DEFAULT_COLLATERAL);

    // Price that makes health just barely negative
    let barely_crash_price: i128 = 44_990 * PRECISION;
    advance_time_and_set_price(&f, TEST_TIMESTAMP + TIME_ADVANCE, barely_crash_price);

    // Must succeed (position is underwater)
    f.pm_client
        .liquidate_position(&f.keeper, &f.trader, &symbol);

    // Confirm position deleted
    f.env.as_contract(&f.pm_addr, || {
        let pos = storage::get_position(&f.env, &f.trader, &symbol);
        assert!(
            pos.is_none(),
            "Barely underwater position must be liquidated"
        );
    });
}

#[test]
fn test_liquidate_borrow_fees_push_position_underwater() {
    // Scenario: Price barely moves, but accumulated borrow fees push the
    // position's health below zero.
    //
    // We seed a large borrow index before opening the position, then advance
    // the index further via update_indices to accumulate significant fees.
    //
    // Open long at $50,000, size=10,000, collateral=1,000.
    // If borrow_fee > 1,000 and pnl ~ 0, health = 1,000 - borrow_fee < 0.
    //
    // borrow_fee = (current_borrow_index - entry_borrow_index) * size / INDEX_PRECISION
    // We need borrow_fee > 1_000 * USDC_UNIT = 1_000_000_000
    // So (delta_index) * 10_000_000_000 / 1e14 > 1_000_000_000
    // delta_index > 1_000_000_000 * 1e14 / 10_000_000_000 = 10_000_000_000_000
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    // Open position with fresh indices (borrow_index will be whatever market has)
    open_long_position(&f, DEFAULT_SIZE, DEFAULT_COLLATERAL);

    // Advance time first so we can set the borrow index at the new timestamp
    advance_time_and_set_price(&f, TEST_TIMESTAMP + TIME_ADVANCE, BTC_PRICE);

    // Now manually bump the market's borrow index to simulate large accumulated fees.
    // Set last_index_update to current time so do_update_indices is a no-op.
    // The position's entry_borrow_index was snapshotted at open time.
    // We need (new_index - entry_index) * size / INDEX_PRECISION > collateral
    let large_borrow_index: i128 = 15_000_000_000_000; // enough to make fee > collateral
    f.env.as_contract(&f.pm_addr, || {
        let mut market = storage::get_market(&f.env, &symbol);
        market.acc_borrow_index += large_borrow_index;
        market.last_index_update = TEST_TIMESTAMP + TIME_ADVANCE;
        storage::set_market(&f.env, &symbol, &market);
    });

    // The borrow fee should exceed collateral, making health negative
    // borrow_fee = 15_000_000_000_000 * 10_000_000_000 / 100_000_000_000_000 = 1_500_000_000
    // health = 1_000_000_000 + 0 - 1_500_000_000 + 0 = -500_000_000
    f.pm_client
        .liquidate_position(&f.keeper, &f.trader, &symbol);

    // Confirm position deleted
    f.env.as_contract(&f.pm_addr, || {
        let pos = storage::get_position(&f.env, &f.trader, &symbol);
        assert!(
            pos.is_none(),
            "Position pushed underwater by borrow fees must be liquidated"
        );
    });
}

// ===========================================================================
// 5. Paused contract
// ===========================================================================

#[test]
fn test_liquidate_succeeds_when_paused() {
    // Scenario: Contract is paused. Liquidations must still work to prevent bad debt.
    let f = setup_full();
    let symbol = symbol_short!("BTC");
    open_long_position(&f, DEFAULT_SIZE, DEFAULT_COLLATERAL);

    // Crash the price
    let crash_price: i128 = 44_000 * PRECISION;
    advance_time_and_set_price(&f, TEST_TIMESTAMP + TIME_ADVANCE, crash_price);

    // Pause the contract
    f.pm_client.pause(&f.admin);

    // Liquidation must succeed even when paused
    f.pm_client
        .liquidate_position(&f.keeper, &f.trader, &symbol);

    // Position must be deleted
    f.env.as_contract(&f.pm_addr, || {
        let pos = storage::get_position(&f.env, &f.trader, &symbol);
        assert!(
            pos.is_none(),
            "Position must be deleted after liquidation while paused"
        );
    });
}

// ===========================================================================
// 6. Multiple position isolation
// ===========================================================================

#[test]
fn test_liquidate_one_position_does_not_affect_other_traders() {
    // Scenario: Two traders have positions. Only one is underwater.
    // Liquidating trader1 must not affect trader2's position.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    let trader2 = Address::generate(&f.env);
    f.usdc_client.mint(&trader2, &TRADER_BALANCE);

    // Trader1: long, 10x leverage (vulnerable)
    open_long_position(&f, DEFAULT_SIZE, DEFAULT_COLLATERAL);

    // Trader2: long, 2x leverage (safe)
    let safe_size = 10_000 * USDC_UNIT;
    let safe_collateral = 5_000 * USDC_UNIT;
    f.pm_client.increase_position(
        &trader2,
        &symbol,
        &safe_size,
        &safe_collateral,
        &true,
        &0,
        &0, &0i128
    );

    // Crash price enough to liquidate trader1 but not trader2
    // Trader1: health = 1,000 + 10,000*(44,000-50,000)/50,000 = 1,000 - 1,200 = -200
    // Trader2: health = 5,000 + 10,000*(44,000-50,000)/50,000 = 5,000 - 1,200 = 3,800 (safe)
    let crash_price: i128 = 44_000 * PRECISION;
    advance_time_and_set_price(&f, TEST_TIMESTAMP + TIME_ADVANCE, crash_price);

    // Liquidate trader1
    f.pm_client
        .liquidate_position(&f.keeper, &f.trader, &symbol);

    // Trader1 position must be gone
    f.env.as_contract(&f.pm_addr, || {
        assert!(
            storage::get_position(&f.env, &f.trader, &symbol).is_none(),
            "Trader1 position must be liquidated"
        );
    });

    // Trader2 position must still exist
    let pos2 = f.pm_client.get_position(&trader2, &symbol);
    assert_eq!(
        pos2.size, safe_size,
        "Trader2 position must be unaffected by trader1 liquidation"
    );
    assert_eq!(
        pos2.collateral, safe_collateral,
        "Trader2 collateral must be unaffected"
    );
}

// ===========================================================================
// 7. Liquidation bounty + escrow forfeit lifecycle
// ===========================================================================

/// Default flat TP/SL execution fee planted by ConfigManager::initialize.
const TP_SL_FEE: i128 = 5_000_000;

/// Default liquidation bounty in basis points planted by ConfigManager::initialize.
const DEFAULT_BOUNTY_BPS: i128 = 100;

#[test]
fn liquidation_pays_bounty_to_caller() {
    // Long BTC at 50k, collateral=1000 USDC. Crash price to 44k so the
    // position is liquidatable. With default liquidation_bounty_bps == 100
    // and collateral_delta == DEFAULT_COLLATERAL, the bounty is
    // `DEFAULT_COLLATERAL * 100 / 10_000 == DEFAULT_COLLATERAL / 100`.
    // pm_to_vault on this crash exceeds the bounty, so the liquidator gets
    // the full bps-derived bounty paid PM -> caller.
    let f = setup_full();
    let symbol = symbol_short!("BTC");
    open_long_position(&f, DEFAULT_SIZE, DEFAULT_COLLATERAL);

    let crash_price: i128 = 44_000 * PRECISION;
    advance_time_and_set_price(&f, TEST_TIMESTAMP + TIME_ADVANCE, crash_price);

    let caller_balance_before = f.usdc_client.balance(&f.keeper);

    f.pm_client
        .liquidate_position(&f.keeper, &f.trader, &symbol);

    let caller_balance_after = f.usdc_client.balance(&f.keeper);
    let expected_bounty = DEFAULT_COLLATERAL * DEFAULT_BOUNTY_BPS / BPS;

    assert_eq!(
        caller_balance_after - caller_balance_before,
        expected_bounty,
        "Liquidator must receive bounty == collateral_delta * bounty_bps / BPS"
    );
}

#[test]
fn liquidation_bounty_clamped_when_absorbed_lt_bounty() {
    // The bounty formula is `min(collateral_delta * bps / BPS, pm_to_vault)`.
    // Under the ConfigManager validation bounds (MAX_LIQUIDATION_BOUNTY_BPS
    // == 1000 and MAX_LIQUIDATION_THRESHOLD_BPS == 1000), the bps amount
    // cannot exceed 10% of collateral_delta, and pm_to_vault on a
    // liquidation is at least 90% of collateral_delta — so the min() will
    // bind on pm_to_vault only when the bps cap is also at its max and the
    // trader_payout is at its max. This test verifies the INVARIANT that
    // the caller never receives more than pm_to_vault for any liquidation,
    // regardless of bounty configuration.
    //
    // Setup: bounty_bps at max (1000); barely-underwater liquidation where
    // the trader keeps a small portion of collateral. The bounty must be
    // capped by pm_to_vault.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    f._config_client.set_fee_config(
        &f.admin,
        &FeeConfig {
            open_fee_bps: 10,
            liquidation_bounty_bps: 1_000, // 10% — max allowed
            tp_sl_execution_fee: TP_SL_FEE,
        },
    );

    open_long_position(&f, DEFAULT_SIZE, DEFAULT_COLLATERAL);

    // Crash JUST enough to be liquidatable. With threshold_bps=200, health
    // must be < 20 USDC. Use barely-crash price (44_990) -> health ~ -2.
    // pm_to_vault ~ DEFAULT_COLLATERAL (fully absorbed). bps_bounty = 100.
    let crash_price: i128 = 44_990 * PRECISION;
    advance_time_and_set_price(&f, TEST_TIMESTAMP + TIME_ADVANCE, crash_price);

    let caller_balance_before = f.usdc_client.balance(&f.keeper);

    f.pm_client
        .liquidate_position(&f.keeper, &f.trader, &symbol);

    let caller_balance_after = f.usdc_client.balance(&f.keeper);
    let received = caller_balance_after - caller_balance_before;

    // Invariant: bounty <= min(bps_bounty, pm_to_vault).
    let bps_bounty = DEFAULT_COLLATERAL * 1_000 / BPS;
    assert!(
        received <= bps_bounty,
        "Bounty must not exceed bps_bounty. Got {}, bps_bounty {}",
        received,
        bps_bounty
    );
    assert!(
        received <= DEFAULT_COLLATERAL,
        "Bounty must not exceed pm_to_vault (≤ collateral_delta). Got {}, collateral {}",
        received,
        DEFAULT_COLLATERAL
    );
    // And it must be POSITIVE (the bounty path actually fired).
    assert!(received > 0, "Bounty must be paid when bps > 0 and pm_to_vault > 0");
}

#[test]
fn liquidation_bounty_zero_when_bps_zero() {
    // Admin disables the liquidation bounty (bps == 0). Caller receives
    // nothing from the bounty path.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    f._config_client.set_fee_config(
        &f.admin,
        &FeeConfig {
            open_fee_bps: 10,
            liquidation_bounty_bps: 0,
            tp_sl_execution_fee: TP_SL_FEE,
        },
    );

    open_long_position(&f, DEFAULT_SIZE, DEFAULT_COLLATERAL);

    let crash_price: i128 = 44_000 * PRECISION;
    advance_time_and_set_price(&f, TEST_TIMESTAMP + TIME_ADVANCE, crash_price);

    let caller_balance_before = f.usdc_client.balance(&f.keeper);
    f.pm_client
        .liquidate_position(&f.keeper, &f.trader, &symbol);
    let caller_balance_after = f.usdc_client.balance(&f.keeper);

    assert_eq!(
        caller_balance_after, caller_balance_before,
        "Caller must receive zero bounty when liquidation_bounty_bps == 0"
    );
}

#[test]
fn liquidation_forfeits_escrow_to_revenue_split() {
    // Open with TP active. The flat `tp_sl_execution_fee` lives in PM as
    // `pos.execution_fee_escrow`. On liquidation the escrow is forfeited:
    // moved PM -> vault and bookkept through `distribute_revenue_fees`, so
    // the (dev+staker) slice grows `vault.unclaimed_fees` and the LP slice
    // stays implicitly in `vault.total_assets`.
    //
    // The default FeeSplits are LP=9000, dev=500, staker=500 (non-LP = 1000
    // bps = 10%). So unclaimed_fees grows by escrow * 10%.
    //
    // The vault exposes `unclaimed_fees()` directly, and raw token custody is
    // `total_assets() + unclaimed_fees()` (total_assets is the LP basis, which
    // excludes the fee buffer). We read both directly rather than inferring.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    open_long_position(&f, DEFAULT_SIZE, DEFAULT_COLLATERAL);

    // Set TP -> trader pays TP_SL_FEE; escrow recorded on position.
    let tp = 55_000 * PRECISION;
    f.pm_client.set_tp_sl(&f.trader, &symbol, &tp, &0);

    let pos = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(
        pos.execution_fee_escrow, TP_SL_FEE,
        "Setup: position must record TP_SL_FEE as escrow"
    );

    let vault_custody_before = f._vault_client.total_assets() + f._vault_client.unclaimed_fees();
    let unclaimed_before = f._vault_client.unclaimed_fees();
    let trader_balance_before = f.usdc_client.balance(&f.trader);
    let caller_balance_before = f.usdc_client.balance(&f.keeper);

    let crash_price: i128 = 44_000 * PRECISION;
    advance_time_and_set_price(&f, TEST_TIMESTAMP + TIME_ADVANCE, crash_price);

    f.pm_client
        .liquidate_position(&f.keeper, &f.trader, &symbol);

    let vault_custody_after = f._vault_client.total_assets() + f._vault_client.unclaimed_fees();
    let unclaimed_after = f._vault_client.unclaimed_fees();
    let trader_balance_after = f.usdc_client.balance(&f.trader);
    let caller_balance_after = f.usdc_client.balance(&f.keeper);

    // The trader must NOT receive the escrow back on liquidation.
    let trader_received = trader_balance_after - trader_balance_before;
    assert!(
        trader_received < TP_SL_FEE,
        "Trader must NOT receive escrow refund on liquidation. Got: {}",
        trader_received
    );

    // The caller (liquidator) receives only the bounty, NOT the escrow.
    let bounty_expected = DEFAULT_COLLATERAL * DEFAULT_BOUNTY_BPS / BPS;
    let caller_received = caller_balance_after - caller_balance_before;
    assert_eq!(
        caller_received, bounty_expected,
        "Liquidator receives bounty only; escrow must NOT flow to caller"
    );

    // Raw custody grows by the absorbed collateral PLUS the forfeited escrow,
    // so the custody delta is at least the escrow amount.
    let custody_delta = vault_custody_after - vault_custody_before;
    assert!(
        custody_delta >= TP_SL_FEE,
        "Vault custody must grow by at least the forfeited escrow. Got: {}",
        custody_delta
    );

    // The non-LP slice of the forfeited escrow must appear in unclaimed_fees.
    let non_lp_slice = TP_SL_FEE / 10;
    let unclaimed_delta = unclaimed_after - unclaimed_before;
    assert!(
        unclaimed_delta >= non_lp_slice,
        "Vault unclaimed_fees must grow by at least the non-LP slice of escrow. Got: {}, expected at least: {}",
        unclaimed_delta,
        non_lp_slice
    );
}

#[test]
fn liquidation_with_zero_escrow_no_extra_transfer() {
    // Position has no TP/SL — escrow == 0. Liquidation must skip the
    // escrow flow entirely: no extra accrual to unclaimed_fees and no
    // additional vault growth beyond the absorbed collateral.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    open_long_position(&f, DEFAULT_SIZE, DEFAULT_COLLATERAL);

    // Confirm no escrow recorded.
    let pos = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(
        pos.execution_fee_escrow, 0,
        "Setup: opening without TP/SL must leave escrow at 0"
    );

    let vault_total_before = f._vault_client.total_assets();

    let crash_price: i128 = 44_000 * PRECISION;
    advance_time_and_set_price(&f, TEST_TIMESTAMP + TIME_ADVANCE, crash_price);

    f.pm_client
        .liquidate_position(&f.keeper, &f.trader, &symbol);

    let vault_total_after = f._vault_client.total_assets();
    let total_delta = vault_total_after - vault_total_before;

    // Vault must grow by at most the absorbed collateral (DEFAULT_COLLATERAL).
    // Without any escrow, there is no additional amount routed in.
    assert!(
        total_delta <= DEFAULT_COLLATERAL,
        "Without escrow, vault must NOT grow beyond absorbed collateral. Got: {}",
        total_delta
    );
}

#[test]
fn liquidation_bounty_priority_over_revenue_fees() {
    // Bounty has priority over revenue fees: revenue fee accrual clamps to
    // `pm_to_vault - bounty`. Use max allowed bounty_bps (1000) so the
    // bounty is the maximum the protocol can pay, then verify the vault
    // receives `pm_to_vault - bounty` rather than the full `pm_to_vault`.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    f._config_client.set_fee_config(
        &f.admin,
        &FeeConfig {
            open_fee_bps: 10,
            liquidation_bounty_bps: 1_000, // max
            tp_sl_execution_fee: TP_SL_FEE,
        },
    );

    open_long_position(&f, DEFAULT_SIZE, DEFAULT_COLLATERAL);

    let vault_total_before = f._vault_client.total_assets();
    let caller_balance_before = f.usdc_client.balance(&f.keeper);

    let crash_price: i128 = 40_000 * PRECISION;
    advance_time_and_set_price(&f, TEST_TIMESTAMP + TIME_ADVANCE, crash_price);

    f.pm_client
        .liquidate_position(&f.keeper, &f.trader, &symbol);

    let vault_total_after = f._vault_client.total_assets();
    let caller_balance_after = f.usdc_client.balance(&f.keeper);

    let caller_received = caller_balance_after - caller_balance_before;
    let total_delta = vault_total_after - vault_total_before;

    // Bounty is the bps-derived amount (does not clamp under valid bounds).
    assert_eq!(
        caller_received,
        DEFAULT_COLLATERAL * 1_000 / BPS,
        "Liquidator receives bps-derived bounty when pm_to_vault > bps_bounty"
    );

    // Vault gain == pm_to_vault - bounty. The bounty stays in PM until
    // paid out; the rest is transferred PM -> vault for absorption.
    assert!(
        total_delta <= DEFAULT_COLLATERAL - caller_received,
        "Vault must receive pm_to_vault - bounty. total_delta={}, bounty={}, collateral={}",
        total_delta,
        caller_received,
        DEFAULT_COLLATERAL
    );

    // Bounty has priority: total_delta must be strictly less than the
    // pre-bounty pm_to_vault (DEFAULT_COLLATERAL on a full wipe).
    assert!(
        total_delta < DEFAULT_COLLATERAL,
        "Bounty must come off the top, so vault gain < collateral_delta. total_delta={}",
        total_delta
    );
}

// ===========================================================================
// 8. Permissionless liquidation
//
// `liquidate_position` is permissionless: any address that signs the call may
// trigger liquidation when the position is underwater. The caller still has
// to `require_auth` so a third party cannot pass someone else's address as
// `caller` to steal the bounty. The KEEPER role check is removed; only the
// underwater health condition gates execution.
// ===========================================================================

#[test]
fn non_keeper_can_liquidate() {
    // Scenario: an arbitrary Address that has NEVER been granted the KEEPER
    // role triggers a liquidation. The position must be deleted and the
    // bounty must flow to the non-keeper caller.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    // Brand-new address — not the fixture's `keeper` and never granted any role.
    let random_liquidator = Address::generate(&f.env);

    // Sanity check: the random liquidator must NOT hold the KEEPER role on
    // the ConfigManager. If this assertion ever fails the test is invalid
    // (the address would be implicitly authorized).
    let keeper_role = Symbol::new(&f.env, "KEEPER");
    assert!(
        !f._config_client.has_role(&keeper_role, &random_liquidator),
        "Setup invariant: random_liquidator must not hold KEEPER"
    );

    open_long_position(&f, DEFAULT_SIZE, DEFAULT_COLLATERAL);

    // Crash the price so the position is underwater.
    let crash_price: i128 = 44_000 * PRECISION;
    advance_time_and_set_price(&f, TEST_TIMESTAMP + TIME_ADVANCE, crash_price);

    let liquidator_balance_before = f.usdc_client.balance(&random_liquidator);

    // Non-keeper triggers the liquidation. Must NOT panic with Unauthorized.
    f.pm_client
        .liquidate_position(&random_liquidator, &f.trader, &symbol);

    // The position must be deleted, exactly like the keeper-driven path.
    f.env.as_contract(&f.pm_addr, || {
        let pos = storage::get_position(&f.env, &f.trader, &symbol);
        assert!(
            pos.is_none(),
            "Permissionless liquidation must delete the underwater position"
        );
    });

    // The non-keeper caller must receive the bounty == collateral_delta * bps / BPS.
    let liquidator_balance_after = f.usdc_client.balance(&random_liquidator);
    let expected_bounty = DEFAULT_COLLATERAL * DEFAULT_BOUNTY_BPS / BPS;
    assert_eq!(
        liquidator_balance_after - liquidator_balance_before,
        expected_bounty,
        "Non-keeper liquidator must receive the bps-derived bounty"
    );
}

#[test]
fn non_keeper_can_liquidate_short() {
    // Mirror of the long case for symmetry — a non-keeper liquidates an
    // underwater short. Validates the permissionless path does not silently
    // depend on position direction.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    let random_liquidator = Address::generate(&f.env);
    let keeper_role = Symbol::new(&f.env, "KEEPER");
    assert!(
        !f._config_client.has_role(&keeper_role, &random_liquidator),
        "Setup invariant: random_liquidator must not hold KEEPER"
    );

    open_short_position(&f, DEFAULT_SIZE, DEFAULT_COLLATERAL);

    let spike_price: i128 = 56_000 * PRECISION;
    advance_time_and_set_price(&f, TEST_TIMESTAMP + TIME_ADVANCE, spike_price);

    let liquidator_balance_before = f.usdc_client.balance(&random_liquidator);

    f.pm_client
        .liquidate_position(&random_liquidator, &f.trader, &symbol);

    f.env.as_contract(&f.pm_addr, || {
        let pos = storage::get_position(&f.env, &f.trader, &symbol);
        assert!(
            pos.is_none(),
            "Permissionless short liquidation must delete the position"
        );
    });

    let liquidator_balance_after = f.usdc_client.balance(&random_liquidator);
    let expected_bounty = DEFAULT_COLLATERAL * DEFAULT_BOUNTY_BPS / BPS;
    assert_eq!(
        liquidator_balance_after - liquidator_balance_before,
        expected_bounty,
        "Non-keeper short liquidator must receive the bounty"
    );
}

#[test]
#[should_panic]
fn liquidator_must_authorize_call() {
    // Regression-lock: the caller passed to `liquidate_position` is the
    // bounty recipient, so the contract MUST call `caller.require_auth()`.
    // Without that check, any third party could pass a victim's Address as
    // `caller` and have the protocol pay the bounty to the victim's account.
    //
    // Soroban's test harness defaults to `mock_all_auths()`, which makes
    // every `require_auth` call succeed silently. To assert the contract
    // actually invokes `require_auth(caller)` we open the position while
    // mock_all_auths is active, then strip auths with `set_auths(&[])`
    // before triggering the liquidation. The liquidation call passes no
    // auth entry for `caller`, so it must panic on the missing requirement.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    // Open the position while mock_all_auths is still active.
    open_long_position(&f, DEFAULT_SIZE, DEFAULT_COLLATERAL);

    // Crash the price so the position is liquidatable.
    let crash_price: i128 = 44_000 * PRECISION;
    advance_time_and_set_price(&f, TEST_TIMESTAMP + TIME_ADVANCE, crash_price);

    let random_liquidator = Address::generate(&f.env);

    // Strip blanket auth mocking. From this point forward, every
    // `require_auth` invocation must be backed by an explicit auth entry.
    f.env.set_auths(&[]);

    // No auth is granted for `random_liquidator`. The contract MUST call
    // `caller.require_auth()`, so this call must panic.
    f.pm_client
        .liquidate_position(&random_liquidator, &f.trader, &symbol);
}
