//! Clean-room black-box tests for the doc-mandated constraints that the
//! original `fee_vault_black_box.rs` suite does not cover.
//!
//! Every expectation in this file derives ONLY from:
//! - `docs/design/2026-07-fee-mechanics-theory-ste100.md` (theory; §14 lists
//!   the 28 required properties, referenced here as P1..P28)
//! - `docs/design/2026-07-fee-vault-contract-mechanics-ste100.md` (contract
//!   mechanics; §6 cash-transition table C-rows, §16 rounding table R-rows,
//!   §18 invariants I18.x)
//! - the public interface traits + scale constants in `contracts/shared/src`
//!   (authoritative scales: PRICE_PRECISION = 1e7, INDEX_PRECISION = 1e14,
//!   BPS = 1e4, vault share decimals offset = 6 so VIRTUAL_SHARES = 1e6 and
//!   VIRTUAL_ASSETS = 1)
//!
//! The harness and wire-format ABI below are copied from
//! `fee_vault_black_box.rs` (the wire-format source of truth) and extended
//! with additional public entry points from the shared interface traits.

use soroban_sdk::{
    contractclient, contracttype, symbol_short, testutils::Address as _, testutils::Ledger as _,
    vec, Address, Env, String, Symbol, Vec,
};
use std::cell::Cell;

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
        pub close_fee_low_bps: u32,
        pub close_fee_high_bps: u32,
        pub max_funding_rate_bps_day: i128,
        pub instant_weight_bps: u32,
        pub market_risk_factor_bps: u32,
        pub max_long_size_open_interest: i128,
        pub max_short_size_open_interest: i128,
        pub max_long_base_exposure: i128,
        pub max_short_base_exposure: i128,
        pub recovery_pnl_factor_bps: u32,
        pub warning_pnl_factor_bps: u32,
        pub adl_pnl_factor_bps: u32,
        pub hard_cap_pnl_factor_bps: u32,
        pub initial_margin_bps: u32,
        pub maintenance_margin_bps: u32,
        pub liquidation_reward_bps: u32,
        pub adl_reward_bps: u32,
    }

    #[contracttype]
    #[derive(Clone, Debug)]
    pub struct GlobalConfig {
        pub min_collateral: i128,
        pub min_position_lifetime: u64,
        pub funding_half_life_seconds: u64,
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
        pub skew_ema: i128,
        pub last_funding_checkpoint: u64,
        pub receiver_payer_remainder: i128,
        pub lp_payer_remainder: i128,
        pub receiver_index_remainder: i128,
        pub pending_remainder: i128,
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
        fn liquidate_position(env: Env, caller: Address, position_id: u64);
        fn deleverage_position(env: Env, caller: Address, position_id: u64);
        fn fund_execution_budget(env: Env, position_id: u64, amount: i128);
        fn withdraw_execution_budget(env: Env, position_id: u64, amount: i128);
        fn update_indices(env: Env, caller: Address, market: Symbol);
        fn set_global_config(env: Env, caller: Address, config: GlobalConfig);
        fn set_market_config(env: Env, caller: Address, market: Symbol, config: MarketConfig);
        fn get_position(env: Env, position_id: u64) -> Position;
        fn get_market(env: Env, market: Symbol) -> Market;
        fn pending_receiver_funding_total(env: Env) -> i128;
        fn protocol_claimable_total(env: Env) -> i128;
        fn risk_keeper_reserve_total(env: Env) -> i128;
        fn non_lp_claims(env: Env) -> i128;
        fn claim_protocol(env: Env, caller: Address, recipient: Address, amount: i128);
        fn recapitalize(env: Env, contributor: Address, amount: i128);
    }

    #[contractclient(name = "RequestRouterClient")]
    pub trait RequestRouter {
        fn request_deposit(env: Env, owner: Address, assets: i128) -> u64;
        fn request_withdrawal(env: Env, owner: Address, shares: i128) -> u64;
        fn resolve_next(env: Env, executor: Address) -> SettlementResult;
        fn get_request(env: Env, request_id: u64) -> LpRequest;
        fn next_request_to_resolve(env: Env) -> u64;
    }

    #[contractclient(name = "VaultClient")]
    pub trait Vault {
        fn set_request_router(env: Env, caller: Address, request_router: Address);
        fn set_lp_config(env: Env, caller: Address, config: LpConfig);
        fn can_create_lp_request(env: Env) -> bool;
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

/// One collateral unit / PRICE_PRECISION (token has 7 decimals; prices,
/// notionals, and cash share the 1e7 scale on this deployment).
const UNIT: i128 = 10_000_000;
/// Authoritative implemented index/rate scale (constants.rs mapping table).
const INDEX_PRECISION: i128 = 100_000_000_000_000;
const BPS: i128 = 10_000;
/// Vault share decimals offset of 6: VIRTUAL_SHARES = 1e6, VIRTUAL_ASSETS = 1.
const VIRTUAL_SHARES: i128 = 1_000_000;
const VIRTUAL_ASSETS: i128 = 1;
const START_TIME: u64 = 1_000_000;
const REQUEST_DELAY: u64 = 60;
const DAY: u64 = 86_400;
const PRICE_100: i128 = 100 * UNIT;

fn ceil_div(n: i128, d: i128) -> i128 {
    assert!(n >= 0 && d > 0);
    (n + d - 1) / d
}

fn floor_div(n: i128, d: i128) -> i128 {
    assert!(n >= 0 && d > 0);
    n / d
}

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
    /// Last price pushed to the sources; helpers republish it so LP
    /// settlement rounds always match the current trading price.
    price: Cell<i128>,
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
                Self::global_config(100, 900),
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
            price: Cell::new(PRICE_100),
        };
        protocol.set_price(PRICE_100);
        protocol
    }

    fn global_config(
        base_borrow_rate_bps_day: i128,
        max_variable_borrow_bps_day: i128,
    ) -> position_manager::GlobalConfig {
        position_manager::GlobalConfig {
            min_collateral: 10 * UNIT,
            min_position_lifetime: 0,
            funding_half_life_seconds: 43_200,
            risk_capacity_limit_bps: 8_000,
            base_borrow_rate_bps_day,
            max_variable_borrow_bps_day,
            lp_revenue_share_bps: 7_000,
            risk_keeper_revenue_share_bps: 1_000,
            hard_cap_factor_limit_bps: 10_000,
            max_adl_reward: 100 * UNIT,
            max_insolvent_touch_reward: 100 * UNIT,
            max_active_markets: 8,
        }
    }

    fn market_config(max_funding_rate_bps_day: i128) -> position_manager::MarketConfig {
        position_manager::MarketConfig {
            close_fee_low_bps: 10,
            close_fee_high_bps: 30,
            max_funding_rate_bps_day,
            // Pure instant skew: the EMA-specific tests override this.
            instant_weight_bps: 10_000,
            market_risk_factor_bps: 5_000,
            max_long_size_open_interest: 1_000_000 * UNIT,
            max_short_size_open_interest: 1_000_000 * UNIT,
            max_long_base_exposure: 1_000_000 * UNIT,
            max_short_base_exposure: 1_000_000 * UNIT,
            recovery_pnl_factor_bps: 2_000,
            warning_pnl_factor_bps: 3_000,
            adl_pnl_factor_bps: 4_000,
            hard_cap_pnl_factor_bps: 5_000,
            initial_margin_bps: 500,
            maintenance_margin_bps: 250,
            liquidation_reward_bps: 100,
            adl_reward_bps: 50,
        }
    }

    /// Zero the borrow rates so funding/PnL tests observe funding alone.
    fn disable_borrow(&self) {
        self.manager()
            .set_global_config(&self.admin, &Self::global_config(0, 0));
    }

    /// Zero the funding rate so borrow/PnL tests observe borrow alone.
    fn disable_funding(&self) {
        self.manager()
            .set_market_config(&self.admin, &self.market, &Self::market_config(0));
    }

    /// Zero funding and the closing fee so PnL-precision tests observe the
    /// price math alone.
    fn disable_funding_and_close_fee(&self) {
        let mut config = Self::market_config(0);
        config.close_fee_low_bps = 0;
        config.close_fee_high_bps = 0;
        self.manager()
            .set_market_config(&self.admin, &self.market, &config);
    }

    fn manager(&self) -> position_manager::Client<'_> {
        position_manager::Client::new(&self.env, &self.position_manager_id)
    }

    fn vault(&self) -> vault::Client<'_> {
        vault::Client::new(&self.env, &self.vault_id)
    }

    fn router(&self) -> request_router::Client<'_> {
        request_router::Client::new(&self.env, &self.request_router_id)
    }

    fn token(&self) -> soroban_sdk::token::Client<'_> {
        soroban_sdk::token::Client::new(&self.env, &self.token_id)
    }

    fn advance(&self, seconds: u64) {
        self.env.ledger().with_mut(|ledger| {
            ledger.timestamp += seconds;
            ledger.sequence_number += 1;
        });
    }

    fn set_price(&self, price: i128) {
        self.price.set(price);
        for id in &self.oracle_ids {
            oracle::Client::new(&self.env, id).set_price(&self.publisher, &self.market, &price);
        }
    }

    /// Refresh the current price at the sources (staleness) without changing it.
    fn refresh_price(&self) {
        self.set_price(self.price.get());
    }

    fn publish_round(&self) -> u64 {
        oracle_router::Client::new(&self.env, &self.oracle_router_id).publish_round(&self.keeper)
    }

    /// Delayed LP deposit resolved at the first eligible round (current price).
    fn deposit(&self, owner: &Address, assets: i128) -> i128 {
        let router = self.router();
        router.request_deposit(owner, &assets);
        self.advance(REQUEST_DELAY);
        self.refresh_price();
        self.publish_round();
        let result = router.resolve_next(&self.keeper);
        assert_eq!(result.status, request_router::SettlementStatus::Settled);
        assert!(result.amount > 0);
        result.amount
    }

    fn seed_lp(&self) -> i128 {
        self.deposit(&self.lp, 100_000 * UNIT)
    }

    fn open(&self, owner: &Address, is_long: bool, size: i128, collateral: i128) -> u64 {
        self.open_with_budget(owner, is_long, size, collateral, 0)
    }

    fn open_with_budget(
        &self,
        owner: &Address,
        is_long: bool,
        size: i128,
        collateral: i128,
        budget: i128,
    ) -> u64 {
        self.manager().open_position(
            owner,
            &self.market,
            &is_long,
            &size,
            &collateral,
            &budget,
            &0,
            &0,
            &0,
        )
    }

    fn close(&self, position_id: u64) {
        let size = self.manager().get_position(&position_id).size;
        self.manager()
            .decrease_position(&position_id, &size, &0, &0);
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
        self.vault()
            .accounting_snapshot(&self.latest_round_for_vault())
    }

    /// Publish a fresh round at the current price, then snapshot.
    fn fresh_snapshot(&self) -> vault::AccountingSnapshot {
        self.refresh_price();
        self.publish_round();
        self.snapshot()
    }
}

// ---------------------------------------------------------------------------
// Funding (theory §5, mechanics §8)
// ---------------------------------------------------------------------------

/// P1: balanced base exposure produces zero funding (theory §5.1: at equal
/// base exposure no side pays; mechanics §8.1: at balance all flows are zero).
#[test]
fn p01_balanced_base_exposure_produces_zero_funding() {
    let p = Protocol::new();
    p.seed_lp();
    p.open(&p.trader_a, true, 10_000 * UNIT, 3_000 * UNIT);
    p.open(&p.trader_b, false, 10_000 * UNIT, 3_000 * UNIT);

    let manager = p.manager();
    let market = manager.get_market(&p.market);
    assert_eq!(market.current_payer_side, abi::PayerSide::None);
    assert_eq!(market.current_payer_rate, 0);
    assert_eq!(market.skew_ema, 0);

    p.advance(DAY);
    p.refresh_price();
    manager.update_indices(&p.keeper, &p.market);
    let after = manager.get_market(&p.market);
    assert_eq!(after.receiver_backed_index_long, 0);
    assert_eq!(after.receiver_backed_index_short, 0);
    assert_eq!(after.lp_backed_index_long, 0);
    assert_eq!(after.lp_backed_index_short, 0);
    assert_eq!(after.receiver_index_long, 0);
    assert_eq!(after.receiver_index_short, 0);
    assert_eq!(manager.pending_receiver_funding_total(), 0);
}

/// P2: funding increases quadratically with normalized skew (theory §5.2:
/// payer_rate_bps_day = max_funding_rate_bps_day × skew_bps² / BPS²). Rates
/// are stored at INDEX_PRECISION scale (constants.rs), so the expected stored
/// rate is max_funding × skew² × INDEX_PRECISION / BPS². Test vectors chosen
/// so every division is exact.
#[test]
fn p02_funding_rate_is_quadratic_in_skew() {
    let p = Protocol::new();
    p.seed_lp();
    let manager = p.manager();

    // long_base = 1e9, short_base = 2.5e8 -> skew = 6_000 bps exactly.
    p.open(&p.trader_a, true, 10_000 * UNIT, 3_000 * UNIT);
    p.open(&p.trader_b, false, 2_500 * UNIT, 1_000 * UNIT);
    let market = manager.get_market(&p.market);
    assert_eq!(market.current_payer_side, abi::PayerSide::Long);
    let expected_6000 = 100 * 6_000 * 6_000 * INDEX_PRECISION / (BPS * BPS);
    assert_eq!(
        market.current_payer_rate, expected_6000,
        "rate at skew 6000"
    );

    // Add short 3_500: short_base = 6e8 -> skew = 4e8×BPS/1.6e9 = 2_500 bps.
    p.open(&p.trader_b, false, 3_500 * UNIT, 1_000 * UNIT);
    let market = manager.get_market(&p.market);
    assert_eq!(market.current_payer_side, abi::PayerSide::Long);
    let expected_2500 = 100 * 2_500 * 2_500 * INDEX_PRECISION / (BPS * BPS);
    assert_eq!(
        market.current_payer_rate, expected_2500,
        "rate at skew 2500"
    );

    // Quadratic ratio check: (6000/2500)² = 5.76.
    assert_eq!(expected_6000 * 2_500 * 2_500, expected_2500 * 6_000 * 6_000);
}

