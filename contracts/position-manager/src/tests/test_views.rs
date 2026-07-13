// ---------------------------------------------------------------------------
// Tests for: bump_position, execute_order (V2 stub), get_position, get_market,
//            pause/unpause interaction
// ---------------------------------------------------------------------------

use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Ledger, LedgerInfo},
    Address, Env, Symbol,
};

use crate::contract::PositionManagerContract;
use shared::constants::PRECISION;
use crate::PositionManagerClient;

use config_manager::{ConfigManagerClient, ConfigManagerContract};
use super::helpers::{self, DualOracle};
use mock_token::{MockToken, MockTokenClient};
use oracle_router::{OracleConfig, OracleRouterClient, OracleRouterContract};
use vault::{VaultContract, VaultContractClient};

const BTC_PRICE: i128 = 50_000 * PRECISION;
const USDC_UNIT: i128 = 1_000_000;
const TRADER_BALANCE: i128 = 100_000 * USDC_UNIT;
const VAULT_DEPOSIT: i128 = 1_000_000 * USDC_UNIT;
const DEFAULT_SIZE: i128 = 10_000 * USDC_UNIT;
const DEFAULT_COLLATERAL: i128 = 1_000 * USDC_UNIT;
const TEST_TIMESTAMP: u64 = 1_700_000_000;

struct TestFixture<'a> {
    env: Env,
    pm_client: PositionManagerClient<'a>,
    // config_client: ConfigManagerClient<'a>,
    // usdc_client: MockTokenClient<'a>,
    oracle_client: DualOracle<'a>,
    admin: Address,
    keeper: Address,
    trader: Address,
    // pm_addr: Address,
}

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

    let config_id = env.register(ConfigManagerContract, (admin.clone(),));
    let config_client = ConfigManagerClient::new(&env, &config_id);

    let pauser_role = Symbol::new(&env, "PAUSER");
    let keeper_role = Symbol::new(&env, "KEEPER");
    // let admin_role = Symbol::new(&env, "ADMIN");
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

    pm_client.set_vault(&admin, &vault_id);
    pm_client.set_max_leverage(&admin, &symbol_short!("BTC"), &100_i128);

    usdc_client.mint(&trader, &TRADER_BALANCE);
    usdc_client.mint(&lp, &VAULT_DEPOSIT);
    vault_client.deposit(&VAULT_DEPOSIT, &lp, &lp, &lp);

    let pm_client = unsafe { core::mem::transmute(pm_client) };
    // let config_client = unsafe { core::mem::transmute(config_client) };
    // let usdc_client = unsafe { core::mem::transmute(usdc_client) };
    let oracle_client = unsafe { core::mem::transmute(oracle_client) };

    TestFixture {
        env,
        pm_client,
        // config_client,
        // usdc_client,
        oracle_client,
        admin,
        keeper,
        trader,
        // pm_addr: pm_id,
    }
}

// ===========================================================================
// bump_position
// ===========================================================================

#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn test_bump_position_reverts_position_not_found() {
    let f = setup_full();
    f.pm_client.bump_position(&f.trader, &symbol_short!("BTC"));
}

#[test]
fn test_bump_position_succeeds_on_existing_position() {
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
    // Should not panic
    f.pm_client.bump_position(&f.trader, &symbol_short!("BTC"));
}

#[test]
fn test_bump_position_callable_by_anyone() {
    // bump_position takes user_address (position owner) — no auth required.
    // Anyone can call it to keep positions alive on-chain.
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
    // Calling with the trader's address should work regardless of who submits the tx
    f.pm_client.bump_position(&f.trader, &symbol_short!("BTC"));
}

// ===========================================================================
// execute_order
// ===========================================================================

#[test]
fn test_execute_order_allowed_when_paused() {
    // TP/SL orders protect traders and must execute during emergencies.
    // This test verifies execute_order does not revert due to pause.
    // It will still revert for other reasons (no position), but NOT #3 (Paused).
    let f = setup_full();

    // Open a position and set TP before pausing
    f.pm_client.increase_position(
        &f.trader,
        &soroban_sdk::symbol_short!("BTC"),
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0, &0i128
    );
    let tp = 55_000 * PRECISION;
    f.pm_client
        .set_tp_sl(&f.trader, &soroban_sdk::symbol_short!("BTC"), &tp, &0);

    f.pm_client.pause(&f.admin);

    // Advance time past min lifetime and set trigger price
    let trigger_price = 56_000 * PRECISION;
    f.env.ledger().set(LedgerInfo {
        timestamp: f.env.ledger().timestamp() + 120,
        sequence_number: f.env.ledger().sequence(),
        protocol_version: 23,
        network_id: [0u8; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 100,
        min_persistent_entry_ttl: 100,
        max_entry_ttl: 10_000_000,
    });
    f.oracle_client
        .set_price(&soroban_sdk::symbol_short!("BTC"), &trigger_price);

    // Should succeed even when paused
    f.pm_client
        .execute_order(&f.keeper, &f.trader, &soroban_sdk::symbol_short!("BTC"));
}

// ===========================================================================
// get_position
// ===========================================================================

#[test]
fn test_get_position_returns_correct_data() {
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
    let pos = f.pm_client.get_position(&f.trader, &symbol_short!("BTC"));
    assert_eq!(pos.size, DEFAULT_SIZE);
    assert_eq!(pos.collateral, DEFAULT_COLLATERAL);
    assert_eq!(pos.entry_price, BTC_PRICE);
    assert!(pos.is_long);
}

#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn test_get_position_reverts_not_found() {
    let f = setup_full();
    f.pm_client.get_position(&f.trader, &symbol_short!("BTC"));
}

// ===========================================================================
// get_market
// ===========================================================================

#[test]
fn test_get_market_returns_defaults_for_unknown_symbol() {
    let f = setup_full();
    let market = f.pm_client.get_market(&symbol_short!("ETH"));
    assert_eq!(market.long_open_interest, 0);
    assert_eq!(market.short_open_interest, 0);
    assert_eq!(market.acc_borrow_index, shared::constants::INDEX_PRECISION);
    assert_eq!(market.acc_long_skew_index, shared::constants::INDEX_PRECISION);
}

#[test]
fn test_get_market_returns_correct_oi_after_increase() {
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
    let market = f.pm_client.get_market(&symbol_short!("BTC"));
    assert_eq!(market.long_open_interest, DEFAULT_SIZE);
    assert_eq!(market.short_open_interest, 0);
}

// ===========================================================================
// pause / unpause interaction
// ===========================================================================

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn test_increase_position_reverts_when_paused() {
    let f = setup_full();
    f.pm_client.pause(&f.admin);
    f.pm_client.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0, &0i128
    );
}

#[test]
fn test_unpause_allows_increase_position_again() {
    let f = setup_full();
    f.pm_client.pause(&f.admin);
    f.pm_client.unpause(&f.admin);
    // Should succeed after unpause
    f.pm_client.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &DEFAULT_SIZE,
        &DEFAULT_COLLATERAL,
        &true,
        &0,
        &0, &0i128
    );
    let pos = f.pm_client.get_position(&f.trader, &symbol_short!("BTC"));
    assert_eq!(pos.size, DEFAULT_SIZE);
}
