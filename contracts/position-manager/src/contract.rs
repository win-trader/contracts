use crate::auth::{require_role, require_vault};
use crate::errors::PositionManagerError;
use crate::events;
use crate::ledger::{self, Ledger};
use crate::{
    borrow, funding, governance, math, position, risk, snapshot, storage, validation,
};
use position::{adl, entry, liquidate, mutation, trigger};
use shared::constants::{
    INDEX_PRECISION, PRICE_DECIMALS, ROLE_ADMIN, ROLE_ORACLE, ROLE_PAUSER, ROLE_PROTOCOL,
    ROLE_UNPAUSER, ROLE_UPGRADER,
};
use shared::{
    AccountingSnapshot, ActionOutcome, ConfigManagerClient, GlobalConfig, Market, MarketConfig,
    MigrationData, OpenPayload, PendingAction, Position, PositionManager, PriceFeedClient,
    TimelockedUpgradeable, UpgradeFailure,
};
use soroban_sdk::{contract, contractimpl, panic_with_error, Address, BytesN, Env, Symbol, Vec};
use stellar_contract_utils::upgradeable::{complete_migration, ensure_can_complete_migration};

#[contract]
pub struct PositionManagerContract;

fn require_feed_decimals(env: &Env, price_feed: &Address) {
    match PriceFeedClient::new(env, price_feed).try_decimals() {
        Ok(Ok(decimals)) if decimals == PRICE_DECIMALS => {}
        _ => panic_with_error!(env, PositionManagerError::PriceUnavailable),
    }
}

fn validate_global_against_markets(env: &Env, config: &GlobalConfig) {
    validation::validate_global(env, config);
    let markets = storage::get_active_markets(env);
    if markets.len() > config.max_active_markets {
        panic_with_error!(env, PositionManagerError::InvalidConfig);
    }
    if risk::hard_cap_factor_sum(env, &markets, None) > config.global_hard_cap_limit_bps as u64 {
        panic_with_error!(env, PositionManagerError::InvalidConfig);
    }
}

fn validate_market_against_set(env: &Env, symbol: &Symbol, config: &MarketConfig) {
    validation::validate_market(env, config);
    let markets = storage::get_active_markets(env);
    let global = storage::get_global_config(env);
    if storage::try_get_market(env, symbol).is_none() && markets.len() >= global.max_active_markets
    {
        panic_with_error!(env, PositionManagerError::MarketLimitExceeded);
    }
    let hard_sum =
        risk::hard_cap_factor_sum(env, &markets, Some((symbol, config.hard_cap_pnl_factor_bps)));
    if hard_sum > global.global_hard_cap_limit_bps as u64 {
        panic_with_error!(env, PositionManagerError::InvalidConfig);
    }
}

fn apply_global(env: &Env, actor: &Address, config: &GlobalConfig) {
    let mut ledger = storage::get_ledger(env);
    let now = env.ledger().timestamp();
    borrow::accrue(env, &mut ledger, Some(actor), now);
    // Accrue funding under the old half-life before it changes.
    for symbol in storage::get_active_markets(env).iter() {
        let mut market = storage::get_market(env, &symbol);
        funding::accrue(env, &mut ledger, &symbol, Some(actor), &mut market, now);
        storage::save_market(env, &symbol, &market);
    }
    storage::save_global_config(env, config);
    borrow::refresh_rate(env, &mut ledger, ledger::physical_cash(env));
    storage::save_ledger(env, &ledger);
    events::emit_global_config_updated(env, actor, config);
}

fn apply_market(env: &Env, actor: &Address, symbol: &Symbol, config: &MarketConfig) {
    let mut ledger = storage::get_ledger(env);
    let now = env.ledger().timestamp();
    borrow::accrue(env, &mut ledger, Some(actor), now);
    match storage::try_get_market(env, symbol) {
        Some(mut market) => {
            if config.market_risk_factor_bps != market.config.market_risk_factor_bps
                && !market_is_empty(&market)
            {
                panic_with_error!(env, PositionManagerError::MarketNotEmpty);
            }
            funding::accrue(env, &mut ledger, symbol, Some(actor), &mut market, now);
            market.config = config.clone();
            funding::refresh_display(env, &mut ledger, &mut market);
            storage::save_market(env, symbol, &market);
            if !storage::is_market_registered(env, symbol) {
                let mut markets = storage::get_active_markets(env);
                markets.push_back(symbol.clone());
                storage::save_active_markets(env, &markets);
            }
        }
        None => {
            let mut markets = storage::get_active_markets(env);
            storage::save_market(env, symbol, &Market::new(config.clone(), now));
            markets.push_back(symbol.clone());
            storage::save_active_markets(env, &markets);
        }
    }
    borrow::refresh_rate(env, &mut ledger, ledger::physical_cash(env));
    storage::save_ledger(env, &ledger);
    events::emit_market_config_updated(env, symbol, actor, config);
}

