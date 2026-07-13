//! Tests for `set_tp_sl` and `execute_order` (take-profit / stop-loss).
//!
//! `set_tp_sl`: trader-authed; requires position; TP/SL placement must respect
//! the side (longs TP > entry, SL < entry; shorts inverted); 0 disables the
//! order.
//!
//! `execute_order`: permissionless; not-paused; uses oracle mark; full close
//! when TP or SL is triggered, else `OrderNotTriggered`.

use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events, Ledger, LedgerInfo},
    Address, Env, Symbol,
};

use crate::contract::PositionManagerContract;
use shared::constants::PRECISION;
use crate::storage;
use crate::PositionManagerClient;

use config_manager::{ConfigManagerClient, ConfigManagerContract};
use super::helpers::{self, DualOracle};
use mock_token::{MockToken, MockTokenClient};
use oracle_router::{OracleConfig, OracleRouterClient, OracleRouterContract};
use shared::FeeConfig;
use soroban_sdk::TryIntoVal;
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
    keeper: Address,
    pm_addr: Address,
    _vault_addr: Address,
}

/// Deploy and wire up ALL protocol contracts needed for TP/SL tests.
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
    let keeper = Address::generate(&env);
    let lp = Address::generate(&env);

    // --- 1. ConfigManager ---
    let config_id = env.register(ConfigManagerContract, (admin.clone(),));
    let config_client = ConfigManagerClient::new(&env, &config_id);

    // Grant roles needed by other contracts
    let pauser_role = Symbol::new(&env, "PAUSER");
    let keeper_role = Symbol::new(&env, "KEEPER");
    let _admin_role = Symbol::new(&env, "ADMIN");
    config_client.grant_role(&admin, &pauser_role, &admin);
    config_client.grant_role(&admin, &keeper_role, &keeper);

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
    usdc_client.mint(&trader, &TRADER_BALANCE);
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
        keeper,
        pm_addr: pm_id,
        _vault_addr: vault_id,
    }
}

// ===========================================================================
// Helpers
// ===========================================================================

/// Advance ledger to a new timestamp.
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

/// Open a BTC position (long or short) with default size/collateral and no TP/SL.
fn open_btc_position(f: &TestFixture, is_long: bool) {
    f.pm_client.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &is_long,
        &0,
        &0, &0i128
    );
}

/// Open a BTC position with specific TP/SL.
fn open_btc_position_with_tp_sl(f: &TestFixture, is_long: bool, tp: i128, sl: i128) {
    f.pm_client.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &is_long,
        &tp,
        &sl, &0i128
    );
}

/// Advance time past MinPositionLifetime + oracle cache, and refresh oracle.
fn advance_past_lifetime(f: &TestFixture) {
    advance_time(f, TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 11);
    f.oracle_client.set_price(&symbol_short!("BTC"), &BTC_PRICE);
}

/// Advance time past MinPositionLifetime + oracle cache, and set a custom price.
fn advance_and_set_price(f: &TestFixture, price: i128) {
    advance_time(f, TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 11);
    f.oracle_client.set_price(&symbol_short!("BTC"), &price);
}

// ===========================================================================
// 1. set_tp_sl tests
// ===========================================================================

#[test]
fn test_set_tp_sl_long_success() {
    // Open a long BTC at $50,000. Set TP=$55,000 (above entry) and SL=$45,000
    // (below entry). Both are valid for longs.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    open_btc_position(&f, true);

    let tp = 55_000 * PRECISION;
    let sl = 45_000 * PRECISION;
    f.pm_client.set_tp_sl(&f.trader, &symbol, &tp, &sl);

    let pos = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(
        pos.take_profit, tp,
        "Take-profit must be updated to {} for long, got {}",
        tp, pos.take_profit
    );
    assert_eq!(
        pos.stop_loss, sl,
        "Stop-loss must be updated to {} for long, got {}",
        sl, pos.stop_loss
    );
}

#[test]
fn test_set_tp_sl_short_success() {
    // Open a short BTC at $50,000. Set TP=$45,000 (below entry) and SL=$55,000
    // (above entry). Both are valid for shorts.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    open_btc_position(&f, false);

    let tp = 45_000 * PRECISION;
    let sl = 55_000 * PRECISION;
    f.pm_client.set_tp_sl(&f.trader, &symbol, &tp, &sl);

    let pos = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(
        pos.take_profit, tp,
        "Take-profit must be updated to {} for short, got {}",
        tp, pos.take_profit
    );
    assert_eq!(
        pos.stop_loss, sl,
        "Stop-loss must be updated to {} for short, got {}",
        sl, pos.stop_loss
    );
}

#[test]
fn test_set_tp_sl_only_tp() {
    // Set only TP (SL=0). SL should remain 0 (not set).
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    open_btc_position(&f, true);

    let tp = 60_000 * PRECISION;
    f.pm_client.set_tp_sl(&f.trader, &symbol, &tp, &0);

    let pos = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(pos.take_profit, tp, "TP must be set to {}", tp);
    assert_eq!(pos.stop_loss, 0, "SL must remain 0 when not set");
}

#[test]
fn test_set_tp_sl_only_sl() {
    // Set only SL (TP=0). TP should remain 0 (not set).
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    open_btc_position(&f, true);

    let sl = 40_000 * PRECISION;
    f.pm_client.set_tp_sl(&f.trader, &symbol, &0, &sl);

    let pos = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(pos.take_profit, 0, "TP must remain 0 when not set");
    assert_eq!(pos.stop_loss, sl, "SL must be set to {}", sl);
}

#[test]
fn test_set_tp_sl_clear_both() {
    // First set TP and SL, then clear both by setting to 0.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    open_btc_position(&f, true);

    // Set valid TP/SL first
    let tp = 55_000 * PRECISION;
    let sl = 45_000 * PRECISION;
    f.pm_client.set_tp_sl(&f.trader, &symbol, &tp, &sl);

    // Verify they are set
    let pos = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(pos.take_profit, tp);
    assert_eq!(pos.stop_loss, sl);

    // Clear both
    f.pm_client.set_tp_sl(&f.trader, &symbol, &0, &0);

    let pos = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(pos.take_profit, 0, "TP must be cleared to 0");
    assert_eq!(pos.stop_loss, 0, "SL must be cleared to 0");
}

#[test]
fn test_set_tp_sl_tp_below_entry_long_succeeds() {
    // Validation no longer pins TP/SL to entry-relative direction — a trader
    // is free to set whatever values they want (immediate-trigger risk is on
    // the frontend). Long with TP $49k while entry is $50k: valid, persists.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    open_btc_position(&f, true);

    let tp = 49_000 * PRECISION;
    f.pm_client.set_tp_sl(&f.trader, &symbol, &tp, &0);
    let pos = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(pos.take_profit, tp);
}

#[test]
fn test_set_tp_sl_tp_above_entry_short_succeeds() {
    // Short with TP above entry: previously rejected, now valid.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    open_btc_position(&f, false);

    let tp = 51_000 * PRECISION;
    f.pm_client.set_tp_sl(&f.trader, &symbol, &tp, &0);
    let pos = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(pos.take_profit, tp);
}

#[test]
fn test_set_tp_sl_sl_above_entry_long_succeeds() {
    // Profit-locking SL on a long: entry $50k, SL $51k. With mark at $52k
    // (or higher), the trader wants to lock in profit by exiting at $51k.
    // The previous entry-pinned validator blocked this — now it persists.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    open_btc_position(&f, true);

    let sl = 51_000 * PRECISION;
    f.pm_client.set_tp_sl(&f.trader, &symbol, &0, &sl);
    let pos = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(pos.stop_loss, sl);
}