/// P4: LPs receive the unmatched collected funding. With a zero light side
/// the receiver flow is zero (mechanics §8.1) and every collected payer
/// stroop becomes LP cash equity at collection (theory §5.4).
#[test]
fn p04_lps_receive_all_funding_when_light_side_is_zero() {
    let p = Protocol::new();
    p.disable_borrow();
    p.seed_lp();
    let manager = p.manager();

    let id = p.open(&p.trader_a, true, 10_000 * UNIT, 3_000 * UNIT);

    p.advance(DAY);
    p.refresh_price();
    manager.update_indices(&p.keeper, &p.market);
    let market = manager.get_market(&p.market);
    assert_eq!(
        market.receiver_backed_index_long, 0,
        "no light side, no receiver-backed accrual"
    );
    assert!(
        market.lp_backed_index_long > 0,
        "all payer accrual is LP-backed"
    );
    assert_eq!(
        manager.pending_receiver_funding_total(),
        0,
        "no receiver claim may accrue without a light side"
    );

    let before = p.fresh_snapshot();
    p.close(id);
    let after = p.snapshot();

    // Collected LP-backed funding = ceil(size × lp_backed_index / INDEX)
    // (§11.2 with a zero debt baseline). Zero PnL at an unchanged price and
    // zero borrow, so this is the only equity change at close.
    let market = manager.get_market(&p.market);
    let collected = ceil_div(10_000 * UNIT * market.lp_backed_index_long, INDEX_PRECISION);
    assert!(collected > 0);
    assert_eq!(after.cash_lp_equity - before.cash_lp_equity, collected);
}

/// P5: a dust light-side position cannot redirect all funding from LPs
/// (theory §5.3: receiver share = light_base / dominant_base; LPs keep the
/// unmatched remainder).
#[test]
fn p05_dust_light_side_cannot_redirect_all_funding() {
    let p = Protocol::new();
    p.seed_lp();
    let manager = p.manager();

    p.open(&p.trader_a, true, 100_000 * UNIT, 30_000 * UNIT);
    p.open(&p.trader_b, false, 20 * UNIT, 15 * UNIT);

    p.advance(DAY);
    p.refresh_price();
    manager.update_indices(&p.keeper, &p.market);
    let market = manager.get_market(&p.market);
    let receiver = market.receiver_backed_index_long;
    let lp = market.lp_backed_index_long;
    let dominant_base = market.long.base_exposure;
    let light_base = market.short.base_exposure;
    assert!(receiver >= 0);
    assert!(lp > 0, "LPs must keep the unmatched funding");
    // receiver accrual = payer accrual × light_base / dominant_base (rounded
    // toward zero), so receiver × dominant <= (receiver + lp) × light.
    assert!(receiver * dominant_base <= (receiver + lp) * light_base);
    // The dust side offsets 0.02% of the dominant exposure; LPs must receive
    // the overwhelming share.
    assert!(lp > receiver * 1_000);
}

/// P6 + P28 + I18.3: receiver credit never exceeds the receiver-backed payer
/// accrual, and the position credit never exceeds the aggregate liability
/// (§8.2/§8.3: round payer obligations up, receiver credits down).
#[test]
fn p06_p28_receiver_credit_bounded_by_receiver_backed_accrual() {
    let p = Protocol::new();
    p.disable_borrow();
    p.seed_lp();
    let manager = p.manager();

    p.open(&p.trader_a, true, 10_000 * UNIT, 3_000 * UNIT);
    let short_collateral = 1_000 * UNIT;
    let short_id = p.open(&p.trader_b, false, 2_500 * UNIT, short_collateral);

    p.advance(99_999); // odd interval to exercise remainders
    p.refresh_price();
    manager.update_indices(&p.keeper, &p.market);

    let market = manager.get_market(&p.market);
    let liability = manager.pending_receiver_funding_total();
    let credit = floor_div(2_500 * UNIT * market.receiver_index_short, INDEX_PRECISION);
    let payer_accrual = ceil_div(
        10_000 * UNIT * market.receiver_backed_index_long,
        INDEX_PRECISION,
    );
    assert!(credit > 0, "test vector must accrue a real credit");
    assert!(
        credit <= payer_accrual,
        "P6/P28: credit above payer accrual"
    );
    assert!(
        credit <= liability,
        "I18.3: credit above aggregate liability"
    );

    // Receiver settlement before payer settlement (§19 test condition): the
    // credit is guaranteed (P7) and is paid even though the payer has not
    // settled. Payout = collateral + credit (zero PnL so no closing fee,
    // zero borrow, light side pays no funding).
    let balance_before = p.token().balance(&p.trader_b);
    p.close(short_id);
    let payout = p.token().balance(&p.trader_b) - balance_before;
    assert_eq!(payout, short_collateral + credit);
    assert_eq!(
        liability - manager.pending_receiver_funding_total(),
        credit,
        "paying the credit consumes exactly that much of the liability"
    );
}

/// P8 + C3 (§6 row "Receiver funding accrual"): accrual moves no cash,
/// raises the non-LP claim, and lowers LP equity by the same amount; the
/// payer's uncollected obligation never appears as LP cash.
#[test]
fn p08_c03_receiver_accrual_lowers_lp_equity_and_uncollected_payer_is_not_lp_cash() {
    let p = Protocol::new();
    p.disable_borrow();
    p.seed_lp();
    let manager = p.manager();

    p.open(&p.trader_a, true, 10_000 * UNIT, 3_000 * UNIT);
    p.open(&p.trader_b, false, 2_500 * UNIT, 1_000 * UNIT);
    let before = p.snapshot();
    let liability_before = manager.pending_receiver_funding_total();

    p.advance(DAY);
    p.refresh_price();
    manager.update_indices(&p.keeper, &p.market);
    p.publish_round();
    let after = p.snapshot();
    let accrued = manager.pending_receiver_funding_total() - liability_before;
    assert!(accrued > 0);

    assert_eq!(after.physical_cash, before.physical_cash, "no cash moves");
    assert_eq!(after.non_lp_claims - before.non_lp_claims, accrued);
    assert_eq!(
        before.cash_lp_equity - after.cash_lp_equity,
        accrued,
        "equity falls only by the guaranteed receiver claim; the payer's \
         larger uncollected accrual must not be recognized (P8)"
    );
}

/// C5/C6 (§6 rows "Receiver-backed payer collection" and "LP-backed fee
/// collection"): collecting payer funding moves no cash, shrinks the
/// position-collateral claim, and restores/raises LP equity by exactly the
/// collected amount (payer obligations round up at the position boundary,
/// §16).
#[test]
fn c05_c06_payer_fee_collection_restores_lp_equity() {
    let p = Protocol::new();
    p.disable_borrow();
    p.seed_lp();
    let manager = p.manager();

    let long_id = p.open(&p.trader_a, true, 10_000 * UNIT, 3_000 * UNIT);
    p.open(&p.trader_b, false, 2_500 * UNIT, 1_000 * UNIT);

    p.advance(12_347);
    p.refresh_price();
    manager.update_indices(&p.keeper, &p.market);
    let market = manager.get_market(&p.market);
    let size = 10_000 * UNIT;
    let pending_rb = ceil_div(size * market.receiver_backed_index_long, INDEX_PRECISION);
    let pending_lp = ceil_div(size * market.lp_backed_index_long, INDEX_PRECISION);
    assert!(pending_rb > 0 && pending_lp > 0);
    // The ceil boundary must actually be exercised on at least one side.
    assert!(
        (size * market.receiver_backed_index_long) % INDEX_PRECISION != 0
            || (size * market.lp_backed_index_long) % INDEX_PRECISION != 0,
        "test vector must exercise the payer rounding boundary"
    );

    let before = p.fresh_snapshot();
    let collateral_before = manager.get_position(&long_id).stored_collateral;
    // Partial decrease at an unchanged price: zero PnL, zero borrow — the
    // only settlement is the funding collection out of stored collateral.
    manager.decrease_position(&long_id, &(4_000 * UNIT), &0, &0);
    let after = p.snapshot();
    let collateral_after = manager.get_position(&long_id).stored_collateral;

    let collected = pending_rb + pending_lp;
    assert_eq!(after.physical_cash, before.physical_cash, "no cash moves");
    assert_eq!(collateral_before - collateral_after, collected);
    assert_eq!(after.cash_lp_equity - before.cash_lp_equity, collected);
    assert_eq!(before.non_lp_claims - after.non_lp_claims, collected);
}

/// C4 (§6 row "Receiver claim becomes collateral") + I18.3: capitalizing a
/// receiver credit is a pure label change — the liability falls, the
/// position's stored collateral rises by the same amount, and neither total
/// non-LP claims, LP equity, nor physical cash changes. The reset baseline
/// equals the current index value (§18.4, receiver credits round down §16).
#[test]
fn c04_receiver_capitalization_is_label_change_only() {
    let p = Protocol::new();
    p.disable_borrow();
    p.seed_lp();
    let manager = p.manager();

    p.open(&p.trader_a, true, 10_000 * UNIT, 3_000 * UNIT);
    let short_id = p.open(&p.trader_b, false, 2_500 * UNIT, 1_000 * UNIT);

    p.advance(54_323);
    p.refresh_price();
    manager.update_indices(&p.keeper, &p.market);
    let market = manager.get_market(&p.market);
    let credit = floor_div(2_500 * UNIT * market.receiver_index_short, INDEX_PRECISION);
    assert!(credit > 0);
    assert!(
        (2_500 * UNIT * market.receiver_index_short) % INDEX_PRECISION != 0,
        "test vector must exercise the receiver floor boundary"
    );

    let before = p.fresh_snapshot();
    let liability_before = manager.pending_receiver_funding_total();
    let collateral_before = manager.get_position(&short_id).stored_collateral;

    // Partial decrease at an unchanged price: zero PnL, zero borrow, the
    // light side pays no funding — the only settlement is the credit
    // capitalization.
    manager.decrease_position(&short_id, &(1_000 * UNIT), &0, &0);

    let after = p.snapshot();
    let position = manager.get_position(&short_id);
    assert_eq!(position.stored_collateral - collateral_before, credit);
    assert_eq!(
        liability_before - manager.pending_receiver_funding_total(),
        credit
    );
    assert_eq!(
        after.non_lp_claims, before.non_lp_claims,
        "label change only"
    );
    assert_eq!(after.cash_lp_equity, before.cash_lp_equity);
    assert_eq!(after.physical_cash, before.physical_cash);

    // §18.4: the remaining size restarts at the current receiver index.
    let market = manager.get_market(&p.market);
    assert_eq!(
        position.funding_received_debt,
        floor_div(position.size * market.receiver_index_short, INDEX_PRECISION)
    );
}

/// P16 + I18.4: new size starts at the current baseline and pays nothing for
/// time before it existed. After heavy accrual, a brand-new position closed
/// flat in the same ledger costs nothing at all.
#[test]
fn p16_new_size_does_not_pay_for_time_before_it_existed() {
    let p = Protocol::new();
    p.seed_lp();
    let manager = p.manager();

    p.open(&p.trader_a, true, 10_000 * UNIT, 5_000 * UNIT);
    p.advance(10 * DAY);
    p.refresh_price();
    manager.update_indices(&p.keeper, &p.market);
    let market = manager.get_market(&p.market);
    assert!(
        market.lp_backed_index_long > 0,
        "indices must be large already"
    );

    let balance_before = p.token().balance(&p.trader_b);
    let size = 5_000 * UNIT;
    let collateral = 1_000 * UNIT;
    let id = p.open(&p.trader_b, true, size, collateral);

    // §18.4: debt baselines equal the current index values (payer up,
    // receiver down, §11.2).
    let position = manager.get_position(&id);
    let market = manager.get_market(&p.market);
    assert_eq!(
        position.funding_paid_to_lps_debt,
        ceil_div(size * market.lp_backed_index_long, INDEX_PRECISION)
    );
    assert_eq!(
        position.funding_paid_to_receivers_debt,
        ceil_div(size * market.receiver_backed_index_long, INDEX_PRECISION)
    );
    assert_eq!(
        position.funding_received_debt,
        floor_div(size * market.receiver_index_long, INDEX_PRECISION)
    );

    // Same-ledger close: no elapsed time and no price move, so there is no
    // fee at all — the closing fee only comes out of positive price PnL
    // (§11.1), and a flat close realizes at most rounding dust.
    p.close(id);
    assert_eq!(p.token().balance(&p.trader_b), balance_before);
}

