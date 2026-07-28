use soroban_sdk::{
    contractclient, contracttype, symbol_short, testutils::Address as _, testutils::Ledger as _,
    vec, Address, Env, String, Symbol, Vec,
};

#[allow(dead_code)]
mod abi {
    use super::*;

    #[contracttype]
    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct OracleConfig {
        pub max_deviation_bps: i128,
        pub staleness_threshold: u64,
        pub cache_duration: u64,
        pub min_required_sources: u32,
    }

    #[contracttype]
    #[derive(Clone, Debug)]
    pub struct Position {
        pub id: u64,
        pub owner: Address,
        pub market: Symbol,
        pub is_long: bool,
        pub size: i128,
        pub base_exposure: i128,
        pub stored_collateral: i128,
        pub risk_units: i128,
        pub borrow_debt: i128,
        pub funding_paid_to_receivers_debt: i128,
        pub funding_paid_to_lps_debt: i128,
        pub funding_received_debt: i128,
        pub execution_budget: i128,
        pub last_increased_time: u64,
        pub take_profit: i128,
        pub stop_loss: i128,
    }

    #[contracttype]
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum RiskState {
        Normal,
        Warning,
        Adl,
        HardCap,
    }

    #[contracttype]
    #[derive(Clone, Debug)]
    pub struct MarketSide {
        pub size_open_interest: i128,
        pub base_exposure: i128,
        pub stored_collateral_total: i128,
        pub risk_units: i128,
        pub risk_state: RiskState,
    }

    #[contracttype]
    #[derive(Clone, Debug)]
    pub struct MarketConfig {
        pub open_fee_low_bps: u32,
        pub open_fee_high_bps: u32,
        pub max_funding_rate_bps_day: i128,
        pub market_risk_factor_bps: u32,
        pub max_long_size_open_interest: i128,
        pub max_short_size_open_interest: i128,
        pub max_long_base_exposure: i128,
        pub max_short_base_exposure: i128,
        pub recovery_pnl_factor_bps: u32,
        pub warning_pnl_factor_bps: u32,
        pub adl_pnl_factor_bps: u32,
        pub hard_cap_pnl_factor_bps: u32,
        pub maintenance_margin_bps: u32,
        pub liquidation_reward_bps: u32,
        pub adl_reward_bps: u32,
    }

    #[contracttype]
    #[derive(Clone, Debug)]
    pub struct GlobalConfig {
        pub min_collateral: i128,
        pub min_position_lifetime: u64,
        pub risk_capacity_limit_bps: u32,
        pub base_borrow_rate_bps_day: i128,
        pub max_variable_borrow_bps_day: i128,
        pub lp_revenue_share_bps: u32,
        pub risk_keeper_revenue_share_bps: u32,
        pub hard_cap_factor_limit_bps: u32,
        pub max_adl_reward: i128,
        pub max_insolvent_touch_reward: i128,
        pub max_active_markets: u32,
    }

    #[contracttype]
    #[derive(Clone, Debug)]
    pub struct LpConfig {
        pub max_withdraw_utilization_bps: u32,
        pub min_deposit_nav_factor_bps: u32,
        pub lp_request_delay: u64,
    }

    #[contracttype]
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum PayerSide {
        None,
        Long,
        Short,
    }

    #[contracttype]
    #[derive(Clone, Debug)]
    pub struct Market {
        pub long: MarketSide,
        pub short: MarketSide,
        pub receiver_backed_index_long: i128,
        pub receiver_backed_index_short: i128,
        pub lp_backed_index_long: i128,
        pub lp_backed_index_short: i128,
        pub receiver_index_long: i128,
        pub receiver_index_short: i128,
        pub current_payer_side: PayerSide,
        pub current_payer_rate: i128,
        pub receiver_flow_per_second: i128,
        pub lp_flow_per_second: i128,
        pub last_funding_checkpoint: u64,
        pub receiver_payer_remainder: i128,
        pub lp_payer_remainder: i128,
        pub receiver_index_remainder: i128,
        pub receiver_flow_remainder: i128,
        pub config: MarketConfig,
    }