fn market_is_empty(market: &Market) -> bool {
    market.long.size_open_interest == 0
        && market.short.size_open_interest == 0
        && market.long.risk_units == 0
        && market.short.risk_units == 0
}

#[contractimpl]
impl PositionManagerContract {
    pub fn __constructor(
        env: Env,
        config_manager: Address,
        price_feed: Address,
        config: GlobalConfig,
    ) {
        validation::validate_global(&env, &config);
        storage::save_config_manager(&env, &config_manager);
        require_feed_decimals(&env, &price_feed);
        storage::save_price_feed(&env, &price_feed);
        storage::save_global_config(&env, &config);
        let initial_rate = math::mul(&env, config.base_borrow_rate_bps_day, INDEX_PRECISION);
        storage::save_ledger(&env, &Ledger::new(env.ledger().timestamp(), initial_rate));
        shared::bump_instance_ttl(&env);
    }
}

#[contractimpl]
impl PositionManager for PositionManagerContract {
    fn propose_price_feed(env: Env, caller: Address, price_feed: Address) {
        require_role(&env, &caller, ROLE_ORACLE);
        require_feed_decimals(&env, &price_feed);
        let now = env.ledger().timestamp();
        governance::store_price_feed_proposal(
            &env,
            &caller,
            &price_feed,
            governance::effective_at(&env, now),
        );
    }

    fn apply_price_feed(env: Env, caller: Address) {
        caller.require_auth();
        let price_feed = governance::take_due_price_feed(&env, env.ledger().timestamp());
        require_feed_decimals(&env, &price_feed);
        storage::save_price_feed(&env, &price_feed);
        events::emit_price_feed_changed(&env, &caller, &price_feed);
    }

    fn cancel_price_feed(env: Env, caller: Address) {
        require_role(&env, &caller, ROLE_ORACLE);
        governance::cancel_price_feed_proposal(&env, &caller);
    }

    fn price_feed(env: Env) -> Address {
        storage::get_price_feed(&env)
    }

    fn set_vault(env: Env, caller: Address, vault: Address) {
        require_role(&env, &caller, ROLE_ADMIN);
        if storage::try_get_vault(&env).is_some() {
            panic_with_error!(&env, PositionManagerError::AlreadyInitialized);
        }
        storage::save_vault(&env, &vault);
    }

    fn create_market_open(env: Env, owner: Address, market: Symbol, request: OpenPayload) -> u64 {
        entry::create_market_open(env, owner, market, request)
    }

    fn create_limit_open(
        env: Env,
        owner: Address,
        market: Symbol,
        request: OpenPayload,
        trigger_price: i128,
    ) -> u64 {
        entry::create_limit_open(env, owner, market, request, trigger_price)
    }

    fn settle_market_open(env: Env, keeper: Address, action_id: u64) -> ActionOutcome {
        entry::settle_market_open(env, keeper, action_id)
    }

    fn settle_limit_open(env: Env, keeper: Address, action_id: u64) -> ActionOutcome {
        entry::settle_limit_open(env, keeper, action_id)
    }

    fn cancel_limit_open(env: Env, action_id: u64) -> i128 {
        entry::cancel_limit_open(env, action_id)
    }

    fn clean_expired_entry(env: Env, keeper: Address, action_id: u64) {
        entry::clean_expired_entry(env, keeper, action_id)
    }

    fn add_collateral(env: Env, position_id: u64, amount: i128) {
        mutation::add_collateral(env, position_id, amount)
    }

    fn create_increase(
        env: Env,
        position_id: u64,
        size_added: i128,
        collateral_added: i128,
        acceptable_price: i128,
    ) -> u64 {
        mutation::create_increase(
            env,
            position_id,
            size_added,
            collateral_added,
            acceptable_price,
        )
    }