#[test]
fn test_set_tp_sl_sl_below_entry_short_succeeds() {
    // Profit-locking SL on a short: entry $50k, SL $49k.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    open_btc_position(&f, false);

    let sl = 49_000 * PRECISION;
    f.pm_client.set_tp_sl(&f.trader, &symbol, &0, &sl);
    let pos = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(pos.stop_loss, sl);
}

#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn test_set_tp_sl_no_position_reverts() {
    // No position exists for this trader/symbol. Must panic PositionNotFound (6).
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    f.pm_client.set_tp_sl(
        &f.trader,
        &symbol,
        &(55_000 * PRECISION),
        &(45_000 * PRECISION),
    );
}

// ===========================================================================
// 1b. set_tp_sl adversarial / edge-case tests
// ===========================================================================

#[test]
fn test_set_tp_sl_tp_equal_to_entry_long_succeeds() {
    // Equality with entry price is now allowed.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    open_btc_position(&f, true);
    f.pm_client.set_tp_sl(&f.trader, &symbol, &BTC_PRICE, &0);
    let pos = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(pos.take_profit, BTC_PRICE);
}

#[test]
fn test_set_tp_sl_tp_equal_to_entry_short_succeeds() {
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    open_btc_position(&f, false);
    f.pm_client.set_tp_sl(&f.trader, &symbol, &BTC_PRICE, &0);
    let pos = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(pos.take_profit, BTC_PRICE);
}

#[test]
fn test_set_tp_sl_sl_equal_to_entry_long_succeeds() {
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    open_btc_position(&f, true);
    f.pm_client.set_tp_sl(&f.trader, &symbol, &0, &BTC_PRICE);
    let pos = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(pos.stop_loss, BTC_PRICE);
}

#[test]
fn test_set_tp_sl_sl_equal_to_entry_short_succeeds() {
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    open_btc_position(&f, false);
    f.pm_client.set_tp_sl(&f.trader, &symbol, &0, &BTC_PRICE);
    let pos = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(pos.stop_loss, BTC_PRICE);
}

#[test]
#[should_panic(expected = "Error(Contract, #14)")]
fn test_set_tp_sl_negative_tp_long_reverts() {
    // Adversarial: negative TP price should be treated as invalid, not as "not set".
    // A negative TP for a long is below entry -> InvalidTpSl.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    open_btc_position(&f, true);

    f.pm_client.set_tp_sl(&f.trader, &symbol, &(-1_i128), &0);
}

#[test]
#[should_panic(expected = "Error(Contract, #14)")]
fn test_set_tp_sl_negative_sl_long_reverts() {
    // Adversarial: negative SL price should be treated as invalid.
    // A negative SL for a long is below entry, so it might pass the direction check,
    // but negative prices are not meaningful. We expect InvalidTpSl.
    // NOTE: The current impl checks `stop_loss > 0` as the gate -- negative values
    // bypass validation entirely. If neg values should be rejected, this test
    // documents that expectation.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    open_btc_position(&f, true);

    f.pm_client
        .set_tp_sl(&f.trader, &symbol, &0, &(-100 * PRECISION));
}

#[test]
fn test_set_tp_sl_overwrite_existing() {
    // Set TP/SL, then overwrite with new values. Verify the new values stick.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    open_btc_position(&f, true);

    let tp1 = 55_000 * PRECISION;
    let sl1 = 45_000 * PRECISION;
    f.pm_client.set_tp_sl(&f.trader, &symbol, &tp1, &sl1);

    let tp2 = 60_000 * PRECISION;
    let sl2 = 40_000 * PRECISION;
    f.pm_client.set_tp_sl(&f.trader, &symbol, &tp2, &sl2);

    let pos = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(pos.take_profit, tp2, "TP must be overwritten to new value");
    assert_eq!(pos.stop_loss, sl2, "SL must be overwritten to new value");
}

// ===========================================================================
// 2. execute_order tests -- TP triggered
// ===========================================================================

#[test]
fn test_execute_order_tp_long_success() {
    // Long BTC at $50,000 with TP=$55,000. Price rises to $56,000 (above TP).
    // Keeper calls execute_order -> position is closed, trader receives profit.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    open_btc_position(&f, true);

    // Set TP
    let tp = 55_000 * PRECISION;
    f.pm_client.set_tp_sl(&f.trader, &symbol, &tp, &0);

    let trader_balance_before = f.usdc_client.balance(&f.trader);

    // Advance time and set price above TP
    let trigger_price = 56_000 * PRECISION;
    advance_and_set_price(&f, trigger_price);

    // Keeper executes order
    f.pm_client.execute_order(&f.keeper, &f.trader, &symbol);

    let trader_balance_after = f.usdc_client.balance(&f.trader);
    let received = trader_balance_after - trader_balance_before;

    // Trader should receive collateral + profit (minus fees)
    assert!(
        received > DEFAULT_COLLATERAL,
        "Trader must receive more than collateral on profitable TP close. Received: {}",
        received
    );

    // Position must be deleted
    f.env.as_contract(&f.pm_addr, || {
        let pos = storage::get_position(&f.env, &f.trader, &symbol);
        assert!(pos.is_none(), "Position must be deleted after TP execution");
    });
}

#[test]
fn test_execute_order_sl_long_success() {
    // Long BTC at $50,000 with SL=$45,000. Price drops to $44,000 (below SL).
    // Keeper calls execute_order -> position is closed with a loss.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    open_btc_position(&f, true);

    // Set SL
    let sl = 45_000 * PRECISION;
    f.pm_client.set_tp_sl(&f.trader, &symbol, &0, &sl);

    let trader_balance_before = f.usdc_client.balance(&f.trader);

    // Advance time and set price below SL
    let trigger_price = 44_000 * PRECISION;
    advance_and_set_price(&f, trigger_price);

    // Keeper executes order
    f.pm_client.execute_order(&f.keeper, &f.trader, &symbol);

    let trader_balance_after = f.usdc_client.balance(&f.trader);
    let received = trader_balance_after - trader_balance_before;

    // Trader loses money -- receives less than collateral
    assert!(
        received < DEFAULT_COLLATERAL,
        "Trader must receive less than collateral on SL loss. Received: {}",
        received
    );

    // Position must be deleted
    f.env.as_contract(&f.pm_addr, || {
        let pos = storage::get_position(&f.env, &f.trader, &symbol);
        assert!(pos.is_none(), "Position must be deleted after SL execution");
    });
}

#[test]
fn test_execute_order_tp_short_success() {
    // Short BTC at $50,000 with TP=$45,000. Price drops to $44,000 (below TP).
    // Keeper calls execute_order -> position is closed, trader receives profit.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    open_btc_position(&f, false);

    // Set TP (below entry for short)
    let tp = 45_000 * PRECISION;
    f.pm_client.set_tp_sl(&f.trader, &symbol, &tp, &0);

    let trader_balance_before = f.usdc_client.balance(&f.trader);

    // Advance time and set price below TP
    let trigger_price = 44_000 * PRECISION;
    advance_and_set_price(&f, trigger_price);

    // Keeper executes order
    f.pm_client.execute_order(&f.keeper, &f.trader, &symbol);

    let trader_balance_after = f.usdc_client.balance(&f.trader);
    let received = trader_balance_after - trader_balance_before;

    // Trader should receive collateral + profit
    assert!(
        received > DEFAULT_COLLATERAL,
        "Short trader must receive more than collateral on profitable TP. Received: {}",
        received
    );

    // Position must be deleted
    f.env.as_contract(&f.pm_addr, || {
        let pos = storage::get_position(&f.env, &f.trader, &symbol);
        assert!(
            pos.is_none(),
            "Position must be deleted after short TP execution"
        );
    });
}

