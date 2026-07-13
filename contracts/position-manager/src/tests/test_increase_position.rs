//! Tests for `increase_position`. Requires a full deployment: ConfigManager,
//! MockToken (USDC), MockOracle, OracleRouter, Vault, and PositionManager.

use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Ledger, LedgerInfo},
    Address, Env, Symbol,
};

use crate::contract::PositionManagerContract;
use crate::math;
use crate::PositionManagerClient;
use shared::constants::PRECISION;

use config_manager::{ConfigManagerClient, ConfigManagerContract};
use shared::FeeConfig;
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

/// Max utilization ratio: 85% = 8500 bps
const _MAX_UTIL_RATIO: i128 = 8_500;

/// Default position size: 10,000 USDC notional
const DEFAULT_SIZE: i128 = 10_000 * USDC_UNIT;

/// Default collateral: 1,000 USDC (10x leverage)
const DEFAULT_COLLATERAL: i128 = 1_000 * USDC_UNIT;

/// Ledger timestamp used in tests
const TEST_TIMESTAMP: u64 = 1_700_000_000;

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

/// Deploy and wire up ALL protocol contracts needed for increase_position tests.
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
            adl_pnl_bps: 9_000,
            adl_utilization_bps: 9_500,
            liquidation_threshold_bps: 200,
        },
    );

    config_client.update_carrying_fee_config(
        &admin,
        &config_manager::CarryingFeeConfig {
            base_borrow_rate_bps: 100,
            slope1_bps: 500,
            slope2_bps: 5_000,
            optimal_utilization_bps: 8_000,
            max_skew_rate_bps: 100,
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
        &oracle_sources,
    );
    oracle_router_client.set_oracle_sources(
        &admin,
        &symbol_short!("ETH"),
        &oracle_sources,
    );

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
    // VaultContract::deposit(assets, receiver, from, operator) -- ERC-4626 interface
    vault_client.deposit(&VAULT_DEPOSIT, &lp, &lp, &lp);

    // SAFETY: env lives in the fixture, clients borrow from it.
    // transmute extends the borrow lifetime to 'static for the fixture pattern.
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
// 1. Guard tests
// ===========================================================================

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn test_increase_position_reverts_when_paused() {
    // Scenario: Contract is initialized then paused. Calling increase_position
    // must revert with Paused (error code 3).
    let f = setup_full();
    f.pm_client.pause(&f.admin);
    f.pm_client.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0,
        &0i128,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn test_increase_position_reverts_on_zero_size() {
    // Scenario: Trader passes size=0. Must revert with ZeroAmount (error 8).
    // A zero-size position is nonsensical and should be rejected.
    let f = setup_full();
    f.pm_client.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &0_i128,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0,
        &0i128,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn test_increase_position_reverts_on_zero_collateral() {
    // Scenario: Trader passes collateral=0. Must revert with ZeroAmount (error 8).
    // Opening a position with zero collateral is infinite leverage -- must be rejected.
    let f = setup_full();
    f.pm_client.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &DEFAULT_SIZE,
        &0_i128,
        &true,
        &0,
        &0,
        &0i128,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn test_increase_position_reverts_on_negative_size() {
    // Adversarial: Trader passes a negative size to attempt underflow or bypass.
    // Must revert with ZeroAmount (error 8) since negative size is invalid.
    let f = setup_full();
    f.pm_client.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &(-1_000_i128),
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0,
        &0i128,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn test_increase_position_reverts_on_negative_collateral() {
    // Adversarial: Trader passes negative collateral to try to extract funds.
    // Must revert with ZeroAmount (error 8).
    let f = setup_full();
    f.pm_client.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &DEFAULT_SIZE,
        &(-500_i128),
        &true,
        &0,
        &0,
        &0i128,
    );
}

// ===========================================================================
// 2. Happy path -- new position
// ===========================================================================

#[test]
fn test_open_new_long_position_stores_correct_fields() {
    // Scenario: Trader opens a new long BTC position. After the call, the
    // stored Position must reflect the correct entry_price (from oracle),
    // collateral, size, is_long=true, and last_increased_time = current timestamp.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0,
        &0i128,
    );

    let pos = f.pm_client.get_position(&f.trader, &symbol);

    assert_eq!(
        pos.collateral, DEFAULT_COLLATERAL,
        "Collateral must match deposited amount"
    );
    assert_eq!(pos.size, DEFAULT_SIZE, "Size must match requested size");
    assert_eq!(
        pos.entry_price, BTC_PRICE,
        "Entry price must equal oracle mark price"
    );
    assert!(pos.is_long, "Position must be long");
    assert_eq!(
        pos.last_increased_time, TEST_TIMESTAMP,
        "last_increased_time must equal current ledger timestamp"
    );
}

#[test]
fn test_open_new_short_position_stores_correct_fields() {
    // Scenario: Trader opens a new short ETH position.
    let f = setup_full();
    let symbol = symbol_short!("ETH");

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &false,
        &0,
        &0,
        &0i128,
    );

    let pos = f.pm_client.get_position(&f.trader, &symbol);

    assert_eq!(pos.collateral, DEFAULT_COLLATERAL, "Collateral must match");
    assert_eq!(pos.size, DEFAULT_SIZE, "Size must match");
    assert_eq!(
        pos.entry_price, ETH_PRICE,
        "Entry price must equal ETH mark price"
    );
    assert!(!pos.is_long, "Position must be short");
    assert_eq!(
        pos.last_increased_time, TEST_TIMESTAMP,
        "Timestamp must match"
    );
}

#[test]
fn test_open_new_long_updates_market_info_oi() {
    // Scenario: After opening a new long, MarketInfo.long_open_interest must
    // increase by the position size, and short_open_interest stays zero.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0,
        &0i128,
    );

    let market = f.pm_client.get_market(&symbol);

    assert_eq!(
        market.long_open_interest, DEFAULT_SIZE,
        "Long OI must increase by position size"
    );
    assert_eq!(
        market.short_open_interest, 0,
        "Short OI must remain zero for a long position"
    );
    assert_eq!(
        market.global_long_avg_price, BTC_PRICE,
        "Global long avg price must equal entry price for first position"
    );
}

#[test]
fn test_open_new_short_updates_market_info_oi() {
    // Scenario: After opening a new short, short_open_interest must increase.
    let f = setup_full();
    let symbol = symbol_short!("ETH");

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &false,
        &0,
        &0,
        &0i128,
    );

    let market = f.pm_client.get_market(&symbol);

    assert_eq!(
        market.short_open_interest, DEFAULT_SIZE,
        "Short OI must increase by position size"
    );
    assert_eq!(
        market.long_open_interest, 0,
        "Long OI must remain zero for a short position"
    );
    assert_eq!(
        market.global_short_avg_price, ETH_PRICE,
        "Global short avg price must equal entry price for first short"
    );
}

