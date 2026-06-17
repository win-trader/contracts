// ---------------------------------------------------------------------------
// Tests for: deleverage_position (Auto-Deleveraging / ADL)
//
// These tests are written BEFORE the implementation (TDD). They MUST compile
// but are expected to FAIL until deleverage_position is fully implemented.
//
// The function under test:
//   fn deleverage_position(env, caller, trader, symbol)
//
// Behavior:
//   - KEEPER-only (caller.require_auth + require_keeper)
//   - Requires initialized + NOT paused
//   - ADL triggers when EITHER:
//       1. pnl_ratio > 9500 bps (net trader PnL > 95% of total_assets)
//       2. utilization > 9500 bps (reserved > 95% of total_assets)
//   - If neither condition met => revert AdlNotTriggered (error 10)
//   - Force-close the position:
//       Trader keeps accrued profits, same settlement as full close,
//       delete position, update OI, update total_reserved, release liquidity.
// ---------------------------------------------------------------------------

use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Ledger, LedgerInfo},
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

/// Ledger timestamp used in tests
const TEST_TIMESTAMP: u64 = 1_700_000_000;

/// Min position lifetime (60s) + oracle cache (10s) + buffer
const TIME_ADVANCE: u64 = 75;

/// Slightly higher BTC price ($50,100) to make long positions profitable for ADL tests.
const BTC_PRICE_UP: i128 = 50_100 * PRECISION;

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