#[test]
fn test_execute_order_sl_short_success() {
    // Short BTC at $50,000 with SL=$55,000. Price rises to $56,000 (above SL).
    // Keeper calls execute_order -> position is closed with a loss.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    open_btc_position(&f, false);

    // Set SL (above entry for short)
    let sl = 55_000 * PRECISION;
    f.pm_client.set_tp_sl(&f.trader, &symbol, &0, &sl);

    let trader_balance_before = f.usdc_client.balance(&f.trader);

    // Advance time and set price above SL
    let trigger_price = 56_000 * PRECISION;
    advance_and_set_price(&f, trigger_price);

    // Keeper executes order
    f.pm_client.execute_order(&f.keeper, &f.trader, &symbol);

    let trader_balance_after = f.usdc_client.balance(&f.trader);
    let received = trader_balance_after - trader_balance_before;

    // Trader loses money
    assert!(
        received < DEFAULT_COLLATERAL,
        "Short trader must receive less than collateral on SL loss. Received: {}",
        received
    );

    // Position must be deleted
    f.env.as_contract(&f.pm_addr, || {
        let pos = storage::get_position(&f.env, &f.trader, &symbol);
        assert!(
            pos.is_none(),
            "Position must be deleted after short SL execution"
        );
    });
}

// ===========================================================================
// 2b. execute_order -- boundary trigger tests
// ===========================================================================

#[test]
fn test_execute_order_tp_long_exact_price() {
    // Long TP=$55,000, mark price exactly $55,000 (mark >= TP). Should trigger.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    open_btc_position(&f, true);

    let tp = 55_000 * PRECISION;
    f.pm_client.set_tp_sl(&f.trader, &symbol, &tp, &0);

    // Set price exactly to TP
    advance_and_set_price(&f, tp);

    // Should NOT panic -- TP is triggered at exact price
    f.pm_client.execute_order(&f.keeper, &f.trader, &symbol);

    // Position must be deleted
    f.env.as_contract(&f.pm_addr, || {
        let pos = storage::get_position(&f.env, &f.trader, &symbol);
        assert!(
            pos.is_none(),
            "Position must be deleted when TP hit exactly"
        );
    });
}

#[test]
fn test_execute_order_sl_long_exact_price() {
    // Long SL=$45,000, mark price exactly $45,000 (mark <= SL). Should trigger.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    open_btc_position(&f, true);

    let sl = 45_000 * PRECISION;
    f.pm_client.set_tp_sl(&f.trader, &symbol, &0, &sl);

    // Set price exactly to SL
    advance_and_set_price(&f, sl);

    // Should NOT panic -- SL is triggered at exact price
    f.pm_client.execute_order(&f.keeper, &f.trader, &symbol);

    // Position must be deleted
    f.env.as_contract(&f.pm_addr, || {
        let pos = storage::get_position(&f.env, &f.trader, &symbol);
        assert!(
            pos.is_none(),
            "Position must be deleted when SL hit exactly"
        );
    });
}

// ===========================================================================
// 2c. execute_order -- revert scenarios
// ===========================================================================

#[test]
#[should_panic(expected = "Error(Contract, #13)")]
fn test_execute_order_not_triggered_reverts() {
    // Long BTC at $50k with TP=$55k and SL=$45k. Price is still $50k.
    // Neither TP nor SL is triggered -> OrderNotTriggered (13).
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    open_btc_position(&f, true);

    let tp = 55_000 * PRECISION;
    let sl = 45_000 * PRECISION;
    f.pm_client.set_tp_sl(&f.trader, &symbol, &tp, &sl);

    // Advance time but keep price the same
    advance_past_lifetime(&f);

    // Should panic -- price has not reached TP or SL
    f.pm_client.execute_order(&f.keeper, &f.trader, &symbol);
}

#[test]
#[should_panic(expected = "Error(Contract, #13)")]
fn test_execute_order_no_tp_sl_set_reverts() {
    // Position exists but has no TP or SL set (both 0).
    // execute_order should panic with OrderNotTriggered (13) regardless of price.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    open_btc_position(&f, true);

    advance_past_lifetime(&f);

    f.pm_client.execute_order(&f.keeper, &f.trader, &symbol);
}

#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn test_execute_order_no_position_reverts() {
    // No position exists for this trader/symbol -> PositionNotFound (6).
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    advance_past_lifetime(&f);

    f.pm_client.execute_order(&f.keeper, &f.trader, &symbol);
}

