use soroban_sdk::{panic_with_error, Address, Env};

use shared::constants::{BPS, PRICE_PRECISION};
use shared::{
    ActionKind, ActionOutcome, ActionPayload, ClosePayload, DecreasePayload, FailureReason,
    GlobalConfig, IncreasePayload, Market, PendingAction, Position, StampedPrice,
};

use crate::errors::PositionManagerError;
use crate::events::{self, CloseReason, FeeSource};
use crate::funding::PendingFees;
use crate::ledger::{self, Ledger};
use crate::risk::LiquidationAssessment;
use crate::settle::{self, ClosingFee};
use crate::{action, borrow, fees, funding, keeper, math, risk, snapshot, storage};

pub fn add_collateral(env: Env, position_id: u64, amount: i128) {
    let mut position = storage::get_position(&env, position_id);
    position.owner.require_auth();
    if amount <= 0 {
        panic_with_error!(&env, PositionManagerError::InvalidAmount);
    }

    let mut ledger = storage::get_ledger(&env);
    let mut market = storage::get_market(&env, &position.market);
    let now = env.ledger().timestamp();
    let actor = position.owner.clone();
    borrow::accrue(&env, &mut ledger, Some(&actor), now);
    funding::accrue(
        &env,
        &mut ledger,
        &position.market,
        Some(&actor),
        &mut market,
        now,
    );

    ledger::receive(&env, &position.owner, amount);
    let is_long = position.is_long;
    ledger::add_stored_collateral(
        &env,
        &mut ledger,
        &mut position,
        market.side_mut(is_long),
        amount,
    );

    storage::save_market(&env, &position.market, &market);
    borrow::refresh_rate(&env, &mut ledger, ledger::physical_cash(&env));
    storage::save_position(&env, &position);
    storage::save_ledger(&env, &ledger);
    events::emit_collateral_added(&env, &position, amount);
}

fn commit(
    env: &Env,
    position: &mut Position,
    market: &Market,
    kind: ActionKind,
    payload: ActionPayload,
    escrow: i128,
    commit_observed_at: u64,
) -> u64 {
    let now = env.ledger().timestamp();
    let delay = market.config.order_execution_delay_seconds;

    let mut ledger = storage::get_ledger(env);
    borrow::accrue(env, &mut ledger, Some(&position.owner), now);
    if escrow > 0 {
        ledger::escrow_in(env, &mut ledger, &position.owner, escrow);
    }

    let action = PendingAction {
        action_id: storage::take_next_action_id(env),
        owner: position.owner.clone(),
        market_id: position.market.clone(),
        kind,
        created_at: now,
        execute_after: now.saturating_add(delay),
        commit_observed_at,
        escrowed_collateral: escrow,
        payload,
    };
    storage::save_pending_action(env, &action);
    position.pending_mutation_action_id = Some(action.action_id);
    storage::save_position(env, position);
    borrow::refresh_rate(env, &mut ledger, ledger::physical_cash(env));
    storage::save_ledger(env, &ledger);
    events::emit_action_committed(env, &action);
    action.action_id
}

fn claim_slot(env: &Env, position_id: u64) -> (Position, Market, StampedPrice) {
    let position = storage::get_position(env, position_id);
    position.owner.require_auth();
    if position.pending_mutation_action_id.is_some() {
        panic_with_error!(env, PositionManagerError::MutationPending);
    }
    let market = storage::get_market(env, &position.market);
    let commit_price = snapshot::read_stamped_price(env, &position.market);
    (position, market, commit_price)
}

