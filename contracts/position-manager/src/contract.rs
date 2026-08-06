//! Entry points. Each state-changing action follows the §10.3 mutation
//! order: load the ledger and market, checkpoint global then market state,
//! apply the mutation, recompute flows and the borrow rate, store once, and
//! emit the action's event.

use shared::constants::{
    BPS, INDEX_PRECISION, ROLE_ADMIN, ROLE_KEEPER, ROLE_PAUSER, ROLE_UPGRADER,
};
use shared::{
    AccountingSnapshot, ConfigManagerClient, GlobalConfig, Market, MarketConfig, MigrationData,
    OracleRound, Position, PositionManager, RiskState, TimelockedUpgradeable, UpgradeFailure,
    VaultClient,
};
use soroban_sdk::{contract, contractimpl, panic_with_error, Address, BytesN, Env, Symbol, Vec};
use stellar_contract_utils::upgradeable::{complete_migration, ensure_can_complete_migration};

use crate::errors::PositionManagerError;
use crate::events::{self, CloseReason};
use crate::ledger::{self, Ledger};
use crate::settle::CloseSummary;
use crate::{checkpoint, fees, funding, math, risk, settle, snapshot, storage, validation};

#[contract]
pub struct PositionManagerContract;

fn require_role(env: &Env, caller: &Address, role: &str) {
    caller.require_auth();
    if !shared::has_role(env, &storage::config_manager(env), role, caller) {
        panic_with_error!(env, PositionManagerError::Unauthorized);
    }
    shared::bump_instance_ttl(env);
}

fn require_initialized(env: &Env) {
    if storage::get::<bool>(env, &storage::Key::Initialized) != Some(true) {
        panic_with_error!(env, PositionManagerError::NotInitialized);
    }
}

fn require_vault(env: &Env, caller: &Address) {
    caller.require_auth();
    if storage::get::<Address>(env, &storage::Key::Vault) != Some(caller.clone()) {
        panic_with_error!(env, PositionManagerError::InvalidCaller);
    }
}

fn require_not_paused(env: &Env) {
    if storage::is_paused(env) {
        panic_with_error!(env, PositionManagerError::Paused);
    }
}

fn load_market(env: &Env, symbol: &Symbol) -> Market {
    storage::market(env, symbol)
        .unwrap_or_else(|| panic_with_error!(env, PositionManagerError::MarketNotConfigured))
}

fn load_position(env: &Env, id: u64) -> Position {
    storage::position(env, id)
        .unwrap_or_else(|| panic_with_error!(env, PositionManagerError::PositionNotFound))
}

fn emit_close(env: &Env, summary: &CloseSummary, reason: CloseReason) {
    events::PositionClosed {
        position_id: summary.position_id,
        owner: summary.owner.clone(),
        market: summary.market.clone(),
        reason,
        size: summary.size_removed,
        price: summary.price,
        raw_pnl: summary.raw_pnl,
        payable_pnl: summary.payable_pnl,
        collateral_payout: summary.collateral_payout,
        bad_debt: summary.bad_debt,
        liquidation_reward: summary.liquidation_reward,
        execution_budget_refunded: summary.execution_budget_refunded,
        closing_fee: summary.closing_fee,
        receiver_funding_paid: summary.fees.receiver_funding_paid,
        lp_funding_paid: summary.fees.lp_funding_paid,
        borrow_paid: summary.fees.borrow_paid,
        funding_received: summary.fees.receiver_credit,
        loss_collected: summary.fees.loss_collected,
    }
    .publish(env);
}