    #[contracttype]
    #[derive(Clone, Debug)]
    pub struct AccountingSnapshot {
        pub physical_cash: i128,
        pub non_lp_claims: i128,
        pub cash_lp_equity: i128,
        pub cash_shortfall: i128,
        pub required_risk_backing: i128,
        pub free_lp_capital: i128,
        pub vault_nav: i128,
        pub total_risk_units: i128,
        pub open_position_count: u64,
        pub lp_blocked_side_count: u32,
    }

    #[contracttype]
    #[derive(Clone, Debug)]
    pub struct RoundPrice {
        pub symbol: Symbol,
        pub price: i128,
    }

    #[contracttype]
    #[derive(Clone, Debug)]
    pub struct OracleRound {
        pub id: u64,
        pub timestamp: u64,
        pub previous_id: u64,
        pub previous_timestamp: u64,
        pub prices: Vec<RoundPrice>,
    }

    #[contracttype]
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum LpRequestKind {
        Deposit,
        Withdrawal,
    }

    #[contracttype]
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum LpRequestStatus {
        Pending,
        Settled,
        Failed,
        Expired,
    }

    #[contracttype]
    #[derive(Clone, Debug)]
    pub struct LpRequest {
        pub id: u64,
        pub owner: Address,
        pub kind: LpRequestKind,
        pub amount: i128,
        pub request_time: u64,
        pub execute_after: u64,
        pub status: LpRequestStatus,
    }

    #[contracttype]
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum SettlementStatus {
        Settled,
        Failed,
    }

    #[contracttype]
    #[derive(Clone, Debug)]
    pub struct SettlementResult {
        pub status: SettlementStatus,
        pub amount: i128,
    }

    #[contractclient(name = "ConfigManagerClient")]
    pub trait ConfigManager {
        fn grant_role(env: Env, caller: Address, role: Symbol, account: Address);
    }

    #[contractclient(name = "OracleClient")]
    pub trait Oracle {
        fn set_price(env: Env, caller: Address, symbol: Symbol, price: i128);
    }

    #[contractclient(name = "OracleRouterClient")]
    pub trait OracleRouter {
        fn set_position_manager(env: Env, caller: Address, position_manager: Address);
        fn publish_round(env: Env, caller: Address) -> u64;
        fn latest_round_id(env: Env) -> u64;
        fn get_round(env: Env, round_id: u64) -> OracleRound;
        fn set_oracle_sources(env: Env, caller: Address, symbol: Symbol, sources: Vec<Address>);
        fn set_oracle_config(env: Env, caller: Address, config: OracleConfig);
    }

    #[contractclient(name = "PositionManagerClient")]
    pub trait PositionManager {
        fn set_vault(env: Env, caller: Address, vault: Address);
        fn open_position(
            env: Env,
            owner: Address,
            market: Symbol,
            is_long: bool,
            size: i128,
            collateral: i128,
            execution_budget: i128,
            take_profit: i128,
            stop_loss: i128,
            acceptable_price: i128,
        ) -> u64;
        fn decrease_position(
            env: Env,
            position_id: u64,
            size_removed: i128,
            collateral_withdrawn: i128,
            acceptable_price: i128,
        );
        fn update_indices(env: Env, caller: Address, market: Symbol);
        fn set_market_config(env: Env, caller: Address, market: Symbol, config: MarketConfig);
        fn get_position(env: Env, position_id: u64) -> Position;
        fn get_market(env: Env, market: Symbol) -> Market;
        fn pending_receiver_funding_total(env: Env) -> i128;
    }

    #[contractclient(name = "RequestRouterClient")]
    pub trait RequestRouter {
        fn request_deposit(env: Env, owner: Address, assets: i128) -> u64;
        fn request_withdrawal(env: Env, owner: Address, shares: i128) -> u64;
        fn resolve_next(env: Env, executor: Address) -> SettlementResult;
        fn get_request(env: Env, request_id: u64) -> LpRequest;
    }