pub fn create_increase(
    env: Env,
    position_id: u64,
    size_added: i128,
    collateral_added: i128,
    acceptable_price: i128,
) -> u64 {
    let (mut position, market, commit_price) = claim_slot(&env, position_id);
    if size_added <= 0 || collateral_added < 0 || acceptable_price < 0 {
        panic_with_error!(&env, PositionManagerError::InvalidAmount);
    }
    if storage::is_paused(&env) {
        panic_with_error!(&env, PositionManagerError::Paused);
    }
    if !storage::is_market_registered(&env, &position.market) {
        panic_with_error!(&env, PositionManagerError::MarketNotConfigured);
    }
    let base_added = math::mul_div_floor(&env, size_added, PRICE_PRECISION, commit_price.price);
    let risk_after = math::risk_units_for(
        &env,
        math::add(&env, position.size, size_added),
        market.config.market_risk_factor_bps,
    );
    if base_added <= 0 || risk_after <= position.risk_units {
        panic_with_error!(&env, PositionManagerError::InvalidAmount);
    }

    let payload = ActionPayload::Increase(IncreasePayload {
        position_id,
        size_added,
        collateral_added,
        acceptable_price,
    });
    commit(
        &env,
        &mut position,
        &market,
        ActionKind::Increase,
        payload,
        collateral_added,
        commit_price.observed_at,
    )
}

pub fn create_decrease(
    env: Env,
    position_id: u64,
    size_removed: i128,
    acceptable_price: i128,
) -> u64 {
    let (mut position, market, commit_price) = claim_slot(&env, position_id);
    if size_removed <= 0 || size_removed >= position.size || acceptable_price < 0 {
        panic_with_error!(&env, PositionManagerError::InvalidAmount);
    }
    // A survivor must keep positive base and risk units.
    let removed = settle::removed_exposure(&env, &position, size_removed, &market.config);
    if removed.base_after <= 0 || removed.risk_after <= 0 {
        panic_with_error!(&env, PositionManagerError::InvalidAmount);
    }
    let payload = ActionPayload::Decrease(DecreasePayload {
        position_id,
        size_removed,
        acceptable_price,
    });
    commit(
        &env,
        &mut position,
        &market,
        ActionKind::Decrease,
        payload,
        0,
        commit_price.observed_at,
    )
}

pub fn create_close(env: Env, position_id: u64, acceptable_price: i128) -> u64 {
    let (mut position, market, commit_price) = claim_slot(&env, position_id);
    if acceptable_price < 0 {
        panic_with_error!(&env, PositionManagerError::InvalidAmount);
    }
    let payload = ActionPayload::Close(ClosePayload {
        position_id,
        acceptable_price,
    });
    commit(
        &env,
        &mut position,
        &market,
        ActionKind::Close,
        payload,
        0,
        commit_price.observed_at,
    )
}

struct Eligible {
    action: PendingAction,
    position: Position,
    market: Market,
    ledger: Ledger,
    config: GlobalConfig,
    pending: PendingFees,
    assessment: LiquidationAssessment,
    price: i128,
    observed_at: u64,
}

fn eligible(
    env: &Env,
    keeper_address: &Address,
    action_id: u64,
    kind: ActionKind,
    removes_exposure: bool,
) -> Result<Eligible, ActionOutcome> {
    keeper_address.require_auth();

    let action = action::load(env, action_id, kind);
    let position = action::load_target(env, &action);
    let now = env.ledger().timestamp();
    let config = storage::get_global_config(env);

    if !action::delay_satisfied(now, action.execute_after) {
        return Err(ActionOutcome::NotReady);
    }
    if removes_exposure && !action::lifetime_satisfied(now, &position, &config) {
        return Err(ActionOutcome::NotReady);
    }
    let fill = snapshot::read_stamped_price(env, &action.market_id);
    if !action::fresh_for_commit(fill.observed_at, action.commit_observed_at, action.created_at) {
        return Err(ActionOutcome::NotReady);
    }

    let mut ledger = storage::get_ledger(env);
    let mut market = storage::get_market(env, &action.market_id);
    borrow::accrue(env, &mut ledger, Some(keeper_address), now);
    funding::accrue(
        env,
        &mut ledger,
        &action.market_id,
        Some(keeper_address),
        &mut market,
        now,
    );
    let physical = ledger::physical_cash(env);
    let equity = ledger.cash_lp_equity(env, physical);
    risk::evaluate_market_risk(
        env,
        &mut ledger,
        &action.market_id,
        keeper_address,
        &mut market,
        fill.price,
        equity,
    );

    let pending = funding::pending_fees(env, &ledger, &position, &market);
    let assessment = risk::evaluate_liquidation(env, &ledger, &position, &market, fill.price);
    if assessment.liquidatable {
        action::persist_accrual(env, &mut ledger, &action.market_id, &market, physical);
        return Err(ActionOutcome::RequiresLiquidation);
    }

    Ok(Eligible {
        action,
        position,
        market,
        ledger,
        config,
        pending,
        assessment,
        price: fill.price,
        observed_at: fill.observed_at,
    })
}