#[test]
fn test_open_position_increases_total_reserved() {
    // Scenario: TotalReserved must increase by the position size after opening
    // a new position. This tracks how much vault liquidity is earmarked.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    // Verify initial total_reserved is zero
    f.env.as_contract(&f.pm_addr, || {
        assert_eq!(
            f.vault_client.reserved_usdc(),
            0,
            "TotalReserved must start at zero"
        );
    });

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0,
        &0i128,
    );

    f.env.as_contract(&f.pm_addr, || {
        assert_eq!(
            f.vault_client.reserved_usdc(),
            DEFAULT_SIZE,
            "TotalReserved must equal position size after first position"
        );
    });
}

#[test]
fn test_open_position_transfers_collateral_from_trader() {
    // Scenario: The trader's USDC balance must decrease by collateral amount,
    // and the PM contract address must receive that collateral.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    let balance_before = f.usdc_client.balance(&f.trader);

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0,
        &0i128,
    );

    let balance_after = f.usdc_client.balance(&f.trader);
    // Trader is charged collateral plus the sidecar open fee on the size
    // delta. With defaults (open_fee_bps = 10), the fee is DEFAULT_SIZE / 1000.
    let expected_open_fee = DEFAULT_SIZE / 1_000;
    assert_eq!(
        balance_before - balance_after,
        DEFAULT_COLLATERAL + expected_open_fee,
        "Trader USDC balance must decrease by collateral + open fee"
    );
}

#[test]
fn test_open_position_initializes_fee_debts_at_current_indices() {
    // New slices record debt baselines at the current carrying-fee indices.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0,
        &0i128,
    );

    let pos = f.pm_client.get_position(&f.trader, &symbol);
    let market = f.pm_client.get_market(&symbol);

    // Fee debt is the position notional valued at the current index.
    assert_eq!(
        pos.borrow_fee_debt,
        math::calc_fee_debt(&f.env, pos.size, market.acc_borrow_index),
        "Borrow fee debt must be initialized at the current index"
    );
    assert_eq!(
        pos.skew_fee_debt,
        math::calc_fee_debt(&f.env, pos.size, market.acc_long_skew_index),
        "Skew fee debt must be initialized at the current side index"
    );
}

// ===========================================================================
// 3. Happy path -- increase existing position
// ===========================================================================

#[test]
fn test_increase_existing_long_adds_size_and_collateral() {
    // Scenario: Trader opens a long, then increases it. The resulting position
    // must have accumulated size and collateral.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    // Open initial position
    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0,
        &0i128,
    );

    let additional_size = 5_000 * USDC_UNIT;
    let additional_collateral = 500 * USDC_UNIT;

    // Increase the position
    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &additional_size,
        &additional_collateral,
        &true,
        &0,
        &0,
        &0i128,
    );

    let pos = f.pm_client.get_position(&f.trader, &symbol);

    assert_eq!(
        pos.size,
        DEFAULT_SIZE + additional_size,
        "Size must be cumulative after increase"
    );
    assert_eq!(
        pos.collateral,
        DEFAULT_COLLATERAL + additional_collateral,
        "Collateral must be cumulative after increase"
    );
}

#[test]
fn test_increase_existing_position_derives_display_entry_from_exposure() {
    // Trader opens at price A, then increases at price B. The display entry
    // is derived from the sum of both exposure slices.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    // Open initial position at BTC_PRICE ($50,000)
    let size1 = 10_000 * USDC_UNIT;
    let col1 = 1_000 * USDC_UNIT;
    f.pm_client
        .increase_position(&f.trader, &symbol, &size1, &col1, &true, &0, &0, &0i128);

    // Change oracle price to $60,000 before second increase.
    // Advance time past the oracle router cache_duration (10s) so the new price is fetched.
    let new_price: i128 = 60_000 * PRECISION;
    f.oracle_client.set_price(&symbol, &new_price);
    f.env.ledger().set(LedgerInfo {
        timestamp: TEST_TIMESTAMP + 11,
        protocol_version: 23,
        sequence_number: 101,
        network_id: [0u8; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 100,
        min_persistent_entry_ttl: 100,
        max_entry_ttl: 10_000_000,
    });

    let size2 = 10_000 * USDC_UNIT;
    let col2 = 1_000 * USDC_UNIT;
    f.pm_client
        .increase_position(&f.trader, &symbol, &size2, &col2, &true, &0, &0, &0i128);

    let pos = f.pm_client.get_position(&f.trader, &symbol);

    // Exposure accounting derives the harmonic entry price.
    let expected_avg_price = math::derive_entry_price(
        &f.env,
        size1 + size2,
        math::calc_base_exposure(&f.env, size1, BTC_PRICE)
            + math::calc_base_exposure(&f.env, size2, new_price),
    );
    assert_eq!(
        pos.entry_price, expected_avg_price,
        "display entry must be derived from additive exposure"
    );
}

#[test]
fn test_increase_existing_position_updates_oi_correctly() {
    // Scenario: Two successive increases should result in cumulative OI.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    let size1 = 10_000 * USDC_UNIT;
    let size2 = 5_000 * USDC_UNIT;

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &size1,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0,
        &0i128,
    );
    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &size2,
        &(500 * USDC_UNIT),
        &true,
        &0,
        &0,
        &0i128,
    );

    let market = f.pm_client.get_market(&symbol);
    assert_eq!(
        market.long_open_interest,
        size1 + size2,
        "Long OI must be cumulative after two increases"
    );
}

#[test]
fn test_increase_existing_position_updates_total_reserved() {
    // Scenario: TotalReserved must increase by each position size increment.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    let size1 = 10_000 * USDC_UNIT;
    let size2 = 5_000 * USDC_UNIT;

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &size1,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0,
        &0i128,
    );
    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &size2,
        &(500 * USDC_UNIT),
        &true,
        &0,
        &0,
        &0i128,
    );

    f.env.as_contract(&f.pm_addr, || {
        assert_eq!(
            f.vault_client.reserved_usdc(),
            size1 + size2,
            "TotalReserved must reflect cumulative reservation"
        );
    });
}