#[contractimpl]
impl PositionManagerContract {
    pub fn __constructor(
        env: Env,
        config_manager: Address,
        oracle_router: Address,
        config: GlobalConfig,
    ) {
        validation::validate_global(&env, &config);
        storage::set(&env, &storage::Key::ConfigManager, &config_manager);
        storage::set(&env, &storage::Key::OracleRouter, &oracle_router);
        storage::set(&env, &storage::Key::GlobalConfig, &config);
        storage::set(&env, &storage::Key::Initialized, &true);
        storage::set(&env, &storage::Key::Paused, &false);
        storage::set(&env, &storage::Key::NextPositionId, &1u64);
        storage::set(
            &env,
            &storage::Key::ActiveMarkets,
            &Vec::<Symbol>::new(&env),
        );
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
        if storage::get::<Address>(&env, &storage::Key::Vault).is_some() {
            panic_with_error!(&env, PositionManagerError::AlreadyInitialized);
        }
        storage::set(&env, &storage::Key::Vault, &vault);
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
        require_initialized(&env);
        owner.require_auth();
        require_not_paused(&env);
        if size <= 0 || collateral <= 0 || execution_budget < 0 {
            panic_with_error!(&env, PositionManagerError::InvalidAmount);
        }
        if storage::is_market_disabled(&env, &market_symbol) {
            panic_with_error!(&env, PositionManagerError::MarketDisabled);
        }
        let mut market = load_market(&env, &market_symbol);
        let mut ledger = storage::ledger(&env);
        let now = env.ledger().timestamp();
        checkpoint::checkpoint_global(&env, &mut ledger, now);
        checkpoint::checkpoint_market(&env, &mut ledger, &mut market, now);
        let price = snapshot::authenticated_price(&env, &market_symbol);
        validation::check_slippage(&env, is_long, true, price, acceptable_price);
        validation::validate_orders(&env, is_long, take_profit, stop_loss, price);

        let total_transfer = math::add(&env, collateral, execution_budget);
        VaultClient::new(&env, &storage::vault(&env)).receive_collateral(
            &env.current_contract_address(),
            &owner,
            &total_transfer,
        );
        ledger.execution_budget_total =
            math::add(&env, ledger.execution_budget_total, execution_budget);
        let id: u64 = storage::get(&env, &storage::Key::NextPositionId).unwrap_or(1);
        storage::set(&env, &storage::Key::NextPositionId, &(id + 1));
        let base = math::base_added(&env, size, price);
        let risk_units = math::risk_added(&env, size, market.config.market_risk_factor_bps);
        let mut position = Position {
            id,
            owner: owner.clone(),
            market: market_symbol.clone(),
            is_long,
            size,
            base_exposure: base,
            stored_collateral: 0,
            risk_units,
            borrow_debt: 0,
            funding_paid_to_receivers_debt: 0,
            funding_paid_to_lps_debt: 0,
            funding_received_debt: 0,
            execution_budget,
            last_increased_time: now,
            take_profit,
            stop_loss,
        };
        ledger::add_stored_collateral(
            &env,
            &mut ledger,
            &mut position,
            market.side_mut(is_long),
            collateral,
        );
        if position.stored_collateral < storage::global_config(&env).min_collateral
            || position.stored_collateral < risk::initial_requirement(&env, size, &market.config)
        {
            panic_with_error!(&env, PositionManagerError::InsufficientCollateral);
        }
        let physical = ledger::physical_cash(&env);
        let equity = ledger.cash_lp_equity(&env, physical);
        risk::evaluate_market_risk(
            &env,
            &mut ledger,
            &market_symbol,
            &mut market,
            price,
            equity,
        );
        if market.side(is_long).risk_state != RiskState::Normal {
            panic_with_error!(&env, PositionManagerError::RiskStateBlocked);
        }
        let was_empty =
            market.long.size_open_interest == 0 && market.short.size_open_interest == 0;
        {
            let side = market.side_mut(is_long);
            side.size_open_interest = math::add(&env, side.size_open_interest, size);
            side.base_exposure = math::add(&env, side.base_exposure, base);
            side.risk_units = math::add(&env, side.risk_units, risk_units);
        }
        if was_empty {
            // §8.1 cold start — an empty book carries no history, and zero is
            // not "no information": it would grant a one-sided launch a
            // decaying discount. The EMA starts at the skew this open creates.
            market.skew_ema =
                math::skew_frac(&env, market.long.base_exposure, market.short.base_exposure);
        }
        ledger.total_risk_units = math::add(&env, ledger.total_risk_units, risk_units);
        risk::enforce_capacity(&env, &ledger, physical, ledger.total_risk_units);
        risk::enforce_market_limits(&env, &market, is_long);
        funding::reset_debts(&env, &ledger, &mut position, &market);
        storage::save_position(&env, &position);
        ledger.open_position_count += 1;
        funding::refresh_display(&env, &mut market);
        storage::save_market(&env, &market_symbol, &market);
        risk::refresh_rate(&env, &mut ledger, physical);
        storage::save_ledger(&env, &ledger);
        events::PositionOpened {
            position_id: id,
            owner,
            market: market_symbol,
            is_long,
            size,
            base_exposure: base,
            stored_collateral: position.stored_collateral,
            execution_budget,
            price,
            take_profit,
            stop_loss,
        }
        .publish(&env);
        id
    }

