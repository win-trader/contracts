//! Entry points. Each state-changing action follows the §10.3 mutation
//! order: load the ledger and market, checkpoint global then market state,
//! apply the mutation, recompute flows and the borrow rate, store once, and
//! emit the action's event.

use crate::auth::{require_initialized, require_role, require_vault};
use crate::errors::PositionManagerError;
use crate::events;
use crate::ledger::{self, Ledger};
use crate::{borrow, funding, math, position, referral, risk, snapshot, storage, validation};
use position::{
    decrease::decrease_position,
    deleverage::deleverage_position,
    entry_order::{cancel_entry_order, execute_entry_order, place_entry_order},
    execute_order::execute_order,
    fund_execution_budget::fund_execution_budget,
    increase::increase_position,
    liquidate::liquidate_position,
    open::open_position,
    set_tp_sl::set_tp_sl,
    withdraw_execution_budget::withdraw_execution_budget,
};
use shared::constants::{INDEX_PRECISION, ROLE_ADMIN, ROLE_KEEPER, ROLE_PAUSER, ROLE_UPGRADER};
use shared::{
    AccountingSnapshot, ConfigManagerClient, EntryOrderParams, GlobalConfig, Market, MarketConfig,
    MigrationData, OracleRound, Position, PositionManager, TimelockedUpgradeable, UpgradeFailure,
};
use soroban_sdk::{contract, contractimpl, panic_with_error, Address, BytesN, Env, Symbol, Vec};
use stellar_contract_utils::upgradeable::{complete_migration, ensure_can_complete_migration};

#[contract]
pub struct PositionManagerContract;

#[contractimpl]
impl PositionManagerContract {
    pub fn __constructor(
        env: Env,
        config_manager: Address,
        oracle_router: Address,
        config: GlobalConfig,
    ) {
        validation::validate_global(&env, &config);
        storage::save_config_manager(&env, &config_manager);
        storage::save_oracle_router(&env, &oracle_router);
        storage::save_global_config(&env, &config);
        storage::save_initialized(&env);
        storage::save_paused(&env, false);
        storage::save_next_position_id(&env, 1);
        storage::save_active_markets(&env, &Vec::<Symbol>::new(&env));
        let initial_rate = math::mul(&env, config.base_borrow_rate_bps_day, INDEX_PRECISION);
        storage::save_ledger(&env, &Ledger::new(env.ledger().timestamp(), initial_rate));
        shared::bump_instance_ttl(&env);
    }
}

#[contractimpl]
impl PositionManager for PositionManagerContract {
    fn set_vault(env: Env, caller: Address, vault: Address) {
        require_initialized(&env);
        require_role(&env, &caller, ROLE_ADMIN);
        if storage::try_get_vault(&env).is_some() {
            panic_with_error!(&env, PositionManagerError::AlreadyInitialized);
        }
        storage::save_vault(&env, &vault);
    }

    fn open_position(
        env: Env,
        owner: Address,
        market_symbol: Symbol,
        is_long: bool,
        size: i128,
        collateral: i128,
        execution_budget: i128,
        take_profit: i128,
        stop_loss: i128,
        acceptable_price: i128,
    ) -> u64 {
        open_position(
            env,
            owner,
            market_symbol,
            is_long,
            size,
            collateral,
            execution_budget,
            take_profit,
            stop_loss,
            acceptable_price,
        )
    }

    fn increase_position(
        env: Env,
        position_id: u64,
        size_added: i128,
        collateral_added: i128,
        acceptable_price: i128,
    ) {
        increase_position(
            env,
            position_id,
            size_added,
            collateral_added,
            acceptable_price,
        )
    }

    fn decrease_position(
        env: Env,
        position_id: u64,
        size_removed: i128,
        collateral_withdrawn: i128,
        acceptable_price: i128,
    ) {
        decrease_position(
            env,
            position_id,
            size_removed,
            collateral_withdrawn,
            acceptable_price,
        )
    }

    fn liquidate_position(env: Env, caller: Address, position_id: u64) {
        liquidate_position(env, caller, position_id)
    }

    fn deleverage_position(env: Env, caller: Address, position_id: u64) {
        deleverage_position(env, caller, position_id)
    }

    fn execute_order(env: Env, caller: Address, position_id: u64) {
        execute_order(env, caller, position_id)
    }