#[test]
fn test_increase_position_updates_last_increased_time() {
    // Scenario: On each increase, last_increased_time must be reset to the
    // current ledger timestamp (anti-front-running lock).
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    // First increase at TEST_TIMESTAMP
    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0,
        &0i128,
    );

    let pos1 = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(pos1.last_increased_time, TEST_TIMESTAMP);

    // Advance time by 60 seconds
    let new_ts = TEST_TIMESTAMP + 60;
    f.env.ledger().set(LedgerInfo {
        timestamp: new_ts,
        protocol_version: 23,
        sequence_number: 101,
        network_id: [0u8; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 100,
        min_persistent_entry_ttl: 100,
        max_entry_ttl: 10_000_000,
    });

    // Also refresh oracle price so it does not become stale
    f.oracle_client.set_price(&symbol_short!("BTC"), &BTC_PRICE);

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &(1_000 * USDC_UNIT),
        &(100 * USDC_UNIT),
        &true,
        &0,
        &0,
        &0i128,
    );

    let pos2 = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(
        pos2.last_increased_time, new_ts,
        "last_increased_time must update to current timestamp on each increase"
    );
}

// ===========================================================================
// 4. Utilization cap
// ===========================================================================

#[test]
fn test_increase_position_succeeds_under_utilization_cap() {
    // Scenario: A position that keeps total utilization well under 85% should
    // succeed without issue. With 1M USDC in vault, a 10k position is fine.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    // This should succeed -- 10k / 1M = 1% utilization, well under 85%
    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0,
        &0i128,
    );

    // If we get here without panic, the test passes
    let pos = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(pos.size, DEFAULT_SIZE);
}

#[test]
#[should_panic(expected = "Error(Contract, #4)")]
fn test_increase_position_reverts_when_utilization_cap_breached() {
    // Scenario: Trader tries to open a position so large that it pushes
    // vault utilization above 85%. Must revert with UtilizationCapBreached (error 4).
    //
    // Vault has 1,000,000 USDC. 85% of that = 850,000. So a position of
    // 900,000 should breach the cap.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    let huge_size = 900_000 * USDC_UNIT;
    let huge_collateral = 90_000 * USDC_UNIT;

    // Mint extra USDC for the large collateral
    f.usdc_client.mint(&f.trader, &huge_collateral);

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &huge_size,
        &huge_collateral,
        &true,
        &0,
        &0,
        &0i128,
    );
}

#[test]
fn test_increase_position_at_exactly_max_utilization_succeeds() {
    // Scenario: Position that pushes utilization to EXACTLY 85% should
    // succeed (not strictly greater than). Boundary condition test.
    //
    // Vault has 1,000,000 USDC. 85% = 850,000 exactly.
    // calc_utilization_bps(850_000, free + 850_000) -- depends on formula.
    // With calc_utilization_bps: reserved * BPS / total_assets
    // At the cap: 850_000 * 10_000 / 1_000_000 = 8_500 = MAX_UTIL_RATIO
    // This should pass if the check is <= (not <).
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    let exact_cap_size = 850_000 * USDC_UNIT;
    let collateral = 85_000 * USDC_UNIT;

    // Mint extra USDC for the large collateral
    f.usdc_client.mint(&f.trader, &collateral);

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &exact_cap_size,
        &collateral,
        &true,
        &0,
        &0,
        &0i128,
    );

    let pos = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(
        pos.size, exact_cap_size,
        "Position at exact utilization cap must succeed"
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #4)")]
fn test_increase_position_just_over_utilization_cap_reverts() {
    // Scenario: Position that pushes utilization to 85% + 1 unit should fail.
    // This tests the exact boundary.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    // Slightly over 85% — need enough to exceed 8500 bps after integer division.
    // With 1M USDC total, need reserved * 10_000 / total > 8500,
    // i.e., reserved >= 850_100 USDC (to get 8501 bps).
    let over_cap_size = 851_000 * USDC_UNIT;
    let collateral = 86_000 * USDC_UNIT;

    f.usdc_client.mint(&f.trader, &(collateral + USDC_UNIT));

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &over_cap_size,
        &collateral,
        &true,
        &0,
        &0,
        &0i128,
    );
}

// ===========================================================================
// 5. Multiple traders / multiple symbols
// ===========================================================================

#[test]
fn test_two_traders_open_positions_same_symbol() {
    // Scenario: Two different traders open positions on the same symbol.
    // Each should have their own independent Position entry, and MarketInfo
    // should reflect the combined OI.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    let trader2 = Address::generate(&f.env);
    f.usdc_client.mint(&trader2, &TRADER_BALANCE);

    let size1 = 10_000 * USDC_UNIT;
    let size2 = 20_000 * USDC_UNIT;

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &size1,
        &(1_000 * USDC_UNIT),
        &true,
        &0,
        &0,
        &0i128,
    );
    f.pm_client.increase_position(
        &trader2,
        &symbol,
        &size2,
        &(2_000 * USDC_UNIT),
        &true,
        &0,
        &0,
        &0i128,
    );

    let pos1 = f.pm_client.get_position(&f.trader, &symbol);
    let pos2 = f.pm_client.get_position(&trader2, &symbol);

    assert_eq!(
        pos1.size, size1,
        "Trader1 position size must be independent"
    );
    assert_eq!(
        pos2.size, size2,
        "Trader2 position size must be independent"
    );

    let market = f.pm_client.get_market(&symbol);
    assert_eq!(
        market.long_open_interest,
        size1 + size2,
        "Total OI must reflect both positions"
    );
}