    fn increase_position(
        env: Env,
        position_id: u64,
        size_added: i128,
        collateral_added: i128,
        acceptable_price: i128,
    ) {
        require_initialized(&env);
        if size_added < 0 || collateral_added < 0 || (size_added == 0 && collateral_added == 0) {
            panic_with_error!(&env, PositionManagerError::InvalidAmount);
        }
        let mut position = load_position(&env, position_id);
        position.owner.require_auth();
        require_not_paused(&env);
        if storage::is_market_disabled(&env, &position.market) {
            panic_with_error!(&env, PositionManagerError::MarketDisabled);
        }
        let mut market = load_market(&env, &position.market);
        let mut ledger = storage::ledger(&env);
        let now = env.ledger().timestamp();
        checkpoint::checkpoint_global(&env, &mut ledger, now);
        checkpoint::checkpoint_market(&env, &mut ledger, &mut market, now);
        let price = snapshot::authenticated_price(&env, &position.market);
        validation::check_slippage(&env, position.is_long, true, price, acceptable_price);
        if collateral_added > 0 {
            VaultClient::new(&env, &storage::vault(&env)).receive_collateral(
                &env.current_contract_address(),
                &position.owner,
                &collateral_added,
            );
            let is_long = position.is_long;
            ledger::add_stored_collateral(
                &env,
                &mut ledger,
                &mut position,
                market.side_mut(is_long),
                collateral_added,
            );
        }
        let collected = fees::capitalize(&env, &mut ledger, &mut position, &mut market, 0);
        if collected.unpaid > 0 {
            panic_with_error!(&env, PositionManagerError::InsufficientCollateral);
        }
        let base = math::base_added(&env, size_added, price);
        let risk_units = math::risk_added(&env, size_added, market.config.market_risk_factor_bps);
        let physical = ledger::physical_cash(&env);
        let equity = ledger.cash_lp_equity(&env, physical);
        risk::evaluate_market_risk(
            &env,
            &mut ledger,
            &position.market,
            &mut market,
            price,
            equity,
        );
        if size_added > 0 && market.side(position.is_long).risk_state != RiskState::Normal {
            panic_with_error!(&env, PositionManagerError::RiskStateBlocked);
        }
        position.size = math::add(&env, position.size, size_added);
        position.base_exposure = math::add(&env, position.base_exposure, base);
        position.risk_units = math::add(&env, position.risk_units, risk_units);
        if size_added > 0 {
            position.last_increased_time = now;
        }
        {
            let side = market.side_mut(position.is_long);
            side.size_open_interest = math::add(&env, side.size_open_interest, size_added);
            side.base_exposure = math::add(&env, side.base_exposure, base);
            side.risk_units = math::add(&env, side.risk_units, risk_units);
        }
        ledger.total_risk_units = math::add(&env, ledger.total_risk_units, risk_units);
        risk::enforce_capacity(&env, &ledger, physical, ledger.total_risk_units);
        risk::enforce_market_limits(&env, &market, position.is_long);
        let health = math::add(
            &env,
            position.stored_collateral,
            math::pnl(
                &env,
                position.is_long,
                position.size,
                position.base_exposure,
                price,
            ),
        );
        // Adding size is held to the initial margin; a pure collateral top-up
        // only de-risks and must clear just the maintenance floor (§12.3).
        let required = if size_added > 0 {
            risk::initial_requirement(&env, position.size, &market.config)
        } else {
            risk::maintenance_requirement(&env, position.size, &market.config)
        };
        if health < required {
            panic_with_error!(&env, PositionManagerError::InsufficientCollateral);
        }
        funding::reset_debts(&env, &ledger, &mut position, &market);
        storage::save_position(&env, &position);
        funding::refresh_display(&env, &mut market);
        storage::save_market(&env, &position.market, &market);
        risk::refresh_rate(&env, &mut ledger, physical);
        storage::save_ledger(&env, &ledger);
        events::PositionIncreased {
            position_id,
            owner: position.owner.clone(),
            market: position.market.clone(),
            size_added,
            base_added: base,
            collateral_added,
            price,
            stored_collateral: position.stored_collateral,
            receiver_funding_paid: collected.receiver_funding_paid,
            lp_funding_paid: collected.lp_funding_paid,
            borrow_paid: collected.borrow_paid,
            funding_received: collected.receiver_credit,
        }
        .publish(&env);
    }

