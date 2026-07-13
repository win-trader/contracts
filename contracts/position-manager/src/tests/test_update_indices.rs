use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Ledger, LedgerInfo},
    Address, Env, String, Symbol,
};

use crate::{PositionManagerClient, PositionManagerContract};
use shared::constants::{INDEX_PRECISION, PRECISION};

use super::helpers;
use oracle_router::{OracleConfig, OracleRouterClient, OracleRouterContract};

const T0: u64 = 1_000_000;

struct Fixture {
    env: Env,
    pm: PositionManagerClient<'static>,
    token: mock_token::MockTokenClient<'static>,
    oracle: helpers::DualOracle<'static>,
    keeper: Address,
}

fn set_time(env: &Env, timestamp: u64) {
    env.ledger().set(LedgerInfo {
        timestamp,
        protocol_version: 23,
        sequence_number: 100,
        network_id: [0; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 100,
        min_persistent_entry_ttl: 100,
        max_entry_ttl: 10_000_000,
    });
}

fn setup() -> Fixture {
    let env = Env::default();
    env.mock_all_auths();
    set_time(&env, T0);
    let admin = Address::generate(&env);
    let keeper = Address::generate(&env);

    let token_id = env.register(mock_token::MockToken, ());
    let token = mock_token::MockTokenClient::new(&env, &token_id);
    token.initialize(
        &admin,
        &7,
        &String::from_str(&env, "USD Coin"),
        &String::from_str(&env, "USDC"),
    );

    let config_id = env.register(config_manager::ConfigManagerContract, (admin.clone(),));
    let config = config_manager::ConfigManagerClient::new(&env, &config_id);
    config.grant_role(&admin, &Symbol::new(&env, "KEEPER"), &keeper);
    config.update_carrying_fee_config(
        &admin,
        &config_manager::CarryingFeeConfig {
            base_borrow_rate_bps: 100,
            slope1_bps: 500,
            slope2_bps: 5_000,
            optimal_utilization_bps: 8_000,
            max_skew_rate_bps: 5_000,
        },
    );

    let (oracle, sources) = helpers::register_dual_oracle(&env);
    oracle.set_price(&symbol_short!("BTC"), &(50_000 * PRECISION));
    let router_id = env.register(OracleRouterContract, (config_id.clone(),));
    let router = OracleRouterClient::new(&env, &router_id);
    router.set_oracle_config(
        &admin,
        &OracleConfig {
            max_deviation_bps: 500,
            staleness_threshold: 86_400,
            cache_duration: 10,
            min_required_sources: 2,
        },
    );
    router.set_oracle_sources(&admin, &symbol_short!("BTC"), &sources);

    let pm_id = env.register(PositionManagerContract, (config_id.clone(), router_id));
    let pm = PositionManagerClient::new(&env, &pm_id);
    let vault_id = env.register(vault::VaultContract, (token_id, config_id, pm_id.clone()));
    let vault = vault::VaultContractClient::new(&env, &vault_id);
    pm.set_vault(&admin, &vault_id);
    pm.set_max_leverage(&admin, &symbol_short!("BTC"), &50);

    let lp = Address::generate(&env);
    token.mint(&lp, &(1_000_000 * PRECISION));
    vault.deposit(&(1_000_000 * PRECISION), &lp, &lp, &lp);

    let pm = unsafe { core::mem::transmute(pm) };
    let token = unsafe { core::mem::transmute(token) };
    let oracle = unsafe { core::mem::transmute(oracle) };

    Fixture {
        env,
        pm,
        token,
        oracle,
        keeper,
    }
}

fn open(f: &Fixture, trader: &Address, size: i128, is_long: bool) {
    f.token
        .mint(trader, &(size / 5 + size / 1_000 + 10 * PRECISION));
    f.pm.increase_position(
        trader,
        &symbol_short!("BTC"),
        &size,
        &(size / 5),
        &is_long,
        &0,
        &0,
        &0,
    );
}

#[test]
fn long_dominant_market_accrues_only_long_skew_index() {
    let f = setup();
    let trader = Address::generate(&f.env);
    open(&f, &trader, 100_000 * PRECISION, true);
    set_time(&f.env, T0 + 86_400);
    f.oracle
        .set_price(&symbol_short!("BTC"), &(50_000 * PRECISION));
    f.pm.update_indices(&f.keeper, &symbol_short!("BTC"));

    let market = f.pm.get_market(&symbol_short!("BTC"));
    assert!(market.acc_long_skew_index > INDEX_PRECISION);
    assert_eq!(market.acc_short_skew_index, INDEX_PRECISION);
}

#[test]
fn balanced_market_accrues_no_skew_index() {
    let f = setup();
    let long = Address::generate(&f.env);
    let short = Address::generate(&f.env);
    open(&f, &long, 100_000 * PRECISION, true);
    open(&f, &short, 100_000 * PRECISION, false);
    set_time(&f.env, T0 + 86_400);
    f.oracle
        .set_price(&symbol_short!("BTC"), &(50_000 * PRECISION));
    f.pm.update_indices(&f.keeper, &symbol_short!("BTC"));

    let market = f.pm.get_market(&symbol_short!("BTC"));
    assert_eq!(market.acc_long_skew_index, INDEX_PRECISION);
    assert_eq!(market.acc_short_skew_index, INDEX_PRECISION);
}

#[test]
fn short_dominant_market_accrues_only_short_skew_index() {
    let f = setup();
    let trader = Address::generate(&f.env);
    open(&f, &trader, 100_000 * PRECISION, false);
    set_time(&f.env, T0 + 86_400);
    f.oracle
        .set_price(&symbol_short!("BTC"), &(50_000 * PRECISION));
    f.pm.update_indices(&f.keeper, &symbol_short!("BTC"));

    let market = f.pm.get_market(&symbol_short!("BTC"));
    assert_eq!(market.acc_long_skew_index, INDEX_PRECISION);
    assert!(market.acc_short_skew_index > INDEX_PRECISION);
}

#[test]
fn same_timestamp_checkpoint_is_idempotent() {
    let f = setup();
    let trader = Address::generate(&f.env);
    open(&f, &trader, 100_000 * PRECISION, true);
    set_time(&f.env, T0 + 3_600);
    f.pm.update_indices(&f.keeper, &symbol_short!("BTC"));
    let first = f.pm.get_market(&symbol_short!("BTC"));
    f.pm.update_indices(&f.keeper, &symbol_short!("BTC"));
    let second = f.pm.get_market(&symbol_short!("BTC"));

    assert_eq!(first.acc_borrow_index, second.acc_borrow_index);
    assert_eq!(first.acc_long_skew_index, second.acc_long_skew_index);
    assert_eq!(first.acc_short_skew_index, second.acc_short_skew_index);
}