/// P17: a partial close capitalizes all old accrual before it resets the
/// baselines — the fee for the full old size is collected, the remaining
/// position restarts at the current indices, and an immediate full close
/// pays no further fees.
#[test]
fn p17_partial_close_capitalizes_old_accrual_before_baseline_reset() {
    let p = Protocol::new();
    p.disable_borrow();
    p.seed_lp();
    let manager = p.manager();

    let old_size = 10_000 * UNIT;
    let id = p.open(&p.trader_a, true, old_size, 3_000 * UNIT);

    p.advance(DAY);
    p.refresh_price();
    manager.update_indices(&p.keeper, &p.market);
    let market = manager.get_market(&p.market);
    // One-sided market: the whole payer flow is LP-backed.
    let pending_full = ceil_div(old_size * market.lp_backed_index_long, INDEX_PRECISION);
    assert!(pending_full > 0);

    let collateral_before = manager.get_position(&id).stored_collateral;
    manager.decrease_position(&id, &(4_000 * UNIT), &0, &0);
    let position = manager.get_position(&id);

    // The complete old-size accrual was capitalized (zero PnL at an
    // unchanged price, zero borrow).
    assert_eq!(collateral_before - position.stored_collateral, pending_full);
    // The remaining size restarts at the current index (§11.5/§18.4).
    assert_eq!(
        position.funding_paid_to_lps_debt,
        ceil_div(position.size * market.lp_backed_index_long, INDEX_PRECISION)
    );

    // Immediate full close in the same ledger: no time has passed, so the
    // payout is exactly the remaining stored collateral — nothing is charged
    // twice for the closed part and no historical debt is carried.
    let balance_before = p.token().balance(&p.trader_a);
    p.close(id);
    assert_eq!(
        p.token().balance(&p.trader_a) - balance_before,
        position.stored_collateral
    );
}

/// I18.4 (stored remainders make split intervals equivalent) + §16 row
/// "Aggregate receiver liability: down, with a carried remainder" + §3
/// ("The checkpoint frequency must not change accrued value"): many
/// checkpoints over an interval accrue exactly what one checkpoint accrues.
#[test]
fn i18_4_split_checkpoints_equal_single_interval() {
    let run = |split: bool| -> (i128, i128, i128, i128) {
        let p = Protocol::new();
        p.seed_lp();
        let manager = p.manager();
        p.open(&p.trader_a, true, 10_000 * UNIT, 3_000 * UNIT);
        p.open(&p.trader_b, false, 2_500 * UNIT, 1_000 * UNIT);

        // 99_991 s is prime, so per-second flows cannot divide it evenly and
        // every intermediate checkpoint leaves a remainder to carry.
        let intervals: &[u64] = if split {
            &[7, 991, 13_337, 85_656]
        } else {
            &[99_991]
        };
        for dt in intervals {
            p.advance(*dt);
            p.refresh_price();
            manager.update_indices(&p.keeper, &p.market);
        }
        let market = manager.get_market(&p.market);
        (
            market.receiver_backed_index_long,
            market.lp_backed_index_long,
            market.receiver_index_short,
            manager.pending_receiver_funding_total(),
        )
    };

    let single = run(false);
    let split = run(true);
    assert_eq!(single.0, split.0, "receiver-backed payer index");
    assert_eq!(single.1, split.1, "LP-backed payer index");
    assert_eq!(single.2, split.2, "receiver index");
    assert_eq!(single.3, split.3, "aggregate receiver liability");
}

// ---------------------------------------------------------------------------
// Borrow (theory §6, mechanics §9) and risk units
// ---------------------------------------------------------------------------

/// P9 (§7.4): uncollected borrow is not revenue — LP equity is unchanged
/// while borrow only accrues, and rises exactly when a position action
/// collects it (borrow splits like the opening fee, §11.4).
#[test]
fn p09_borrow_becomes_revenue_only_on_collection() {
    let p = Protocol::new();
    p.disable_funding();
    p.seed_lp();
    let manager = p.manager();

    let id = p.open(&p.trader_a, true, 10_000 * UNIT, 3_000 * UNIT);
    // Price-independent fields only, so the seed round is a valid basis.
    let before = p.snapshot();
    let protocol_before = manager.protocol_claimable_total();
    let keeper_before = manager.risk_keeper_reserve_total();

    p.advance(DAY);
    p.refresh_price();
    manager.update_indices(&p.keeper, &p.market);
    p.publish_round();
    let accrued_only = p.snapshot();
    assert_eq!(
        accrued_only.cash_lp_equity, before.cash_lp_equity,
        "P9: accrued-but-uncollected borrow must not be LP cash"
    );
    assert_eq!(accrued_only.non_lp_claims, before.non_lp_claims);

    // Collection: a partial decrease capitalizes the pending borrow out of
    // stored collateral (no PnL at an unchanged price, no funding).
    let collateral_before = manager.get_position(&id).stored_collateral;
    manager.decrease_position(&id, &(4_000 * UNIT), &0, &0);
    let collected = collateral_before - manager.get_position(&id).stored_collateral;
    assert!(collected > 0);

    let after = p.snapshot();
    let lp_share = floor_div(collected * 7_000, BPS);
    let keeper_share = floor_div(collected * 1_000, BPS);
    let protocol_share = collected - lp_share - keeper_share;
    assert_eq!(after.cash_lp_equity - before.cash_lp_equity, lp_share);
    assert_eq!(
        manager.risk_keeper_reserve_total() - keeper_before,
        keeper_share
    );
    assert_eq!(
        manager.protocol_claimable_total() - protocol_before,
        protocol_share
    );
}

/// P10 + R16 "borrow obligation up": the borrow rate is
/// base + max_variable × utilization² / BPS² (per day), accrued on risk
/// units. Verified at two utilizations with exact doc arithmetic; the two
/// points also pin the quadratic term (9× the variable rate at 2× the
/// utilization would fail a linear or cubic curve).
#[test]
fn p10_borrow_rate_is_quadratic_in_utilization() {
    // Returns the borrow collected on a one-day-old position closed at an
    // unchanged price with funding disabled.
    let collected_at = |deposit: i128| -> i128 {
        let p = Protocol::new();
        p.disable_funding();
        p.deposit(&p.lp, deposit);

        let size = 10_000 * UNIT;
        let collateral = 3_000 * UNIT;
        let id = p.open(&p.trader_a, true, size, collateral);

        p.advance(DAY);
        p.refresh_price();
        let balance_before = p.token().balance(&p.trader_a);
        p.close(id);
        let payout = p.token().balance(&p.trader_a) - balance_before;
        collateral - payout
    };

    // risk_units = floor(size × 5_000 / BPS) = 5_000 UNIT.
    let risk = 5_000 * UNIT;
    let expected = |equity: i128| -> i128 {
        let utilization = risk * BPS / equity; // exact by construction
                                               // Stored-rate scale is INDEX_PRECISION (constants.rs).
        let rate =
            100 * INDEX_PRECISION + 900 * INDEX_PRECISION * utilization * utilization / (BPS * BPS);
        // One day: index_delta = rate × dt / (BPS × SECONDS_PER_DAY) = rate/BPS.
        let index_delta = rate / BPS;
        ceil_div(risk * index_delta, INDEX_PRECISION)
    };

    // With no opening fee the post-open equity is the deposit itself.
    // equity 100_000 -> utilization 500 bps; equity 50_000 -> 1_000 bps.
    let low = collected_at(100_000 * UNIT);
    let high = collected_at(50_000 * UNIT);

    // ±1 stroop: the doc allows the ceil-of-baseline vs ceil-of-delta
    // difference on the debt baseline; nothing else may round.
    assert!(
        (low - expected(100_000 * UNIT)).abs() <= 1,
        "utilization 500: collected {low}, doc formula {}",
        expected(100_000 * UNIT)
    );
    assert!(
        (high - expected(50_000 * UNIT)).abs() <= 1,
        "utilization 1000: collected {high}, doc formula {}",
        expected(50_000 * UNIT)
    );
    // 511_250_000 and 545_000_000 stroops: the variable part quadruples when
    // utilization doubles.
    assert_eq!(expected(100_000 * UNIT), 511_250_000);
    assert_eq!(expected(50_000 * UNIT), 545_000_000);
}

/// P11: risk units measure gross risk — floor(size × risk_factor / BPS) per
/// position, summed across both sides, not netted (theory §6.1).
#[test]
fn p11_risk_units_measure_gross_not_net_exposure() {
    let p = Protocol::new();
    p.seed_lp();
    let manager = p.manager();

    let long_id = p.open(&p.trader_a, true, 10_001 * UNIT, 3_000 * UNIT);
    let short_id = p.open(&p.trader_b, false, 10_001 * UNIT, 3_000 * UNIT);

    let expected_each = floor_div(10_001 * UNIT * 5_000, BPS);
    assert_eq!(manager.get_position(&long_id).risk_units, expected_each);
    assert_eq!(manager.get_position(&short_id).risk_units, expected_each);

    // A perfectly balanced market has zero net exposure but full gross risk.
    let snapshot = p.snapshot();
    assert_eq!(snapshot.total_risk_units, 2 * expected_each);
}

/// P12: oracle price changes do not change risk units (theory §6.1,
/// mechanics §15.3).
#[test]
fn p12_price_change_does_not_change_risk_units() {
    let p = Protocol::new();
    p.seed_lp();
    let manager = p.manager();
    let id = p.open(&p.trader_a, true, 10_000 * UNIT, 3_000 * UNIT);

    // Price-independent fields only, so the seed round is a valid basis.
    let before = p.snapshot();
    let position_risk = manager.get_position(&id).risk_units;

    p.advance(31);
    p.set_price(160 * UNIT);
    p.publish_round();
    let after = p.snapshot();

    assert_eq!(after.total_risk_units, before.total_risk_units);
    assert_eq!(manager.get_position(&id).risk_units, position_risk);
    assert_eq!(
        after.required_risk_backing, before.required_risk_backing,
        "risk backing derives from risk units, not from price"
    );
    assert_eq!(manager.get_market(&p.market).long.risk_units, position_risk);
}

/// P14 + I18.5: new risk must pass the market-side size cap and the
/// market-side base-exposure cap, and a rejection mutates nothing.
#[test]
fn p14_market_side_limits_reject_new_risk() {
    let p = Protocol::new();
    p.seed_lp();
    let manager = p.manager();

    // Size cap: 9_999 max, 10_000 attempted.
    let mut config = Protocol::market_config(100);
    config.max_long_size_open_interest = 9_999 * UNIT;
    manager.set_market_config(&p.admin, &p.market, &config);
    let attempt = manager.try_open_position(
        &p.trader_a,
        &p.market,
        &true,
        &(10_000 * UNIT),
        &(3_000 * UNIT),
        &0,
        &0,
        &0,
        &0,
    );
    assert!(attempt.is_err(), "size above the side cap must be rejected");

    // Base-exposure cap: size fits, base does not (base = size / price = 100
    // units of the asset at the 1e7 scale = 1e9).
    let mut config = Protocol::market_config(100);
    config.max_long_base_exposure = 100 * UNIT - 1;
    manager.set_market_config(&p.admin, &p.market, &config);
    let attempt = manager.try_open_position(
        &p.trader_a,
        &p.market,
        &true,
        &(10_000 * UNIT),
        &(3_000 * UNIT),
        &0,
        &0,
        &0,
        &0,
    );
    assert!(attempt.is_err(), "base above the side cap must be rejected");

    let market = manager.get_market(&p.market);
    assert_eq!(
        market.long.size_open_interest, 0,
        "rejection must not mutate"
    );
    assert_eq!(market.long.base_exposure, 0);
    assert_eq!(market.long.risk_units, 0);

    // Boundary: exactly at the base cap the open is accepted.
    let mut config = Protocol::market_config(100);
    config.max_long_base_exposure = 100 * UNIT;
    manager.set_market_config(&p.admin, &p.market, &config);
    p.open(&p.trader_a, true, 10_000 * UNIT, 3_000 * UNIT);
    assert_eq!(manager.get_market(&p.market).long.base_exposure, 100 * UNIT);
}

/// P18 + I18.8: accrued fees enter effective collateral, so a position that
/// was healthy at entry becomes liquidatable through fee accrual alone, with
/// no price movement (§12.3 uses effective collateral for the test).
#[test]
fn p18_accrued_fees_make_position_liquidatable_without_price_move() {
    let p = Protocol::new();
    p.seed_lp();
    let manager = p.manager();

    // size 10_000: initial margin 5% = 500, maintenance 2.5% = 250.
    // Collateral after the 30-UNIT opening fee is 570 — the open clears the
    // initial margin by 70 and sits 320 above the maintenance floor.
    let id = p.open(&p.trader_a, true, 10_000 * UNIT, 600 * UNIT);
    assert!(
        manager.try_liquidate_position(&p.keeper, &id).is_err(),
        "healthy position must not be liquidatable"
    );

    // One-sided market: funding = 100 bps/day on size (100 UNIT/day) plus
    // borrow ~51 UNIT/day — three days eat well through the 320-UNIT buffer.
    p.advance(3 * DAY);
    p.refresh_price();
    manager.liquidate_position(&p.keeper, &id);
    assert!(
        manager.try_get_position(&id).is_err(),
        "position must be gone"
    );

    // I18.2: the side aggregates return to zero with the only position gone.
    let side = manager.get_market(&p.market).long;
    assert_eq!(side.size_open_interest, 0);
    assert_eq!(side.base_exposure, 0);
    assert_eq!(side.risk_units, 0);
    assert_eq!(side.stored_collateral_total, 0);
}