    fn decrease_position(
        env: Env,
        position_id: u64,
        size_removed: i128,
        collateral_withdrawn: i128,
        acceptable_price: i128,
    ) {
        require_initialized(&env);
        let position = load_position(&env, position_id);
        position.owner.require_auth();
        if size_removed < 0
            || size_removed > position.size
            || collateral_withdrawn < 0
            || (size_removed == 0 && collateral_withdrawn == 0)
        {
            panic_with_error!(&env, PositionManagerError::InvalidAmount);
        }
        let config = storage::global_config(&env);
        let now = env.ledger().timestamp();
        if now
            < position
                .last_increased_time
                .saturating_add(config.min_position_lifetime)
        {
            panic_with_error!(&env, PositionManagerError::TooEarly);
        }
        let mut market = load_market(&env, &position.market);
        let mut ledger = storage::ledger(&env);
        checkpoint::checkpoint_global(&env, &mut ledger, now);
        checkpoint::checkpoint_market(&env, &mut ledger, &mut market, now);
        let price = snapshot::authenticated_price(&env, &position.market);
        validation::check_slippage(&env, position.is_long, false, price, acceptable_price);
        let summary = settle::settle_close(
            &env,
            &mut ledger,
            position,
            market,
            size_removed,
            collateral_withdrawn,
            price,
            None,
        );
        storage::save_ledger(&env, &ledger);
        if summary.closed {
            emit_close(&env, &summary, CloseReason::Trader);
        } else {
            events::PositionDecreased {
                position_id,
                owner: summary.owner.clone(),
                market: summary.market.clone(),
                size_removed,
                price,
                raw_pnl: summary.raw_pnl,
                payable_pnl: summary.payable_pnl,
                realized_payout: summary.realized_payout,
                collateral_withdrawn: summary.collateral_withdrawn,
                closing_fee: summary.closing_fee,
                receiver_funding_paid: summary.fees.receiver_funding_paid,
                lp_funding_paid: summary.fees.lp_funding_paid,
                borrow_paid: summary.fees.borrow_paid,
                funding_received: summary.fees.receiver_credit,
                loss_collected: summary.fees.loss_collected,
            }
            .publish(&env);
        }
    }

    fn liquidate_position(env: Env, caller: Address, position_id: u64) {
        require_initialized(&env);
        caller.require_auth();
        let position = load_position(&env, position_id);
        let mut market = load_market(&env, &position.market);
        let mut ledger = storage::ledger(&env);
        let now = env.ledger().timestamp();
        checkpoint::checkpoint_global(&env, &mut ledger, now);
        checkpoint::checkpoint_market(&env, &mut ledger, &mut market, now);
        let price = snapshot::authenticated_price(&env, &position.market);
        let physical = ledger::physical_cash(&env);
        let equity = ledger.cash_lp_equity(&env, physical);
        risk::evaluate_market_risk(
            &env,
            &mut ledger,
            &position.market,
            &mut market,
            price,
            equity,
        );
        let pending = funding::pending_fees(&env, &ledger, &position, &market);
        let payable = settle::payable_price_pnl(
            &env,
            &ledger,
            &position,
            &market,
            position.size,
            position.base_exposure,
            price,
            physical,
        );
        let effective = math::add(
            &env,
            math::sub(
                &env,
                math::sub(
                    &env,
                    math::sub(
                        &env,
                        math::add(&env, position.stored_collateral, pending.funding_received),
                        pending.funding_paid_to_receivers,
                    ),
                    pending.funding_paid_to_lps,
                ),
                pending.borrow,
            ),
            payable,
        );
        if effective >= risk::maintenance_requirement(&env, position.size, &market.config) {
            panic_with_error!(&env, PositionManagerError::PositionHealthy);
        }
        let insolvent = effective < 0;
        let size = position.size;
        let summary = settle::settle_close(
            &env,
            &mut ledger,
            position,
            market,
            size,
            0,
            price,
            Some(&caller),
        );
        if summary.closed && insolvent {
            let reward = core::cmp::min(
                ledger.risk_keeper_reserve_total,
                storage::global_config(&env).max_insolvent_touch_reward,
            );
            if reward > 0 {
                ledger.risk_keeper_reserve_total =
                    math::sub(&env, ledger.risk_keeper_reserve_total, reward);
                VaultClient::new(&env, &storage::vault(&env)).transfer_safety_claim(
                    &env.current_contract_address(),
                    &caller,
                    &reward,
                );
                events::InsolvencyRewardPaid {
                    position_id,
                    keeper: caller,
                    amount: reward,
                }
                .publish(&env);
            }
        }
        storage::save_ledger(&env, &ledger);
        emit_close(&env, &summary, CloseReason::Liquidation);
    }