/// Deploy all protocol contracts. The vault receives a SMALL deposit so that
/// positions can easily push utilization above the ADL thresholds.
///
/// Vault deposit: 100,000 USDC (small, so 96k reserved = 96% utilization)
fn setup_adl<'a>() -> TestFixture<'a> {
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

    // --- 6. Vault with SMALL deposit (binds the trusted PM in its constructor) ---
    let vault_id = env.register(VaultContract, (usdc_id.clone(), config_id.clone(), pm_id.clone()));
    let vault_client = VaultContractClient::new(&env, &vault_id);

    // Only 100,000 USDC in vault -- makes it easy to breach 95% utilization
    let vault_deposit: i128 = 100_000 * USDC_UNIT;
    usdc_client.mint(&lp, &vault_deposit);
    vault_client.deposit(&vault_deposit, &lp, &lp, &lp);

    // --- 7. Wire the Vault into the PositionManager (ADMIN-gated, one-shot) ---
    pm_client.set_vault(&admin, &vault_id);
    pm_client.set_max_leverage(&admin, &symbol_short!("BTC"), &100_i128);

    // --- Fund trader ---
    usdc_client.mint(&trader, &TRADER_BALANCE);

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

/// Deploy with a large vault (1M USDC) so utilization stays low.
/// Used for tests that should NOT trigger ADL.
fn setup_no_adl<'a>() -> TestFixture<'a> {
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

    let config_id = env.register(ConfigManagerContract, (admin.clone(),));
    let config_client = ConfigManagerClient::new(&env, &config_id);

    let pauser_role = Symbol::new(&env, "PAUSER");
    let keeper_role = Symbol::new(&env, "KEEPER");
    let _admin_role = Symbol::new(&env, "ADMIN");
    config_client.grant_role(&admin, &pauser_role, &admin);
    config_client.grant_role(&admin, &keeper_role, &admin);
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

    let usdc_id = env.register(MockToken, ());
    let usdc_client = MockTokenClient::new(&env, &usdc_id);
    usdc_client.initialize(
        &admin,
        &6u32,
        &soroban_sdk::String::from_str(&env, "USD Coin"),
        &soroban_sdk::String::from_str(&env, "USDC"),
    );

    let (oracle_client, oracle_sources) = helpers::register_dual_oracle(&env);
    oracle_client.set_price(&symbol_short!("BTC"), &BTC_PRICE);

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

    let pm_id = env.register(PositionManagerContract, (config_id.clone(), oracle_router_id.clone()));
    let pm_client = PositionManagerClient::new(&env, &pm_id);

    let vault_id = env.register(VaultContract, (usdc_id.clone(), config_id.clone(), pm_id.clone()));
    let vault_client = VaultContractClient::new(&env, &vault_id);

    // Large vault: 1,000,000 USDC
    let vault_deposit: i128 = 1_000_000 * USDC_UNIT;
    usdc_client.mint(&lp, &vault_deposit);
    vault_client.deposit(&vault_deposit, &lp, &lp, &lp);

    pm_client.set_vault(&admin, &vault_id);
    pm_client.set_max_leverage(&admin, &symbol_short!("BTC"), &100_i128);
    usdc_client.mint(&trader, &TRADER_BALANCE);

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

/// Force Vault.reserved_usdc to `target` via reserve/release. Vault is the
/// single source of truth for reserved liquidity (#8).
fn force_reserved(f: &TestFixture, target: i128) {
    let cur = f._vault_client.reserved_usdc();
    if target > cur {
        f._vault_client.reserve_liquidity(&f.pm_addr, &(target - cur));
    } else if cur > target {
        f._vault_client.release_liquidity(&f.pm_addr, &(cur - target));
    }
}

// ===========================================================================
// 1. Guard tests
// ===========================================================================

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn test_adl_reverts_unauthorized_caller() {
    // Scenario: A non-KEEPER address attempts ADL. Must revert with
    // PositionManagerError::Unauthorized (7) — panic comes from PM's wrapper.
    let f = setup_no_adl();
    let random_caller = Address::generate(&f.env);

    f.pm_client
        .deleverage_position(&random_caller, &f.trader, &symbol_short!("BTC"));
}

// ===========================================================================
// 2. ADL trigger check -- conditions not met (both pnl_ratio and utilization <= 9500 bps)
// ===========================================================================

#[test]
#[should_panic(expected = "Error(Contract, #10)")]
fn test_adl_reverts_when_utilization_is_normal() {
    // Scenario: Vault has 1M USDC, position uses 10k (1% utilization).
    // Neither pnl_ratio > 9500 bps nor utilization > 9500 bps is met.
    // Must revert with AdlNotTriggered (error 10).
    let f = setup_no_adl();
    let symbol = symbol_short!("BTC");

    let size = 10_000 * USDC_UNIT;
    let collateral = 1_000 * USDC_UNIT;
    f.pm_client
        .increase_position(&f.trader, &symbol, &size, &collateral, &true, &0, &0, &0i128);

    // Advance time past oracle cache
    advance_time_and_set_price(&f, TEST_TIMESTAMP + TIME_ADVANCE, BTC_PRICE);

    // Utilization is ~1%, well below ADL thresholds
    f.pm_client
        .deleverage_position(&f.keeper, &f.trader, &symbol);
}

#[test]
#[should_panic(expected = "Error(Contract, #10)")]
fn test_adl_reverts_at_50_percent_utilization() {
    // Scenario: Vault has 1M USDC, position uses 500k (50% utilization).
    // Still below both pnl_ratio and utilization 9500 bps thresholds.
    let f = setup_no_adl();
    let symbol = symbol_short!("BTC");

    // Need large collateral for 500k position
    let size = 500_000 * USDC_UNIT;
    let collateral = 50_000 * USDC_UNIT;
    f.usdc_client.mint(&f.trader, &(collateral * 2));

    f.pm_client
        .increase_position(&f.trader, &symbol, &size, &collateral, &true, &0, &0, &0i128);

    advance_time_and_set_price(&f, TEST_TIMESTAMP + TIME_ADVANCE, BTC_PRICE);

    f.pm_client
        .deleverage_position(&f.keeper, &f.trader, &symbol);
}

// ===========================================================================
// 3. ADL trigger -- conditions met (utilization > 95% of total_assets)
// ===========================================================================

#[test]
fn test_adl_succeeds_when_reserved_exceeds_95_percent() {
    // Scenario: Vault has 100,000 USDC. We open a position with 84,000 size
    // (84% utilization, under the increase cap). Then we manually set
    // total_reserved to 96,000 (simulating other positions) to breach the
    // 95% utilization threshold (9500 bps).
    let f = setup_adl();
    let symbol = symbol_short!("BTC");

    // Open at just under 85% of 100k = 84k reserved
    let size = 84_000 * USDC_UNIT;
    let collateral = 8_400 * USDC_UNIT;
    f.pm_client
        .increase_position(&f.trader, &symbol, &size, &collateral, &true, &0, &0, &0i128);

    // Manually push total_reserved above 95% to simulate ADL condition.
    // In production this could happen if the vault's total_assets decreases
    // (e.g., due to trader profit payouts shrinking free liquidity).
    force_reserved(&f, 96_000 * USDC_UNIT);

    // Advance time past oracle cache; price up slightly so position is profitable
    advance_time_and_set_price(&f, TEST_TIMESTAMP + TIME_ADVANCE, BTC_PRICE_UP);

    // ADL should succeed because utilization (96k/100k = 9600 bps) > 9500 bps
    f.pm_client
        .deleverage_position(&f.keeper, &f.trader, &symbol);

    // Position must be deleted
    f.env.as_contract(&f.pm_addr, || {
        let pos = storage::get_position(&f.env, &f.trader, &symbol);
        assert!(pos.is_none(), "Position must be deleted after ADL");
    });
}

// ===========================================================================
// 4. Successful ADL -- profitable position
// ===========================================================================

#[test]
fn test_adl_profitable_long_trader_receives_profits() {
    // Scenario: Trader has a profitable long position that gets ADL'd.
    // The trader should receive their collateral + profits.
    //
    // Open long at $50,000, size=84,000, collateral=8,400.
    // Push total_reserved above 95% (utilization > 9500 bps).
    // Advance price to $55,000 (profitable).
    //   pnl = 84,000 * (55,000 - 50,000) / 50,000 = 8,400 USDC profit
    // Trader should get back collateral + profit = 8,400 + 8,400 = 16,800
    let f = setup_adl();
    let symbol = symbol_short!("BTC");

    let size = 84_000 * USDC_UNIT;
    let collateral = 8_400 * USDC_UNIT;
    f.pm_client
        .increase_position(&f.trader, &symbol, &size, &collateral, &true, &0, &0, &0i128);

    // Push reserved above 95% threshold (utilization > 9500 bps)
    force_reserved(&f, 96_000 * USDC_UNIT);

    // Price increases (trader is profitable)
    let higher_price: i128 = 55_000 * PRECISION;
    advance_time_and_set_price(&f, TEST_TIMESTAMP + TIME_ADVANCE, higher_price);

    let trader_balance_before = f.usdc_client.balance(&f.trader);

    // ADL the position
    f.pm_client
        .deleverage_position(&f.keeper, &f.trader, &symbol);

    let trader_balance_after = f.usdc_client.balance(&f.trader);

    // Trader should receive funds (collateral + profit)
    assert!(
        trader_balance_after > trader_balance_before,
        "Trader must receive funds after profitable ADL. Before: {}, After: {}",
        trader_balance_before,
        trader_balance_after
    );

    // Position must be deleted
    f.env.as_contract(&f.pm_addr, || {
        let pos = storage::get_position(&f.env, &f.trader, &symbol);
        assert!(pos.is_none(), "Position must be deleted after ADL");
    });
}

#[test]
fn test_adl_deletes_position_and_decreases_oi() {
    // Scenario: After ADL, the position must be deleted and market OI must
    // decrease by the position's size.
    let f = setup_adl();
    let symbol = symbol_short!("BTC");

    let size = 84_000 * USDC_UNIT;
    let collateral = 8_400 * USDC_UNIT;
    f.pm_client
        .increase_position(&f.trader, &symbol, &size, &collateral, &true, &0, &0, &0i128);

    let market_before = f.pm_client.get_market(&symbol);

    // Push reserved above 95% (utilization > 9500 bps)
    force_reserved(&f, 96_000 * USDC_UNIT);

    advance_time_and_set_price(&f, TEST_TIMESTAMP + TIME_ADVANCE, BTC_PRICE_UP);

    f.pm_client
        .deleverage_position(&f.keeper, &f.trader, &symbol);

    // OI must decrease
    let market_after = f.pm_client.get_market(&symbol);
    assert_eq!(
        market_after.long_open_interest,
        market_before.long_open_interest - size,
        "Long OI must decrease by position size after ADL"
    );

    // Total reserved must decrease
    let total_reserved_after = f
        .env
        .as_contract(&f.pm_addr, || f._vault_client.reserved_usdc());
    // The new total_reserved should be 96,000 - 84,000 = 12,000 (or thereabouts,
    // depending on exact settlement logic)
    assert!(
        total_reserved_after < 96_000 * USDC_UNIT,
        "Total reserved must decrease after ADL. Got: {}",
        total_reserved_after
    );
}

#[test]
fn test_adl_decreases_total_reserved() {
    // Scenario: Verify total_reserved is properly decreased after ADL.
    let f = setup_adl();
    let symbol = symbol_short!("BTC");

    let size = 80_000 * USDC_UNIT;
    let collateral = 8_000 * USDC_UNIT;
    f.pm_client
        .increase_position(&f.trader, &symbol, &size, &collateral, &true, &0, &0, &0i128);

    // Push reserved above 95% (utilization > 9500 bps)
    force_reserved(&f, 96_000 * USDC_UNIT);

    advance_time_and_set_price(&f, TEST_TIMESTAMP + TIME_ADVANCE, BTC_PRICE_UP);

    f.pm_client
        .deleverage_position(&f.keeper, &f.trader, &symbol);

    let total_reserved_after = f
        .env
        .as_contract(&f.pm_addr, || f._vault_client.reserved_usdc());

    // After closing an 80k position from 96k reserved, should be ~16k
    assert_eq!(
        total_reserved_after,
        96_000 * USDC_UNIT - size,
        "Total reserved must decrease by exactly the position size"
    );
}

// ===========================================================================
// 5. Paused contract -- ADL works even when paused (critical safety mechanism)
// ===========================================================================

#[test]
fn test_adl_succeeds_when_paused() {
    // Scenario: Contract is paused. ADL must still work during crises,
    // just like liquidations, to prevent vault insolvency.
    let f = setup_adl();
    let symbol = symbol_short!("BTC");

    let size = 84_000 * USDC_UNIT;
    let collateral = 8_400 * USDC_UNIT;
    f.pm_client
        .increase_position(&f.trader, &symbol, &size, &collateral, &true, &0, &0, &0i128);

    force_reserved(&f, 96_000 * USDC_UNIT);

    advance_time_and_set_price(&f, TEST_TIMESTAMP + TIME_ADVANCE, BTC_PRICE_UP);

    // Pause the contract
    f.pm_client.pause(&f.admin);

    // ADL should succeed even though paused
    f.pm_client
        .deleverage_position(&f.keeper, &f.trader, &symbol);

    // Position should be deleted
    let result = f.pm_client.try_get_position(&f.trader, &symbol);
    assert!(result.is_err(), "Position must be deleted after ADL");
}

// ===========================================================================
// 6. ADL does not affect other positions
// ===========================================================================

#[test]
fn test_adl_does_not_affect_other_traders_positions() {
    // Scenario: Two traders have positions, ADL is triggered on trader1.
    // Trader2's position must remain intact.
    let f = setup_adl();
    let symbol = symbol_short!("BTC");

    let trader2 = Address::generate(&f.env);
    f.usdc_client.mint(&trader2, &TRADER_BALANCE);

    // Trader1: small position
    let size1 = 40_000 * USDC_UNIT;
    let collateral1 = 4_000 * USDC_UNIT;
    f.pm_client
        .increase_position(&f.trader, &symbol, &size1, &collateral1, &true, &0, &0, &0i128);

    // Trader2: small position
    let size2 = 40_000 * USDC_UNIT;
    let collateral2 = 4_000 * USDC_UNIT;
    f.pm_client
        .increase_position(&trader2, &symbol, &size2, &collateral2, &true, &0, &0, &0i128);

    // Push reserved above 95% (utilization > 9500 bps)
    force_reserved(&f, 96_000 * USDC_UNIT);

    advance_time_and_set_price(&f, TEST_TIMESTAMP + TIME_ADVANCE, BTC_PRICE_UP);

    // ADL trader1 only
    f.pm_client
        .deleverage_position(&f.keeper, &f.trader, &symbol);

    // Trader1 position must be gone
    f.env.as_contract(&f.pm_addr, || {
        assert!(
            storage::get_position(&f.env, &f.trader, &symbol).is_none(),
            "Trader1 position must be deleted after ADL"
        );
    });

    // Trader2 position must still exist
    let pos2 = f.pm_client.get_position(&trader2, &symbol);
    assert_eq!(
        pos2.size, size2,
        "Trader2 position must be unaffected by trader1 ADL"
    );
}

// ===========================================================================
// 7. ADL escrow lifecycle
// ===========================================================================

/// Default flat TP/SL execution fee planted by ConfigManager::initialize.
const TP_SL_FEE: i128 = 5_000_000;

#[test]
fn adl_refunds_escrow_to_trader() {
    // Open two identical longs, one for `trader` WITH TP (escrow charged)
    // and one for `trader2` WITHOUT TP (no escrow). Trigger ADL for both
    // and compare the trader receipts: the difference must equal the
    // escrow refund.
    let f = setup_adl();
    let symbol = symbol_short!("BTC");

    let trader2 = Address::generate(&f.env);
    f.usdc_client.mint(&trader2, &TRADER_BALANCE);

    let size = 40_000 * USDC_UNIT;
    let collateral = 4_000 * USDC_UNIT;
    // Trader 1 (escrow) -> open + set TP
    f.pm_client
        .increase_position(&f.trader, &symbol, &size, &collateral, &true, &0, &0, &0i128);
    let tp = 55_000 * PRECISION;
    f.pm_client.set_tp_sl(&f.trader, &symbol, &tp, &0);

    let pos = f.pm_client.get_position(&f.trader, &symbol);
    assert_eq!(
        pos.execution_fee_escrow, TP_SL_FEE,
        "Setup: trader's position must record TP_SL_FEE as escrow"
    );

    // Trader 2 (no escrow)
    f.pm_client
        .increase_position(&trader2, &symbol, &size, &collateral, &true, &0, &0, &0i128);

    // Push utilization > 95%.
    force_reserved(&f, 96_000 * USDC_UNIT);

    let higher_price: i128 = 55_000 * PRECISION;
    advance_time_and_set_price(&f, TEST_TIMESTAMP + TIME_ADVANCE, higher_price);

    let trader_balance_before = f.usdc_client.balance(&f.trader);
    let trader2_balance_before = f.usdc_client.balance(&trader2);
    let keeper_balance_before = f.usdc_client.balance(&f.keeper);

    // ADL trader1 first.
    f.pm_client
        .deleverage_position(&f.keeper, &f.trader, &symbol);
    // Re-arm utilization so trader2's ADL trigger still fires after the
    // first close released reservations. Keep the ledger timestamp fixed so
    // the two closes accrue identical borrow/funding fees — the asymmetry
    // under test is the escrow refund only.
    force_reserved(&f, 96_000 * USDC_UNIT);
    f.pm_client
        .deleverage_position(&f.keeper, &trader2, &symbol);

    let trader_received = f.usdc_client.balance(&f.trader) - trader_balance_before;
    let trader2_received = f.usdc_client.balance(&trader2) - trader2_balance_before;
    let keeper_received = f.usdc_client.balance(&f.keeper) - keeper_balance_before;

    // The difference equals the escrow — the only asymmetry between the
    // two ADL closes is the escrow refund.
    assert_eq!(
        trader_received - trader2_received,
        TP_SL_FEE,
        "ADL must refund the escrow to the trader. Diff: {}",
        trader_received - trader2_received
    );

    // Keeper must NOT receive any escrow on ADL.
    assert_eq!(
        keeper_received, 0,
        "ADL must NOT pay any escrow to the keeper. Got: {}",
        keeper_received
    );

    // Position deleted.
    f.env.as_contract(&f.pm_addr, || {
        let p = storage::get_position(&f.env, &f.trader, &symbol);
        assert!(p.is_none(), "Position must be deleted after ADL");
    });
}

#[test]
fn adl_with_zero_escrow_no_extra_transfer() {
    // Run two ADL closes side-by-side: both positions have NO TP/SL.
    // Confirm the receipts are identical — no phantom escrow flow distorts
    // the no-escrow path. This is a regression check; any future change
    // that routes TP_SL_FEE on the no-escrow path would diverge the two.
    let f = setup_adl();
    let symbol = symbol_short!("BTC");

    let trader2 = Address::generate(&f.env);
    f.usdc_client.mint(&trader2, &TRADER_BALANCE);

    let size = 40_000 * USDC_UNIT;
    let collateral = 4_000 * USDC_UNIT;
    f.pm_client
        .increase_position(&f.trader, &symbol, &size, &collateral, &true, &0, &0, &0i128);
    f.pm_client
        .increase_position(&trader2, &symbol, &size, &collateral, &true, &0, &0, &0i128);

    // Both positions have escrow == 0.
    let p1 = f.pm_client.get_position(&f.trader, &symbol);
    let p2 = f.pm_client.get_position(&trader2, &symbol);
    assert_eq!(p1.execution_fee_escrow, 0);
    assert_eq!(p2.execution_fee_escrow, 0);

    force_reserved(&f, 96_000 * USDC_UNIT);

    let higher_price: i128 = 55_000 * PRECISION;
    advance_time_and_set_price(&f, TEST_TIMESTAMP + TIME_ADVANCE, higher_price);

    let trader_balance_before = f.usdc_client.balance(&f.trader);
    let trader2_balance_before = f.usdc_client.balance(&trader2);
    let keeper_balance_before = f.usdc_client.balance(&f.keeper);

    f.pm_client
        .deleverage_position(&f.keeper, &f.trader, &symbol);
    // Re-arm utilization for the second ADL. Keep the ledger timestamp
    // fixed so both closes accrue identical fees — any divergence in
    // receipts would point at a phantom escrow flow.
    force_reserved(&f, 96_000 * USDC_UNIT);
    f.pm_client
        .deleverage_position(&f.keeper, &trader2, &symbol);

    let trader_received = f.usdc_client.balance(&f.trader) - trader_balance_before;
    let trader2_received = f.usdc_client.balance(&trader2) - trader2_balance_before;
    let keeper_received = f.usdc_client.balance(&f.keeper) - keeper_balance_before;

    // Symmetric outcome — no escrow path fires on either close.
    assert_eq!(
        trader_received, trader2_received,
        "Without escrow, ADL receipts must be identical across traders. {} vs {}",
        trader_received, trader2_received
    );

    // Keeper got nothing — ADL never pays the keeper from the escrow path.
    assert_eq!(
        keeper_received, 0,
        "Keeper must NOT receive funds on ADL. Got: {}",
        keeper_received
    );

}