fn lp_share(env: &Env, collected: i128, share_bps: u32) -> i128 {
    math::mul_div_floor(env, collected, share_bps as i128, BPS)
}

pub fn settle_increase(env: Env, keeper_address: Address, action_id: u64) -> ActionOutcome {
    let mut e = match eligible(
        &env,
        &keeper_address,
        action_id,
        ActionKind::Increase,
        false,
    ) {
        Ok(e) => e,
        Err(outcome) => return outcome,
    };
    let ActionPayload::Increase(payload) = e.action.payload.clone() else {
        panic_with_error!(&env, PositionManagerError::WrongActionKind);
    };
    let reward = keeper::reward_for(&e.config, keeper::RewardKind::Increase);

    if let Some(reason) = increase_preflight(&env, &e, &payload, reward) {
        return action::fail_position_action(
            &env,
            &mut e.ledger,
            &mut e.market,
            &mut e.position,
            &mut e.action,
            &keeper_address,
            reward,
            reason,
            &e.assessment,
            e.price,
        );
    }

    execute_increase(&env, &mut e, &payload, &keeper_address, reward);
    ActionOutcome::Executed
}

fn increase_preflight(
    env: &Env,
    e: &Eligible,
    payload: &IncreasePayload,
    reward: i128,
) -> Option<FailureReason> {
    if !action::entry_price_allowed(e.position.is_long, e.price, payload.acceptable_price) {
        return Some(FailureReason::PriceBoundExceeded);
    }
    if storage::is_market_disabled(env, &e.action.market_id) {
        return Some(FailureReason::MarketPaused);
    }
    let side = e.market.side(e.position.is_long);
    if !risk::side_accepts_new_exposure(env, side) {
        return Some(if storage::is_paused(env) {
            FailureReason::MarketPaused
        } else {
            FailureReason::SideRestricted
        });
    }

    let Some(exposure) = math::try_added_exposure(
        env,
        e.position.is_long,
        e.position.size,
        e.position.risk_units,
        payload.size_added,
        e.price,
        e.market.config.market_risk_factor_bps,
    ) else {
        return Some(FailureReason::SizeTooSmall);
    };
    let opening_fee = fees::calculate_opening_fee(env, payload.size_added, &e.market.config);

    let owed = math::add(
        env,
        math::add(
            env,
            e.pending.funding_paid_to_receivers,
            e.pending.funding_paid_to_lps,
        ),
        e.pending.borrow,
    );
    let after_window = math::sub(
        env,
        math::add(env, e.position.stored_collateral, e.pending.funding_received),
        owed,
    );
    if after_window < 0 {
        return Some(FailureReason::InsufficientCollateral);
    }
    let after_charges = math::sub(
        env,
        math::sub(
            env,
            math::add(env, after_window, payload.collateral_added),
            opening_fee,
        ),
        reward,
    );
    if after_charges < e.config.min_collateral {
        return Some(FailureReason::InsufficientCollateral);
    }

    let resulting_size = math::add(env, e.position.size, payload.size_added);
    let resulting_base = math::add(env, e.position.base_exposure, exposure.base_added);
    let resulting_risk = math::add(env, e.position.risk_units, exposure.risk_added);
    let effective = math::add(
        env,
        after_charges,
        risk::payable_pnl(
            env,
            math::pnl(env, e.position.is_long, resulting_size, resulting_base, e.price),
            side,
        ),
    );

    let projected_equity = projected_equity_after_increase(env, e, opening_fee);
    let projected_total_risk = math::add(env, e.ledger.total_risk_units, exposure.risk_added);
    let projected_minimum = borrow::projected_minimum(
        env,
        borrow::rate_at(env, projected_total_risk, projected_equity),
        resulting_risk,
    );
    let required = math::add(
        env,
        risk::initial_requirement(env, resulting_size, &e.market.config),
        projected_minimum,
    );
    if effective < required {
        return Some(FailureReason::InsufficientCollateral);
    }

    let capacity = math::mul_div_floor(
        env,
        projected_equity,
        e.config.risk_capacity_limit_bps as i128,
        BPS,
    );
    if projected_total_risk > capacity {
        return Some(FailureReason::CapacityExceeded);
    }

    let (size_cap, base_cap) = if e.position.is_long {
        (
            e.market.config.max_long_size_open_interest,
            e.market.config.max_long_base_exposure,
        )
    } else {
        (
            e.market.config.max_short_size_open_interest,
            e.market.config.max_short_base_exposure,
        )
    };
    if math::add(env, side.size_open_interest, payload.size_added) > size_cap
        || math::add(env, side.base_exposure, exposure.base_added) > base_cap
    {
        return Some(FailureReason::ExposureCapExceeded);
    }
    None
}