    fn set_tp_sl(env: Env, position_id: u64, take_profit: i128, stop_loss: i128) {
        set_tp_sl(env, position_id, take_profit, stop_loss)
    }

    fn fund_execution_budget(env: Env, position_id: u64, amount: i128) {
        fund_execution_budget(env, position_id, amount)
    }

    fn withdraw_execution_budget(env: Env, position_id: u64, amount: i128) {
        withdraw_execution_budget(env, position_id, amount)
    }

    fn place_entry_order(
        env: Env,
        owner: Address,
        market: Symbol,
        params: EntryOrderParams,
    ) -> u64 {
        place_entry_order(env, owner, market, params)
    }

    fn execute_entry_order(env: Env, caller: Address, order_id: u64) {
        execute_entry_order(env, caller, order_id)
    }

    fn cancel_entry_order(env: Env, order_id: u64) {
        cancel_entry_order(env, order_id)
    }

    fn get_entry_order(env: Env, order_id: u64) -> shared::EntryOrder {
        storage::get_entry_order(&env, order_id)
    }

    fn register_referral_code(env: Env, owner: Address, code: Symbol) {
        referral::register_code(env, owner, code)
    }

    fn set_referrer(env: Env, trader: Address, code: Symbol) {
        referral::set_referrer(env, trader, code)
    }

    fn claim_referral(env: Env, referrer: Address) {
        referral::claim(env, referrer)
    }

    fn get_referrer(env: Env, trader: Address) -> Option<Address> {
        storage::get_referrer(&env, &trader)
    }

    fn referral_code_owner(env: Env, code: Symbol) -> Option<Address> {
        storage::try_get_referral_code_owner(&env, &code)
    }

    fn referral_balance(env: Env, referrer: Address) -> i128 {
        storage::get_referral_balance(&env, &referrer)
    }

    fn referral_claimable_total(env: Env) -> i128 {
        storage::get_ledger(&env).referral_claimable_total
    }

    fn update_indices(env: Env, caller: Address, market_symbol: Symbol) {
        require_role(&env, &caller, ROLE_KEEPER);
        let mut ledger = storage::get_ledger(&env);
        let now = env.ledger().timestamp();
        borrow::accrue(&env, &mut ledger, now);
        let mut market = storage::get_market(&env, &market_symbol);
        funding::accrue(&env, &mut ledger, &mut market, now);
        storage::save_market(&env, &market_symbol, &market);
        let physical = ledger::physical_cash(&env);
        borrow::refresh_rate(&env, &mut ledger, physical);
        storage::save_ledger(&env, &ledger);
        events::emit_market_checkpoint(&env, &market_symbol, &market, &ledger, now);
    }

    fn set_global_config(env: Env, caller: Address, config: GlobalConfig) {
        require_role(&env, &caller, ROLE_ADMIN);
        validation::validate_global(&env, &config);
        let mut ledger = storage::get_ledger(&env);
        borrow::accrue(&env, &mut ledger, env.ledger().timestamp());
        let markets = storage::get_active_markets(&env);
        if markets.len() > config.max_active_markets {
            panic_with_error!(&env, PositionManagerError::InvalidConfig);
        }
        if risk::hard_cap_factor_sum(&env, &markets, None) > config.hard_cap_factor_limit_bps as u64
        {
            panic_with_error!(&env, PositionManagerError::InvalidConfig);
        }
        storage::save_global_config(&env, &config);
        borrow::refresh_rate(&env, &mut ledger, ledger::physical_cash(&env));
        storage::save_ledger(&env, &ledger);
        events::emit_global_config_updated(&env, &config);
    }