#[test]
fn test_same_trader_opens_different_symbols() {
    // Scenario: One trader opens positions on BTC and ETH. Each symbol
    // should have its own Position and MarketInfo.
    let f = setup_full();
    let btc = symbol_short!("BTC");
    let eth = symbol_short!("ETH");

    f.pm_client.increase_position(
        &f.trader,
        &btc,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0,
        &0i128,
    );
    f.pm_client.increase_position(
        &f.trader,
        &eth,
        &(5_000 * USDC_UNIT),
        &(500 * USDC_UNIT),
        &false,
        &0,
        &0,
        &0i128,
    );

    let btc_pos = f.pm_client.get_position(&f.trader, &btc);
    let eth_pos = f.pm_client.get_position(&f.trader, &eth);

    assert_eq!(btc_pos.size, DEFAULT_SIZE, "BTC position must be correct");
    assert!(btc_pos.is_long, "BTC must be long");
    assert_eq!(
        eth_pos.size,
        5_000 * USDC_UNIT,
        "ETH position must be correct"
    );
    assert!(!eth_pos.is_long, "ETH must be short");

    let btc_market = f.pm_client.get_market(&btc);
    let eth_market = f.pm_client.get_market(&eth);
    assert_eq!(btc_market.long_open_interest, DEFAULT_SIZE);
    assert_eq!(eth_market.short_open_interest, 5_000 * USDC_UNIT);
}

// ===========================================================================
// 6. Edge cases and adversarial scenarios
// ===========================================================================

#[test]
#[should_panic(expected = "Error(Contract, #16)")]
fn test_minimum_position_size() {
    // Scenario: Dust positions (below min_collateral) must be rejected.
    // min_collateral is set to 1_000_000 (1 USDC) in test setup.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    f.pm_client.increase_position(
        &f.trader, &symbol, &1_i128, // 1 unit of size
        &1_i128, // 1 unit of collateral — below min_collateral
        &true, &0, &0, &0i128,
    );
}

#[test]
fn test_max_leverage_position_at_boundary() {
    // Scenario: Position at exactly MAX_LEVERAGE (100x) should succeed.
    // size = collateral * 100 exactly.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    // 100x leverage: 100 USDC collateral, 10,000 USDC size
    let high_lev_collateral = 100 * USDC_UNIT;
    let high_lev_size = 10_000 * USDC_UNIT;

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &high_lev_size,
        &high_lev_collateral,
        &true,
        &0,
        &0,
        &0i128,
    );

    let pos = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(
        pos.size, high_lev_size,
        "100x leverage position must be stored"
    );
    assert_eq!(pos.collateral, high_lev_collateral, "Collateral must match");
}

#[test]
#[should_panic(expected = "Error(Contract, #11)")]
fn test_excessive_leverage_reverts() {
    // Scenario: Position exceeding MAX_LEVERAGE (>100x) must revert with
    // ExcessiveLeverage (error 11).
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    // 101x leverage: 100 USDC collateral, 10,100 USDC size
    let collateral = 100 * USDC_UNIT;
    let size = 10_100 * USDC_UNIT;

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &size,
        &collateral,
        &true,
        &0,
        &0,
        &0i128,
    );
}

#[test]
fn test_vault_reserve_liquidity_called() {
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    let reserved_before = f.vault_client.reserved_usdc();

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0,
        &0i128,
    );

    let reserved_after = f.vault_client.reserved_usdc();

    assert_eq!(
        reserved_after - reserved_before,
        DEFAULT_SIZE,
        "Vault reserved_usdc must grow by exactly the position size."
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn test_open_position_with_both_zero_reverts() {
    // Adversarial: Both size and collateral are zero. Must still revert ZeroAmount (error 8).
    let f = setup_full();
    f.pm_client.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &0_i128,
        &0_i128,
        &true,
        &0,
        &0,
        &0i128,
    );
}

#[test]
fn test_global_display_price_derived_after_multiple_traders() {
    // Two traders open longs at different prices. The global display price
    // must be derived from aggregate exposure.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    let trader2 = Address::generate(&f.env);
    f.usdc_client.mint(&trader2, &TRADER_BALANCE);

    // Trader1: 10k size at $50,000
    let size1 = 10_000 * USDC_UNIT;
    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &size1,
        &(1_000 * USDC_UNIT),
        &true,
        &0,
        &0,
        &0i128,
    );

    // Change price to $60,000.
    // Advance time past the oracle router cache_duration (10s) so the new price is fetched.
    let new_price: i128 = 60_000 * PRECISION;
    f.oracle_client.set_price(&symbol, &new_price);
    f.env.ledger().set(LedgerInfo {
        timestamp: TEST_TIMESTAMP + 11,
        protocol_version: 23,
        sequence_number: 101,
        network_id: [0u8; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 100,
        min_persistent_entry_ttl: 100,
        max_entry_ttl: 10_000_000,
    });

    // Trader2: 20k size at $60,000
    let size2 = 20_000 * USDC_UNIT;
    f.pm_client.increase_position(
        &trader2,
        &symbol,
        &size2,
        &(2_000 * USDC_UNIT),
        &true,
        &0,
        &0,
        &0i128,
    );

    let market = f.pm_client.get_market(&symbol);

    let expected = math::derive_entry_price(
        &f.env,
        size1 + size2,
        math::calc_base_exposure(&f.env, size1, BTC_PRICE)
            + math::calc_base_exposure(&f.env, size2, new_price),
    );
    assert_eq!(
        market.global_long_avg_price, expected,
        "Global avg price must be volume-weighted across all traders"
    );
}

#[test]
fn test_increase_position_does_not_affect_opposite_side_oi() {
    // Scenario: Opening a long must not change short_open_interest, and vice versa.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0,
        &0i128,
    );

    let market = f.pm_client.get_market(&symbol);
    assert_eq!(
        market.short_open_interest, 0,
        "Short OI must not be affected by long open"
    );
    assert_eq!(
        market.global_short_avg_price, 0,
        "Short avg price must remain zero"
    );
}

#[test]
#[should_panic]
fn test_trader_collateral_insufficient_reverts() {
    // Adversarial: Trader tries to deposit more collateral than they own.
    // The token transfer should fail, causing a revert.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    // Trader has TRADER_BALANCE USDC; try to deposit more than that
    let excess_collateral = TRADER_BALANCE + USDC_UNIT;

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &excess_collateral,
        &true,
        &0,
        &0,
        &0i128,
    );
}