fn projected_equity_after_increase(env: &Env, e: &Eligible, opening_fee: i128) -> i128 {
    let equity = e
        .ledger
        .cash_lp_equity(env, ledger::physical_cash(env));
    let mut projected = math::add(env, equity, e.pending.funding_paid_to_receivers);
    projected = math::add(env, projected, e.pending.funding_paid_to_lps);
    projected = math::add(
        env,
        projected,
        lp_share(env, e.pending.borrow, e.config.borrow_lp_revenue_share_bps),
    );
    math::add(
        env,
        projected,
        lp_share(env, opening_fee, e.config.fee_lp_revenue_share_bps),
    )
}

fn execute_increase(
    env: &Env,
    e: &mut Eligible,
    payload: &IncreasePayload,
    keeper_address: &Address,
    reward: i128,
) {
    let is_long = e.position.is_long;
    let collected = fees::capitalize_for_surviving_mutation(
        env,
        &mut e.ledger,
        &e.action.market_id,
        keeper_address,
        &mut e.position,
        &mut e.market,
    );

    let escrow = e.action.escrowed_collateral;
    e.action.escrowed_collateral = 0;
    ledger::escrow_to_collateral(
        env,
        &mut e.ledger,
        &mut e.position,
        e.market.side_mut(is_long),
        escrow,
    );

    let opening_fee = fees::calculate_opening_fee(env, payload.size_added, &e.market.config);
    if opening_fee > 0 {
        let charged = ledger::collect_stored_collateral(
            env,
            &mut e.ledger,
            &mut e.position,
            e.market.side_mut(is_long),
            opening_fee,
        );
        if charged < opening_fee {
            panic_with_error!(env, PositionManagerError::InsufficientCollateral);
        }
        let owner = e.position.owner.clone();
        fees::distribute_open_close_revenue(
            env,
            &mut e.ledger,
            &e.action.market_id,
            keeper_address,
            charged,
            &owner,
            FeeSource::Opening,
            e.position.id,
        );
    }
    keeper::pay_from_position(
        env,
        &mut e.ledger,
        &mut e.position,
        e.market.side_mut(is_long),
        keeper_address,
        reward,
    );

    let exposure = math::derive_added_exposure(
        env,
        is_long,
        e.position.size,
        e.position.risk_units,
        payload.size_added,
        e.price,
        e.market.config.market_risk_factor_bps,
    );
    funding::reset_receiver_distribution_remainder(&mut e.market, is_long);
    e.position.size = math::add(env, e.position.size, payload.size_added);
    e.position.base_exposure = math::add(env, e.position.base_exposure, exposure.base_added);
    e.position.risk_units = math::add(env, e.position.risk_units, exposure.risk_added);
    e.position.last_size_increase_at = env.ledger().timestamp();
    {
        let side = e.market.side_mut(is_long);
        side.size_open_interest = math::add(env, side.size_open_interest, payload.size_added);
        side.base_exposure = math::add(env, side.base_exposure, exposure.base_added);
        side.risk_units = math::add(env, side.risk_units, exposure.risk_added);
    }
    risk::register_exposure(env, &mut e.ledger, exposure.risk_added);

    funding::snapshot_funding_indices(&mut e.position, &e.market);
    funding::refresh_display(env, &mut e.ledger, &mut e.market);
    let physical = ledger::physical_cash(env);
    let equity = e.ledger.cash_lp_equity(env, physical);
    risk::evaluate_market_risk(
        env,
        &mut e.ledger,
        &e.action.market_id,
        keeper_address,
        &mut e.market,
        e.price,
        equity,
    );
    storage::save_market(env, &e.action.market_id, &e.market);
    borrow::refresh_rate(env, &mut e.ledger, physical);
    borrow::initialize_window(env, &e.ledger, &mut e.position);

    e.position.pending_mutation_action_id = None;
    storage::remove_pending_action(env, e.action.action_id);
    storage::save_position(env, &e.position);
    storage::save_ledger(env, &e.ledger);
    events::emit_action_settled(
        env,
        &e.action.market_id,
        keeper_address,
        e.action.action_id,
        &e.action.owner,
        e.action.kind,
        e.position.id,
        e.price,
        e.observed_at,
        reward,
    );
    events::emit_increased(
        env,
        keeper_address,
        &e.position,
        payload.size_added,
        exposure.base_added,
        escrow,
        e.price,
        &collected,
    );
}