    fn set_market_config(env: Env, caller: Address, market_symbol: Symbol, config: MarketConfig) {
        require_role(&env, &caller, ROLE_ADMIN);
        validation::validate_market(&env, &config);
        let mut ledger = storage::get_ledger(&env);
        let now = env.ledger().timestamp();
        borrow::accrue(&env, &mut ledger, now);
        let markets = storage::get_active_markets(&env);
        let existing = storage::try_get_market(&env, &market_symbol);
        if existing.is_none()
            && markets.len() >= storage::get_global_config(&env).max_active_markets
        {
            panic_with_error!(&env, PositionManagerError::MarketLimitExceeded);
        }
        let hard_sum = risk::hard_cap_factor_sum(
            &env,
            &markets,
            Some((&market_symbol, config.hard_cap_pnl_factor_bps)),
        );
        if hard_sum > storage::get_global_config(&env).hard_cap_factor_limit_bps as u64 {
            panic_with_error!(&env, PositionManagerError::InvalidConfig);
        }
        if let Some(mut market) = existing {
            funding::accrue(&env, &mut ledger, &mut market, now);
            market.config = config.clone();
            funding::refresh_display(&env, &mut market);
            storage::save_market(&env, &market_symbol, &market);
        } else {
            let mut markets = markets;
            storage::save_market(&env, &market_symbol, &Market::new(config.clone(), now));
            markets.push_back(market_symbol.clone());
            storage::save_active_markets(&env, &markets);
        }
        borrow::refresh_rate(&env, &mut ledger, ledger::physical_cash(&env));
        storage::save_ledger(&env, &ledger);
        events::emit_market_config_updated(&env, &market_symbol, &config);
    }

    fn disable_market(env: Env, caller: Address, market: Symbol) {
        require_role(&env, &caller, ROLE_PAUSER);
        storage::set_market_disabled(&env, &market, true);
        events::emit_market_status_changed(&env, &market, true);
    }

    fn enable_market(env: Env, caller: Address, market: Symbol) {
        require_role(&env, &caller, ROLE_PAUSER);
        storage::set_market_disabled(&env, &market, false);
        events::emit_market_status_changed(&env, &market, false);
    }

    fn is_market_disabled(env: Env, market: Symbol) -> bool {
        storage::is_market_disabled(&env, &market)
    }

    fn prepare_lp_snapshot(
        env: Env,
        caller: Address,
        round: OracleRound,
        physical: i128,
    ) -> AccountingSnapshot {
        require_vault(&env, &caller);
        let mut ledger = storage::get_ledger(&env);
        let now = env.ledger().timestamp();
        borrow::accrue(&env, &mut ledger, now);
        // §8.3 — the receiver liability accrues per-market, so LP pricing
        // checkpoints every active market (bounded by max_active_markets)
        // rather than trusting the keeper sweep's cadence.
        for symbol in storage::get_active_markets(&env).iter() {
            let mut market = storage::get_market(&env, &symbol);
            funding::accrue(&env, &mut ledger, &mut market, now);
            storage::save_market(&env, &symbol, &market);
        }
        let result = snapshot::build_snapshot(&env, &mut ledger, &round, physical, true);
        borrow::refresh_rate(&env, &mut ledger, physical);
        storage::save_ledger(&env, &ledger);
        result
    }

    fn refresh_borrow_rate(env: Env, caller: Address, physical: i128) {
        require_vault(&env, &caller);
        let mut ledger = storage::get_ledger(&env);
        borrow::accrue(&env, &mut ledger, env.ledger().timestamp());
        borrow::refresh_rate(&env, &mut ledger, physical);
        storage::save_ledger(&env, &ledger);
    }

    fn can_create_lp_request(env: Env, caller: Address, physical: i128) -> bool {
        require_vault(&env, &caller);
        let mut ledger = storage::get_ledger(&env);
        borrow::accrue(&env, &mut ledger, env.ledger().timestamp());
        let claims = ledger.non_lp_claims(&env);
        borrow::refresh_rate(&env, &mut ledger, physical);
        storage::save_ledger(&env, &ledger);
        claims <= physical && ledger.lp_blocked_side_count == 0
    }

    fn accounting_snapshot(env: Env, round: OracleRound, physical: i128) -> AccountingSnapshot {
        let mut ledger = storage::get_ledger(&env);
        snapshot::build_snapshot(&env, &mut ledger, &round, physical, false)
    }

    fn get_position(env: Env, position_id: u64) -> Position {
        storage::get_position(&env, position_id)
    }

    fn get_market(env: Env, market: Symbol) -> Market {
        storage::get_market(&env, &market)
    }

    fn active_markets(env: Env) -> Vec<Symbol> {
        storage::get_active_markets(&env)
    }

    fn global_config(env: Env) -> GlobalConfig {
        storage::get_global_config(&env)
    }

    fn pending_receiver_funding_total(env: Env) -> i128 {
        storage::get_ledger(&env).pending_receiver_funding_total
    }

    fn protocol_claimable_total(env: Env) -> i128 {
        storage::get_ledger(&env).protocol_claimable_total
    }