#[test]
fn test_execute_order_succeeds_when_paused() {
    // TP/SL orders protect traders and must execute during emergencies.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    open_btc_position(&f, true);

    let tp = 55_000 * PRECISION;
    f.pm_client.set_tp_sl(&f.trader, &symbol, &tp, &0);

    // Pause the contract
    f.pm_client.pause(&f.admin);

    let trigger_price = 56_000 * PRECISION;
    advance_and_set_price(&f, trigger_price);

    // execute_order should succeed even when paused
    f.pm_client.execute_order(&f.keeper, &f.trader, &symbol);

    // Verify position is gone
    f.env.as_contract(&f.pm_addr, || {
        let pos = storage::get_position(&f.env, &f.trader, &symbol);
        assert!(
            pos.is_none(),
            "Position must be deleted after execute_order"
        );
    });
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn test_set_tp_sl_reverts_when_paused() {
    // Setting TP/SL is a non-emergency action and must be blocked when paused.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    open_btc_position(&f, true);

    f.pm_client.pause(&f.admin);

    let tp = 55_000 * PRECISION;
    f.pm_client.set_tp_sl(&f.trader, &symbol, &tp, &0);
}

#[test]
fn test_execute_order_position_deleted_after() {
    // After execute_order completes, calling get_position should panic with
    // PositionNotFound (6), confirming the position is fully deleted.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    open_btc_position(&f, true);

    let tp = 55_000 * PRECISION;
    f.pm_client.set_tp_sl(&f.trader, &symbol, &tp, &0);

    let trigger_price = 56_000 * PRECISION;
    advance_and_set_price(&f, trigger_price);

    f.pm_client.execute_order(&f.keeper, &f.trader, &symbol);

    // Verify position is gone via storage
    f.env.as_contract(&f.pm_addr, || {
        let pos = storage::get_position(&f.env, &f.trader, &symbol);
        assert!(
            pos.is_none(),
            "Position must be deleted after execute_order"
        );
    });
}

#[test]
fn test_execute_order_market_oi_updated() {
    // After execute_order, market OI must decrease by the position's full size.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    open_btc_position(&f, true);

    let market_before = f.pm_client.get_market(&symbol);
    assert_eq!(market_before.long_open_interest, DEFAULT_SIZE);

    let tp = 55_000 * PRECISION;
    f.pm_client.set_tp_sl(&f.trader, &symbol, &tp, &0);

    let trigger_price = 56_000 * PRECISION;
    advance_and_set_price(&f, trigger_price);

    f.pm_client.execute_order(&f.keeper, &f.trader, &symbol);

    let market_after = f.pm_client.get_market(&symbol);
    assert_eq!(
        market_after.long_open_interest, 0,
        "Long OI must be zero after TP execution of only position"
    );
}

#[test]
fn test_execute_order_total_reserved_updated() {
    // After execute_order, total_reserved must decrease by position size.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    open_btc_position(&f, true);

    f.env.as_contract(&f.pm_addr, || {
        assert_eq!(f.vault_client.reserved_usdc(), DEFAULT_SIZE);
    });

    let tp = 55_000 * PRECISION;
    f.pm_client.set_tp_sl(&f.trader, &symbol, &tp, &0);

    let trigger_price = 56_000 * PRECISION;
    advance_and_set_price(&f, trigger_price);

    f.pm_client.execute_order(&f.keeper, &f.trader, &symbol);

    f.env.as_contract(&f.pm_addr, || {
        assert_eq!(
            f.vault_client.reserved_usdc(),
            0,
            "TotalReserved must be zero after execute_order full close"
        );
    });
}

#[test]
fn test_execute_order_vault_liquidity_released() {
    // After execute_order, vault free liquidity should increase.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    open_btc_position(&f, true);

    let free_liq_before = f.vault_client.free_liquidity();

    let tp = 55_000 * PRECISION;
    f.pm_client.set_tp_sl(&f.trader, &symbol, &tp, &0);

    let trigger_price = 56_000 * PRECISION;
    advance_and_set_price(&f, trigger_price);

    f.pm_client.execute_order(&f.keeper, &f.trader, &symbol);

    let free_liq_after = f.vault_client.free_liquidity();
    assert!(
        free_liq_after > free_liq_before,
        "Vault free liquidity must increase after execute_order. Before: {}, After: {}",
        free_liq_before,
        free_liq_after
    );
}

// ===========================================================================
// 2d. execute_order -- adversarial edge cases
// ===========================================================================

#[test]
#[should_panic(expected = "Error(Contract, #13)")]
fn test_execute_order_price_one_tick_below_tp_long_not_triggered() {
    // Long with TP=$55,000. Price is $54,999.9999999 (one unit below).
    // TP condition: mark >= TP. This is strictly less -> NOT triggered.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    open_btc_position(&f, true);

    let tp = 55_000 * PRECISION;
    f.pm_client.set_tp_sl(&f.trader, &symbol, &tp, &0);

    // One PRECISION unit below TP
    let almost_tp = tp - 1;
    advance_and_set_price(&f, almost_tp);

    // Should panic -- not quite triggered
    f.pm_client.execute_order(&f.keeper, &f.trader, &symbol);
}

#[test]
#[should_panic(expected = "Error(Contract, #13)")]
fn test_execute_order_price_one_tick_above_sl_long_not_triggered() {
    // Long with SL=$45,000. Price is $45,000.0000001 (one unit above).
    // SL condition: mark <= SL. This is strictly greater -> NOT triggered.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    open_btc_position(&f, true);

    let sl = 45_000 * PRECISION;
    f.pm_client.set_tp_sl(&f.trader, &symbol, &0, &sl);

    // One PRECISION unit above SL
    let almost_sl = sl + 1;
    advance_and_set_price(&f, almost_sl);

    // Should panic -- not quite triggered
    f.pm_client.execute_order(&f.keeper, &f.trader, &symbol);
}

#[test]
fn test_execute_order_short_oi_updated() {
    // Short position: after execute_order, short OI must decrease.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    open_btc_position(&f, false);

    let market_before = f.pm_client.get_market(&symbol);
    assert_eq!(market_before.short_open_interest, DEFAULT_SIZE);

    let tp = 45_000 * PRECISION;
    f.pm_client.set_tp_sl(&f.trader, &symbol, &tp, &0);

    let trigger_price = 44_000 * PRECISION;
    advance_and_set_price(&f, trigger_price);

    f.pm_client.execute_order(&f.keeper, &f.trader, &symbol);

    let market_after = f.pm_client.get_market(&symbol);
    assert_eq!(
        market_after.short_open_interest, 0,
        "Short OI must be zero after TP execution of only short position"
    );
    assert_eq!(
        market_after.long_open_interest, 0,
        "Long OI must remain zero -- only short position was closed"
    );
}

// ===========================================================================
// 3. increase_position with TP/SL
// ===========================================================================

#[test]
fn test_increase_position_with_tp_sl() {
    // Open a new long BTC position with TP=$55,000 and SL=$45,000.
    // Verify they're stored in the position.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    let tp = 55_000 * PRECISION;
    let sl = 45_000 * PRECISION;

    open_btc_position_with_tp_sl(&f, true, tp, sl);

    let pos = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(
        pos.take_profit, tp,
        "New position must store TP from increase_position"
    );
    assert_eq!(
        pos.stop_loss, sl,
        "New position must store SL from increase_position"
    );
}

#[test]
fn test_increase_position_updates_tp_sl() {
    // Open position with TP=$55k, SL=$45k. Then increase position with new
    // TP=$60k, SL=$40k. Verify TP/SL are updated.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    let tp1 = 55_000 * PRECISION;
    let sl1 = 45_000 * PRECISION;
    open_btc_position_with_tp_sl(&f, true, tp1, sl1);

    // Increase position with new TP/SL
    let tp2 = 60_000 * PRECISION;
    let sl2 = 40_000 * PRECISION;
    f.pm_client.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &tp2,
        &sl2, &0i128
    );

    let pos = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(
        pos.take_profit, tp2,
        "TP must be updated on increase_position with non-zero TP"
    );
    assert_eq!(
        pos.stop_loss, sl2,
        "SL must be updated on increase_position with non-zero SL"
    );
}

#[test]
fn test_increase_position_zero_tp_sl_preserves_existing() {
    // Open position with TP=$55k, SL=$45k. Then increase with TP=0, SL=0.
    // Existing TP/SL should be preserved (0 means "don't change" on increase).
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    let tp = 55_000 * PRECISION;
    let sl = 45_000 * PRECISION;
    open_btc_position_with_tp_sl(&f, true, tp, sl);

    // Increase with zero TP/SL -- should preserve existing
    f.pm_client.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0, &0i128
    );

    let pos = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(
        pos.take_profit, tp,
        "TP must be preserved when increase_position passes 0"
    );
    assert_eq!(
        pos.stop_loss, sl,
        "SL must be preserved when increase_position passes 0"
    );
}

#[test]
fn test_increase_position_tp_below_entry_long_succeeds() {
    // Open long with TP below entry — no longer rejected. The position is
    // immediately eligible for execution on the next favourable mark, which
    // matches the trader's intent for "close as soon as profitable".
    let f = setup_full();

    let tp = 49_000 * PRECISION;
    let sl = 45_000 * PRECISION;

    f.pm_client.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &tp,
        &sl,
        &0i128,
    );
    let pos = f.pm_client.get_position(&f.trader, &symbol_short!("BTC"));
    assert_eq!(pos.take_profit, tp);
}

#[test]
fn test_increase_position_sl_above_entry_long_succeeds() {
    // Open long with SL above entry. Same rationale.
    let f = setup_full();

    let tp = 55_000 * PRECISION;
    let sl = 51_000 * PRECISION;

    f.pm_client.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &tp,
        &sl,
        &0i128,
    );
    let pos = f.pm_client.get_position(&f.trader, &symbol_short!("BTC"));
    assert_eq!(pos.stop_loss, sl);
}

#[test]
fn test_increase_position_new_short_with_tp_sl() {
    // Open a new short position with TP (below entry) and SL (above entry).
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    let tp = 45_000 * PRECISION; // below entry for short
    let sl = 55_000 * PRECISION; // above entry for short

    f.pm_client.increase_position(
        &f.trader,
        &symbol,
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &false,
        &tp,
        &sl, &0i128
    );

    let pos = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(pos.take_profit, tp);
    assert_eq!(pos.stop_loss, sl);
    assert!(!pos.is_long);
}

#[test]
fn test_increase_position_short_tp_above_entry_succeeds() {
    // Open short with TP above entry — previously rejected, now valid.
    let f = setup_full();

    let tp = 55_000 * PRECISION;

    f.pm_client.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &false,
        &tp,
        &0,
        &0i128,
    );
    let pos = f.pm_client.get_position(&f.trader, &symbol_short!("BTC"));
    assert_eq!(pos.take_profit, tp);
}

// ===========================================================================
// 4. Integration: full lifecycle tests
// ===========================================================================