#[test]
fn test_cumulative_utilization_across_multiple_positions() {
    // Scenario: Multiple positions accumulate utilization. The third position
    // should fail if cumulative utilization breaches the cap.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    let trader2 = Address::generate(&f.env);
    let trader3 = Address::generate(&f.env);
    f.usdc_client.mint(&trader2, &TRADER_BALANCE);
    f.usdc_client.mint(&trader3, &TRADER_BALANCE);

    // Position 1: 400k (40% utilization)
    f.usdc_client.mint(&f.trader, &(400_000 * USDC_UNIT));
    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &(400_000 * USDC_UNIT),
        &(40_000 * USDC_UNIT),
        &true,
        &0,
        &0,
        &0i128,
    );

    // Position 2: 400k (cumulative 80% utilization -- under cap)
    f.usdc_client.mint(&trader2, &(400_000 * USDC_UNIT));
    f.pm_client.increase_position(
        &trader2,
        &symbol,
        &(400_000 * USDC_UNIT),
        &(40_000 * USDC_UNIT),
        &true,
        &0,
        &0,
        &0i128,
    );

    // Position 3: 100k would push to 90% utilization -- should breach cap.
    // We cannot use #[should_panic] on this test because the first two
    // positions must succeed. Instead, use try_increase_position via
    // a separate test function.

    // Verify cumulative total_reserved is 800k
    f.env.as_contract(&f.pm_addr, || {
        assert_eq!(
            f.vault_client.reserved_usdc(),
            800_000 * USDC_UNIT,
            "TotalReserved must be 800k after two 400k positions"
        );
    });
}

// ===========================================================================
// 7. Open-fee + TP/SL execution-fee escrow charging
//
// Every increase_position call charges an open fee on the size DELTA computed
// as `size * open_fee_bps / BPS`. The fee is taken on TOP of the collateral
// (sidecar) and forwarded to the vault. Vault.accrue_fees is called with the
// non-LP slice; the LP slice stays in vault total_assets implicitly.
//
// The first time a position has TP or SL set, the flat `tp_sl_execution_fee`
// from FeeConfig is charged once and stored in position.execution_fee_escrow.
// Subsequent increases that touch TP/SL while escrow > 0 must NOT re-charge.
//
// The default FeeConfig planted by ConfigManager::initialize is:
//   open_fee_bps                = DEFAULT_OPEN_FEE_BPS                = 10
//   liquidation_bounty_bps      = DEFAULT_LIQUIDATION_BOUNTY_BPS      = 100
//   tp_sl_execution_fee         = DEFAULT_TP_SL_EXECUTION_FEE         = 5_000_000
//
// For DEFAULT_SIZE = 10_000 * USDC_UNIT = 10_000_000_000, open_fee at 10 bps
// is 10_000_000_000 * 10 / 10_000 = 10_000_000.
// ===========================================================================

/// Open-fee charged for a given size delta at the default 10 bps rate.
const DEFAULT_OPEN_FEE: i128 = DEFAULT_SIZE / 1_000; // 10 bps == /1000

/// Default flat TP/SL execution fee planted by ConfigManager::initialize.
const TP_SL_ESCROW: i128 = 5_000_000;

/// A TP price strictly above BTC_PRICE — valid for a long.
const VALID_TP_LONG: i128 = 55_000 * PRECISION;
/// An SL price strictly below BTC_PRICE — valid for a long.
const VALID_SL_LONG: i128 = 45_000 * PRECISION;

#[test]
fn test_open_position_no_tp_sl_charges_open_fee() {
    // Opening with TP=SL=0 charges only collateral + open_fee on the size delta.
    // Trader pays collateral + open_fee; vault total_assets grows by open_fee;
    // PM holds collateral only; position.execution_fee_escrow == 0.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    let trader_balance_before = f.usdc_client.balance(&f.trader);
    // Raw custody = LP basis + unclaimed fee buffer.
    let vault_custody_before = f.vault_client.total_assets() + f.vault_client.unclaimed_fees();
    let pm_balance_before = f.usdc_client.balance(&f.pm_addr);

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0,
        &0i128,
    );

    let trader_balance_after = f.usdc_client.balance(&f.trader);
    let vault_custody_after = f.vault_client.total_assets() + f.vault_client.unclaimed_fees();
    let pm_balance_after = f.usdc_client.balance(&f.pm_addr);

    assert_eq!(
        trader_balance_before - trader_balance_after,
        DEFAULT_COLLATERAL + DEFAULT_OPEN_FEE,
        "Trader must pay collateral + open_fee"
    );
    assert_eq!(
        vault_custody_after - vault_custody_before,
        DEFAULT_OPEN_FEE,
        "Vault custody must grow by exactly the open fee"
    );
    assert_eq!(
        pm_balance_after - pm_balance_before,
        DEFAULT_COLLATERAL,
        "PM must hold only the collateral; fee is forwarded to vault"
    );

    let pos = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(
        pos.collateral, DEFAULT_COLLATERAL,
        "Stored collateral must equal the param (fee is sidecar, not deducted)"
    );
    assert_eq!(
        pos.execution_fee_escrow, 0,
        "No TP or SL means no execution-fee escrow"
    );
}

#[test]
fn test_open_position_with_zero_open_fee_bps_no_fee_charged() {
    // With open_fee_bps == 0, the trader pays only collateral.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    f._config_client.set_fee_config(
        &f.admin,
        &FeeConfig {
            open_fee_bps: 0,
            liquidation_bounty_bps: 100,
            tp_sl_execution_fee: TP_SL_ESCROW,
        },
    );

    let trader_balance_before = f.usdc_client.balance(&f.trader);
    let vault_total_before = f.vault_client.total_assets();

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0,
        &0i128,
    );

    let trader_balance_after = f.usdc_client.balance(&f.trader);
    let vault_total_after = f.vault_client.total_assets();

    assert_eq!(
        trader_balance_before - trader_balance_after,
        DEFAULT_COLLATERAL,
        "Trader must pay only collateral when open_fee_bps == 0"
    );
    assert_eq!(
        vault_total_after, vault_total_before,
        "Vault total_assets must NOT grow when open_fee_bps == 0"
    );
}