    #[contractclient(name = "VaultClient")]
    pub trait Vault {
        fn set_request_router(env: Env, caller: Address, request_router: Address);
        fn accounting_snapshot(env: Env, round: OracleRound) -> AccountingSnapshot;
        fn physical_cash(env: Env) -> i128;
        fn total_share_supply(env: Env) -> i128;
        fn balance(env: Env, account: Address) -> i128;
    }

    #[contractclient(name = "MockTokenAdminClient")]
    pub trait MockTokenAdmin {
        fn initialize(env: Env, admin: Address, decimals: u32, name: String, symbol: String);
        fn admin_mint(env: Env, admin: Address, to: Address, amount: i128);
    }
}

mod config_manager {
    pub use super::abi::ConfigManagerClient as Client;
    pub const WASM: &[u8] =
        include_bytes!("../../target/wasm32v1-none/release/config_manager.wasm");
}

mod oracle {
    pub use super::abi::OracleClient as Client;
    pub const WASM: &[u8] = include_bytes!("../../target/wasm32v1-none/release/oracle.wasm");
}

mod oracle_router {
    pub use super::abi::{OracleConfig, OracleRouterClient as Client};
    pub const WASM: &[u8] = include_bytes!("../../target/wasm32v1-none/release/oracle_router.wasm");
}

mod position_manager {
    pub use super::abi::{GlobalConfig, MarketConfig, PositionManagerClient as Client};
    pub const WASM: &[u8] =
        include_bytes!("../../target/wasm32v1-none/release/position_manager.wasm");
}

mod request_router {
    pub use super::abi::{LpRequestStatus, RequestRouterClient as Client, SettlementStatus};
    pub const WASM: &[u8] =
        include_bytes!("../../target/wasm32v1-none/release/request_router.wasm");
}

mod vault {
    pub use super::abi::{
        AccountingSnapshot, LpConfig, OracleRound, RoundPrice, VaultClient as Client,
    };
    pub const WASM: &[u8] = include_bytes!("../../target/wasm32v1-none/release/vault.wasm");
}

mod mock_token {
    pub use super::abi::MockTokenAdminClient as Client;
    pub const WASM: &[u8] = include_bytes!("../../target/wasm32v1-none/release/mock_token.wasm");
}

const UNIT: i128 = 10_000_000;
const START_TIME: u64 = 1_000_000;
const REQUEST_DELAY: u64 = 60;
const DAY: u64 = 86_400;

struct Protocol {
    env: Env,
    admin: Address,
    keeper: Address,
    publisher: Address,
    lp: Address,
    trader_a: Address,
    trader_b: Address,
    token_id: Address,
    oracle_ids: [Address; 3],
    oracle_router_id: Address,
    position_manager_id: Address,
    vault_id: Address,
    request_router_id: Address,
    market: Symbol,
}