    fn deleverage_position(env: Env, caller: Address, position_id: u64) {
        require_initialized(&env);
        require_role(&env, &caller, ROLE_KEEPER);
        let position = load_position(&env, position_id);
        let mut market = load_market(&env, &position.market);
        let mut ledger = storage::ledger(&env);
        let now = env.ledger().timestamp();
        checkpoint::checkpoint_global(&env, &mut ledger, now);
        checkpoint::checkpoint_market(&env, &mut ledger, &mut market, now);
        let price = snapshot::authenticated_price(&env, &position.market);
        let physical = ledger::physical_cash(&env);
        let equity = ledger.cash_lp_equity(&env, physical);
        risk::evaluate_market_risk(
            &env,
            &mut ledger,
            &position.market,
            &mut market,
            price,
            equity,
        );
        let side_state = market.side(position.is_long).risk_state;
        if side_state != RiskState::Adl && side_state != RiskState::HardCap {
            panic_with_error!(&env, PositionManagerError::RiskStateBlocked);
        }
        if math::pnl(
            &env,
            position.is_long,
            position.size,
            position.base_exposure,
            price,
        ) <= 0
        {
            panic_with_error!(&env, PositionManagerError::RiskStateBlocked);
        }
        let size = position.size;
        let reward_bps = market.config.adl_reward_bps;
        let summary =
            settle::settle_close(&env, &mut ledger, position, market, size, 0, price, None);
        let configured_reward = math::mul_div_floor(&env, size, reward_bps as i128, BPS);
        let reward = core::cmp::min(
            core::cmp::min(
                ledger.risk_keeper_reserve_total,
                storage::global_config(&env).max_adl_reward,
            ),
            configured_reward,
        );
        if reward > 0 {
            ledger.risk_keeper_reserve_total =
                math::sub(&env, ledger.risk_keeper_reserve_total, reward);
            VaultClient::new(&env, &storage::vault(&env)).transfer_safety_claim(
                &env.current_contract_address(),
                &caller,
                &reward,
            );
            events::AdlRewardPaid {
                position_id,
                keeper: caller,
                amount: reward,
            }
            .publish(&env);
        }
        storage::save_ledger(&env, &ledger);
        emit_close(&env, &summary, CloseReason::Deleverage);
    }

    fn execute_order(env: Env, caller: Address, position_id: u64) {
        require_initialized(&env);
        caller.require_auth();
        let mut position = load_position(&env, position_id);
        let price = snapshot::authenticated_price(&env, &position.market);
        let triggered = if position.is_long {
            (position.take_profit > 0 && price >= position.take_profit)
                || (position.stop_loss > 0 && price <= position.stop_loss)
        } else {
            (position.take_profit > 0 && price <= position.take_profit)
                || (position.stop_loss > 0 && price >= position.stop_loss)
        };
        if !triggered {
            panic_with_error!(&env, PositionManagerError::InvalidOrder);
        }
        if position.execution_budget <= 0 {
            panic_with_error!(&env, PositionManagerError::InsufficientExecutionBudget);
        }
        let mut ledger = storage::ledger(&env);
        let budget = position.execution_budget;
        position.execution_budget = 0;
        storage::save_position(&env, &position);
        ledger.execution_budget_total = math::sub(&env, ledger.execution_budget_total, budget);
        VaultClient::new(&env, &storage::vault(&env)).transfer_safety_claim(
            &env.current_contract_address(),
            &caller,
            &budget,
        );
        let mut market = load_market(&env, &position.market);
        let now = env.ledger().timestamp();
        checkpoint::checkpoint_global(&env, &mut ledger, now);
        checkpoint::checkpoint_market(&env, &mut ledger, &mut market, now);
        let size = position.size;
        let summary =
            settle::settle_close(&env, &mut ledger, position, market, size, 0, price, None);
        storage::save_ledger(&env, &ledger);
        events::OrderExecuted {
            position_id,
            executor: caller,
            budget_paid: budget,
        }
        .publish(&env);
        emit_close(&env, &summary, CloseReason::Order);
    }

