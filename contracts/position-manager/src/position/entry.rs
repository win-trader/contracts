use soroban_sdk::{panic_with_error, Address, Env, Symbol};

use shared::constants::BPS;
use shared::{
    ActionKind, ActionOutcome, ActionPayload, FailureReason, GlobalConfig, Market, OpenPayload,
    PendingAction, Position, TriggerCondition,
};

use crate::auth::require_market_active;
use crate::errors::PositionManagerError;
use crate::events::{self, FeeSource};
use crate::ledger::{self, Ledger};
use crate::math::AddedExposure;
use crate::{action, borrow, fees, funding, keeper, math, risk, snapshot, storage, validation};

use super::trigger;

struct EntryKind {
    kind: ActionKind,
    reward: keeper::RewardKind,
    resting: bool,
}

const MARKET: EntryKind = EntryKind {
    kind: ActionKind::MarketOpen,
    reward: keeper::RewardKind::MarketOpen,
    resting: false,
};

const LIMIT: EntryKind = EntryKind {
    kind: ActionKind::LimitOpen,
    reward: keeper::RewardKind::LimitOpen,
    resting: true,
};

pub fn create_market_open(env: Env, owner: Address, market: Symbol, request: OpenPayload) -> u64 {
    create_entry(&env, owner, market, request, 0, &MARKET)
}

pub fn create_limit_open(
    env: Env,
    owner: Address,
    market: Symbol,
    request: OpenPayload,
    trigger_price: i128,
) -> u64 {
    if trigger_price <= 0 {
        panic_with_error!(&env, PositionManagerError::InvalidOrder);
    }
    create_entry(&env, owner, market, request, trigger_price, &LIMIT)
}

fn create_entry(
    env: &Env,
    owner: Address,
    market_symbol: Symbol,
    request: OpenPayload,
    trigger_price: i128,
    entry: &EntryKind,
) -> u64 {
    owner.require_auth();
    require_market_active(env, &market_symbol);
    if !storage::is_market_registered(env, &market_symbol) {
        panic_with_error!(env, PositionManagerError::MarketNotConfigured);
    }
    if request.size <= 0 || request.submitted_collateral <= 0 {
        panic_with_error!(env, PositionManagerError::InvalidAmount);
    }

    let market = storage::get_market(env, &market_symbol);
    let config = storage::get_global_config(env);
    let now = env.ledger().timestamp();
    let commit = snapshot::read_stamped_price(env, &market_symbol);

    if storage::is_paused(env) {
        panic_with_error!(env, PositionManagerError::Paused);
    }
    if !risk::side_accepts_new_exposure(env, market.side(request.is_long)) {
        panic_with_error!(env, PositionManagerError::RiskStateBlocked);
    }

    let reference_price = if entry.resting {
        trigger_price
    } else {
        commit.price
    };
    validation::validate_orders(
        env,
        request.is_long,
        request.take_profit,
        request.stop_loss,
        reference_price,
    );

    let execute_after = now.saturating_add(market.config.order_execution_delay_seconds);
    let max_lifetime = if entry.resting {
        config.max_order_lifetime_seconds
    } else {
        config.max_market_order_lifetime
    };
    if request.expires_at <= execute_after
        || request.expires_at > now.saturating_add(max_lifetime)
    {
        panic_with_error!(env, PositionManagerError::InvalidOrder);
    }

    if math::base_added(env, request.size, commit.price, request.is_long) <= 0
        || math::risk_units_for(env, request.size, market.config.market_risk_factor_bps) <= 0
    {
        panic_with_error!(env, PositionManagerError::InvalidAmount);
    }

    let opening_fee = fees::calculate_opening_fee(env, request.size, &market.config);
    let reward = keeper::reward_for(&config, entry.reward);
    let after_charges = math::sub(
        env,
        math::sub(env, request.submitted_collateral, opening_fee),
        reward,
    );
    if after_charges < config.min_collateral
        || request.submitted_collateral < config.keeper_rewards.expiry
        || after_charges < risk::initial_requirement(env, request.size, &market.config)
    {
        panic_with_error!(env, PositionManagerError::InsufficientCollateral);
    }

    let mut ledger = storage::get_ledger(env);
    borrow::accrue(env, &mut ledger, Some(&owner), now);
    ledger::escrow_in(env, &mut ledger, &owner, request.submitted_collateral);

    let payload = if entry.resting {
        ActionPayload::LimitOpen(
            request.clone(),
            TriggerCondition {
                trigger_price,
                trigger_above: trigger_price >= commit.price,
            },
        )
    } else {
        ActionPayload::MarketOpen(request.clone())
    };
    let action = PendingAction {
        action_id: storage::take_next_action_id(env),
        owner,
        market_id: market_symbol,
        kind: entry.kind,
        created_at: now,
        execute_after,
        commit_observed_at: commit.observed_at,
        escrowed_collateral: request.submitted_collateral,
        payload,
    };
    storage::save_pending_action(env, &action);
    borrow::refresh_rate(env, &mut ledger, ledger::physical_cash(env));
    storage::save_ledger(env, &ledger);
    events::emit_action_committed(env, &action);
    action.action_id
}