impl Protocol {
    fn new() -> Self {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().with_mut(|ledger| {
            ledger.timestamp = START_TIME;
            ledger.sequence_number = 1;
        });

        let admin = Address::generate(&env);
        let keeper = Address::generate(&env);
        let publisher = Address::generate(&env);
        let lp = Address::generate(&env);
        let trader_a = Address::generate(&env);
        let trader_b = Address::generate(&env);
        let market = symbol_short!("BTC");

        let config_manager_id = env.register(config_manager::WASM, (&admin,));
        let config = config_manager::Client::new(&env, &config_manager_id);
        config.grant_role(&admin, &symbol_short!("KEEPER"), &keeper);

        let token_id = env.register(mock_token::WASM, ());
        let token = mock_token::Client::new(&env, &token_id);
        token.initialize(
            &admin,
            &7,
            &String::from_str(&env, "Test USD"),
            &String::from_str(&env, "TUSD"),
        );

        let oracle_ids = [
            env.register(oracle::WASM, (&config_manager_id, &publisher)),
            env.register(oracle::WASM, (&config_manager_id, &publisher)),
            env.register(oracle::WASM, (&config_manager_id, &publisher)),
        ];

        let oracle_router_id = env.register(oracle_router::WASM, (&config_manager_id,));
        let oracle_router = oracle_router::Client::new(&env, &oracle_router_id);
        oracle_router.set_oracle_config(
            &admin,
            &oracle_router::OracleConfig {
                max_deviation_bps: 100,
                staleness_threshold: DAY,
                cache_duration: 30,
                min_required_sources: 3,
            },
        );
        oracle_router.set_oracle_sources(
            &admin,
            &market,
            &vec![
                &env,
                oracle_ids[0].clone(),
                oracle_ids[1].clone(),
                oracle_ids[2].clone(),
            ],
        );

        let position_manager_id = env.register(
            position_manager::WASM,
            (
                &config_manager_id,
                &oracle_router_id,
                position_manager::GlobalConfig {
                    min_collateral: 10 * UNIT,
                    min_position_lifetime: 0,
                    risk_capacity_limit_bps: 8_000,
                    base_borrow_rate_bps_day: 100,
                    max_variable_borrow_bps_day: 900,
                    lp_revenue_share_bps: 7_000,
                    risk_keeper_revenue_share_bps: 1_000,
                    hard_cap_factor_limit_bps: 10_000,
                    max_adl_reward: 100 * UNIT,
                    max_insolvent_touch_reward: 100 * UNIT,
                    max_active_markets: 8,
                },
            ),
        );

        let vault_id = env.register(
            vault::WASM,
            (
                &token_id,
                &config_manager_id,
                &position_manager_id,
                vault::LpConfig {
                    max_withdraw_utilization_bps: 8_000,
                    min_deposit_nav_factor_bps: 5_000,
                    lp_request_delay: REQUEST_DELAY,
                },
            ),
        );

        let position_manager = position_manager::Client::new(&env, &position_manager_id);
        position_manager.set_vault(&admin, &vault_id);
        position_manager.set_market_config(&admin, &market, &Self::market_config(100));
        oracle_router.set_position_manager(&admin, &position_manager_id);

        let request_router_id = env.register(
            request_router::WASM,
            (&token_id, &vault_id, &oracle_router_id, &config_manager_id),
        );
        vault::Client::new(&env, &vault_id).set_request_router(&admin, &request_router_id);

        for account in [&lp, &trader_a, &trader_b] {
            token.admin_mint(&admin, account, &(1_000_000 * UNIT));
        }

        let protocol = Self {
            env,
            admin,
            keeper,
            publisher,
            lp,
            trader_a,
            trader_b,
            token_id,
            oracle_ids,
            oracle_router_id,
            position_manager_id,
            vault_id,
            request_router_id,
            market,
        };
        protocol.set_price(100 * UNIT);
        protocol
    }

    fn market_config(max_funding_rate_bps_day: i128) -> position_manager::MarketConfig {
        position_manager::MarketConfig {
            open_fee_low_bps: 10,
            open_fee_high_bps: 30,
            max_funding_rate_bps_day,
            market_risk_factor_bps: 5_000,
            max_long_size_open_interest: 1_000_000 * UNIT,
            max_short_size_open_interest: 1_000_000 * UNIT,
            max_long_base_exposure: 1_000_000 * UNIT,
            max_short_base_exposure: 1_000_000 * UNIT,
            recovery_pnl_factor_bps: 2_000,
            warning_pnl_factor_bps: 3_000,
            adl_pnl_factor_bps: 4_000,
            hard_cap_pnl_factor_bps: 5_000,
            maintenance_margin_bps: 500,
            liquidation_reward_bps: 100,
            adl_reward_bps: 50,
        }
    }

    fn advance(&self, seconds: u64) {
        self.env.ledger().with_mut(|ledger| {
            ledger.timestamp += seconds;
            ledger.sequence_number += 1;
        });
    }

    fn set_price(&self, price: i128) {
        for id in &self.oracle_ids {
            oracle::Client::new(&self.env, id).set_price(&self.publisher, &self.market, &price);
        }
    }

    fn publish_round(&self) -> u64 {
        oracle_router::Client::new(&self.env, &self.oracle_router_id).publish_round(&self.keeper)
    }