#[test]
fn test_open_position_with_max_open_fee_bps_correct_amount() {
    // open_fee_bps = 100 (1%). Fee on DEFAULT_SIZE = DEFAULT_SIZE / 100.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    f._config_client.set_fee_config(
        &f.admin,
        &FeeConfig {
            open_fee_bps: 100,
            liquidation_bounty_bps: 100,
            tp_sl_execution_fee: TP_SL_ESCROW,
        },
    );

    let expected_fee = DEFAULT_SIZE / 100;

    let trader_balance_before = f.usdc_client.balance(&f.trader);
    let vault_custody_before = f.vault_client.total_assets() + f.vault_client.unclaimed_fees();

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0,
        &0i128,
    );

    let trader_balance_after = f.usdc_client.balance(&f.trader);
    let vault_custody_after = f.vault_client.total_assets() + f.vault_client.unclaimed_fees();

    assert_eq!(
        trader_balance_before - trader_balance_after,
        DEFAULT_COLLATERAL + expected_fee,
        "Trader must pay collateral + 1% of size at max open_fee_bps"
    );
    assert_eq!(
        vault_custody_after - vault_custody_before,
        expected_fee,
        "Vault custody must grow by exactly the 1% open fee"
    );
}

#[test]
fn test_open_fee_accrues_non_lp_portion_to_vault_unclaimed_fees() {
    // The non-LP slice (dev_bps + staker_bps) of the open fee is accrued into
    // vault.unclaimed_fees via vault.accrue_fees. The LP slice stays in
    // total_assets implicitly. With defaults FeeSplits { lp:9000, dev:500,
    // staker:500 }, the non-LP slice is 1000 bps == 10% of the open fee.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    // free_liquidity = total_assets - reserved - unclaimed_fees - max(0, pnl)
    // Before: free_liq = total - 0 - 0 - 0 = total
    let total_before = f.vault_client.total_assets();
    let free_before = f.vault_client.free_liquidity();
    assert_eq!(
        total_before, free_before,
        "Precondition: no fees, no reservations, free == total"
    );

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0,
        &0i128,
    );

    let total_after = f.vault_client.total_assets();
    let free_after = f.vault_client.free_liquidity();

    // total_assets is the LP basis (custody minus unclaimed fees), so it
    // grows by the LP slice only; the non-LP slice lands in unclaimed_fees.
    // free_after  = total_after - reserved
    //             = (total_before + lp_fee) - size
    // free_before - free_after = size - lp_fee
    let expected_non_lp_fee = DEFAULT_OPEN_FEE * 1_000 / 10_000; // dev+staker = 1000 bps
    let expected_lp_fee = DEFAULT_OPEN_FEE - expected_non_lp_fee;

    assert_eq!(
        total_after - total_before,
        expected_lp_fee,
        "Vault total_assets (LP basis) grows by the LP slice of the open_fee"
    );
    assert_eq!(
        f.vault_client.unclaimed_fees(),
        expected_non_lp_fee,
        "The non-LP slice of the open_fee lands in unclaimed_fees"
    );
    // free dropped by `size` (reserved) and additionally by the non-LP slice
    // (now in unclaimed_fees), but rose by the LP slice (implicit in total).
    // Net: free_before - free_after == size + non_lp_fee - open_fee == size - lp_fee
    assert_eq!(
        free_before - free_after,
        DEFAULT_SIZE - expected_lp_fee,
        "Free liquidity drop == reserved size minus the LP slice of the fee"
    );
}

#[test]
fn test_open_position_with_tp_charges_escrow() {
    // Opening with non-zero TP and zero SL charges the flat tp_sl_execution_fee
    // once. Trader pays collateral + open_fee + tp_sl_execution_fee. The escrow
    // amount stays in PM (NOT moved to vault); position.execution_fee_escrow
    // records it. Vault grows only by the open fee.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    let trader_balance_before = f.usdc_client.balance(&f.trader);
    let vault_custody_before = f.vault_client.total_assets() + f.vault_client.unclaimed_fees();
    let pm_balance_before = f.usdc_client.balance(&f.pm_addr);

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &VALID_TP_LONG,
        &0,
        &0i128,
    );

    let trader_balance_after = f.usdc_client.balance(&f.trader);
    let vault_custody_after = f.vault_client.total_assets() + f.vault_client.unclaimed_fees();
    let pm_balance_after = f.usdc_client.balance(&f.pm_addr);

    assert_eq!(
        trader_balance_before - trader_balance_after,
        DEFAULT_COLLATERAL + DEFAULT_OPEN_FEE + TP_SL_ESCROW,
        "Trader must pay collateral + open_fee + escrow"
    );
    assert_eq!(
        vault_custody_after - vault_custody_before,
        DEFAULT_OPEN_FEE,
        "Vault custody grows only by the open fee; escrow does NOT flow to vault"
    );
    assert_eq!(
        pm_balance_after - pm_balance_before,
        DEFAULT_COLLATERAL + TP_SL_ESCROW,
        "PM must hold collateral + escrow"
    );

    let pos = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(
        pos.execution_fee_escrow, TP_SL_ESCROW,
        "Position must record the escrowed execution fee"
    );
    assert_eq!(
        pos.take_profit, VALID_TP_LONG,
        "TP must be set on the position"
    );
}

#[test]
fn test_open_position_with_sl_charges_escrow() {
    // Opening with zero TP and non-zero SL charges the same flat escrow.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    let trader_balance_before = f.usdc_client.balance(&f.trader);

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &VALID_SL_LONG,
        &0i128,
    );

    let trader_balance_after = f.usdc_client.balance(&f.trader);
    assert_eq!(
        trader_balance_before - trader_balance_after,
        DEFAULT_COLLATERAL + DEFAULT_OPEN_FEE + TP_SL_ESCROW,
        "SL-only open must still charge collateral + open_fee + escrow"
    );

    let pos = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(pos.execution_fee_escrow, TP_SL_ESCROW);
    assert_eq!(pos.stop_loss, VALID_SL_LONG);
    assert_eq!(pos.take_profit, 0);
}

#[test]
fn test_open_position_with_both_tp_and_sl_charges_single_escrow() {
    // Setting BOTH TP and SL on open charges the escrow exactly ONCE — it is
    // a per-position flat fee, NOT per trigger.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    let trader_balance_before = f.usdc_client.balance(&f.trader);

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &VALID_TP_LONG,
        &VALID_SL_LONG,
        &0i128,
    );

    let trader_balance_after = f.usdc_client.balance(&f.trader);
    assert_eq!(
        trader_balance_before - trader_balance_after,
        DEFAULT_COLLATERAL + DEFAULT_OPEN_FEE + TP_SL_ESCROW,
        "Both TP and SL set must charge exactly ONE flat escrow"
    );

    let pos = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(
        pos.execution_fee_escrow, TP_SL_ESCROW,
        "Escrow stored is a SINGLE flat fee, not 2x"
    );
}