pub fn settle_market_open(env: Env, keeper_address: Address, action_id: u64) -> ActionOutcome {
    settle_entry(&env, keeper_address, action_id, &MARKET)
}

pub fn settle_limit_open(env: Env, keeper_address: Address, action_id: u64) -> ActionOutcome {
    settle_entry(&env, keeper_address, action_id, &LIMIT)
}

fn settle_entry(
    env: &Env,
    keeper_address: Address,
    action_id: u64,
    entry: &EntryKind,
) -> ActionOutcome {
    keeper_address.require_auth();

    let mut action = action::load(env, action_id, entry.kind);
    let open = action
        .payload
        .open()
        .unwrap_or_else(|| panic_with_error!(env, PositionManagerError::WrongActionKind))
        .clone();
    let now = env.ledger().timestamp();

    if action::expired(now, open.expires_at) {
        return ActionOutcome::Expired;
    }
    if !action::delay_satisfied(now, action.execute_after) {
        return ActionOutcome::NotReady;
    }
    let fill = snapshot::read_stamped_price(env, &action.market_id);
    if !action::fresh_for_commit(fill.observed_at, action.commit_observed_at, action.created_at) {
        return ActionOutcome::NotReady;
    }
    if let ActionPayload::LimitOpen(_, condition) = &action.payload {
        if !action::trigger_crossed(condition.trigger_above, fill.price, condition.trigger_price) {
            return ActionOutcome::Pending;
        }
    }

    let mut ledger = storage::get_ledger(env);
    let mut market = storage::get_market(env, &action.market_id);
    borrow::accrue(env, &mut ledger, Some(&keeper_address), now);
    funding::accrue(
        env,
        &mut ledger,
        &action.market_id,
        Some(&keeper_address),
        &mut market,
        now,
    );
    let physical = ledger::physical_cash(env);
    let equity = ledger.cash_lp_equity(env, physical);
    risk::evaluate_market_risk(
        env,
        &mut ledger,
        &action.market_id,
        &keeper_address,
        &mut market,
        fill.price,
        equity,
    );

    let config = storage::get_global_config(env);
    let reward = keeper::reward_for(&config, entry.reward);
    let opening_fee = fees::calculate_opening_fee(env, open.size, &market.config);
    let after_charges = math::sub(
        env,
        math::sub(env, action.escrowed_collateral, reward),
        opening_fee,
    );
    let Some(exposure) = math::try_added_exposure(
        env,
        open.is_long,
        0,
        0,
        open.size,
        fill.price,
        market.config.market_risk_factor_bps,
    ) else {
        return action::fail_entry_action(
            env,
            &mut ledger,
            &market,
            &mut action,
            &keeper_address,
            reward,
            FailureReason::SizeTooSmall,
        );
    };

    if let Some(reason) = preflight(
        env,
        &ledger,
        &market,
        &action.market_id,
        &open,
        &exposure,
        &config,
        fill.price,
        after_charges,
        math::add(
            env,
            equity,
            math::mul_div_floor(
                env,
                opening_fee,
                config.fee_lp_revenue_share_bps as i128,
                BPS,
            ),
        ),
    ) {
        return action::fail_entry_action(
            env,
            &mut ledger,
            &market,
            &mut action,
            &keeper_address,
            reward,
            reason,
        );
    }

    execute(
        env,
        &mut ledger,
        &mut market,
        &mut action,
        &open,
        &exposure,
        &keeper_address,
        reward,
        opening_fee,
        fill.price,
        fill.observed_at,
        now,
    );
    ActionOutcome::Executed
}