    fn deposit(&self, owner: &Address, assets: i128) -> i128 {
        let router = request_router::Client::new(&self.env, &self.request_router_id);
        let vault = vault::Client::new(&self.env, &self.vault_id);
        router.request_deposit(owner, &assets);
        self.advance(REQUEST_DELAY);
        self.set_price(100 * UNIT);
        self.publish_round();
        let result = router.resolve_next(&self.keeper);
        assert_eq!(result.status, request_router::SettlementStatus::Settled);
        assert!(result.amount > 0);
        assert_eq!(vault.balance(owner), result.amount);
        result.amount
    }

    fn seed_lp(&self) -> i128 {
        self.deposit(&self.lp, 100_000 * UNIT)
    }

    fn open(&self, owner: &Address, is_long: bool, size: i128, collateral: i128) -> u64 {
        position_manager::Client::new(&self.env, &self.position_manager_id).open_position(
            owner,
            &self.market,
            &is_long,
            &size,
            &collateral,
            &0,
            &0,
            &0,
            &(100 * UNIT),
        )
    }

    fn latest_round_for_vault(&self) -> vault::OracleRound {
        let router = oracle_router::Client::new(&self.env, &self.oracle_router_id);
        let source = router.get_round(&router.latest_round_id());
        let mut prices = Vec::new(&self.env);
        for item in source.prices.iter() {
            prices.push_back(vault::RoundPrice {
                symbol: item.symbol,
                price: item.price,
            });
        }
        vault::OracleRound {
            id: source.id,
            timestamp: source.timestamp,
            previous_id: source.previous_id,
            previous_timestamp: source.previous_timestamp,
            prices,
        }
    }

    fn snapshot(&self) -> vault::AccountingSnapshot {
        vault::Client::new(&self.env, &self.vault_id)
            .accounting_snapshot(&self.latest_round_for_vault())
    }
}

#[test]
fn physical_cash_is_the_source_of_truth_and_donations_belong_to_lps() {
    let p = Protocol::new();
    p.seed_lp();

    let before = p.snapshot();
    assert_eq!(before.physical_cash, 100_000 * UNIT);
    assert_eq!(before.non_lp_claims, 0);
    assert_eq!(before.cash_lp_equity, before.physical_cash);
    assert_eq!(before.vault_nav, before.cash_lp_equity);

    let donation = 17 * UNIT;
    let token = soroban_sdk::token::Client::new(&p.env, &p.token_id);
    token.transfer(&p.trader_a, &p.vault_id, &donation);

    let after = p.snapshot();
    assert_eq!(after.physical_cash - before.physical_cash, donation);
    assert_eq!(after.non_lp_claims, before.non_lp_claims);
    assert_eq!(after.cash_lp_equity - before.cash_lp_equity, donation);
    assert_eq!(after.vault_nav - before.vault_nav, donation);
}

#[test]
fn opening_fee_uses_high_tier_for_worse_skew_and_low_tier_for_better_skew() {
    let p = Protocol::new();
    p.seed_lp();
    let manager = position_manager::Client::new(&p.env, &p.position_manager_id);

    let before = p.snapshot();
    let long_id = p.open(&p.trader_a, true, 10_000 * UNIT, 2_000 * UNIT);
    let long = manager.get_position(&long_id);
    let high_fee = 30 * UNIT;
    assert_eq!(long.stored_collateral, 2_000 * UNIT - high_fee);

    let after_long = p.snapshot();
    assert_eq!(
        after_long.cash_lp_equity - before.cash_lp_equity,
        high_fee * 7_000 / 10_000
    );

    let short_id = p.open(&p.trader_b, false, 5_000 * UNIT, 1_000 * UNIT);
    let short = manager.get_position(&short_id);
    let low_fee = 5 * UNIT;
    assert_eq!(short.stored_collateral, 1_000 * UNIT - low_fee);

    let market = manager.get_market(&p.market);
    assert_eq!(market.long.size_open_interest, 10_000 * UNIT);
    assert_eq!(market.short.size_open_interest, 5_000 * UNIT);
    assert_eq!(
        market.long.stored_collateral_total,
        manager.get_position(&long_id).stored_collateral
    );
    assert_eq!(
        market.short.stored_collateral_total,
        short.stored_collateral
    );
}