/// P22 + I18.6: recognized trader loss is capped at the side collateral
/// aggregate — NAV rises by at most the stored collateral even when the raw
/// loss is far larger (§7.3).
#[test]
fn p22_recognized_loss_capped_by_side_collateral_aggregate() {
    let p = Protocol::new();
    p.seed_lp();
    let manager = p.manager();
    p.open(&p.trader_a, true, 10_000 * UNIT, 3_000 * UNIT);

    // Equity is price-independent, so the seed round is a valid basis.
    let entry = p.snapshot();
    let side_collateral = manager.get_market(&p.market).long.stored_collateral_total;

    // Crash 100 -> 10: raw loss = 100 units × 90 = 9_000 UNIT, three times
    // the ~2_970-UNIT side collateral.
    p.advance(31);
    p.set_price(10 * UNIT);
    p.publish_round();
    let crashed = p.snapshot();

    assert_eq!(crashed.cash_lp_equity, entry.cash_lp_equity);
    assert_eq!(
        crashed.vault_nav,
        crashed.cash_lp_equity + side_collateral,
        "NAV may recognize the loss only up to the side collateral aggregate"
    );
}

/// P25 + I18.5: a withdrawal cannot consume required risk backing — beyond
/// free capital it fails full-or-nothing; within free capital it settles.
#[test]
fn p25_withdrawal_cannot_consume_required_risk_backing() {
    let p = Protocol::new();
    let shares = p.seed_lp();
    p.open(&p.trader_a, true, 10_000 * UNIT, 3_000 * UNIT);

    // Price-independent fields only, so the seed round is a valid basis.
    let snapshot = p.snapshot();
    // required = ceil(5_000 UNIT × BPS / 8_000) = 6_250 UNIT locked.
    assert_eq!(snapshot.required_risk_backing, 6_250 * UNIT);
    assert_eq!(
        snapshot.free_lp_capital,
        snapshot.cash_lp_equity - snapshot.required_risk_backing
    );

    // 95% of the shares is worth ~95_000 UNIT > free capital (~93_771):
    // the withdrawal must fail completely and return every share.
    let router = p.router();
    let vault = p.vault();
    let too_many = shares * 95 / 100;
    router.request_withdrawal(&p.lp, &too_many);
    p.advance(REQUEST_DELAY);
    p.refresh_price();
    p.publish_round();
    let result = router.resolve_next(&p.keeper);
    assert_eq!(result.status, request_router::SettlementStatus::Failed);
    assert_eq!(vault.balance(&p.lp), shares, "full escrow returned");

    // Half the shares (~50_000 UNIT) is inside free capital: settles.
    p.advance(1);
    let half = shares / 2;
    router.request_withdrawal(&p.lp, &half);
    p.advance(REQUEST_DELAY);
    p.refresh_price();
    p.publish_round();
    let result = router.resolve_next(&p.keeper);
    assert_eq!(result.status, request_router::SettlementStatus::Settled);
    assert!(result.amount > 0);

    let after = p.snapshot();
    assert!(
        after.cash_lp_equity >= after.required_risk_backing,
        "the settled withdrawal must leave the risk backing intact"
    );
}

/// P26 + I18.8 + §14: a warning-state side stops ordinary LP actions — the
/// pending request fails with a full refund, the blocked-side count is
/// nonzero, new LP requests are rejected, and new risk on the affected side
/// is rejected.
#[test]
fn p26_warning_state_stops_lp_actions_and_new_risk() {
    let p = Protocol::new();
    p.seed_lp();
    let manager = p.manager();
    let router = p.router();
    p.open(&p.trader_a, true, 10_000 * UNIT, 3_000 * UNIT);

    // Queue a deposit while the market is still NORMAL.
    let balance_before = p.token().balance(&p.trader_b);
    let request_id = router.request_deposit(&p.trader_b, &(1_000 * UNIT));

    // Long profit 100 units × 350 = 35_000 UNIT on ~100_021 equity: factor
    // ~3_499 bps — inside [warning 3_000, adl 4_000).
    p.advance(REQUEST_DELAY);
    p.set_price(450 * UNIT);
    p.publish_round();

    // §14: the settlement evaluates the risk state, resolves the request as
    // failed, refunds the escrow, and keeps the risk-state update.
    let result = router.resolve_next(&p.keeper);
    assert_eq!(result.status, request_router::SettlementStatus::Failed);
    assert_eq!(
        router.get_request(&request_id).status,
        request_router::LpRequestStatus::Failed
    );
    assert_eq!(p.token().balance(&p.trader_b), balance_before);

    let snapshot = p.snapshot();
    assert_eq!(
        snapshot.lp_blocked_side_count, 1,
        "I18.8 blocked-side count"
    );
    assert!(!p.vault().can_create_lp_request());
    assert!(router
        .try_request_deposit(&p.trader_b, &(1_000 * UNIT))
        .is_err());
    assert!(router.try_request_withdrawal(&p.lp, &1_000_000).is_err());

    // §9.1/§14: no new risk on the affected side at the warning factor.
    assert!(manager
        .try_open_position(
            &p.trader_a,
            &p.market,
            &true,
            &(1_000 * UNIT),
            &(500 * UNIT),
            &0,
            &0,
            &0,
            &0,
        )
        .is_err());
}

// ---------------------------------------------------------------------------
// Rounding (§16) and PnL cash conversion
// ---------------------------------------------------------------------------

/// R16 "Closing fee charged: up": closing_fee = ceil(size_removed × fee_bps
/// / BPS), capped at the realized positive PnL (§11.1). Opening charges
/// nothing: stored collateral equals the amount paid.
#[test]
fn r16_closing_fee_rounds_up() {
    let p = Protocol::new();
    p.disable_borrow();
    p.disable_funding();
    p.seed_lp();
    let manager = p.manager();

    // size × 10 / 10_000 = 3_333_333.3 stroops — must charge 3_333_334.
    // Closing the book's only long empties it: skew 10_000 -> 0, low tier.
    let size = 3_333_333_300;
    assert!(
        size * 10 % BPS != 0,
        "vector must exercise the ceil boundary"
    );
    let fee = ceil_div(size * 10, BPS);
    assert_eq!(fee, 3_333_334);

    let collateral = 500 * UNIT;
    let balance_before = p.token().balance(&p.trader_a);
    let id = p.open(&p.trader_a, true, size, collateral);
    assert_eq!(
        manager.get_position(&id).stored_collateral,
        collateral,
        "no fee is charged at open"
    );

    // 1% up: pnl = 33_333_333 stroops, far above the fee, so the cap does
    // not bind and the round trip costs exactly the ceil'd fee.
    p.advance(31);
    p.set_price(101 * UNIT);
    p.publish_round();
    p.close(id);
    assert_eq!(
        p.token().balance(&p.trader_a),
        balance_before + 33_333_333 - fee
    );
}

/// R16 "Keeper share: down" + "Protocol share: exact remainder" (§11.4):
/// lp = floor(c × 7_000 / BPS), keeper = floor(c × 1_000 / BPS),
/// protocol = c - lp - keeper.
#[test]
fn r16_fee_split_keeper_down_protocol_exact_remainder() {
    let p = Protocol::new();
    p.disable_borrow();
    p.disable_funding();
    p.seed_lp();
    let manager = p.manager();

    // closing fee = ceil(3_333_330_100 × 10 / 10_000) = 3_333_331 stroops,
    // which splits with remainders on every share.
    let size = 3_333_330_100;
    let fee = ceil_div(size * 10, BPS);
    assert_eq!(fee, 3_333_331);
    let lp_share = floor_div(fee * 7_000, BPS);
    let keeper_share = floor_div(fee * 1_000, BPS);
    let protocol_share = fee - lp_share - keeper_share;
    assert!(fee * 7_000 % BPS != 0 && fee * 1_000 % BPS != 0);

    let id = p.open(&p.trader_a, true, size, 500 * UNIT);
    p.advance(31);
    p.set_price(101 * UNIT);
    p.publish_round();
    let payable = 33_333_301;

    let before = p.snapshot();
    let keeper_before = manager.risk_keeper_reserve_total();
    let protocol_before = manager.protocol_claimable_total();
    p.close(id);
    let after = p.snapshot();

    assert_eq!(
        manager.risk_keeper_reserve_total() - keeper_before,
        keeper_share
    );
    assert_eq!(
        manager.protocol_claimable_total() - protocol_before,
        protocol_share
    );
    // The close pays the trader's profit out of LP cash and keeps only the
    // fee's LP share behind.
    assert_eq!(
        after.cash_lp_equity - before.cash_lp_equity,
        lp_share - payable
    );
    // The three shares recompose the collected fee exactly (P27: rounding
    // creates no value).
    assert_eq!(lp_share + keeper_share + protocol_share, fee);
}

/// R16 "LP deposit shares: down" (§13.4): deposit_shares = floor(assets ×
/// (supply + VIRTUAL_SHARES) / (nav + VIRTUAL_ASSETS)), with the share token
/// at a decimals offset of 6.
#[test]
fn r16_lp_deposit_shares_round_down() {
    let p = Protocol::new();
    let first_shares = p.seed_lp();
    // Clean first deposit: floor(assets × (0 + 1e6) / (0 + 1)).
    assert_eq!(first_shares, 100_000 * UNIT * VIRTUAL_SHARES);

    // Make the share price irrational in stroops via an odd donation.
    p.token().transfer(&p.trader_b, &p.vault_id, &1_234_567);

    let vault = p.vault();
    let supply = vault.total_share_supply();
    // No open positions: NAV equals cash LP equity and is price-independent,
    // so the seed round is a valid basis.
    let nav = p.snapshot().vault_nav;
    let assets = 777_777_777;
    let expected = floor_div(assets * (supply + VIRTUAL_SHARES), nav + VIRTUAL_ASSETS);
    assert!(
        assets * (supply + VIRTUAL_SHARES) % (nav + VIRTUAL_ASSETS) != 0,
        "vector must exercise the floor boundary"
    );

    p.advance(1);
    let minted = p.deposit(&p.trader_a, assets);
    assert_eq!(minted, expected);
    assert_eq!(vault.balance(&p.trader_a), expected);
}

/// R16 "LP withdrawal assets: down" (§13.4): withdrawal_assets =
/// floor(shares × (nav + VIRTUAL_ASSETS) / (supply + VIRTUAL_SHARES)).
#[test]
fn r16_lp_withdrawal_assets_round_down() {
    let p = Protocol::new();
    p.seed_lp();
    p.token().transfer(&p.trader_b, &p.vault_id, &7_654_321);

    let vault = p.vault();
    let router = p.router();
    let supply = vault.total_share_supply();
    // No open positions: NAV equals cash LP equity and is price-independent,
    // so the seed round is a valid basis.
    let nav = p.snapshot().vault_nav;
    let shares = 333_333_333_333_333;
    let expected = floor_div(shares * (nav + VIRTUAL_ASSETS), supply + VIRTUAL_SHARES);
    assert!(
        shares * (nav + VIRTUAL_ASSETS) % (supply + VIRTUAL_SHARES) != 0,
        "vector must exercise the floor boundary"
    );

    p.advance(1);
    let balance_before = p.token().balance(&p.lp);
    router.request_withdrawal(&p.lp, &shares);
    p.advance(REQUEST_DELAY);
    p.refresh_price();
    p.publish_round();
    let result = router.resolve_next(&p.keeper);
    assert_eq!(result.status, request_router::SettlementStatus::Settled);
    assert_eq!(result.amount, expected);
    assert_eq!(p.token().balance(&p.lp) - balance_before, expected);
}

/// R16 "Required risk backing: up" + I18.5 (risk units reduce free capital,
/// not equity or NAV): backing = ceil(total_risk_units × BPS /
/// risk_capacity_limit_bps).
#[test]
fn r16_required_risk_backing_rounds_up() {
    let p = Protocol::new();
    p.seed_lp();

    // size = 1e11 + 100 -> risk = 5e10 + 50; ×1.25 = 62_500_000_062.5, so
    // the ceil must land on ...063.
    let size = 100_000_000_100;
    let risk = floor_div(size * 5_000, BPS);
    assert!(
        risk * BPS % 8_000 != 0,
        "vector must exercise the ceil boundary"
    );
    let backing = ceil_div(risk * BPS, 8_000);
    assert_eq!(backing, 62_500_000_063);

    // Price-independent fields only, so the seed round is a valid basis.
    let before = p.snapshot();
    p.open(&p.trader_a, true, size, 3_000 * UNIT);
    let after = p.snapshot();

    assert_eq!(after.total_risk_units, risk);
    assert_eq!(after.required_risk_backing, backing);
    assert_eq!(after.free_lp_capital, after.cash_lp_equity - backing);
    // Risk locks capital but does not reduce ownership: with no opening fee
    // the open moves LP equity not at all.
    assert_eq!(after.cash_lp_equity, before.cash_lp_equity);
}