    fn set_tp_sl(env: Env, position_id: u64, take_profit: i128, stop_loss: i128) {
        let mut position = load_position(&env, position_id);
        position.owner.require_auth();
        let price = snapshot::authenticated_price(&env, &position.market);
        validation::validate_orders(&env, position.is_long, take_profit, stop_loss, price);
        position.take_profit = take_profit;
        position.stop_loss = stop_loss;
        storage::save_position(&env, &position);
        events::TpSlUpdated {
            position_id,
            owner: position.owner.clone(),
            market: position.market.clone(),
            take_profit,
            stop_loss,
        }
        .publish(&env);
    }

    fn fund_execution_budget(env: Env, position_id: u64, amount: i128) {
        if amount <= 0 {
            panic_with_error!(&env, PositionManagerError::InvalidAmount);
        }
        let mut position = load_position(&env, position_id);
        position.owner.require_auth();
        let mut ledger = storage::ledger(&env);
        checkpoint::checkpoint_global(&env, &mut ledger, env.ledger().timestamp());
        VaultClient::new(&env, &storage::vault(&env)).receive_collateral(
            &env.current_contract_address(),
            &position.owner,
            &amount,
        );
        position.execution_budget = math::add(&env, position.execution_budget, amount);
        ledger.execution_budget_total = math::add(&env, ledger.execution_budget_total, amount);
        storage::save_position(&env, &position);
        risk::refresh_rate(&env, &mut ledger, ledger::physical_cash(&env));
        storage::save_ledger(&env, &ledger);
        events::ExecutionBudgetFunded {
            position_id,
            amount,
        }
        .publish(&env);
    }

    fn withdraw_execution_budget(env: Env, position_id: u64, amount: i128) {
        if amount <= 0 {
            panic_with_error!(&env, PositionManagerError::InvalidAmount);
        }
        let mut position = load_position(&env, position_id);
        position.owner.require_auth();
        let mut ledger = storage::ledger(&env);
        checkpoint::checkpoint_global(&env, &mut ledger, env.ledger().timestamp());
        if amount > position.execution_budget {
            panic_with_error!(&env, PositionManagerError::InsufficientExecutionBudget);
        }
        position.execution_budget = math::sub(&env, position.execution_budget, amount);
        ledger.execution_budget_total = math::sub(&env, ledger.execution_budget_total, amount);
        storage::save_position(&env, &position);
        VaultClient::new(&env, &storage::vault(&env)).transfer_claim(
            &env.current_contract_address(),
            &position.owner,
            &amount,
            &ledger.non_lp_claims(&env),
        );
        risk::refresh_rate(&env, &mut ledger, ledger::physical_cash(&env));
        storage::save_ledger(&env, &ledger);
        events::ExecutionBudgetWithdrawn {
            position_id,
            amount,
        }
        .publish(&env);
    }

    fn update_indices(env: Env, caller: Address, market_symbol: Symbol) {
        require_role(&env, &caller, ROLE_KEEPER);
        let mut ledger = storage::ledger(&env);
        let now = env.ledger().timestamp();
        checkpoint::checkpoint_global(&env, &mut ledger, now);
        let mut market = load_market(&env, &market_symbol);
        checkpoint::checkpoint_market(&env, &mut ledger, &mut market, now);
        storage::save_market(&env, &market_symbol, &market);
        let physical = ledger::physical_cash(&env);
        risk::refresh_rate(&env, &mut ledger, physical);
        storage::save_ledger(&env, &ledger);
        events::MarketCheckpoint {
            market: market_symbol,
            receiver_backed_index_long: market.receiver_backed_index_long,
            receiver_backed_index_short: market.receiver_backed_index_short,
            lp_backed_index_long: market.lp_backed_index_long,
            lp_backed_index_short: market.lp_backed_index_short,
            receiver_index_long: market.receiver_index_long,
            receiver_index_short: market.receiver_index_short,
            current_payer_side: market.current_payer_side,
            current_payer_rate: market.current_payer_rate,
            skew_ema: market.skew_ema,
            borrow_index: ledger.borrow_index,
            current_borrow_rate: ledger.current_borrow_rate,
            timestamp: now,
        }
        .publish(&env);
    }