#[test]
fn funding_is_split_by_counter_exposure_and_same_time_checkpoint_is_idempotent() {
    let p = Protocol::new();
    p.seed_lp();
    p.open(&p.trader_a, true, 10_000 * UNIT, 3_000 * UNIT);
    p.open(&p.trader_b, false, 2_500 * UNIT, 1_000 * UNIT);

    let manager = position_manager::Client::new(&p.env, &p.position_manager_id);
    let flow = manager.get_market(&p.market);
    assert_eq!(flow.current_payer_side, abi::PayerSide::Long);
    assert!(flow.current_payer_rate > 0);
    assert!(flow.receiver_flow_per_second > 0);
    assert!(flow.lp_flow_per_second > 0);

    let complete_flow = flow.receiver_flow_per_second + flow.lp_flow_per_second;
    assert!(
        (flow.receiver_flow_per_second * 4 - complete_flow).abs() <= 2,
        "the 1:4 receiver allocation may differ only by carried integer dust"
    );

    p.advance(DAY);
    manager.update_indices(&p.keeper, &p.market);
    let accrued = manager.get_market(&p.market);
    let receiver_claim = manager.pending_receiver_funding_total();
    assert!(accrued.receiver_backed_index_long > 0);
    assert!(accrued.lp_backed_index_long > 0);
    assert!(accrued.receiver_index_short > 0);
    assert!(receiver_claim > 0);

    manager.update_indices(&p.keeper, &p.market);
    let repeated = manager.get_market(&p.market);
    assert_eq!(
        repeated.receiver_backed_index_long,
        accrued.receiver_backed_index_long
    );
    assert_eq!(repeated.lp_backed_index_long, accrued.lp_backed_index_long);
    assert_eq!(repeated.receiver_index_short, accrued.receiver_index_short);
    assert_eq!(manager.pending_receiver_funding_total(), receiver_claim);
}

#[test]
fn funding_parameter_change_applies_old_rate_before_new_rate() {
    let p = Protocol::new();
    p.seed_lp();
    p.open(&p.trader_a, true, 10_000 * UNIT, 3_000 * UNIT);
    let manager = position_manager::Client::new(&p.env, &p.position_manager_id);

    p.advance(DAY / 2);
    let before = manager.get_market(&p.market);
    manager.set_market_config(&p.admin, &p.market, &Protocol::market_config(200));
    let at_change = manager.get_market(&p.market);
    let first_delta = at_change.lp_backed_index_long - before.lp_backed_index_long;
    assert!(first_delta > 0);

    p.advance(DAY / 2);
    manager.update_indices(&p.keeper, &p.market);
    let after = manager.get_market(&p.market);
    let second_delta = after.lp_backed_index_long - at_change.lp_backed_index_long;

    assert!(
        (second_delta - first_delta * 2).abs() <= 2,
        "doubling the rate must double future accrual, within index rounding dust"
    );
}

#[test]
fn marked_nav_includes_unrealized_trader_profit_from_the_canonical_round() {
    let p = Protocol::new();
    p.seed_lp();
    p.open(&p.trader_a, true, 10_000 * UNIT, 3_000 * UNIT);
    let at_entry = p.snapshot();

    p.advance(31);
    p.set_price(200 * UNIT);
    p.publish_round();
    let doubled = p.snapshot();

    assert_eq!(doubled.cash_lp_equity, at_entry.cash_lp_equity);
    assert_eq!(at_entry.vault_nav - doubled.vault_nav, 10_000 * UNIT);
}