#[test]
fn test_open_position_with_zero_tp_sl_no_escrow_charged() {
    // With both TP and SL == 0, no escrow is charged regardless of the
    // configured tp_sl_execution_fee value.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    let trader_balance_before = f.usdc_client.balance(&f.trader);

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0,
        &0i128,
    );

    let trader_balance_after = f.usdc_client.balance(&f.trader);
    assert_eq!(
        trader_balance_before - trader_balance_after,
        DEFAULT_COLLATERAL + DEFAULT_OPEN_FEE,
        "Without TP/SL the trader must NOT pay the escrow"
    );

    let pos = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(
        pos.execution_fee_escrow, 0,
        "No TP/SL means execution_fee_escrow stays at 0"
    );
}

#[test]
fn test_open_position_with_zero_execution_fee_no_charge_even_with_tp() {
    // If admin sets tp_sl_execution_fee to 0, opening with a TP must not
    // charge any escrow and position.execution_fee_escrow must remain 0.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    f._config_client.set_fee_config(
        &f.admin,
        &FeeConfig {
            open_fee_bps: 10,
            liquidation_bounty_bps: 100,
            tp_sl_execution_fee: 0,
        },
    );

    let trader_balance_before = f.usdc_client.balance(&f.trader);

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &VALID_TP_LONG,
        &0,
        &0i128,
    );

    let trader_balance_after = f.usdc_client.balance(&f.trader);
    assert_eq!(
        trader_balance_before - trader_balance_after,
        DEFAULT_COLLATERAL + DEFAULT_OPEN_FEE,
        "Zero configured escrow must result in zero escrow charge"
    );

    let pos = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(
        pos.execution_fee_escrow, 0,
        "Position escrow must be 0 when tp_sl_execution_fee == 0"
    );
}

// ---------------------------------------------------------------------------
// Add-to-position fee mechanics
// ---------------------------------------------------------------------------

#[test]
fn test_add_to_position_charges_open_fee_on_size_delta_only() {
    // Trader opens with size = DEFAULT_SIZE then adds size_delta = half.
    // The second call's open fee must be computed on the DELTA only, not the
    // cumulative total. Cumulative trader spend = 1.5 * DEFAULT_SIZE worth of
    // fee, NOT 2.5 * DEFAULT_SIZE worth.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    let add_size: i128 = DEFAULT_SIZE / 2;
    let add_collateral: i128 = DEFAULT_COLLATERAL / 2;
    let expected_fee_open_1 = DEFAULT_SIZE / 1_000;
    let expected_fee_open_2 = add_size / 1_000;

    let balance_before = f.usdc_client.balance(&f.trader);

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0,
        &0i128,
    );
    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &add_size,
        &add_collateral,
        &true,
        &0,
        &0,
        &0i128,
    );

    let balance_after = f.usdc_client.balance(&f.trader);

    let expected_total_spend = DEFAULT_COLLATERAL
        + add_collateral
        + expected_fee_open_1
        + expected_fee_open_2;

    assert_eq!(
        balance_before - balance_after,
        expected_total_spend,
        "Open fee on add MUST be charged on size DELTA, not cumulative size"
    );
}

#[test]
fn test_add_to_position_no_second_escrow_if_already_paid() {
    // The position already has an escrow recorded from the first open.
    // Adding to it (with TP unchanged or explicitly re-asserted) MUST NOT
    // double-charge the trader.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &VALID_TP_LONG,
        &0,
        &0i128,
    );

    let balance_after_open = f.usdc_client.balance(&f.trader);

    let add_size: i128 = DEFAULT_SIZE / 4;
    let add_collateral: i128 = DEFAULT_COLLATERAL / 4;
    let expected_fee_delta = add_size / 1_000;

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &add_size,
        &add_collateral,
        &true,
        &VALID_TP_LONG,
        &0,
        &0i128,
    );

    let balance_after_add = f.usdc_client.balance(&f.trader);

    assert_eq!(
        balance_after_open - balance_after_add,
        add_collateral + expected_fee_delta,
        "Add must charge ONLY collateral + open_fee (no second escrow)"
    );

    let pos = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(
        pos.execution_fee_escrow, TP_SL_ESCROW,
        "Escrow on the position must remain unchanged after re-asserting TP"
    );
}

#[test]
fn test_add_to_position_charges_escrow_if_first_time_setting_tp() {
    // First open is plain (no TP, no escrow). The second call introduces a
    // non-zero TP for the FIRST time — the escrow MUST be charged then.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0,
        &0i128,
    );

    let pos_after_open = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(
        pos_after_open.execution_fee_escrow, 0,
        "Precondition: plain open leaves escrow at 0"
    );

    let balance_after_open = f.usdc_client.balance(&f.trader);

    let add_size: i128 = DEFAULT_SIZE / 4;
    let add_collateral: i128 = DEFAULT_COLLATERAL / 4;
    let expected_fee_delta = add_size / 1_000;

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &add_size,
        &add_collateral,
        &true,
        &VALID_TP_LONG,
        &0,
        &0i128,
    );

    let balance_after_add = f.usdc_client.balance(&f.trader);

    assert_eq!(
        balance_after_open - balance_after_add,
        add_collateral + expected_fee_delta + TP_SL_ESCROW,
        "First time setting TP on add must charge the escrow"
    );

    let pos = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(
        pos.execution_fee_escrow, TP_SL_ESCROW,
        "Escrow must be recorded on the position after first TP set"
    );
}

#[test]
fn test_add_to_position_keeps_existing_escrow_when_tp_zero_in_increase_call() {
    // Existing semantics: passing take_profit=0 to an increase means "do not
    // change current TP". The position retains its TP and its escrow. The add
    // must NOT re-charge the escrow.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &VALID_TP_LONG,
        &0,
        &0i128,
    );

    let balance_after_open = f.usdc_client.balance(&f.trader);

    let add_size: i128 = DEFAULT_SIZE / 4;
    let add_collateral: i128 = DEFAULT_COLLATERAL / 4;
    let expected_fee_delta = add_size / 1_000;

    // take_profit = 0 here means "leave TP untouched", per the existing
    // do_increase_position semantics.
    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &add_size,
        &add_collateral,
        &true,
        &0,
        &0,
        &0i128,
    );

    let balance_after_add = f.usdc_client.balance(&f.trader);

    assert_eq!(
        balance_after_open - balance_after_add,
        add_collateral + expected_fee_delta,
        "Add with tp=0,sl=0 must NOT re-charge escrow when position already has one"
    );

    let pos = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(
        pos.take_profit, VALID_TP_LONG,
        "TP must be preserved when increase passes 0 (existing semantics)"
    );
    assert_eq!(
        pos.execution_fee_escrow, TP_SL_ESCROW,
        "Escrow must remain unchanged when no new TP/SL state is introduced"
    );
}