    fn set_global_config(env: Env, caller: Address, config: GlobalConfig) {
        require_role(&env, &caller, ROLE_ADMIN);
        validation::validate_global(&env, &config);
        let mut ledger = storage::ledger(&env);
        checkpoint::checkpoint_global(&env, &mut ledger, env.ledger().timestamp());
        let markets = storage::active_markets(&env);
        if markets.len() > config.max_active_markets {
            panic_with_error!(&env, PositionManagerError::InvalidConfig);
        }
        if risk::hard_cap_factor_sum(&env, &markets, None) > config.hard_cap_factor_limit_bps as u64
        {
            panic_with_error!(&env, PositionManagerError::InvalidConfig);
        }
        storage::set(&env, &storage::Key::GlobalConfig, &config);
        risk::refresh_rate(&env, &mut ledger, ledger::physical_cash(&env));
        storage::save_ledger(&env, &ledger);
        events::GlobalConfigUpdated { config }.publish(&env);
    }

    fn set_market_config(env: Env, caller: Address, market_symbol: Symbol, config: MarketConfig) {
        require_role(&env, &caller, ROLE_ADMIN);
        validation::validate_market(&env, &config);
        let mut ledger = storage::ledger(&env);
        let now = env.ledger().timestamp();
        checkpoint::checkpoint_global(&env, &mut ledger, now);
        let markets = storage::active_markets(&env);
        let existing = storage::market(&env, &market_symbol);
        if existing.is_none() && markets.len() >= storage::global_config(&env).max_active_markets {
            panic_with_error!(&env, PositionManagerError::MarketLimitExceeded);
        }
        let hard_sum = risk::hard_cap_factor_sum(
            &env,
            &markets,
            Some((&market_symbol, config.hard_cap_pnl_factor_bps)),
        );
        if hard_sum > storage::global_config(&env).hard_cap_factor_limit_bps as u64 {
            panic_with_error!(&env, PositionManagerError::InvalidConfig);
        }
        if let Some(mut market) = existing {
            checkpoint::checkpoint_market(&env, &mut ledger, &mut market, now);
            market.config = config.clone();
            funding::refresh_display(&env, &mut market);
            storage::save_market(&env, &market_symbol, &market);
        } else {
            let mut markets = markets;
            storage::save_market(&env, &market_symbol, &Market::new(config.clone(), now));
            markets.push_back(market_symbol.clone());
            storage::set(&env, &storage::Key::ActiveMarkets, &markets);
        }
        risk::refresh_rate(&env, &mut ledger, ledger::physical_cash(&env));
        storage::save_ledger(&env, &ledger);
        events::MarketConfigUpdated {
            market: market_symbol,
            config,
        }
        .publish(&env);
    }

    fn disable_market(env: Env, caller: Address, market: Symbol) {
        require_role(&env, &caller, ROLE_PAUSER);
        storage::set(&env, &storage::Key::MarketDisabled(market.clone()), &true);
        events::MarketStatusChanged {
            market,
            disabled: true,
        }
        .publish(&env);
    }

    fn enable_market(env: Env, caller: Address, market: Symbol) {
        require_role(&env, &caller, ROLE_PAUSER);
        storage::set(&env, &storage::Key::MarketDisabled(market.clone()), &false);
        events::MarketStatusChanged {
            market,
            disabled: false,
        }
        .publish(&env);
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
        let mut ledger = storage::ledger(&env);
        let now = env.ledger().timestamp();
        checkpoint::checkpoint_global(&env, &mut ledger, now);
        // §8.3 — the receiver liability accrues per-market, so LP pricing
        // checkpoints every active market (bounded by max_active_markets)
        // rather than trusting the keeper sweep's cadence.
        for symbol in storage::active_markets(&env).iter() {
            let mut market = load_market(&env, &symbol);
            checkpoint::checkpoint_market(&env, &mut ledger, &mut market, now);
            storage::save_market(&env, &symbol, &market);
        }
        let result = snapshot::build_snapshot(&env, &mut ledger, &round, physical, true);
        risk::refresh_rate(&env, &mut ledger, physical);
        storage::save_ledger(&env, &ledger);
        result
    }

    fn refresh_borrow_rate(env: Env, caller: Address, physical: i128) {
        require_vault(&env, &caller);
        let mut ledger = storage::ledger(&env);
        checkpoint::checkpoint_global(&env, &mut ledger, env.ledger().timestamp());
        risk::refresh_rate(&env, &mut ledger, physical);
        storage::save_ledger(&env, &ledger);
    }