/// R16 "Post-withdraw utilization: up" (§13.6): a withdrawal whose
/// post-equity puts total_risk_units × BPS / post_equity strictly between
/// 4_000 and 4_001 must fail — only the up-rounding produces 4_001 there.
#[test]
fn r16_post_withdraw_utilization_rounds_up() {
    let p = Protocol::new();
    let shares = p.seed_lp();
    // Bind the utilization gate below the capacity gate (8_000) so the
    // free-capital check cannot mask it.
    p.vault().set_lp_config(
        &p.admin,
        &vault::LpConfig {
            max_withdraw_utilization_bps: 4_000,
            min_deposit_nav_factor_bps: 5_000,
            lp_request_delay: REQUEST_DELAY,
        },
    );
    p.open(&p.trader_a, true, 10_000 * UNIT, 3_000 * UNIT);

    // Equity, NAV (zero PnL at the entry price), risk are all
    // price-independent here, so the seed round is a valid basis.
    let snapshot = p.snapshot();
    let risk = snapshot.total_risk_units;
    assert_eq!(risk, 5_000 * UNIT);
    let equity = snapshot.cash_lp_equity;
    let nav = snapshot.vault_nav;
    assert_eq!(nav, equity, "no PnL at the entry price");
    let supply = p.vault().total_share_supply();

    // Fail window: post_equity in (risk×BPS/4_001, risk×BPS/4_000) =
    // (12_496.875.., 12_500) UNIT — utilization there floors to 4_000 but
    // ceils to 4_001, so only the doc's up-rounding rejects it.
    let target_post = 12_498 * UNIT;
    let withdraw_assets = equity - target_post;
    // Invert the §13.4 conversion, then verify the executable assets land
    // inside the ~3-UNIT-wide fail window.
    let shares_to_burn = withdraw_assets * (supply + VIRTUAL_SHARES) / (nav + VIRTUAL_ASSETS);
    let assets_out = floor_div(
        shares_to_burn * (nav + VIRTUAL_ASSETS),
        supply + VIRTUAL_SHARES,
    );
    let post = equity - assets_out;
    assert!(
        risk * BPS / post == 4_000 && risk * BPS % post != 0,
        "post-equity must sit strictly inside the (4_000, 4_001) ceil window"
    );
    assert!(
        assets_out <= snapshot.free_lp_capital,
        "the free-capital gate must not mask the utilization gate"
    );

    let router = p.router();
    router.request_withdrawal(&p.lp, &shares_to_burn);
    p.advance(REQUEST_DELAY);
    p.refresh_price();
    p.publish_round();
    let result = router.resolve_next(&p.keeper);
    assert_eq!(
        result.status,
        request_router::SettlementStatus::Failed,
        "floor rounding would pass this withdrawal; the doc requires ceil"
    );
    assert_eq!(p.vault().balance(&p.lp), shares, "full escrow returned");

    // A withdrawal leaving utilization at exactly 4_000 settles.
    p.advance(1);
    let clean_post = 12_500 * UNIT; // risk × BPS / post = 4_000 exactly
    let withdraw_assets = equity - clean_post;
    let shares_to_burn = withdraw_assets * (supply + VIRTUAL_SHARES) / (nav + VIRTUAL_ASSETS);
    router.request_withdrawal(&p.lp, &shares_to_burn);
    p.advance(REQUEST_DELAY);
    p.refresh_price();
    p.publish_round();
    let result = router.resolve_next(&p.keeper);
    assert_eq!(result.status, request_router::SettlementStatus::Settled);
}

/// R16 "Trader profit paid: down" / "Trader loss collected: up" + C12/C13
/// (§6 settlement rows): the PnL numerator converts to cash exactly once
/// with directional rounding (§12.2), and the settlement moves cash, claims,
/// and equity exactly as the transition table specifies.
#[test]
fn r16_trader_profit_rounds_down_and_loss_rounds_up() {
    let scenario = |close_price: i128| -> (i128, i128, i128, i128, i128) {
        let p = Protocol::new();
        p.disable_borrow();
        // Close fee off too: this vector pins the PnL conversion to the
        // stroop, and a profit-side fee would sit on top of it.
        p.disable_funding_and_close_fee();
        p.seed_lp();
        let manager = p.manager();

        // size = 1e11 + 150 -> base = floor(size × 1e7 / 1e9) = 1e9 + 1,
        // which makes the close-side numerator indivisible by 1e7.
        let size = 100_000_000_150;
        let id = p.open(&p.trader_a, true, size, 3_000 * UNIT);
        let position = manager.get_position(&id);
        assert_eq!(position.base_exposure, 1_000_000_001);

        p.advance(31);
        p.set_price(close_price);
        p.publish_round();
        let before = p.snapshot();
        let balance_before = p.token().balance(&p.trader_a);
        p.close(id);
        let after = p.snapshot();
        let payout = p.token().balance(&p.trader_a) - balance_before;

        let pnl_numerator = position.base_exposure * close_price - size * UNIT;
        assert!(
            pnl_numerator % UNIT != 0,
            "vector must exercise the boundary"
        );
        (
            payout,
            position.stored_collateral,
            pnl_numerator,
            before.cash_lp_equity - after.cash_lp_equity,
            before.non_lp_claims - after.non_lp_claims,
        )
    };

    // Profit at 100.5: numerator/1e7 = 499_999_950.5 -> pay 499_999_950.
    let (payout, collateral, numerator, equity_drop, claims_drop) = scenario(1_005_000_000);
    let profit = floor_div(numerator, UNIT);
    assert_eq!(payout, collateral + profit, "profit must round down");
    assert_eq!(
        equity_drop, profit,
        "C12: LP equity falls by the paid profit"
    );
    assert_eq!(
        claims_drop, collateral,
        "C12: claims fall by the collateral"
    );

    // Loss at 99.5: |numerator|/1e7 = 500_000_050.5 -> collect 500_000_051.
    let (payout, collateral, numerator, equity_drop, claims_drop) = scenario(995_000_000);
    let loss = ceil_div(-numerator, UNIT);
    assert_eq!(payout, collateral - loss, "loss must round up");
    assert_eq!(
        equity_drop, -loss,
        "C13: LP equity rises by the collected loss"
    );
    assert_eq!(
        claims_drop, collateral,
        "C13: claims fall by the collateral"
    );
}

// ---------------------------------------------------------------------------
// Cash-transition table (§6) and cash-ownership invariant (§18.1)
// ---------------------------------------------------------------------------

/// C1/C2 (deposits raise cash and claims equally), C7/C8 (fee collection
/// moves value between claims), and C9 (non-LP claim withdrawals lower cash
/// and claims equally, never LP equity).
#[test]
fn c06_claim_rows_never_move_lp_equity() {
    let p = Protocol::new();
    p.disable_borrow();
    p.disable_funding();
    p.seed_lp();
    let manager = p.manager();

    // Open = trader collateral deposit + execution-budget deposit, nothing
    // else: no fee is charged, so cash and claims rise by the same amount
    // and LP equity does not move at all.
    let size = 10_000 * UNIT;
    let collateral = 3_000 * UNIT;
    let budget = 50 * UNIT;
    let before = p.snapshot();
    let id = p.open_with_budget(&p.trader_a, true, size, collateral, budget);
    let after_open = p.snapshot();
    assert_eq!(
        after_open.physical_cash - before.physical_cash,
        collateral + budget
    );
    assert_eq!(
        after_open.non_lp_claims - before.non_lp_claims,
        collateral + budget
    );
    assert_eq!(after_open.cash_lp_equity, before.cash_lp_equity);
    assert_eq!(manager.get_position(&id).execution_budget, budget);

    // C9: withdrawing execution budget lowers cash and claims by the same
    // amount and leaves LP equity untouched.
    manager.withdraw_execution_budget(&id, &(20 * UNIT));
    let after_withdraw = p.snapshot();
    assert_eq!(
        after_open.physical_cash - after_withdraw.physical_cash,
        20 * UNIT
    );
    assert_eq!(
        after_open.non_lp_claims - after_withdraw.non_lp_claims,
        20 * UNIT
    );
    assert_eq!(after_withdraw.cash_lp_equity, after_open.cash_lp_equity);
    assert_eq!(manager.get_position(&id).execution_budget, 30 * UNIT);

    // C2: funding the budget raises cash and claims by the same amount.
    manager.fund_execution_budget(&id, &(5 * UNIT));
    let after_fund = p.snapshot();
    assert_eq!(
        after_fund.physical_cash - after_withdraw.physical_cash,
        5 * UNIT
    );
    assert_eq!(
        after_fund.non_lp_claims - after_withdraw.non_lp_claims,
        5 * UNIT
    );
    assert_eq!(after_fund.cash_lp_equity, after_withdraw.cash_lp_equity);

    // C7/C8: a profitable close collects the closing fee and splits it
    // between the claim rows; only the LP share (less the paid profit)
    // touches equity. 1% up on 10_000 size: profit 100 UNIT, low-tier fee
    // 10 UNIT (the close empties the book, improving skew).
    p.advance(31);
    p.set_price(101 * UNIT);
    p.publish_round();
    let profit = 100 * UNIT;
    let fee = floor_div(size * 10, BPS);
    let lp_share = floor_div(fee * 7_000, BPS);
    let keeper_share = floor_div(fee * 1_000, BPS);
    let before_close = p.snapshot();
    p.close(id);
    let after_close = p.snapshot();
    assert_eq!(
        after_close.cash_lp_equity - before_close.cash_lp_equity,
        lp_share - profit,
        "C12 + C7: equity pays the profit and keeps the LP fee share"
    );

    // C9: paying out the protocol claim lowers cash and claims equally.
    let claimable = manager.protocol_claimable_total();
    assert_eq!(claimable, fee - lp_share - keeper_share);
    let recipient_before = p.token().balance(&p.trader_b);
    manager.claim_protocol(&p.admin, &p.trader_b, &claimable);
    let after_claim = p.snapshot();
    assert_eq!(
        after_close.physical_cash - after_claim.physical_cash,
        claimable
    );
    assert_eq!(
        after_close.non_lp_claims - after_claim.non_lp_claims,
        claimable
    );
    assert_eq!(after_claim.cash_lp_equity, after_close.cash_lp_equity);
    assert_eq!(p.token().balance(&p.trader_b) - recipient_before, claimable);
    assert_eq!(manager.protocol_claimable_total(), 0);
}

/// C10/C11: an LP deposit or withdrawal moves cash and LP equity by the same
/// amount and never touches non-LP claims.
#[test]
fn c10_c11_lp_flows_move_only_lp_equity() {
    let p = Protocol::new();
    p.disable_borrow();
    p.disable_funding();
    p.seed_lp();
    p.open(&p.trader_a, true, 10_000 * UNIT, 3_000 * UNIT);
    let router = p.router();

    // C10: deposit.
    let assets = 1_234_567_891;
    router.request_deposit(&p.trader_b, &assets);
    p.advance(REQUEST_DELAY);
    p.refresh_price();
    p.publish_round();
    let before = p.snapshot();
    let result = router.resolve_next(&p.keeper);
    assert_eq!(result.status, request_router::SettlementStatus::Settled);
    let after = p.snapshot();
    assert_eq!(after.physical_cash - before.physical_cash, assets);
    assert_eq!(after.non_lp_claims, before.non_lp_claims);
    assert_eq!(after.cash_lp_equity - before.cash_lp_equity, assets);

    // C11: withdrawal.
    p.advance(1);
    router.request_withdrawal(&p.trader_b, &result.amount);
    p.advance(REQUEST_DELAY);
    p.refresh_price();
    p.publish_round();
    let before = p.snapshot();
    let result = router.resolve_next(&p.keeper);
    assert_eq!(result.status, request_router::SettlementStatus::Settled);
    let after = p.snapshot();
    assert_eq!(before.physical_cash - after.physical_cash, result.amount);
    assert_eq!(after.non_lp_claims, before.non_lp_claims);
    assert_eq!(before.cash_lp_equity - after.cash_lp_equity, result.amount);
}

/// P19 + I18.1 + §4.2 + P27: through a full lifecycle with odd sizes and a
/// price move, one physical balance always reconciles exactly with LP equity
/// plus the enumerated non-LP claims — rounding never creates value.
#[test]
fn i18_1_physical_cash_reconciles_through_lifecycle() {
    let p = Protocol::new();
    let manager = p.manager();

    let check = |budgets: i128| {
        let s = p.snapshot();
        assert_eq!(s.cash_shortfall, 0);
        assert_eq!(
            s.physical_cash,
            s.cash_lp_equity + s.non_lp_claims,
            "§18.1 cash-ownership identity"
        );
        assert_eq!(s.non_lp_claims, manager.non_lp_claims());
        let market = manager.get_market(&p.market);
        assert_eq!(
            s.non_lp_claims,
            market.long.stored_collateral_total
                + market.short.stored_collateral_total
                + manager.pending_receiver_funding_total()
                + manager.protocol_claimable_total()
                + manager.risk_keeper_reserve_total()
                + budgets,
            "§4.2 non-LP claims are exactly the five enumerated claims"
        );
    };

    p.seed_lp();
    check(0);

    let budget = 7 * UNIT;
    let a = p.open_with_budget(
        &p.trader_a,
        true,
        10_001 * UNIT + 13,
        3_000 * UNIT + 7,
        budget,
    );
    check(budget);
    let b = p.open(&p.trader_b, false, 3_333 * UNIT + 41, 1_111 * UNIT + 3);
    check(budget);

    p.advance(12_345);
    p.refresh_price();
    manager.update_indices(&p.keeper, &p.market);
    p.publish_round();
    check(budget);

    manager.decrease_position(&a, &(2_000 * UNIT + 11), &0, &0);
    check(budget);

    p.advance(31);
    p.set_price(1_037_000_000); // 103.7
    p.publish_round();
    check(budget);

    p.close(b);
    check(budget);
    p.close(a);
    check(0);

    p.advance(1);
    let router = p.router();
    router.request_withdrawal(&p.lp, &(p.vault().balance(&p.lp) / 2));
    p.advance(REQUEST_DELAY);
    p.refresh_price();
    p.publish_round();
    let result = router.resolve_next(&p.keeper);
    assert_eq!(result.status, request_router::SettlementStatus::Settled);
    check(0);
}

// ---------------------------------------------------------------------------
// LP request settlement (§13, §18.7)
// ---------------------------------------------------------------------------