#[test]
fn test_full_lifecycle_open_set_tp_sl_execute_tp() {
    // Full lifecycle: open long -> set TP/SL -> price rises -> execute TP.
    // Verify position is closed and trader is profitable.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    // Step 1: Open long at $50k
    open_btc_position(&f, true);

    // Step 2: Set TP=$55k, SL=$45k
    let tp = 55_000 * PRECISION;
    let sl = 45_000 * PRECISION;
    f.pm_client.set_tp_sl(&f.trader, &symbol, &tp, &sl);

    let balance_before = f.usdc_client.balance(&f.trader);

    // Step 3: Price rises to $56k (above TP)
    let trigger_price = 56_000 * PRECISION;
    advance_and_set_price(&f, trigger_price);

    // Step 4: Keeper executes
    f.pm_client.execute_order(&f.keeper, &f.trader, &symbol);

    // Step 5: Verify
    let balance_after = f.usdc_client.balance(&f.trader);
    assert!(
        balance_after > balance_before,
        "Trader must profit from TP execution"
    );

    // Position gone
    f.env.as_contract(&f.pm_addr, || {
        assert!(storage::get_position(&f.env, &f.trader, &symbol).is_none());
    });

    // Market OI reset
    let market = f.pm_client.get_market(&symbol);
    assert_eq!(market.long_open_interest, 0);
}

#[test]
fn test_full_lifecycle_open_with_tp_sl_execute_sl() {
    // Open long with TP/SL inline, then SL triggers.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    let tp = 55_000 * PRECISION;
    let sl = 48_000 * PRECISION;
    open_btc_position_with_tp_sl(&f, true, tp, sl);

    let balance_before = f.usdc_client.balance(&f.trader);

    // Price drops to $47k (below SL)
    let trigger_price = 47_000 * PRECISION;
    advance_and_set_price(&f, trigger_price);

    f.pm_client.execute_order(&f.keeper, &f.trader, &symbol);

    let balance_after = f.usdc_client.balance(&f.trader);
    let received = balance_after - balance_before;

    // Trader gets back something (collateral minus loss), but less than full collateral
    // PnL = 10,000 * (47,000 - 50,000) / 50,000 = -600 USDC
    // health = 1,000 - 600 = 400 (approx)
    assert!(
        received > 0,
        "Trader should receive remaining collateral after SL loss. Received: {}",
        received
    );
    assert!(
        received < DEFAULT_COLLATERAL,
        "Trader must receive less than collateral on SL loss. Received: {}",
        received
    );
}

#[test]
fn test_execute_order_does_not_affect_other_positions() {
    // Two traders have positions. Executing one trader's TP/SL does not affect
    // the other trader's position.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    let trader2 = Address::generate(&f.env);
    f.usdc_client.mint(&trader2, &TRADER_BALANCE);

    // Trader 1: long with TP
    open_btc_position(&f, true);
    let tp = 55_000 * PRECISION;
    f.pm_client.set_tp_sl(&f.trader, &symbol, &tp, &0);

    // Trader 2: long, no TP/SL
    f.pm_client.increase_position(
        &trader2,
        &symbol,
        &(20_000 * USDC_UNIT),
        &(2_000 * USDC_UNIT),
        &true,
        &0,
        &0, &0i128
    );

    // Price rises above TP
    let trigger_price = 56_000 * PRECISION;
    advance_and_set_price(&f, trigger_price);

    // Execute trader 1's TP
    f.pm_client.execute_order(&f.keeper, &f.trader, &symbol);

    // Trader 2's position must be unaffected
    let pos2 = f.pm_client.get_position(&trader2, &symbol);
    assert_eq!(
        pos2.size,
        20_000 * USDC_UNIT,
        "Trader2 position size must be unaffected by trader1's TP execution"
    );
}

// ===========================================================================
// 5. set_tp_sl escrow lifecycle
//
// set_tp_sl is a standalone entrypoint: TP/SL params are direct-assigned, so
// `0` MEANS clear (unlike increase_position where `0` MEANS preserve).
//
// Escrow rules:
//   prior_escrow = pos.execution_fee_escrow         (what trader previously paid)
//   has_orders   = (take_profit != 0) || (stop_loss != 0)
//
//   IF prior_escrow == 0 AND has_orders:
//       transfer(trader -> PM, fee_config.tp_sl_execution_fee)
//       pos.execution_fee_escrow = fee_config.tp_sl_execution_fee
//       (skip when fee == 0)
//
//   ELSE IF prior_escrow > 0 AND !has_orders:
//       transfer(PM -> trader, prior_escrow)
//       pos.execution_fee_escrow = 0
//
//   ELSE: no escrow change.
//
// Refund amount is the STORED escrow, not the current fee_config value — if
// admin changes the fee between charge and refund the trader gets back what
// they actually paid.
// ===========================================================================

/// Default flat TP/SL execution fee planted by ConfigManager::initialize.
const TP_SL_FEE: i128 = 5_000_000;

/// A TP price strictly above BTC_PRICE — valid for a long.
const TP_LONG: i128 = 55_000 * PRECISION;
/// An SL price strictly below BTC_PRICE — valid for a long.
const SL_LONG: i128 = 45_000 * PRECISION;

#[test]
fn test_set_tp_sl_charges_escrow_on_first_time_tp_only() {
    // Open a long with no TP/SL (no escrow). Setting only TP charges the flat
    // tp_sl_execution_fee from trader to PM and stores it on the position.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    open_btc_position(&f, true);

    let trader_balance_before = f.usdc_client.balance(&f.trader);
    let pm_balance_before = f.usdc_client.balance(&f.pm_addr);

    f.pm_client.set_tp_sl(&f.trader, &symbol, &TP_LONG, &0);

    let trader_balance_after = f.usdc_client.balance(&f.trader);
    let pm_balance_after = f.usdc_client.balance(&f.pm_addr);

    assert_eq!(
        trader_balance_before - trader_balance_after,
        TP_SL_FEE,
        "Trader must pay flat tp_sl_execution_fee on first-time TP set"
    );
    assert_eq!(
        pm_balance_after - pm_balance_before,
        TP_SL_FEE,
        "PM must receive the escrow; fee stays in PM (not vault)"
    );

    let pos = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(
        pos.execution_fee_escrow, TP_SL_FEE,
        "Position must record the escrow amount on first-time TP set"
    );
    assert_eq!(pos.take_profit, TP_LONG, "TP must be stored");
    assert_eq!(pos.stop_loss, 0, "SL must remain 0");
}

#[test]
fn test_set_tp_sl_charges_escrow_on_first_time_sl_only() {
    // Open a long with no TP/SL (no escrow). Setting only SL charges the same
    // flat fee and stores it on the position.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    open_btc_position(&f, true);

    let trader_balance_before = f.usdc_client.balance(&f.trader);
    let pm_balance_before = f.usdc_client.balance(&f.pm_addr);

    f.pm_client.set_tp_sl(&f.trader, &symbol, &0, &SL_LONG);

    let trader_balance_after = f.usdc_client.balance(&f.trader);
    let pm_balance_after = f.usdc_client.balance(&f.pm_addr);

    assert_eq!(
        trader_balance_before - trader_balance_after,
        TP_SL_FEE,
        "Trader must pay flat tp_sl_execution_fee on first-time SL set"
    );
    assert_eq!(
        pm_balance_after - pm_balance_before,
        TP_SL_FEE,
        "PM must receive the escrow"
    );

    let pos = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(
        pos.execution_fee_escrow, TP_SL_FEE,
        "Position must record the escrow amount on first-time SL set"
    );
    assert_eq!(pos.take_profit, 0, "TP must remain 0");
    assert_eq!(pos.stop_loss, SL_LONG, "SL must be stored");
}