pub fn settle_decrease(env: Env, keeper_address: Address, action_id: u64) -> ActionOutcome {
    let mut e = match eligible(&env, &keeper_address, action_id, ActionKind::Decrease, true) {
        Ok(e) => e,
        Err(outcome) => return outcome,
    };
    let ActionPayload::Decrease(payload) = e.action.payload.clone() else {
        panic_with_error!(&env, PositionManagerError::WrongActionKind);
    };
    let reward = keeper::reward_for(&e.config, keeper::RewardKind::Decrease);

    if let Some(reason) = decrease_preflight(&env, &e, payload.size_removed, payload.acceptable_price, reward) {
        return action::fail_position_action(
            &env,
            &mut e.ledger,
            &mut e.market,
            &mut e.position,
            &mut e.action,
            &keeper_address,
            reward,
            reason,
            &e.assessment,
            e.price,
        );
    }

    e.position.pending_mutation_action_id = None;
    storage::remove_pending_action(&env, e.action.action_id);
    events::emit_action_settled(
        &env,
        &e.action.market_id,
        &keeper_address,
        e.action.action_id,
        &e.action.owner,
        e.action.kind,
        e.position.id,
        e.price,
        e.observed_at,
        reward,
    );
    let Eligible {
        position,
        market,
        mut ledger,
        price,
        ..
    } = e;
    let settled = settle::settle(
        &env,
        &mut ledger,
        position,
        market,
        payload.size_removed,
        price,
        settle::Keeper {
            recipient: &keeper_address,
            reward,
            liquidation: false,
        },
        ClosingFee::Charged,
    );
    storage::save_ledger(&env, &ledger);
    match &settled {
        settle::Settled::Partial(header) => {
            events::emit_decreased(&env, &keeper_address, header)
        }
        settle::Settled::Closed(..) => {
            panic_with_error!(&env, PositionManagerError::InvariantViolation)
        }
    }
    ActionOutcome::Executed
}