/// I18.7 + §13.3: requests resolve strictly in request-ID order through the
/// single FIFO head, for deposits and withdrawals alike.
#[test]
fn i18_7_fifo_resolves_in_request_id_order() {
    let p = Protocol::new();
    p.seed_lp();
    let router = p.router();

    p.advance(1);
    let a = router.request_deposit(&p.trader_a, &(1_000 * UNIT));
    let b = router.request_deposit(&p.trader_b, &(2_000 * UNIT));
    let c = router.request_withdrawal(&p.lp, &(1_000 * VIRTUAL_SHARES));
    assert!(a < b && b < c);
    assert_eq!(router.next_request_to_resolve(), a);

    p.advance(REQUEST_DELAY);
    p.refresh_price();
    p.publish_round();

    let result = router.resolve_next(&p.keeper);
    assert_eq!(result.status, request_router::SettlementStatus::Settled);
    assert_eq!(
        router.get_request(&a).status,
        request_router::LpRequestStatus::Settled
    );
    assert_eq!(
        router.get_request(&b).status,
        request_router::LpRequestStatus::Pending
    );
    assert_eq!(router.next_request_to_resolve(), b);

    let result = router.resolve_next(&p.keeper);
    assert_eq!(result.status, request_router::SettlementStatus::Settled);
    assert_eq!(router.next_request_to_resolve(), c);

    let result = router.resolve_next(&p.keeper);
    assert_eq!(result.status, request_router::SettlementStatus::Settled);
    assert_eq!(
        router.get_request(&c).status,
        request_router::LpRequestStatus::Settled
    );
    assert_eq!(router.next_request_to_resolve(), c + 1);
}

/// I18.7 + §13.2: execute_after = request_time + lp_request_delay, and a
/// round timestamped before execute_after can neither settle nor expire the
/// request — the settlement round is the first round at or after
/// eligibility, which nobody can substitute.
#[test]
fn i18_7_round_before_execute_after_cannot_settle() {
    let p = Protocol::new();
    p.seed_lp();
    let router = p.router();

    p.advance(1);
    let escrow = 1_000 * UNIT;
    let balance_before = p.token().balance(&p.trader_a);
    let id = router.request_deposit(&p.trader_a, &escrow);
    let request = router.get_request(&id);
    assert_eq!(request.execute_after, request.request_time + REQUEST_DELAY);

    // A round one second before eligibility.
    p.advance(REQUEST_DELAY - 1);
    p.refresh_price();
    p.publish_round();
    if let Ok(Ok(result)) = router.try_resolve_next(&p.keeper) {
        assert_ne!(
            result.status,
            request_router::SettlementStatus::Settled,
            "a pre-eligibility round must not settle the request"
        );
    }
    assert_eq!(
        router.get_request(&id).status,
        request_router::LpRequestStatus::Pending,
        "the request must stay pending: its assigned round does not exist yet"
    );
    assert_eq!(p.token().balance(&p.trader_a), balance_before - escrow);

    // The first round at eligibility settles it.
    p.advance(1);
    p.refresh_price();
    p.publish_round();
    let result = router.resolve_next(&p.keeper);
    assert_eq!(result.status, request_router::SettlementStatus::Settled);
    assert_eq!(
        router.get_request(&id).status,
        request_router::LpRequestStatus::Settled
    );
}

// ---------------------------------------------------------------------------
// Governance bounds and safety (§18.8, §14)
// ---------------------------------------------------------------------------

/// I18.8 "Keep complete revenue shares at or below BPS": a configuration
/// whose LP + risk-keeper shares exceed BPS must be rejected.
#[test]
fn i18_8_revenue_shares_above_bps_rejected() {
    let p = Protocol::new();
    let mut config = Protocol::global_config(100, 900);
    config.lp_revenue_share_bps = 7_000;
    config.risk_keeper_revenue_share_bps = 3_001; // sum 10_001 > BPS
    assert!(p
        .manager()
        .try_set_global_config(&p.admin, &config)
        .is_err());
}

/// I18.8 "Set lp_request_delay to a nonzero value": a zero delay would let
/// an immediate LP action front-run the oracle (§13.2) and must be rejected.
#[test]
fn i18_8_zero_lp_request_delay_rejected() {
    let p = Protocol::new();
    let attempt = p.vault().try_set_lp_config(
        &p.admin,
        &vault::LpConfig {
            max_withdraw_utilization_bps: 8_000,
            min_deposit_nav_factor_bps: 5_000,
            lp_request_delay: 0,
        },
    );
    assert!(attempt.is_err());
}

/// I18.8 "Keep the sum of side hard-cap factors within the global limit"
/// (§14): market configurations whose combined hard-cap factors break
/// `global_hard_cap_factor_limit_bps` = 10_000 must be rejected. Vectors are
/// chosen so per-market and per-side readings of the sum agree on every
/// verdict.
#[test]
fn i18_8_hard_cap_factor_sum_bounded_by_global_limit() {
    let p = Protocol::new();
    let manager = p.manager();
    let oracle_router = oracle_router::Client::new(&p.env, &p.oracle_router_id);

    let capped = |recovery: u32, warning: u32, adl: u32, hard: u32| {
        let mut config = Protocol::market_config(100);
        config.recovery_pnl_factor_bps = recovery;
        config.warning_pnl_factor_bps = warning;
        config.adl_pnl_factor_bps = adl;
        config.hard_cap_pnl_factor_bps = hard;
        config
    };
    let add_sources = |symbol: &Symbol| {
        for id in &p.oracle_ids {
            oracle::Client::new(&p.env, id).set_price(&p.publisher, symbol, &PRICE_100);
        }
        oracle_router.set_oracle_sources(
            &p.admin,
            symbol,
            &vec![
                &p.env,
                p.oracle_ids[0].clone(),
                p.oracle_ids[1].clone(),
                p.oracle_ids[2].clone(),
            ],
        );
    };

    // Shrink BTC to 2_500 so a compliant second market exists under either
    // reading of "sum of side factors".
    manager.set_market_config(&p.admin, &p.market, &capped(500, 1_000, 2_000, 2_500));

    // ETH at 2_400: totals 4_900 per-market / 9_800 per-side — both within
    // the 10_000 limit, so this must be accepted.
    let eth = symbol_short!("ETH");
    add_sources(&eth);
    manager.set_market_config(&p.admin, &eth, &capped(500, 1_000, 2_000, 2_400));

    // XLM at 5_200: totals 10_100 per-market / 20_200 per-side — both above
    // the limit, so this must be rejected.
    let xlm = symbol_short!("XLM");
    add_sources(&xlm);
    assert!(manager
        .try_set_market_config(&p.admin, &xlm, &capped(2_000, 3_000, 4_000, 5_200))
        .is_err());
}

/// I18.8 "Pay ADL rewards only after a qualifying risk reduction" and "only
/// from the risk-keeper reserve" (§14): deleveraging is rejected in a NORMAL
/// state, and in the ADL state the keeper's reward is debited exactly from
/// the risk-keeper reserve, capped by max_adl_reward.
#[test]
fn i18_8_adl_reward_paid_only_in_adl_state_and_from_reserve() {
    let p = Protocol::new();
    // No time-based fees: the deleverage close then collects nothing, so the
    // risk-keeper reserve can move only by the ADL reward itself.
    p.disable_borrow();
    p.disable_funding();
    p.seed_lp();
    let manager = p.manager();
    let router = p.router();

    let id = p.open(&p.trader_a, true, 10_000 * UNIT, 3_000 * UNIT);
    // A second small long whose profitable close will fund the risk-keeper
    // reserve through the closing-fee split.
    let funder_id = p.open(&p.trader_b, true, 100 * UNIT, 30 * UNIT);
    assert!(
        manager.try_deleverage_position(&p.keeper, &id).is_err(),
        "no ADL reward without a qualifying risk state"
    );

    // Queue a deposit while NORMAL; its failed settlement after the price
    // jump persists the risk-state update (§14).
    router.request_deposit(&p.trader_b, &(1_000 * UNIT));
    p.advance(REQUEST_DELAY);
    // Profit ~101 units × 410 = ~41_400 UNIT on ~100_000 equity: factor
    // ~4_1xx bps — inside [adl 4_000, hard cap 5_000).
    p.set_price(510 * UNIT);
    p.publish_round();
    let result = router.resolve_next(&p.keeper);
    assert_eq!(result.status, request_router::SettlementStatus::Failed);

    p.close(funder_id);
    let reserve_before = manager.risk_keeper_reserve_total();
    assert!(
        reserve_before > 0,
        "the closing-fee split must have funded the reserve"
    );
    let keeper_before = p.token().balance(&p.keeper);
    manager.deleverage_position(&p.keeper, &id);

    let reward = p.token().balance(&p.keeper) - keeper_before;
    assert!(reward > 0, "a qualifying ADL action pays a reward");
    // The profitable ADL close itself collects a closing fee (10 UNIT low
    // tier on 10_000 size) whose keeper share tops up the reserve in the
    // same action; the reward then drains the reserve completely.
    let close_fee_keeper_share = UNIT;
    assert_eq!(
        reward,
        reserve_before + close_fee_keeper_share,
        "the reward is debited exactly from the risk-keeper reserve"
    );
    assert_eq!(manager.risk_keeper_reserve_total(), 0);
    assert!(reward <= 100 * UNIT, "capped by max_adl_reward");
    assert!(
        manager.try_get_position(&id).is_err(),
        "the qualifying action reduced the profitable exposure"
    );
}

/// I18.8 "Do not mint shares during recapitalization" (§15.2): the
/// contribution raises physical cash and LP equity, mints nothing, and adds
/// no non-LP claim.
#[test]
fn i18_8_recapitalization_mints_no_shares() {
    let p = Protocol::new();
    p.seed_lp();
    let supply_before = p.vault().total_share_supply();
    let before = p.snapshot();

    let amount = 1_000 * UNIT;
    p.manager().recapitalize(&p.trader_b, &amount);

    let after = p.snapshot();
    assert_eq!(
        p.vault().total_share_supply(),
        supply_before,
        "no shares minted"
    );
    assert_eq!(after.physical_cash - before.physical_cash, amount);
    assert_eq!(after.non_lp_claims, before.non_lp_claims);
    assert_eq!(after.cash_lp_equity - before.cash_lp_equity, amount);
}

/// I18.8 "Keep the global hard-cap limit at or below BPS".
#[test]
fn i18_8_global_hard_cap_limit_above_bps_rejected() {
    let p = Protocol::new();
    let mut config = Protocol::global_config(100, 900);
    config.hard_cap_factor_limit_bps = 10_001;
    assert!(p
        .manager()
        .try_set_global_config(&p.admin, &config)
        .is_err());
}

/// §16 telescoping row "clean final LP redemption" + theory §12: in a clean
/// terminal state the final LP redeems ALL cash LP equity — the virtual-
/// quantity floor may not strand dust in the vault.
#[test]
fn r16_clean_final_redemption_pays_all_lp_cash() {
    let p = Protocol::new();
    let shares = p.seed_lp();
    // Odd dust that plain floor conversion would strand.
    p.token().transfer(&p.trader_b, &p.vault_id, &33);
    let equity = p.vault().physical_cash();
    assert_eq!(equity, 100_000 * UNIT + 33);

    p.advance(1);
    let router = p.router();
    let balance_before = p.token().balance(&p.lp);
    router.request_withdrawal(&p.lp, &shares);
    p.advance(REQUEST_DELAY);
    p.refresh_price();
    p.publish_round();
    let result = router.resolve_next(&p.keeper);
    assert_eq!(result.status, request_router::SettlementStatus::Settled);
    assert_eq!(result.amount, equity, "the final LP receives all LP cash");
    assert_eq!(p.token().balance(&p.lp) - balance_before, equity);
    assert_eq!(p.vault().physical_cash(), 0, "no stranded dust");
    assert_eq!(p.vault().total_share_supply(), 0);
}

// ---------------------------------------------------------------------------
// Insolvency, NAV floor, recovery, and complexity guards
// ---------------------------------------------------------------------------

/// R16 "loss up, but never above available value" + the C13 insolvent edge +
/// I18.1 under stress: when the raw loss dwarfs the position's value, the
/// trader's payout clamps at exactly zero, LP equity gains exactly the stored
/// collateral (all there was), and the cash-ownership identity holds. Both
/// exit paths — voluntary close and liquidation — must agree.
#[test]
fn r16_underwater_loss_collects_only_available_value() {
    let scenario = |liquidate: bool| {
        let p = Protocol::new();
        p.disable_borrow();
        p.disable_funding();
        p.seed_lp();
        let manager = p.manager();
        let id = p.open(&p.trader_a, true, 10_000 * UNIT, 3_000 * UNIT);
        let stored = manager.get_position(&id).stored_collateral;

        // Crash 100 -> 10: raw loss = 100 units × 90 = 9_000 UNIT, three
        // times the ~2_970-UNIT stored collateral.
        p.advance(31);
        p.set_price(10 * UNIT);
        p.publish_round();
        let before = p.snapshot();

        let trader_before = p.token().balance(&p.trader_a);
        let keeper_before = p.token().balance(&p.keeper);
        if liquidate {
            manager.liquidate_position(&p.keeper, &id);
        } else {
            p.close(id);
        }
        let after = p.snapshot();

        assert_eq!(
            p.token().balance(&p.trader_a),
            trader_before,
            "the payout must clamp at exactly zero, never go negative"
        );
        // Any insolvency-touch reward is paid from the risk-keeper reserve
        // claim, so it lowers cash and claims equally and never LP equity.
        let keeper_reward = p.token().balance(&p.keeper) - keeper_before;

        assert_eq!(
            after.cash_lp_equity - before.cash_lp_equity,
            stored,
            "collected loss is exactly the available stored collateral"
        );
        assert_eq!(
            before.non_lp_claims - after.non_lp_claims,
            stored + keeper_reward
        );
        assert_eq!(before.physical_cash - after.physical_cash, keeper_reward);
        // I18.1 with unpaid obligations outstanding.
        assert_eq!(
            after.physical_cash,
            after.cash_lp_equity + after.non_lp_claims
        );

        // I18.2: the aggregates telescope to zero with the position gone.
        let side = manager.get_market(&p.market).long;
        assert_eq!(side.size_open_interest, 0);
        assert_eq!(side.base_exposure, 0);
        assert_eq!(side.risk_units, 0);
        assert_eq!(side.stored_collateral_total, 0);
        assert_eq!(after.open_position_count, 0);
        assert_eq!(
            after.vault_nav, after.cash_lp_equity,
            "no open positions: NAV equals cash equity"
        );
    };
    scenario(false);
    scenario(true);
}