    fn create_decrease(
        env: Env,
        position_id: u64,
        size_removed: i128,
        acceptable_price: i128,
    ) -> u64 {
        mutation::create_decrease(env, position_id, size_removed, acceptable_price)
    }

    fn create_close(env: Env, position_id: u64, acceptable_price: i128) -> u64 {
        mutation::create_close(env, position_id, acceptable_price)
    }

    fn settle_increase(env: Env, keeper: Address, action_id: u64) -> ActionOutcome {
        mutation::settle_increase(env, keeper, action_id)
    }

    fn settle_decrease(env: Env, keeper: Address, action_id: u64) -> ActionOutcome {
        mutation::settle_decrease(env, keeper, action_id)
    }

    fn settle_close(env: Env, keeper: Address, action_id: u64) -> ActionOutcome {
        mutation::settle_close(env, keeper, action_id)
    }

    fn set_take_profit(env: Env, position_id: u64, trigger_price: i128, acceptable_price: i128) {
        trigger::set_take_profit(env, position_id, trigger_price, acceptable_price)
    }

    fn clear_take_profit(env: Env, position_id: u64) {
        trigger::clear_take_profit(env, position_id)
    }

    fn set_stop_loss(env: Env, position_id: u64, trigger_price: i128, acceptable_price: i128) {
        trigger::set_stop_loss(env, position_id, trigger_price, acceptable_price)
    }

    fn clear_stop_loss(env: Env, position_id: u64) {
        trigger::clear_stop_loss(env, position_id)
    }

    fn execute_take_profit(env: Env, keeper: Address, position_id: u64) -> ActionOutcome {
        trigger::execute_take_profit(env, keeper, position_id)
    }

    fn execute_stop_loss(env: Env, keeper: Address, position_id: u64) -> ActionOutcome {
        trigger::execute_stop_loss(env, keeper, position_id)
    }

    fn liquidate_position(env: Env, keeper: Address, position_id: u64) {
        liquidate::liquidate_position(env, keeper, position_id)
    }

    fn execute_adl(env: Env, keeper: Address, position_id: u64) -> ActionOutcome {
        adl::execute_adl(env, keeper, position_id)
    }

    fn get_pending_action(env: Env, action_id: u64) -> PendingAction {
        storage::get_pending_action(&env, action_id)
    }

    fn update_indices(env: Env, caller: Address, market_symbol: Symbol) {
        caller.require_auth();
        let mut ledger = storage::get_ledger(&env);
        let now = env.ledger().timestamp();
        borrow::accrue(&env, &mut ledger, Some(&caller), now);
        let mut market = storage::get_market(&env, &market_symbol);
        funding::accrue(&env, &mut ledger, &market_symbol, Some(&caller), &mut market, now);
        storage::save_market(&env, &market_symbol, &market);
        let physical = ledger::physical_cash(&env);
        borrow::refresh_rate(&env, &mut ledger, physical);
        storage::save_ledger(&env, &ledger);
        events::emit_market_checkpoint(&env, &market_symbol, &caller, &market, &ledger, now);
    }

    fn propose_global_config(env: Env, caller: Address, config: GlobalConfig) {
        governance::require_configuration_authority(&env, &caller);
        validate_global_against_markets(&env, &config);
        let now = env.ledger().timestamp();
        if governance::global_is_conservative(&storage::get_global_config(&env), &config) {
            apply_global(&env, &caller, &config);
            return;
        }
        governance::store_global_proposal(&env, &caller, &config, governance::effective_at(&env, now));
    }

    fn apply_global_config(env: Env, caller: Address) {
        caller.require_auth();
        let now = env.ledger().timestamp();
        let config = governance::take_due_global_proposal(&env, now);
        validate_global_against_markets(&env, &config);
        apply_global(&env, &caller, &config);
    }

    fn cancel_global_config(env: Env, caller: Address) {
        governance::require_configuration_authority(&env, &caller);
        governance::cancel_global_proposal(&env, &caller);
    }