#[allow(clippy::too_many_arguments)]
fn preflight(
    env: &Env,
    ledger: &Ledger,
    market: &Market,
    market_symbol: &Symbol,
    open: &OpenPayload,
    exposure: &AddedExposure,
    config: &GlobalConfig,
    price: i128,
    after_charges: i128,
    projected_equity: i128,
) -> Option<FailureReason> {
    if !action::entry_price_allowed(open.is_long, price, open.acceptable_price) {
        return Some(FailureReason::PriceBoundExceeded);
    }
    if storage::is_market_disabled(env, market_symbol)
        || !storage::is_market_registered(env, market_symbol)
    {
        return Some(FailureReason::MarketPaused);
    }
    let side = market.side(open.is_long);
    if !risk::side_accepts_new_exposure(env, side) {
        return Some(if storage::is_paused(env) {
            FailureReason::MarketPaused
        } else {
            FailureReason::SideRestricted
        });
    }
    if after_charges < config.min_collateral {
        return Some(FailureReason::InsufficientCollateral);
    }

    let projected_risk = math::add(env, ledger.total_risk_units, exposure.risk_added);
    let projected_rate = borrow::rate_at(env, projected_risk, projected_equity);
    let projected_minimum = borrow::projected_minimum(env, projected_rate, exposure.risk_added);
    let required = math::add(
        env,
        risk::initial_requirement(env, open.size, &market.config),
        projected_minimum,
    );
    if after_charges < required {
        return Some(FailureReason::InsufficientCollateral);
    }

    let capacity = math::mul_div_floor(
        env,
        projected_equity,
        config.risk_capacity_limit_bps as i128,
        BPS,
    );
    if projected_risk > capacity {
        return Some(FailureReason::CapacityExceeded);
    }

    let (size_cap, base_cap) = if open.is_long {
        (
            market.config.max_long_size_open_interest,
            market.config.max_long_base_exposure,
        )
    } else {
        (
            market.config.max_short_size_open_interest,
            market.config.max_short_base_exposure,
        )
    };
    if math::add(env, side.size_open_interest, open.size) > size_cap
        || math::add(env, side.base_exposure, exposure.base_added) > base_cap
    {
        return Some(FailureReason::ExposureCapExceeded);
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn execute(
    env: &Env,
    ledger: &mut Ledger,
    market: &mut Market,
    action: &mut PendingAction,
    open: &OpenPayload,
    exposure: &AddedExposure,
    keeper_address: &Address,
    reward: i128,
    opening_fee: i128,
    price: i128,
    observed_at: u64,
    now: u64,
) {
    let position_id = storage::get_next_position_id(env);
    storage::update_position_id(env);
    let is_long = open.is_long;

    keeper::pay_from_escrow(
        env,
        ledger,
        &mut action.escrowed_collateral,
        keeper_address,
        reward,
    );

    if opening_fee > 0 {
        action.escrowed_collateral = math::sub(env, action.escrowed_collateral, opening_fee);
        ledger::spend_escrow(env, ledger, opening_fee);
        fees::distribute_open_close_revenue(
            env,
            ledger,
            &action.market_id,
            keeper_address,
            opening_fee,
            &action.owner,
            FeeSource::Opening,
            position_id,
        );
    }

    let execution_delay = market.config.order_execution_delay_seconds;
    let mut position = Position {
        id: position_id,
        owner: action.owner.clone(),
        market: action.market_id.clone(),
        is_long,
        size: open.size,
        base_exposure: exposure.base_added,
        stored_collateral: 0,
        risk_units: exposure.risk_added,
        borrow_index_snapshot: 0,
        stored_minimum_borrow_fee: 0,
        receiver_payer_index_snapshot: 0,
        lp_payer_index_snapshot: 0,
        receiver_index_snapshot: 0,
        opened_at: now,
        last_size_increase_at: now,
        pending_mutation_action_id: None,
        take_profit: trigger::attach(
            open.take_profit,
            0,
            now,
            execution_delay,
            observed_at,
        ),
        stop_loss: trigger::attach(open.stop_loss, 0, now, execution_delay, observed_at),
    };

    let position_collateral = action.escrowed_collateral;
    action.escrowed_collateral = 0;
    ledger::escrow_to_collateral(
        env,
        ledger,
        &mut position,
        market.side_mut(is_long),
        position_collateral,
    );

    let was_empty = market.long.size_open_interest == 0 && market.short.size_open_interest == 0;
    funding::reset_receiver_distribution_remainder(market, is_long);
    {
        let side = market.side_mut(is_long);
        side.size_open_interest = math::add(env, side.size_open_interest, open.size);
        side.base_exposure = math::add(env, side.base_exposure, exposure.base_added);
        side.risk_units = math::add(env, side.risk_units, exposure.risk_added);
    }
    if was_empty {
        funding::cold_start(env, market);
    }
    risk::register_exposure(env, ledger, exposure.risk_added);
    risk::register_position(ledger);
    funding::snapshot_funding_indices(&mut position, market);
    funding::refresh_display(env, ledger, market);
    let physical_after = ledger::physical_cash(env);
    let equity_after = ledger.cash_lp_equity(env, physical_after);
    risk::evaluate_market_risk(
        env,
        ledger,
        &action.market_id,
        keeper_address,
        market,
        price,
        equity_after,
    );
    storage::save_market(env, &action.market_id, market);
    borrow::refresh_rate(env, ledger, physical_after);
    borrow::initialize_window(env, ledger, &mut position);
    storage::save_position(env, &position);
    storage::remove_pending_action(env, action.action_id);
    storage::save_ledger(env, ledger);
    events::emit_opened(env, keeper_address, &position, price);
    events::emit_action_settled(
        env,
        &action.market_id,
        keeper_address,
        action.action_id,
        &action.owner,
        action.kind,
        position_id,
        price,
        observed_at,
        reward,
    );
}

pub fn cancel_limit_open(env: Env, action_id: u64) -> i128 {
    let mut action = action::load(&env, action_id, ActionKind::LimitOpen);
    action.owner.require_auth();
    let expires_at = action
        .payload
        .open()
        .unwrap_or_else(|| panic_with_error!(&env, PositionManagerError::WrongActionKind))
        .expires_at;
    if action::expired(env.ledger().timestamp(), expires_at) {
        panic_with_error!(&env, PositionManagerError::InvalidOrder);
    }

    let mut ledger = storage::get_ledger(&env);
    borrow::accrue(
        &env,
        &mut ledger,
        Some(&action.owner),
        env.ledger().timestamp(),
    );
    let refund = action.escrowed_collateral;
    action.escrowed_collateral = 0;
    ledger::refund_escrow(&env, &mut ledger, &action.owner, refund);
    storage::remove_pending_action(&env, action_id);
    borrow::refresh_rate(&env, &mut ledger, ledger::physical_cash(&env));
    storage::save_ledger(&env, &ledger);
    events::emit_action_cancelled(&env, action_id, &action.owner, &action.market_id, refund);
    refund
}

pub fn clean_expired_entry(env: Env, keeper_address: Address, action_id: u64) {
    keeper_address.require_auth();
    let mut action = action::load_entry(&env, action_id);
    let expires_at = action.payload.open().unwrap().expires_at;
    let now = env.ledger().timestamp();
    if !action::expired(now, expires_at) {
        panic_with_error!(&env, PositionManagerError::TooEarly);
    }

    let mut ledger = storage::get_ledger(&env);
    borrow::accrue(&env, &mut ledger, Some(&keeper_address), now);
    let reward = keeper::reward_for(&storage::get_global_config(&env), keeper::RewardKind::Expiry);
    keeper::pay_from_escrow(
        &env,
        &mut ledger,
        &mut action.escrowed_collateral,
        &keeper_address,
        reward,
    );
    let refund = action.escrowed_collateral;
    action.escrowed_collateral = 0;
    ledger::refund_escrow(&env, &mut ledger, &action.owner, refund);
    storage::remove_pending_action(&env, action_id);
    borrow::refresh_rate(&env, &mut ledger, ledger::physical_cash(&env));
    storage::save_ledger(&env, &ledger);
    events::emit_action_expired(
        &env,
        action_id,
        &action.owner,
        &action.market_id,
        &keeper_address,
        reward,
        refund,
    );
}