/// I18.6 "nav = max(equity − recognized PnL, 0)": when recognized trader
/// profit exceeds all LP equity the marked NAV floors at exactly zero — a
/// negative NAV would corrupt every share-pricing computation downstream.
#[test]
fn i18_6_marked_nav_floors_at_zero_when_profit_exceeds_equity() {
    let p = Protocol::new();
    p.disable_borrow();
    p.disable_funding();
    p.deposit(&p.lp, 10_000 * UNIT);
    p.open(&p.trader_a, true, 10_000 * UNIT, 3_000 * UNIT);

    // Equity is price-independent, so the seed round is a valid basis.
    let entry = p.snapshot();

    // 100 -> 250: recognized profit = 100 units × 150 = 15_000 UNIT against
    // ~10_021 UNIT of equity (10_000 seed + 21 LP fee share).
    p.advance(31);
    p.set_price(250 * UNIT);
    p.publish_round();
    let mooned = p.snapshot();

    assert!(
        15_000 * UNIT > mooned.cash_lp_equity,
        "vector must drive recognized profit past equity"
    );
    assert_eq!(mooned.cash_lp_equity, entry.cash_lp_equity);
    assert_eq!(
        mooned.vault_nav, 0,
        "I18.6: NAV floors at zero, it never goes negative"
    );
}

/// I18.8 "the blocked-side count equals the number of restricted sides" —
/// the recovery half. Entering warning is covered by p26; here the price
/// reverts and the side must actually unblock: the freshly-evaluated count
/// returns to zero, and after the next mutating action the stored counter
/// follows, reopening the LP pipeline end to end.
#[test]
fn i18_8_blocked_side_count_recovers_and_lp_pipeline_reopens() {
    let p = Protocol::new();
    p.seed_lp();
    let router = p.router();
    p.open(&p.trader_a, true, 10_000 * UNIT, 3_000 * UNIT);

    // Enter warning exactly as p26 does: a settlement at the pumped price
    // evaluates the risk state and persists the blocked count.
    router.request_deposit(&p.trader_b, &(1_000 * UNIT));
    p.advance(REQUEST_DELAY);
    p.set_price(450 * UNIT);
    p.publish_round();
    let result = router.resolve_next(&p.keeper);
    assert_eq!(result.status, request_router::SettlementStatus::Failed);
    assert_eq!(p.snapshot().lp_blocked_side_count, 1);
    assert!(!p.vault().can_create_lp_request());

    // Price reverts below the recovery factor (profit 0 < 2_000 bps latch).
    p.advance(31);
    p.set_price(PRICE_100);
    p.publish_round();

    // The non-mutating snapshot re-evaluates and already reports recovery;
    // the stored counter (which gates request creation) updates only on the
    // next mutating path — pin both halves of that behavior.
    assert_eq!(p.snapshot().lp_blocked_side_count, 0);
    assert!(!p.vault().can_create_lp_request());

    // Any position mutation re-evaluates market risk and unlatches.
    p.open(&p.trader_b, false, 100 * UNIT, 50 * UNIT);
    assert!(p.vault().can_create_lp_request());

    // The LP pipeline works end to end again.
    p.advance(1);
    p.deposit(&p.trader_b, 1_000 * UNIT);
}

/// P23: an LP settlement reads per-market aggregates, never the position
/// book — its metered cost must not grow with the number of open positions.
/// This is the only guard against an O(positions) regression that would
/// brick LP exits at scale.
#[test]
fn p23_lp_settlement_cost_independent_of_open_positions() {
    let instructions_with = |position_count: u32| -> i64 {
        let p = Protocol::new();
        p.seed_lp();
        for i in 0..position_count {
            let long = i % 2 == 0;
            let owner = if long { &p.trader_a } else { &p.trader_b };
            p.open(owner, long, 100 * UNIT, 60 * UNIT);
        }
        let router = p.router();
        router.request_deposit(&p.lp, &(1_000 * UNIT));
        p.advance(REQUEST_DELAY);
        p.refresh_price();
        p.publish_round();
        let result = router.resolve_next(&p.keeper);
        assert_eq!(result.status, request_router::SettlementStatus::Settled);
        // Metered resources of the last top-level invocation: resolve_next.
        p.env.cost_estimate().resources().instructions
    };

    let one = instructions_with(1);
    let fifty = instructions_with(50);
    assert!(
        fifty <= one + one / 10,
        "settlement cost must not grow with the position book: \
         1 position = {one} instructions, 50 positions = {fifty}"
    );
}

/// R16 "borrow obligation rounds up", pinned strictly. p10 verifies the
/// quadratic curve with a ±1 baseline tolerance a floor implementation could
/// hide inside; this vector zeroes the baseline residue (open at 864s: the
/// pre-open index is exactly 1e10, divisible by risk/PRECISION) and makes
/// the close-side product indivisible, so ceil and floor differ by exactly
/// one stroop and only ceil passes.
#[test]
fn r16_borrow_obligation_rounds_up_exact_vector() {
    let p = Protocol::new();
    p.disable_funding();
    // No opening fee: post-open equity is the deposit itself, making
    // utilization exactly 500 bps (p10's construction).
    p.deposit(&p.lp, 100_000 * UNIT);

    // Ledger t0 = fixture start; the deposit consumed REQUEST_DELAY = 60s.
    // Open at t0 + 864s: pre-open index = 100×1e14×864 / (1e4×86_400) = 1e10
    // exactly, with zero carried remainder.
    p.advance(804);
    let size = 10_000 * UNIT;
    let collateral = 3_000 * UNIT;
    let id = p.open(&p.trader_a, true, size, collateral);

    let risk = 5_000 * UNIT;
    let seconds_per_day = DAY as i128;
    let denominator = BPS * seconds_per_day;
    let index_open = 100 * INDEX_PRECISION * 864 / denominator;
    assert_eq!(index_open, 10_000_000_000);
    assert_eq!(100 * INDEX_PRECISION * 864 % denominator, 0, "no carry");
    let baseline = ceil_div(risk * index_open, INDEX_PRECISION);
    assert_eq!(
        risk * index_open % INDEX_PRECISION,
        0,
        "the baseline must not round, so the close-side ceil is isolated"
    );

    // rate = 100 + 900 × (500/BPS)² = 102.25 bps/day at the stored scale.
    let rate = 100 * INDEX_PRECISION + 900 * INDEX_PRECISION * 500 * 500 / (BPS * BPS);
    let elapsed = 500i128;
    let index_close = index_open + rate * elapsed / denominator;
    assert!(
        risk * index_close % INDEX_PRECISION != 0,
        "vector must exercise the rounding boundary"
    );
    let expected = ceil_div(risk * index_close, INDEX_PRECISION) - baseline;
    // One stroop above the floor result — the assertion a floor
    // implementation cannot pass.
    assert_eq!(expected, 2_958_623);

    p.advance(elapsed as u64);
    p.refresh_price();
    let balance_before = p.token().balance(&p.trader_a);
    p.close(id);
    let payout = p.token().balance(&p.trader_a) - balance_before;
    assert_eq!(
        collateral - payout,
        expected,
        "borrow collection must ceil with zero tolerance"
    );
}

/// P21 / I18.6 trust boundary: `accounting_snapshot` validates the round's
/// structure (price count, symbol order, positive prices) but deliberately
/// trusts the caller for provenance — it is a quote-at-these-prices view.
/// Settlement provenance is enforced by the router-gated call chain instead.
/// Pin both halves so a change to either is a conscious decision.
#[test]
fn p21_snapshot_validates_round_structure_but_trusts_caller_prices() {
    let p = Protocol::new();
    p.seed_lp();
    p.open(&p.trader_a, true, 10_000 * UNIT, 3_000 * UNIT);

    p.advance(31);
    p.set_price(50 * UNIT);
    p.publish_round();
    let genuine = p.latest_round_for_vault();
    let vault = p.vault();

    // Structural rejections: wrong price count, wrong symbol, non-positive
    // price all panic with InvalidOracleRound.
    let mut empty = genuine.clone();
    empty.prices = Vec::new(&p.env);
    assert!(vault.try_accounting_snapshot(&empty).is_err());

    let mut wrong_symbol = genuine.clone();
    wrong_symbol.prices = vec![
        &p.env,
        vault::RoundPrice {
            symbol: symbol_short!("ETH"),
            price: 50 * UNIT,
        },
    ];
    assert!(vault.try_accounting_snapshot(&wrong_symbol).is_err());

    let mut zero_price = genuine.clone();
    zero_price.prices = vec![
        &p.env,
        vault::RoundPrice {
            symbol: p.market.clone(),
            price: 0,
        },
    ];
    assert!(vault.try_accounting_snapshot(&zero_price).is_err());

    // Provenance is NOT checked: a forged id/timestamp/price is accepted and
    // the snapshot is computed at the supplied price. At the genuine crashed
    // price NAV recognizes the (capped) loss; the forged entry-price round
    // reports NAV = equity. Off-chain consumers must source rounds from the
    // oracle router.
    let genuine_snapshot = vault.accounting_snapshot(&genuine);
    let mut forged = genuine.clone();
    forged.id += 1_000;
    forged.timestamp += 999_999;
    forged.prices = vec![
        &p.env,
        vault::RoundPrice {
            symbol: p.market.clone(),
            price: PRICE_100,
        },
    ];
    let forged_snapshot = vault.accounting_snapshot(&forged);
    assert_eq!(forged_snapshot.vault_nav, forged_snapshot.cash_lp_equity);
    assert!(
        genuine_snapshot.vault_nav > genuine_snapshot.cash_lp_equity,
        "the genuine crashed round recognizes the trader loss"
    );
}

// ---------------------------------------------------------------------------
// Tail hygiene: proportionality, monotonicity, and boundary pins
// ---------------------------------------------------------------------------

/// P3: light-side traders receive funding in proportion to their own
/// counter-exposure — two simultaneous receivers of different sizes each get
/// exactly floor(size × receiver_index / INDEX_PRECISION), so the larger
/// receives its pro-rata share, not the whole stream.
#[test]
fn p03_two_receivers_credited_in_proportion_to_exposure() {
    let p = Protocol::new();
    p.disable_borrow();
    p.seed_lp();
    let manager = p.manager();

    // All three opened at the same timestamp: every funding baseline is 0.
    p.open(&p.trader_a, true, 10_000 * UNIT, 3_000 * UNIT);
    let big = p.open(&p.trader_b, false, 2_000 * UNIT, 600 * UNIT);
    let small = p.open(&p.trader_b, false, 1_000 * UNIT, 300 * UNIT);
    let stored_big = manager.get_position(&big).stored_collateral;
    let stored_small = manager.get_position(&small).stored_collateral;

    p.advance(DAY);
    p.refresh_price();

    // Both closes at one timestamp share one receiver index; price is
    // unchanged and the shorts pay nothing (the long side is the payer), so
    // each payout is stored collateral plus exactly the receiver credit.
    let before_big = p.token().balance(&p.trader_b);
    p.close(big);
    let credit_big = p.token().balance(&p.trader_b) - before_big - stored_big;
    let before_small = p.token().balance(&p.trader_b);
    p.close(small);
    let credit_small = p.token().balance(&p.trader_b) - before_small - stored_small;

    let index = manager.get_market(&p.market).receiver_index_short;
    assert!(index > 0, "a day of one-sided funding must accrue");
    assert_eq!(
        credit_big,
        floor_div(2_000 * UNIT * index, INDEX_PRECISION),
        "credit is the per-position floor formula, not a stream share"
    );
    assert_eq!(
        credit_small,
        floor_div(1_000 * UNIT * index, INDEX_PRECISION)
    );
    // The doc's proportionality claim, within floor dust.
    assert!((credit_big - 2 * credit_small).abs() <= 2);
}