    fn propose_market_config(env: Env, caller: Address, market_symbol: Symbol, config: MarketConfig) {
        governance::require_configuration_authority(&env, &caller);
        validate_market_against_set(&env, &market_symbol, &config);
        let existing = storage::try_get_market(&env, &market_symbol);
        let exempt = match &existing {
            None => true,
            Some(_) if !storage::is_market_registered(&env, &market_symbol) => true,
            Some(market) => governance::market_is_conservative(&market.config, &config),
        };
        if exempt {
            apply_market(&env, &caller, &market_symbol, &config);
            return;
        }
        let now = env.ledger().timestamp();
        governance::store_market_proposal(
            &env,
            &caller,
            &market_symbol,
            &config,
            governance::effective_at(&env, now),
        );
    }

    fn apply_market_config(env: Env, caller: Address, market_symbol: Symbol) {
        caller.require_auth();
        let now = env.ledger().timestamp();
        let config = governance::take_due_market_proposal(&env, &market_symbol, now);
        validate_market_against_set(&env, &market_symbol, &config);
        apply_market(&env, &caller, &market_symbol, &config);
    }

    fn cancel_market_config(env: Env, caller: Address, market_symbol: Symbol) {
        governance::require_configuration_authority(&env, &caller);
        governance::cancel_market_proposal(&env, &caller, &market_symbol);
    }

    fn deregister_market(env: Env, caller: Address, market_symbol: Symbol) {
        governance::require_configuration_authority(&env, &caller);
        let market = storage::get_market(&env, &market_symbol);
        let empty = market.long.size_open_interest == 0
            && market.short.size_open_interest == 0
            && market.long.base_exposure == 0
            && market.short.base_exposure == 0
            && market.long.risk_units == 0
            && market.short.risk_units == 0
            && market.pending_receiver_funding == 0
            && market.long.risk_state == shared::RiskState::Normal
            && market.short.risk_state == shared::RiskState::Normal;
        if !empty {
            panic_with_error!(&env, PositionManagerError::MarketNotEmpty);
        }
        let markets = storage::get_active_markets(&env);
        let mut remaining = Vec::<Symbol>::new(&env);
        for symbol in markets.iter() {
            if symbol != market_symbol {
                remaining.push_back(symbol);
            }
        }
        storage::save_active_markets(&env, &remaining);
        events::emit_market_status_changed(&env, &market_symbol, &caller, true);
    }

    fn disable_market(env: Env, caller: Address, market: Symbol) {
        require_role(&env, &caller, ROLE_PAUSER);
        storage::get_market(&env, &market);
        storage::set_market_disabled(&env, &market, true);
        events::emit_market_status_changed(&env, &market, &caller, true);
    }

    fn enable_market(env: Env, caller: Address, market: Symbol) {
        require_role(&env, &caller, ROLE_UNPAUSER);
        storage::set_market_disabled(&env, &market, false);
        events::emit_market_status_changed(&env, &market, &caller, false);
    }

    fn is_market_disabled(env: Env, market: Symbol) -> bool {
        storage::is_market_disabled(&env, &market)
    }

    fn prepare_lp_snapshot(env: Env, caller: Address, physical: i128) -> AccountingSnapshot {
        require_vault(&env, &caller);
        let mut ledger = storage::get_ledger(&env);
        let now = env.ledger().timestamp();
        borrow::accrue(&env, &mut ledger, Some(&caller), now);
        for symbol in storage::get_active_markets(&env).iter() {
            let mut market = storage::get_market(&env, &symbol);
            funding::accrue(&env, &mut ledger, &symbol, Some(&caller), &mut market, now);
            storage::save_market(&env, &symbol, &market);
        }
        let result = snapshot::build_snapshot(&env, &mut ledger, &caller, physical, true);
        borrow::refresh_rate(&env, &mut ledger, physical);
        storage::save_ledger(&env, &ledger);
        result
    }

    fn refresh_borrow_rate(env: Env, caller: Address, physical: i128) {
        require_vault(&env, &caller);
        let mut ledger = storage::get_ledger(&env);
        borrow::accrue(&env, &mut ledger, Some(&caller), env.ledger().timestamp());
        borrow::refresh_rate(&env, &mut ledger, physical);
        storage::save_ledger(&env, &ledger);
    }

    fn is_paused(env: Env) -> bool {
        storage::is_paused(&env)
    }