    fn risk_keeper_reserve_total(env: Env) -> i128 {
        storage::get_ledger(&env).risk_keeper_reserve_total
    }

    fn non_lp_claims(env: Env) -> i128 {
        let ledger = storage::get_ledger(&env);
        ledger.non_lp_claims(&env)
    }

    fn claim_protocol(env: Env, caller: Address, recipient: Address, amount: i128) {
        require_role(&env, &caller, ROLE_ADMIN);
        let mut ledger = storage::get_ledger(&env);
        borrow::accrue(&env, &mut ledger, env.ledger().timestamp());
        if amount <= 0 || amount > ledger.protocol_claimable_total {
            panic_with_error!(&env, PositionManagerError::InvalidAmount);
        }
        ledger::payout_checked(
            &env,
            &mut ledger,
            ledger::Bucket::ProtocolClaimable,
            &recipient,
            amount,
        );
        borrow::refresh_rate(&env, &mut ledger, ledger::physical_cash(&env));
        storage::save_ledger(&env, &ledger);
        events::emit_protocol_claimed(&env, &recipient, amount);
    }

    fn recapitalize(env: Env, contributor: Address, amount: i128) {
        contributor.require_auth();
        if amount <= 0 {
            panic_with_error!(&env, PositionManagerError::InvalidAmount);
        }
        // Deliberately labels no bucket: a recapitalization is a pure
        // LP-equity donation.
        ledger::receive(&env, &contributor, amount);
        let mut ledger = storage::get_ledger(&env);
        borrow::accrue(&env, &mut ledger, env.ledger().timestamp());
        borrow::refresh_rate(&env, &mut ledger, ledger::physical_cash(&env));
        storage::save_ledger(&env, &ledger);
        events::emit_recapitalized(&env, &contributor, amount);
    }

    fn pause(env: Env, caller: Address) {
        require_role(&env, &caller, ROLE_PAUSER);
        storage::save_paused(&env, true);
        events::emit_pause_changed(&env, true);
    }

    fn unpause(env: Env, caller: Address) {
        require_role(&env, &caller, ROLE_PAUSER);
        storage::save_paused(&env, false);
        events::emit_pause_changed(&env, false);
    }

    fn propose_upgrade(env: Env, caller: Address, wasm_hash: BytesN<32>) {
        <Self as TimelockedUpgradeable>::propose(&env, caller, wasm_hash);
    }

    fn cancel_upgrade(env: Env, caller: Address) {
        <Self as TimelockedUpgradeable>::cancel(&env, caller);
    }

    fn bump_position(env: Env, position_id: u64) {
        let position = storage::get_position(&env, position_id);
        storage::save_position(&env, &position);
        shared::bump_instance_ttl(&env);
    }
}

#[contractimpl]
impl PositionManagerContract {
    pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>, operator: Address) {
        <Self as TimelockedUpgradeable>::execute(&env, operator, new_wasm_hash);
    }

    pub fn migrate(env: Env, migration_data: MigrationData, operator: Address) {
        require_role(&env, &operator, ROLE_UPGRADER);
        ensure_can_complete_migration(&env);
        storage::save_version(&env, migration_data.version);
        complete_migration(&env);
    }
}

impl TimelockedUpgradeable for PositionManagerContract {
    fn _require_proposer(env: &Env, caller: &Address) {
        require_role(env, caller, ROLE_UPGRADER);
    }
    fn _require_executor(env: &Env, caller: &Address) {
        require_role(env, caller, ROLE_UPGRADER);
    }
    fn _require_canceller(env: &Env, caller: &Address) {
        require_role(env, caller, ROLE_PAUSER);
    }
    fn _timelock_seconds(env: &Env) -> u64 {
        ConfigManagerClient::new(env, &storage::get_config_manager(env)).get_upgrade_timelock()
    }
    fn _panic_with_upgrade_error(env: &Env, failure: UpgradeFailure) -> ! {
        match failure {
            UpgradeFailure::NoPendingUpgrade => {
                panic_with_error!(env, PositionManagerError::UpgradeNoPending)
            }
            UpgradeFailure::TimelockNotElapsed => {
                panic_with_error!(env, PositionManagerError::UpgradeTimelockNotElapsed)
            }
            UpgradeFailure::HashMismatch => {
                panic_with_error!(env, PositionManagerError::UpgradeHashMismatch)
            }
        }
    }
}