#[test]
fn test_set_tp_sl_charges_escrow_on_first_time_both() {
    // Open a long with no TP/SL. Setting both TP and SL fires the charge exactly
    // ONCE (single flat fee, not 2x).
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    open_btc_position(&f, true);

    let trader_balance_before = f.usdc_client.balance(&f.trader);
    let pm_balance_before = f.usdc_client.balance(&f.pm_addr);

    f.pm_client.set_tp_sl(&f.trader, &symbol, &TP_LONG, &SL_LONG);

    let trader_balance_after = f.usdc_client.balance(&f.trader);
    let pm_balance_after = f.usdc_client.balance(&f.pm_addr);

    assert_eq!(
        trader_balance_before - trader_balance_after,
        TP_SL_FEE,
        "Trader must pay a SINGLE flat fee even when setting both TP and SL"
    );
    assert_eq!(
        pm_balance_after - pm_balance_before,
        TP_SL_FEE,
        "PM must receive a SINGLE escrow"
    );

    let pos = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(
        pos.execution_fee_escrow, TP_SL_FEE,
        "Position must record a single flat escrow, not 2x"
    );
    assert_eq!(pos.take_profit, TP_LONG);
    assert_eq!(pos.stop_loss, SL_LONG);
}

#[test]
fn test_set_tp_sl_no_charge_on_subsequent_update() {
    // Open a position, set TP (charges escrow). A SECOND call updating TP must
    // NOT re-charge — escrow already exists and the position still has orders.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    open_btc_position(&f, true);

    // First call: charges escrow
    f.pm_client.set_tp_sl(&f.trader, &symbol, &TP_LONG, &0);

    let trader_balance_before = f.usdc_client.balance(&f.trader);
    let pm_balance_before = f.usdc_client.balance(&f.pm_addr);
    let pos_before = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(pos_before.execution_fee_escrow, TP_SL_FEE);

    // Second call: new TP value, still has orders -> no charge
    let new_tp = 60_000 * PRECISION;
    f.pm_client.set_tp_sl(&f.trader, &symbol, &new_tp, &0);

    let trader_balance_after = f.usdc_client.balance(&f.trader);
    let pm_balance_after = f.usdc_client.balance(&f.pm_addr);
    let pos_after = f.pm_client.get_position(&f.trader, &symbol);

    assert_eq!(
        trader_balance_after, trader_balance_before,
        "Trader balance must NOT change on subsequent TP update"
    );
    assert_eq!(
        pm_balance_after, pm_balance_before,
        "PM balance must NOT change on subsequent TP update"
    );
    assert_eq!(
        pos_after.execution_fee_escrow, TP_SL_FEE,
        "Stored escrow must stay at the originally-charged amount"
    );
    assert_eq!(
        pos_after.take_profit, new_tp,
        "TP must be updated to the new value"
    );
}

#[test]
fn test_set_tp_sl_no_charge_when_swapping_tp_for_sl() {
    // Open and set TP only -> escrow paid. Swap TP for SL (still has_orders).
    // No new charge, no refund, escrow unchanged.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    open_btc_position(&f, true);
    f.pm_client.set_tp_sl(&f.trader, &symbol, &TP_LONG, &0);

    let trader_balance_before = f.usdc_client.balance(&f.trader);
    let pm_balance_before = f.usdc_client.balance(&f.pm_addr);

    // Swap: clear TP, set SL -- still has_orders, no escrow change
    f.pm_client.set_tp_sl(&f.trader, &symbol, &0, &SL_LONG);

    let trader_balance_after = f.usdc_client.balance(&f.trader);
    let pm_balance_after = f.usdc_client.balance(&f.pm_addr);
    let pos = f.pm_client.get_position(&f.trader, &symbol);

    assert_eq!(
        trader_balance_after, trader_balance_before,
        "Trader balance must NOT change when swapping TP for SL"
    );
    assert_eq!(
        pm_balance_after, pm_balance_before,
        "PM balance must NOT change when swapping TP for SL"
    );
    assert_eq!(
        pos.execution_fee_escrow, TP_SL_FEE,
        "Escrow must remain at the originally-charged amount"
    );
    assert_eq!(pos.take_profit, 0, "TP must be cleared");
    assert_eq!(pos.stop_loss, SL_LONG, "SL must be set");
}

#[test]
fn test_set_tp_sl_refunds_escrow_on_clear_both() {
    // Open and set TP (escrow paid). Clear both TP and SL -> refund escrow
    // back to trader. PM balance drops, trader balance rises, escrow becomes 0.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    open_btc_position(&f, true);
    f.pm_client.set_tp_sl(&f.trader, &symbol, &TP_LONG, &0);

    let trader_balance_before = f.usdc_client.balance(&f.trader);
    let pm_balance_before = f.usdc_client.balance(&f.pm_addr);
    let pos_before = f.pm_client.get_position(&f.trader, &symbol);
    let prior_escrow = pos_before.execution_fee_escrow;
    assert_eq!(prior_escrow, TP_SL_FEE);

    // Clear both -> refund
    f.pm_client.set_tp_sl(&f.trader, &symbol, &0, &0);

    let trader_balance_after = f.usdc_client.balance(&f.trader);
    let pm_balance_after = f.usdc_client.balance(&f.pm_addr);
    let pos_after = f.pm_client.get_position(&f.trader, &symbol);

    assert_eq!(
        trader_balance_after - trader_balance_before,
        prior_escrow,
        "Trader balance must increase by the prior escrow on clear-both"
    );
    assert_eq!(
        pm_balance_before - pm_balance_after,
        prior_escrow,
        "PM balance must decrease by the prior escrow on clear-both"
    );
    assert_eq!(
        pos_after.execution_fee_escrow, 0,
        "Position execution_fee_escrow must be cleared to 0 after refund"
    );
    assert_eq!(pos_after.take_profit, 0);
    assert_eq!(pos_after.stop_loss, 0);
}

#[test]
fn test_set_tp_sl_no_op_when_no_escrow_and_cleared() {
    // Open with no TP/SL (no escrow). Calling set_tp_sl(0, 0) is a no-op:
    // no transfer in either direction, escrow stays 0.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    open_btc_position(&f, true);

    let trader_balance_before = f.usdc_client.balance(&f.trader);
    let pm_balance_before = f.usdc_client.balance(&f.pm_addr);

    f.pm_client.set_tp_sl(&f.trader, &symbol, &0, &0);

    let trader_balance_after = f.usdc_client.balance(&f.trader);
    let pm_balance_after = f.usdc_client.balance(&f.pm_addr);
    let pos = f.pm_client.get_position(&f.trader, &symbol);

    assert_eq!(
        trader_balance_after, trader_balance_before,
        "No-op clear must not change trader balance"
    );
    assert_eq!(
        pm_balance_after, pm_balance_before,
        "No-op clear must not change PM balance"
    );
    assert_eq!(
        pos.execution_fee_escrow, 0,
        "Escrow must stay at 0 when there was nothing to refund"
    );
    assert_eq!(pos.take_profit, 0);
    assert_eq!(pos.stop_loss, 0);
}