// ---------------------------------------------------------------------------
// Min-collateral / leverage interactions
// ---------------------------------------------------------------------------

#[test]
fn test_min_collateral_check_uses_param_not_post_fee() {
    // The min_collateral check uses the collateral parameter passed by the
    // trader, NOT the param minus fees. Fee is a sidecar — it is taken on TOP
    // of the collateral. Passing collateral == min_collateral exactly must
    // succeed.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    let limits = f._config_client.get_protocol_limits();
    let min_col = limits.min_collateral;

    // Size scaled to keep leverage within bounds: collateral * 10.
    let size = min_col * 10;

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &size,
        &min_col,
        &true,
        &0,
        &0,
        &0i128,
    );

    let pos = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(
        pos.collateral, min_col,
        "Position at the min_collateral boundary must succeed (fee does not reduce stored collateral)"
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #16)")]
fn test_collateral_below_min_panics_even_with_fee_logic() {
    // Adversarial: trader passes collateral < min_collateral. The new fee
    // logic must NOT silently top up the collateral to compensate. The pre-fee
    // BelowMinCollateral check (error 16) still applies.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    let limits = f._config_client.get_protocol_limits();
    let too_low = limits.min_collateral - 1;
    let size = too_low * 10;

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &size,
        &too_low,
        &true,
        &0,
        &0,
        &0i128,
    );
}

// ---------------------------------------------------------------------------
// Auth / balance
// ---------------------------------------------------------------------------

#[test]
#[should_panic]
fn test_trader_must_have_sufficient_balance_for_collateral_plus_fees() {
    // Adversarial: trader balance is EXACTLY collateral. The fee + escrow
    // bring the required total above the balance, so the bundled transfer
    // must fail (token-side panic).
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    // Burn excess trader balance down to exactly DEFAULT_COLLATERAL.
    let balance = f.usdc_client.balance(&f.trader);
    let burn_amount = balance - DEFAULT_COLLATERAL;
    if burn_amount > 0 {
        f.usdc_client.burn(&f.trader, &burn_amount);
    }

    // This must panic: trader has only DEFAULT_COLLATERAL but owes
    // DEFAULT_COLLATERAL + DEFAULT_OPEN_FEE + TP_SL_ESCROW.
    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &VALID_TP_LONG,
        &VALID_SL_LONG,
        &0i128,
    );
}

// ---------------------------------------------------------------------------
// Reservation vs fee accounting
// ---------------------------------------------------------------------------

#[test]
fn test_vault_reserved_increases_by_size_not_fee_inclusive() {
    // The vault's ReservedUsdc must grow by the position notional SIZE only.
    // Fees never reserve liquidity — they are revenue, not collateralisation.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    let reserved_before = f.vault_client.reserved_usdc();

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &VALID_TP_LONG,
        &VALID_SL_LONG,
        &0i128,
    );

    let reserved_after = f.vault_client.reserved_usdc();

    assert_eq!(
        reserved_after - reserved_before,
        DEFAULT_SIZE,
        "ReservedUsdc must grow by SIZE only — open fee and escrow do NOT reserve"
    );
}

// ===========================================================================
// Slippage (`acceptable_price`) tests
// ===========================================================================
//
// For opens: long reverts when mark > acceptable; short reverts when
// mark < acceptable. `acceptable_price = 0` bypasses the check.

#[test]
#[should_panic(expected = "Error(Contract, #19)")]
fn test_increase_long_reverts_when_mark_above_acceptable() {
    let f = setup_full();
    let symbol = symbol_short!("BTC");
    // Mark $50k, trader bounds to $49k → revert
    let acceptable = 49_000 * PRECISION;

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0,
        &acceptable,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #19)")]
fn test_increase_short_reverts_when_mark_below_acceptable() {
    let f = setup_full();
    let symbol = symbol_short!("BTC");
    // Mark $50k, trader bounds to $51k → revert (shorts want high mark)
    let acceptable = 51_000 * PRECISION;

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &false,
        &0,
        &0,
        &acceptable,
    );
}

#[test]
fn test_increase_long_succeeds_when_mark_below_acceptable() {
    let f = setup_full();
    let symbol = symbol_short!("BTC");
    let acceptable = 51_000 * PRECISION;

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0,
        &acceptable,
    );
    let pos = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(pos.entry_price, BTC_PRICE);
}

#[test]
fn test_increase_short_succeeds_when_mark_above_acceptable() {
    let f = setup_full();
    let symbol = symbol_short!("BTC");
    let acceptable = 49_000 * PRECISION;

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &false,
        &0,
        &0,
        &acceptable,
    );
    let pos = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(pos.entry_price, BTC_PRICE);
}

#[test]
fn test_increase_zero_acceptable_price_bypasses_check() {
    // Default opt-out behaviour: `acceptable_price = 0` skips the slippage
    // gate entirely, regardless of mark.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0,
        &0_i128,
    );
    let pos = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(pos.entry_price, BTC_PRICE);
}

#[test]
fn test_increase_boundary_mark_equals_acceptable_long() {
    // Exact equality at the inclusive bound (mark <= acceptable for longs).
    let f = setup_full();
    let symbol = symbol_short!("BTC");
    let acceptable = BTC_PRICE;

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0,
        &acceptable,
    );
    let pos = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(pos.entry_price, BTC_PRICE);
}

#[test]
fn test_increase_boundary_mark_equals_acceptable_short() {
    let f = setup_full();
    let symbol = symbol_short!("BTC");
    let acceptable = BTC_PRICE;

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &false,
        &0,
        &0,
        &acceptable,
    );
    let pos = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(pos.entry_price, BTC_PRICE);
}