#[test]
fn risk_capacity_gate_rejects_new_risk_without_mutating_aggregates() {
    let p = Protocol::new();
    p.deposit(&p.lp, 1_000 * UNIT);
    let manager = position_manager::Client::new(&p.env, &p.position_manager_id);
    let before = manager.get_market(&p.market);

    let attempt = manager.try_open_position(
        &p.trader_a,
        &p.market,
        &true,
        &(2_000 * UNIT),
        &(1_000 * UNIT),
        &0,
        &0,
        &0,
        &(100 * UNIT),
    );
    assert!(attempt.is_err());

    let after = manager.get_market(&p.market);
    assert_eq!(
        after.long.size_open_interest,
        before.long.size_open_interest
    );
    assert_eq!(after.long.base_exposure, before.long.base_exposure);
    assert_eq!(after.long.risk_units, before.long.risk_units);
}

#[test]
fn missed_assigned_round_expires_and_refunds_the_complete_lp_escrow() {
    let p = Protocol::new();
    p.seed_lp();
    let token = soroban_sdk::token::Client::new(&p.env, &p.token_id);
    let router = request_router::Client::new(&p.env, &p.request_router_id);

    let assets = 1_000 * UNIT;
    let balance_before = token.balance(&p.trader_a);
    let request_id = router.request_deposit(&p.trader_a, &assets);
    assert_eq!(token.balance(&p.trader_a), balance_before - assets);

    p.advance(REQUEST_DELAY);
    p.set_price(100 * UNIT);
    p.publish_round();
    p.advance(1);
    p.set_price(100 * UNIT);
    p.publish_round();

    let result = router.resolve_next(&p.keeper);
    assert_eq!(result.status, request_router::SettlementStatus::Failed);
    assert_eq!(result.amount, 0);
    assert_eq!(
        router.get_request(&request_id).status,
        request_router::LpRequestStatus::Expired
    );
    assert_eq!(token.balance(&p.trader_a), balance_before);
}

#[test]
fn failed_final_withdrawal_is_all_or_zero_and_returns_every_share() {
    let p = Protocol::new();
    let shares = p.seed_lp();
    p.open(&p.trader_a, true, 10_000 * UNIT, 3_000 * UNIT);

    let router = request_router::Client::new(&p.env, &p.request_router_id);
    let vault = vault::Client::new(&p.env, &p.vault_id);
    let cash_before = vault.physical_cash();
    let supply_before = vault.total_share_supply();
    assert_eq!(vault.balance(&p.lp), shares);

    let request_id = router.request_withdrawal(&p.lp, &shares);
    assert_eq!(vault.balance(&p.lp), 0);
    p.advance(REQUEST_DELAY);
    p.set_price(100 * UNIT);
    p.publish_round();

    let result = router.resolve_next(&p.keeper);
    assert_eq!(result.status, request_router::SettlementStatus::Failed);
    assert_eq!(result.amount, 0);
    assert_eq!(
        router.get_request(&request_id).status,
        request_router::LpRequestStatus::Failed
    );
    assert_eq!(vault.balance(&p.lp), shares);
    assert_eq!(vault.total_share_supply(), supply_before);
    assert_eq!(vault.physical_cash(), cash_before);
}

#[test]
fn partial_then_final_close_telescopes_all_market_aggregates_to_zero() {
    let p = Protocol::new();
    p.seed_lp();
    let manager = position_manager::Client::new(&p.env, &p.position_manager_id);
    let position_id = p.open(&p.trader_a, true, 10_001 * UNIT, 3_000 * UNIT);

    manager.decrease_position(&position_id, &(3_333 * UNIT), &0, &(100 * UNIT));
    let remaining = manager.get_position(&position_id);
    let side = manager.get_market(&p.market).long;
    assert_eq!(side.size_open_interest, remaining.size);
    assert_eq!(side.base_exposure, remaining.base_exposure);
    assert_eq!(side.risk_units, remaining.risk_units);
    assert_eq!(side.stored_collateral_total, remaining.stored_collateral);

    manager.decrease_position(&position_id, &remaining.size, &0, &(100 * UNIT));
    let terminal = manager.get_market(&p.market).long;
    assert_eq!(terminal.size_open_interest, 0);
    assert_eq!(terminal.base_exposure, 0);
    assert_eq!(terminal.risk_units, 0);
    assert_eq!(terminal.stored_collateral_total, 0);
}