#[test]
fn test_set_tp_sl_refund_amount_is_stored_escrow_not_current_fee() {
    // Adversarial: admin changes the fee between charge and refund. Refund must
    // match what the trader actually paid (the stored escrow), not whatever the
    // current fee_config happens to be.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    // Charge at fee A = DEFAULT (5_000_000).
    open_btc_position(&f, true);
    f.pm_client.set_tp_sl(&f.trader, &symbol, &TP_LONG, &0);
    let pos_after_charge = f.pm_client.get_position(&f.trader, &symbol);
    let paid_escrow = pos_after_charge.execution_fee_escrow;
    assert_eq!(paid_escrow, TP_SL_FEE);

    // Admin bumps the fee to a much higher value B.
    let new_fee_b: i128 = 87_654_321;
    assert!(new_fee_b != paid_escrow, "Setup: fee B must differ from A");
    f._config_client.set_fee_config(
        &f.admin,
        &FeeConfig {
            open_fee_bps: 10,
            liquidation_bounty_bps: 100,
            tp_sl_execution_fee: new_fee_b,
        },
    );

    let trader_balance_before = f.usdc_client.balance(&f.trader);
    let pm_balance_before = f.usdc_client.balance(&f.pm_addr);

    // Clear -> refund
    f.pm_client.set_tp_sl(&f.trader, &symbol, &0, &0);

    let trader_balance_after = f.usdc_client.balance(&f.trader);
    let pm_balance_after = f.usdc_client.balance(&f.pm_addr);

    assert_eq!(
        trader_balance_after - trader_balance_before,
        paid_escrow,
        "Refund must equal the originally-paid escrow (A), NOT the current fee (B)"
    );
    assert_eq!(
        pm_balance_before - pm_balance_after,
        paid_escrow,
        "PM must release exactly the stored escrow (A), NOT the current fee (B)"
    );
}

#[test]
fn test_set_tp_sl_with_zero_fee_does_not_charge_or_store_escrow() {
    // With tp_sl_execution_fee == 0 the charge path must be skipped entirely:
    // no transfer, escrow remains 0, but TP/SL fields ARE still updated.
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

    open_btc_position(&f, true);

    let trader_balance_before = f.usdc_client.balance(&f.trader);
    let pm_balance_before = f.usdc_client.balance(&f.pm_addr);

    f.pm_client.set_tp_sl(&f.trader, &symbol, &TP_LONG, &0);

    let trader_balance_after = f.usdc_client.balance(&f.trader);
    let pm_balance_after = f.usdc_client.balance(&f.pm_addr);
    let pos = f.pm_client.get_position(&f.trader, &symbol);

    assert_eq!(
        trader_balance_after, trader_balance_before,
        "Trader balance must NOT move when configured fee is 0"
    );
    assert_eq!(
        pm_balance_after, pm_balance_before,
        "PM balance must NOT move when configured fee is 0"
    );
    assert_eq!(
        pos.execution_fee_escrow, 0,
        "Position escrow must stay 0 when configured fee is 0"
    );
    assert_eq!(pos.take_profit, TP_LONG, "TP must still be stored");
}

#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn test_set_tp_sl_panics_when_position_missing() {
    // Adversarial: trader has no open position. set_tp_sl must panic with
    // PositionNotFound (6) regardless of whether escrow logic would fire.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    f.pm_client.set_tp_sl(&f.trader, &symbol, &TP_LONG, &0);
}

#[test]
#[should_panic]
fn test_set_tp_sl_panics_when_trader_lacks_balance_for_escrow() {
    // Adversarial: trader has no spare USDC to cover the escrow. The token
    // transfer inside do_set_tp_sl must panic. Token errors come from outside
    // this contract — no explicit error code expected.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    // Open position without TP/SL (no escrow charge yet). Trader keeps the
    // remainder of TRADER_BALANCE after collateral + open_fee.
    open_btc_position(&f, true);

    // Drain trader balance down to ZERO so the escrow transfer cannot be
    // covered.
    let remaining = f.usdc_client.balance(&f.trader);
    if remaining > 0 {
        f.usdc_client.burn(&f.trader, &remaining);
    }
    assert_eq!(
        f.usdc_client.balance(&f.trader),
        0,
        "Setup: trader must have 0 USDC before set_tp_sl"
    );

    // Should panic on the token transfer (insufficient balance).
    f.pm_client.set_tp_sl(&f.trader, &symbol, &TP_LONG, &0);
}

#[test]
fn test_set_tp_sl_emits_event_with_unchanged_existing_pattern() {
    // Escrow logic must not interfere with the existing SetTpSl event emission.
    // The `tp_sl` event must still publish with the trader, symbol, and the
    // newly-assigned TP/SL values.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    open_btc_position(&f, true);

    f.pm_client.set_tp_sl(&f.trader, &symbol, &TP_LONG, &SL_LONG);

    let tp_sl_topic: Symbol = symbol_short!("tp_sl");
    let all = f.env.events().all();
    let mut found_payload: Option<(Symbol, i128, i128)> = None;
    for entry in all.iter().rev() {
        let (contract, topics, data) = entry;
        if contract != f.pm_addr {
            continue;
        }
        if topics.len() == 0 {
            continue;
        }
        let first_topic_val = topics.get(0).unwrap();
        let first_topic: Result<Symbol, _> = first_topic_val.try_into_val(&f.env);
        if let Ok(s) = first_topic {
            if s == tp_sl_topic {
                let parsed: Result<(Symbol, i128, i128), _> = data.try_into_val(&f.env);
                found_payload = Some(parsed.expect(
                    "tp_sl payload must unpack as (symbol, take_profit, stop_loss)",
                ));
                break;
            }
        }
    }

    let (sym, tp, sl) = found_payload
        .expect("set_tp_sl must emit a `tp_sl` event from the PM contract");
    assert_eq!(sym, symbol, "Event must carry the position symbol");
    assert_eq!(tp, TP_LONG, "Event must carry the assigned take_profit");
    assert_eq!(sl, SL_LONG, "Event must carry the assigned stop_loss");
}

#[test]
fn test_set_tp_sl_refund_clears_field_to_zero_in_storage() {
    // Defensive: after a refund the in-memory position is dropped — verify the
    // persisted storage record actually has execution_fee_escrow == 0. Guards
    // against the bug where the field is mutated in memory but never written.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    open_btc_position(&f, true);
    f.pm_client.set_tp_sl(&f.trader, &symbol, &TP_LONG, &SL_LONG);

    // Confirm escrow is currently set via raw storage.
    f.env.as_contract(&f.pm_addr, || {
        let pos = storage::get_position(&f.env, &f.trader, &symbol)
            .expect("Position must exist after open + set_tp_sl");
        assert_eq!(
            pos.execution_fee_escrow, TP_SL_FEE,
            "Setup: storage must reflect the paid escrow before refund"
        );
    });

    // Refund path
    f.pm_client.set_tp_sl(&f.trader, &symbol, &0, &0);

    // Re-read from raw storage — escrow must be persisted as 0.
    f.env.as_contract(&f.pm_addr, || {
        let pos = storage::get_position(&f.env, &f.trader, &symbol)
            .expect("Position must still exist after refund");
        assert_eq!(
            pos.execution_fee_escrow, 0,
            "Storage record must persist execution_fee_escrow == 0 after refund"
        );
        assert_eq!(pos.take_profit, 0);
        assert_eq!(pos.stop_loss, 0);
    });
}

// ===========================================================================
// 6. OrderExecution escrow lifecycle
// ===========================================================================

#[test]
fn order_execution_pays_escrow_to_executor() {
    // Open long, set TP -> escrow paid PM. Price moves above TP. Keeper
    // calls execute_order. The escrow must flow PM -> caller (keeper). The
    // trader must NOT receive the escrow back.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    open_btc_position(&f, true);

    let tp = 55_000 * PRECISION;
    f.pm_client.set_tp_sl(&f.trader, &symbol, &tp, &0);

    let pos_before = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(
        pos_before.execution_fee_escrow, TP_SL_FEE,
        "Setup: position must record escrow before execution"
    );

    let trigger_price = 56_000 * PRECISION;
    advance_and_set_price(&f, trigger_price);

    let keeper_balance_before = f.usdc_client.balance(&f.keeper);
    let trader_balance_before = f.usdc_client.balance(&f.trader);

    f.pm_client.execute_order(&f.keeper, &f.trader, &symbol);

    let keeper_balance_after = f.usdc_client.balance(&f.keeper);
    let trader_balance_after = f.usdc_client.balance(&f.trader);

    let keeper_received = keeper_balance_after - keeper_balance_before;
    let trader_received = trader_balance_after - trader_balance_before;

    // Keeper receives exactly the escrow.
    assert_eq!(
        keeper_received, TP_SL_FEE,
        "Executor must receive the escrow on order execution"
    );

    // Trader receives the position payout (collateral + profit) but NOT
    // the escrow. Profit at $56k from $50k entry, size=10k -> +1200 USDC.
    // The trader path delta excludes TP_SL_FEE entirely.
    // Bound: trader_received < collateral + profit + TP_SL_FEE.
    let approx_collateral_plus_profit = DEFAULT_COLLATERAL + 1_200 * USDC_UNIT;
    assert!(
        trader_received < approx_collateral_plus_profit + TP_SL_FEE,
        "Trader must NOT receive escrow refund on order execution. Got: {}",
        trader_received
    );
}