    fn can_create_lp_request(env: Env, caller: Address, physical: i128) -> bool {
        require_vault(&env, &caller);
        let mut ledger = storage::ledger(&env);
        checkpoint::checkpoint_global(&env, &mut ledger, env.ledger().timestamp());
        let claims = ledger.non_lp_claims(&env);
        risk::refresh_rate(&env, &mut ledger, physical);
        storage::save_ledger(&env, &ledger);
        claims <= physical && ledger.lp_blocked_side_count == 0
    }

    fn accounting_snapshot(env: Env, round: OracleRound, physical: i128) -> AccountingSnapshot {
        let mut ledger = storage::ledger(&env);
        snapshot::build_snapshot(&env, &mut ledger, &round, physical, false)
    }

    fn get_position(env: Env, position_id: u64) -> Position {
        load_position(&env, position_id)
    }

    fn get_market(env: Env, market: Symbol) -> Market {
        load_market(&env, &market)
    }

    fn active_markets(env: Env) -> Vec<Symbol> {
        storage::active_markets(&env)
    }

    fn global_config(env: Env) -> GlobalConfig {
        storage::global_config(&env)
    }

    fn pending_receiver_funding_total(env: Env) -> i128 {
        storage::ledger(&env).pending_receiver_funding_total
    }

    fn protocol_claimable_total(env: Env) -> i128 {
        storage::ledger(&env).protocol_claimable_total
    }

    fn risk_keeper_reserve_total(env: Env) -> i128 {
        storage::ledger(&env).risk_keeper_reserve_total
    }

    fn non_lp_claims(env: Env) -> i128 {
        let ledger = storage::ledger(&env);
        ledger.non_lp_claims(&env)
    }

    fn claim_protocol(env: Env, caller: Address, recipient: Address, amount: i128) {
        require_role(&env, &caller, ROLE_ADMIN);
        let mut ledger = storage::ledger(&env);
        checkpoint::checkpoint_global(&env, &mut ledger, env.ledger().timestamp());
        if amount <= 0 || amount > ledger.protocol_claimable_total {
            panic_with_error!(&env, PositionManagerError::InvalidAmount);
        }
        ledger.protocol_claimable_total = math::sub(&env, ledger.protocol_claimable_total, amount);
        VaultClient::new(&env, &storage::vault(&env)).transfer_claim(
            &env.current_contract_address(),
            &recipient,
            &amount,
            &ledger.non_lp_claims(&env),
        );
        risk::refresh_rate(&env, &mut ledger, ledger::physical_cash(&env));
        storage::save_ledger(&env, &ledger);
        events::ProtocolClaimed { recipient, amount }.publish(&env);
    }

    fn recapitalize(env: Env, contributor: Address, amount: i128) {
        contributor.require_auth();
        if amount <= 0 {
            panic_with_error!(&env, PositionManagerError::InvalidAmount);
        }
        VaultClient::new(&env, &storage::vault(&env)).receive_collateral(
            &env.current_contract_address(),
            &contributor,
            &amount,
        );
        let mut ledger = storage::ledger(&env);
        checkpoint::checkpoint_global(&env, &mut ledger, env.ledger().timestamp());
        risk::refresh_rate(&env, &mut ledger, ledger::physical_cash(&env));
        storage::save_ledger(&env, &ledger);
        events::Recapitalized {
            contributor,
            amount,
        }
        .publish(&env);
    }

    fn pause(env: Env, caller: Address) {
        require_role(&env, &caller, ROLE_PAUSER);
        storage::set(&env, &storage::Key::Paused, &true);
        events::PauseChanged { paused: true }.publish(&env);
    }

    fn unpause(env: Env, caller: Address) {
        require_role(&env, &caller, ROLE_PAUSER);
        storage::set(&env, &storage::Key::Paused, &false);
        events::PauseChanged { paused: false }.publish(&env);
    }

    fn propose_upgrade(env: Env, caller: Address, wasm_hash: BytesN<32>) {
        <Self as TimelockedUpgradeable>::propose(&env, caller, wasm_hash);
    }

    fn cancel_upgrade(env: Env, caller: Address) {
        <Self as TimelockedUpgradeable>::cancel(&env, caller);
    }

    fn bump_position(env: Env, position_id: u64) {
        let position = load_position(&env, position_id);
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
        ConfigManagerClient::new(env, &storage::config_manager(env)).get_upgrade_timelock()
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