    fn accounting_snapshot(env: Env, physical: i128) -> AccountingSnapshot {
        let mut ledger = storage::get_ledger(&env);
        let reader = env.current_contract_address();
        snapshot::build_snapshot(&env, &mut ledger, &reader, physical, false)
    }

    fn get_position(env: Env, position_id: u64) -> Position {
        storage::get_position(&env, position_id)
    }

    fn pending_fees(env: Env, position_id: u64, now: u64) -> shared::PendingFeesView {
        let position = storage::get_position(&env, position_id);
        let market = storage::get_market(&env, &position.market);
        let ledger = storage::get_ledger(&env);
        let pending = funding::preview_pending_fees(&env, &ledger, &position, &market, now);
        shared::PendingFeesView {
            funding_paid_to_receivers: pending.funding_paid_to_receivers,
            funding_paid_to_lps: pending.funding_paid_to_lps,
            funding_received: pending.funding_received,
            borrow: pending.borrow,
        }
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

    fn non_lp_claims(env: Env) -> i128 {
        let ledger = storage::get_ledger(&env);
        ledger.non_lp_claims(&env)
    }

    fn claim_protocol(env: Env, caller: Address, recipient: Address, amount: i128) {
        require_role(&env, &caller, ROLE_PROTOCOL);
        if storage::is_paused(&env) {
            panic_with_error!(&env, PositionManagerError::Paused);
        }
        let mut ledger = storage::get_ledger(&env);
        borrow::accrue(&env, &mut ledger, Some(&caller), env.ledger().timestamp());
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
        events::emit_protocol_claimed(&env, &caller, &recipient, amount);
    }

    fn recapitalize(env: Env, contributor: Address, amount: i128) {
        contributor.require_auth();
        if amount <= 0 {
            panic_with_error!(&env, PositionManagerError::InvalidAmount);
        }
        ledger::receive(&env, &contributor, amount);
        let mut ledger = storage::get_ledger(&env);
        borrow::accrue(&env, &mut ledger, Some(&contributor), env.ledger().timestamp());
        borrow::refresh_rate(&env, &mut ledger, ledger::physical_cash(&env));
        storage::save_ledger(&env, &ledger);
        events::emit_recapitalized(&env, &contributor, amount);
    }

    fn claim_payout(env: Env, owner: Address) -> i128 {
        owner.require_auth();
        let mut ledger = storage::get_ledger(&env);
        borrow::accrue(&env, &mut ledger, Some(&owner), env.ledger().timestamp());
        let amount = ledger::claim_unclaimed_payout(&env, &mut ledger, &owner);
        borrow::refresh_rate(&env, &mut ledger, ledger::physical_cash(&env));
        storage::save_ledger(&env, &ledger);
        events::emit_payout_claimed(&env, &owner, amount);
        amount
    }

    fn unclaimed_payout(env: Env, owner: Address) -> i128 {
        storage::get_unclaimed_payout(&env, &owner)
    }

    fn unclaimed_payout_total(env: Env) -> i128 {
        storage::get_ledger(&env).unclaimed_payout_total
    }

    fn pause(env: Env, caller: Address) {
        require_role(&env, &caller, ROLE_PAUSER);
        storage::save_paused(&env, true);
        events::emit_pause_changed(&env, &caller, true);
    }

    fn unpause(env: Env, caller: Address) {
        require_role(&env, &caller, ROLE_UNPAUSER);
        storage::save_paused(&env, false);
        events::emit_pause_changed(&env, &caller, false);
    }

    fn propose_upgrade(env: Env, caller: Address, wasm_hash: BytesN<32>) {
        <Self as TimelockedUpgradeable>::propose(&env, caller, wasm_hash);
    }

    fn cancel_upgrade(env: Env, caller: Address) {
        <Self as TimelockedUpgradeable>::cancel(&env, caller);
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
        if migration_data.version != ledger::STATE_VERSION {
            panic_with_error!(&env, PositionManagerError::StateVersionMismatch);
        }
        let mut ledger = storage::get_ledger_unguarded(&env);
        ledger.state_version = ledger::STATE_VERSION;
        storage::save_ledger(&env, &ledger);
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
        let upgrade =
            ConfigManagerClient::new(env, &storage::get_config_manager(env)).get_upgrade_timelock();
        core::cmp::max(upgrade, storage::get_global_config(env).config_timelock_seconds)
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