fn decrease_preflight(
    env: &Env,
    e: &Eligible,
    size_removed: i128,
    acceptable_price: i128,
    reward: i128,
) -> Option<FailureReason> {
    if !action::exit_price_allowed(e.position.is_long, e.price, acceptable_price) {
        return Some(FailureReason::PriceBoundExceeded);
    }

    let removed = settle::removed_exposure(env, &e.position, size_removed, &e.market.config);
    if removed.base_after <= 0 || removed.risk_after <= 0 {
        return Some(FailureReason::SizeTooSmall);
    }
    let raw_pnl = math::pnl(
        env,
        e.position.is_long,
        size_removed,
        removed.base_removed,
        e.price,
    );
    let side = e.market.side(e.position.is_long);
    let payable = core::cmp::max(risk::payable_pnl(env, raw_pnl, side), 0);
    let negative = core::cmp::max(-raw_pnl, 0);

    let equity = e.ledger.cash_lp_equity(env, ledger::physical_cash(env));
    if payable > equity {
        return Some(FailureReason::UnpayableProfit);
    }

    let credits = math::add(
        env,
        math::add(env, e.position.stored_collateral, payable),
        e.pending.funding_received,
    );
    let debits = math::add(
        env,
        math::add(
            env,
            math::add(env, e.pending.funding_paid_to_receivers, negative),
            e.pending.funding_paid_to_lps,
        ),
        e.pending.borrow,
    );
    let after_waterfall = math::sub(env, credits, debits);
    if after_waterfall < 0 {
        return Some(FailureReason::InsufficientCollateral);
    }
    let after_reward = math::sub(env, after_waterfall, reward);
    if after_reward < 0 {
        return Some(FailureReason::InsufficientCollateral);
    }
    let closing = fees::calculate_closing_fee(
        env,
        size_removed,
        payable,
        &e.pending,
        e.pending.borrow,
        reward,
        &e.market.config,
    );
    let stored_final = math::sub(env, after_reward, closing);

    if stored_final < e.config.min_collateral {
        return Some(FailureReason::InsufficientCollateral);
    }
    let remaining_pnl = risk::payable_pnl(
        env,
        math::pnl(
            env,
            e.position.is_long,
            removed.new_size,
            removed.base_after,
            e.price,
        ),
        side,
    );
    let threshold = core::cmp::max(
        risk::maintenance_requirement(env, removed.new_size, &e.market.config),
        e.config.keeper_rewards.liquidation,
    );
    // Upper bound on the survivor's rate: only the profit credit lowers equity.
    let projected_rate = borrow::rate_at(
        env,
        math::sub(env, e.ledger.total_risk_units, removed.risk_removed),
        core::cmp::max(math::sub(env, equity, payable), 0),
    );
    let projected_minimum = borrow::projected_minimum(env, projected_rate, removed.risk_after);
    let surviving_effective = math::sub(
        env,
        math::add(env, stored_final, remaining_pnl),
        projected_minimum,
    );
    if surviving_effective <= threshold {
        return Some(FailureReason::InsufficientCollateral);
    }
    None
}

pub fn settle_close(env: Env, keeper_address: Address, action_id: u64) -> ActionOutcome {
    let mut e = match eligible(&env, &keeper_address, action_id, ActionKind::Close, true) {
        Ok(e) => e,
        Err(outcome) => return outcome,
    };
    let ActionPayload::Close(payload) = e.action.payload.clone() else {
        panic_with_error!(&env, PositionManagerError::WrongActionKind);
    };
    let reward = keeper::reward_for(&e.config, keeper::RewardKind::Close);

    if !action::exit_price_allowed(e.position.is_long, e.price, payload.acceptable_price) {
        return action::fail_position_action(
            &env,
            &mut e.ledger,
            &mut e.market,
            &mut e.position,
            &mut e.action,
            &keeper_address,
            reward,
            FailureReason::PriceBoundExceeded,
            &e.assessment,
            e.price,
        );
    }

    e.position.pending_mutation_action_id = None;
    storage::remove_pending_action(&env, e.action.action_id);
    events::emit_action_settled(
        &env,
        &e.action.market_id,
        &keeper_address,
        e.action.action_id,
        &e.action.owner,
        e.action.kind,
        e.position.id,
        e.price,
        e.observed_at,
        reward,
    );
    let Eligible {
        position,
        market,
        mut ledger,
        price,
        ..
    } = e;
    let size = position.size;
    let settled = settle::settle(
        &env,
        &mut ledger,
        position,
        market,
        size,
        price,
        settle::Keeper {
            recipient: &keeper_address,
            reward,
            liquidation: false,
        },
        ClosingFee::Charged,
    );
    storage::save_ledger(&env, &ledger);
    super::emit_terminal(&env, &keeper_address, &settled, CloseReason::VoluntaryClose);
    ActionOutcome::Executed
}