#[test]
fn order_execution_with_zero_escrow_no_transfer() {
    // Admin sets tp_sl_execution_fee == 0. Set TP -> charge path skipped,
    // position has TP but escrow == 0. Trigger execute_order. No escrow
    // transfer must happen; close proceeds normally.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    // Switch the configured execution fee to 0 BEFORE setting TP.
    f._config_client.set_fee_config(
        &f.admin,
        &FeeConfig {
            open_fee_bps: 10,
            liquidation_bounty_bps: 100,
            tp_sl_execution_fee: 0,
        },
    );

    open_btc_position(&f, true);

    let tp = 55_000 * PRECISION;
    f.pm_client.set_tp_sl(&f.trader, &symbol, &tp, &0);

    let pos = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(
        pos.execution_fee_escrow, 0,
        "Setup: zero-fee config must leave escrow at 0 even with TP set"
    );
    assert_eq!(pos.take_profit, tp);

    let trigger_price = 56_000 * PRECISION;
    advance_and_set_price(&f, trigger_price);

    let keeper_balance_before = f.usdc_client.balance(&f.keeper);

    f.pm_client.execute_order(&f.keeper, &f.trader, &symbol);

    let keeper_balance_after = f.usdc_client.balance(&f.keeper);

    // Keeper receives ZERO from the escrow path — there is no escrow.
    assert_eq!(
        keeper_balance_after, keeper_balance_before,
        "Keeper must NOT receive any escrow transfer when escrow == 0"
    );

    // The close itself still proceeds.
    f.env.as_contract(&f.pm_addr, || {
        let p = storage::get_position(&f.env, &f.trader, &symbol);
        assert!(p.is_none(), "Position must be deleted after execute_order");
    });
}

// ===========================================================================
// 7. Permissionless execute_order
//
// `execute_order` is permissionless: any address that signs the call may
// trigger a TP/SL fill once the price condition is satisfied. The caller
// still `require_auth`s so a third party cannot pass someone else's Address
// to redirect the escrow payout. The KEEPER role check is removed; only
// the trigger condition gates execution.
// ===========================================================================

#[test]
fn non_keeper_can_execute_order() {
    // Scenario: an arbitrary Address that has NEVER been granted the KEEPER
    // role triggers a TP fill. The position must be deleted and the
    // escrow must flow to the non-keeper caller.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    // Brand-new address — not the fixture's `keeper` and never granted any role.
    let random_executor = Address::generate(&f.env);

    // Sanity check: the random executor must NOT hold KEEPER. If this
    // assertion ever fails the test is invalid.
    let keeper_role = Symbol::new(&f.env, "KEEPER");
    assert!(
        !f._config_client.has_role(&keeper_role, &random_executor),
        "Setup invariant: random_executor must not hold KEEPER"
    );

    // Open long, set TP -> escrow paid PM by trader.
    open_btc_position(&f, true);
    let tp = 55_000 * PRECISION;
    f.pm_client.set_tp_sl(&f.trader, &symbol, &tp, &0);

    let pos_before = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(
        pos_before.execution_fee_escrow, TP_SL_FEE,
        "Setup: position must record escrow before execution"
    );

    // Move price above TP.
    let trigger_price = 56_000 * PRECISION;
    advance_and_set_price(&f, trigger_price);

    let executor_balance_before = f.usdc_client.balance(&random_executor);

    // Non-keeper triggers the order. Must NOT panic with Unauthorized.
    f.pm_client
        .execute_order(&random_executor, &f.trader, &symbol);

    // Position must be deleted.
    f.env.as_contract(&f.pm_addr, || {
        let p = storage::get_position(&f.env, &f.trader, &symbol);
        assert!(
            p.is_none(),
            "Permissionless execute_order must delete the position"
        );
    });

    // Non-keeper executor must receive the escrow.
    let executor_balance_after = f.usdc_client.balance(&random_executor);
    assert_eq!(
        executor_balance_after - executor_balance_before,
        TP_SL_FEE,
        "Non-keeper executor must receive the escrow on order execution"
    );
}

#[test]
fn non_keeper_can_execute_order_sl_short() {
    // Mirror coverage for the short SL trigger path. Confirms the
    // permissionless gate does not silently depend on direction or
    // trigger type (TP vs SL).
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    let random_executor = Address::generate(&f.env);
    let keeper_role = Symbol::new(&f.env, "KEEPER");
    assert!(
        !f._config_client.has_role(&keeper_role, &random_executor),
        "Setup invariant: random_executor must not hold KEEPER"
    );

    open_btc_position(&f, false); // short
    let sl = 55_000 * PRECISION;
    f.pm_client.set_tp_sl(&f.trader, &symbol, &0, &sl);

    let pos_before = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(
        pos_before.execution_fee_escrow, TP_SL_FEE,
        "Setup: short position must record escrow before SL execution"
    );

    // Spike price above SL.
    let trigger_price = 56_000 * PRECISION;
    advance_and_set_price(&f, trigger_price);

    let executor_balance_before = f.usdc_client.balance(&random_executor);

    f.pm_client
        .execute_order(&random_executor, &f.trader, &symbol);

    f.env.as_contract(&f.pm_addr, || {
        let p = storage::get_position(&f.env, &f.trader, &symbol);
        assert!(
            p.is_none(),
            "Permissionless execute_order (short SL) must delete the position"
        );
    });

    let executor_balance_after = f.usdc_client.balance(&random_executor);
    assert_eq!(
        executor_balance_after - executor_balance_before,
        TP_SL_FEE,
        "Non-keeper executor must receive the escrow on short SL execution"
    );
}

#[test]
#[should_panic]
fn execute_order_caller_must_authorize_call() {
    // Regression-lock: the caller passed to `execute_order` is the escrow
    // recipient, so the contract MUST call `caller.require_auth()`.
    // Without that check, any third party could pass a victim's Address
    // as `caller` and redirect the escrow.
    //
    // Setup runs under mock_all_auths so the open + set_tp_sl can succeed.
    // After staging, blanket auth mocking is stripped via set_auths(&[]).
    // The execute call provides NO auth entry for `random_executor`, so it
    // must panic on the missing requirement.
    let f = setup_full();
    let symbol = symbol_short!("BTC");

    open_btc_position(&f, true);
    let tp = 55_000 * PRECISION;
    f.pm_client.set_tp_sl(&f.trader, &symbol, &tp, &0);

    let trigger_price = 56_000 * PRECISION;
    advance_and_set_price(&f, trigger_price);

    let random_executor = Address::generate(&f.env);

    // Strip blanket auth mocking — every require_auth now needs an
    // explicit entry.
    f.env.set_auths(&[]);

    // No auth is granted for `random_executor`. The contract MUST call
    // `caller.require_auth()`, so this call must panic.
    f.pm_client
        .execute_order(&random_executor, &f.trader, &symbol);
}