/// I18.4 "an index never decreases": swept across opens on both sides, a
/// funding-rate config change, closes, and keeper checkpoints — every one of
/// the six market indices must be monotone non-decreasing at every step.
#[test]
fn i18_4_indices_are_monotone_across_lifecycle() {
    let p = Protocol::new();
    p.seed_lp();
    let manager = p.manager();

    let indices = || -> [i128; 6] {
        let m = manager.get_market(&p.market);
        [
            m.receiver_backed_index_long,
            m.receiver_backed_index_short,
            m.lp_backed_index_long,
            m.lp_backed_index_short,
            m.receiver_index_long,
            m.receiver_index_short,
        ]
    };
    let mut previous = indices();
    let mut step = |label: &str| {
        let current = indices();
        for (i, (now, before)) in current.iter().zip(previous.iter()).enumerate() {
            assert!(
                now >= before,
                "index {i} decreased after {label}: {before} -> {now}"
            );
        }
        previous = current;
    };

    p.open(&p.trader_a, true, 10_000 * UNIT, 3_000 * UNIT);
    let short_a = p.open(&p.trader_b, false, 4_000 * UNIT, 1_200 * UNIT);
    step("both sides opened");

    p.advance(1_000);
    manager.update_indices(&p.keeper, &p.market);
    step("first keeper checkpoint");

    p.advance(500);
    p.open(&p.trader_b, false, 2_000 * UNIT, 600 * UNIT);
    step("second short opened");

    p.advance(700);
    manager.update_indices(&p.keeper, &p.market);
    step("second keeper checkpoint");

    // A rate change checkpoints with the old rate before applying the new.
    manager.set_market_config(&p.admin, &p.market, &Protocol::market_config(37));
    step("funding rate reconfigured");

    p.advance(900);
    manager.update_indices(&p.keeper, &p.market);
    step("checkpoint at the new rate");

    p.advance(31);
    p.refresh_price();
    p.close(short_a);
    step("receiver closed");

    p.advance(600);
    manager.update_indices(&p.keeper, &p.market);
    step("final checkpoint");
}

/// P13 boundary: the global capacity gate accepts new risk exactly at the
/// limit and rejects one stroop past it, mutating nothing on rejection.
/// Equity is tuned so required backing = ceil(risk × BPS / capacity_bps)
/// lands exactly on it: size 128_000 -> risk 64_000 -> backing 80_000 UNIT.
#[test]
fn p13_capacity_gate_boundary_accept_and_reject() {
    // No opening fee: post-open equity is exactly the deposit.
    let boundary_deposit = 80_000 * UNIT;

    let attempt = |deposit: i128| -> (Protocol, bool) {
        let p = Protocol::new();
        p.deposit(&p.lp, deposit);
        let accepted = p
            .manager()
            .try_open_position(
                &p.trader_a,
                &p.market,
                &true,
                &(128_000 * UNIT),
                &(7_000 * UNIT),
                &0,
                &0,
                &0,
                &0,
            )
            .is_ok();
        (p, accepted)
    };

    let (at_limit, accepted) = attempt(boundary_deposit);
    assert!(accepted, "risk exactly at the capacity limit is accepted");
    assert_eq!(
        at_limit.snapshot().required_risk_backing,
        80_000 * UNIT,
        "vector must land exactly on the boundary"
    );

    let (past_limit, accepted) = attempt(boundary_deposit - 1);
    assert!(accepted == false, "one stroop past the limit is rejected");
    let market = past_limit.manager().get_market(&past_limit.market);
    assert_eq!(market.long.size_open_interest, 0, "rejection must not mutate");
    assert_eq!(market.long.risk_units, 0);
    assert_eq!(past_limit.snapshot().total_risk_units, 0);
}

/// R16 "aggregate receiver liability rounds down, remainder carried": the
/// pending receiver total advances by exactly
/// floor(payer_size × receiver_backed_index_delta / PRECISION) with the
/// sub-stroop remainder carried per market, so split intervals telescope to
/// the joined total exactly (§8.3).
#[test]
fn r16_receiver_liability_floors_with_carried_remainder() {
    let p = Protocol::new();
    p.disable_borrow();
    p.seed_lp();
    let manager = p.manager();

    // One-sided-enough market so the receiver accrual is nonzero.
    p.open(&p.trader_a, true, 10_000 * UNIT, 3_000 * UNIT);
    p.open(&p.trader_b, false, 3_000 * UNIT, 900 * UNIT);
    let payer_size = 10_000 * UNIT;
    let index_start = manager.get_market(&p.market).receiver_backed_index_long;
    let start = manager.pending_receiver_funding_total();

    // Two odd intervals: each advance floors against INDEX_PRECISION and
    // carries its remainder forward.
    p.advance(997);
    p.refresh_price();
    manager.update_indices(&p.keeper, &p.market);
    let first_delta =
        manager.get_market(&p.market).receiver_backed_index_long - index_start;
    let first = manager.pending_receiver_funding_total() - start;
    assert!(first_delta > 0);
    assert_eq!(
        first,
        floor_div(payer_size * first_delta, INDEX_PRECISION),
        "floor, never round"
    );

    // The carried remainder joins the second interval, so the two-step total
    // equals what the joined index delta implies — the split loses nothing.
    p.advance(1_003);
    p.refresh_price();
    manager.update_indices(&p.keeper, &p.market);
    let total_delta =
        manager.get_market(&p.market).receiver_backed_index_long - index_start;
    let total = manager.pending_receiver_funding_total() - start;
    assert_eq!(total, floor_div(payer_size * total_delta, INDEX_PRECISION));
}

/// §13.2 cutoff semantics: a canonical round's prices are observations AT
/// the round timestamp — publish_round must aggregate fresh from sources,
/// never serve the router cache. Otherwise the permissionless get_price
/// cache write lets anyone pin a pre-cutoff observation into a post-cutoff
/// round and defeat the delayed-LP-settlement guarantee.
#[test]
fn i13_2_round_prices_aggregate_fresh_never_cached() {
    let p = Protocol::new();
    // seed_lp publishes a round at 100 UNIT, which also writes the router
    // cache at that price.
    p.seed_lp();

    // Sources move 5s later — well inside the 30s cache window, so a cached
    // fetch would still return the old price.
    p.advance(5);
    p.set_price(120 * UNIT);
    let id = p.publish_round();

    let router = oracle_router::Client::new(&p.env, &p.oracle_router_id);
    let round = router.get_round(&id);
    assert_eq!(round.timestamp, p.env.ledger().timestamp());
    assert_eq!(
        round.prices.get(0).unwrap().price,
        120 * UNIT,
        "a round must carry the fresh source aggregate, not the cache"
    );
}

/// P15 for position mutations: a skew-changing open checkpoints the market
/// first, so time before the mutation accrues at the old rate and the
/// steeper post-mutation rate applies only afterwards. (The config-change
/// variant lives in the original suite.)
#[test]
fn p15_skew_mutation_reprices_only_forward() {
    let p = Protocol::new();
    p.disable_borrow();
    p.seed_lp();
    let manager = p.manager();

    // Long is the payer side throughout: 10_000 vs 8_000, then 18_000.
    p.open(&p.trader_a, true, 10_000 * UNIT, 3_000 * UNIT);
    p.open(&p.trader_b, false, 8_000 * UNIT, 2_400 * UNIT);
    let payer_index = || {
        let m = manager.get_market(&p.market);
        (
            m.receiver_backed_index_long + m.lp_backed_index_long,
            m.current_payer_rate,
        )
    };
    let (i0, rate_before) = payer_index();

    // The mutation: a second long steepens the skew mid-stream. Its open
    // checkpoints [t0, t1] with the OLD flows before recomputing.
    p.advance(1_000);
    p.open(&p.trader_a, true, 8_000 * UNIT, 2_400 * UNIT);
    let (i1, rate_after) = payer_index();
    assert!(
        rate_after > rate_before,
        "the mutation must actually raise the payer rate"
    );

    p.advance(1_000);
    manager.update_indices(&p.keeper, &p.market);
    let (i2, _) = payer_index();

    assert!(i1 > i0);
    assert!(
        i2 - i1 > i1 - i0,
        "equal intervals: the pre-mutation interval must have accrued at the \
         shallower old rate ({} vs {})",
        i1 - i0,
        i2 - i1
    );
}

// ---------------------------------------------------------------------------
// §8.1 EMA funding: history-blended skew
// ---------------------------------------------------------------------------

/// §8.1 EMA: the trader who balances the book keeps receiving for a while.
/// With the book perfectly balanced the blended integral skew is still
/// nonzero from history, the old dominant side keeps paying, and the share
/// cap routes every stroop to the balancers — LPs collect nothing they did
/// not match.
#[test]
fn ema_funding_rewards_the_balancer_after_the_book_levels() {
    let p = Protocol::new();
    p.disable_borrow();
    p.seed_lp();
    let manager = p.manager();
    let mut config = Protocol::market_config(100);
    config.instant_weight_bps = 3_000;
    manager.set_market_config(&p.admin, &p.market, &config);

    // Long-only book for a day: the EMA charges up toward full long skew.
    p.open(&p.trader_a, true, 10_000 * UNIT, 3_000 * UNIT);
    p.advance(DAY);
    p.refresh_price();
    manager.update_indices(&p.keeper, &p.market);
    let charged = manager.get_market(&p.market);
    assert!(charged.skew_ema > 0, "history remembers the long skew");

    // A short balances the book exactly. Instant skew is zero, but the
    // blend still points long: longs keep paying.
    let short_id = p.open(&p.trader_b, false, 10_000 * UNIT, 3_000 * UNIT);
    let market = manager.get_market(&p.market);
    assert_eq!(market.current_payer_side, abi::PayerSide::Long);
    assert!(market.current_payer_rate > 0);

    // Over the next hour the balancer collects real credit, and the LP
    // index does not move — the whole balanced book matches the payer flow.
    let balance_before = p.token().balance(&p.trader_b);
    p.advance(3_600);
    p.refresh_price();
    manager.update_indices(&p.keeper, &p.market);
    let after = manager.get_market(&p.market);
    assert!(
        after.receiver_index_short > 0,
        "the balancer earns credit on a balanced book"
    );
    assert_eq!(
        after.lp_backed_index_long, charged.lp_backed_index_long,
        "nothing unmatched for LPs while the book is balanced"
    );

    // The credit is real cash on close (flat price, so no fee, no PnL).
    p.close(short_id);
    let payout = p.token().balance(&p.trader_b) - balance_before;
    assert!(
        payout > 3_000 * UNIT,
        "balancing paid: got {payout} back on 3_000 collateral"
    );
}

/// §8.1 EMA: after a hard flip the payer can be the *lighter* side. The
/// receiver share cap keeps the LP slice non-negative — without it this
/// exact shape drove the LP-backed index backwards, which
/// `funding::pending_fees` treats as an invariant violation on every later
/// action (a bricked market).
#[test]
fn ema_funding_survives_a_lighter_side_payer() {
    let p = Protocol::new();
    p.disable_borrow();
    p.seed_lp();
    let manager = p.manager();
    let mut config = Protocol::market_config(100);
    config.instant_weight_bps = 3_000;
    manager.set_market_config(&p.admin, &p.market, &config);

    // Longs dominate for a day; the EMA charges long.
    let long_id = p.open(&p.trader_a, true, 10_000 * UNIT, 3_000 * UNIT);
    p.advance(DAY);
    p.refresh_price();
    manager.update_indices(&p.keeper, &p.market);

    // The book flips hard short: instant skew −0.5, memory ≈ +0.75 — the
    // blend stays long, so the payer is now the lighter side.
    p.open(&p.trader_b, false, 30_000 * UNIT, 9_000 * UNIT);
    let market = manager.get_market(&p.market);
    assert_eq!(
        market.current_payer_side,
        abi::PayerSide::Long,
        "memory outweighs the flip"
    );

    // An hour of lighter-side paying: payer indices advance, the credit
    // flows to the heavy side, and the LP index stays exactly flat.
    let before = manager.get_market(&p.market);
    p.advance(3_600);
    p.refresh_price();
    manager.update_indices(&p.keeper, &p.market);
    let after = manager.get_market(&p.market);
    assert!(after.receiver_backed_index_long > before.receiver_backed_index_long);
    assert!(after.receiver_index_short > before.receiver_index_short);
    assert_eq!(
        after.lp_backed_index_long, before.lp_backed_index_long,
        "the share cap leaves nothing unmatched for LPs"
    );

    // The market still functions end to end — the negative-flow shape used
    // to brick every action here.
    p.close(long_id);
    assert!(manager.try_get_position(&long_id).is_err());
}

/// §3 under the EMA: checkpoint frequency cannot change accrued value
/// beyond the decay table's quantization. Exact equality holds at
/// `instant_weight = BPS` (`i18_4_split_checkpoints_equal_single_interval`);
/// at a real blend the split may drift only by cash-invisible index dust
/// (≈10 stroops at these sizes for the tolerance below).
#[test]
fn i18_4_ema_split_checkpoints_match_single_interval_within_tolerance() {
    let run = |split: bool| -> (i128, i128, i128) {
        let p = Protocol::new();
        p.disable_borrow();
        p.seed_lp();
        let manager = p.manager();
        let mut config = Protocol::market_config(100);
        config.instant_weight_bps = 3_000;
        manager.set_market_config(&p.admin, &p.market, &config);
        p.open(&p.trader_a, true, 10_000 * UNIT, 3_000 * UNIT);
        p.open(&p.trader_b, false, 2_500 * UNIT, 1_000 * UNIT);
        let intervals: &[u64] = if split {
            &[7, 991, 13_337, 85_656]
        } else {
            &[99_991]
        };
        for dt in intervals {
            p.advance(*dt);
            p.refresh_price();
            manager.update_indices(&p.keeper, &p.market);
        }
        let market = manager.get_market(&p.market);
        (
            market.receiver_backed_index_long,
            market.lp_backed_index_long,
            market.receiver_index_short,
        )
    };

    let single = run(false);
    let split = run(true);
    const TOL: i128 = 10_000;
    assert!(
        (single.0 - split.0).abs() <= TOL,
        "receiver-backed drift: {} vs {}",
        single.0,
        split.0
    );
    assert!(
        (single.1 - split.1).abs() <= TOL,
        "LP-backed drift: {} vs {}",
        single.1,
        split.1
    );
    assert!(
        (single.2 - split.2).abs() <= TOL,
        "credit drift: {} vs {}",
        single.2,
        split.2
    );
}
