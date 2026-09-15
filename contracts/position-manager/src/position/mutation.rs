//! §7.7–7.10 — position mutations: add collateral, increase, decrease,
//! close.
//!
//! `add_collateral` is immediate. The other three are commit-then-settle,
//! and a position holds at most one of them at a time (§8.4): creation fails
//! while `pending_mutation_action_id` is occupied, so two commitments can
//! never assume the same starting size, collateral, and debt baselines.
//!
//! Every settlement here runs a complete preflight against the hypothetical
//! post-settlement state and *returns* a failure reason rather than
//! reverting. The preflight is not defensive duplication: it is what turns
//! §6.10's and §6.13's `require`s — which would revert, preserve the action,
//! and hand the trader a free retry — into terminal outcomes that pay the
//! keeper and consume the commitment.

use soroban_sdk::{panic_with_error, Address, Env};

use shared::constants::{BPS, PRICE_PRECISION};
use shared::{
    ActionKind, ActionOutcome, ActionPayload, ClosePayload, DecreasePayload, FailureReason,
    GlobalConfig, IncreasePayload, Market, PendingAction, Position, StampedPrice,
};

use crate::auth::require_initialized;
use crate::errors::PositionManagerError;
use crate::events::{self, CloseReason, FeeSource};
use crate::funding::PendingFees;
use crate::ledger::{self, Ledger};
use crate::risk::LiquidationAssessment;
use crate::settle::{self, ClosingFee};
use crate::{action, borrow, fees, funding, keeper, math, risk, snapshot, storage};

// ---------------------------------------------------------------------------
// §7.7 Add collateral — the one position mutation that needs no commitment.
// ---------------------------------------------------------------------------

/// §7.7 — add collateral immediately.
///
/// It adds no price exposure and so cannot exploit a stale execution price,
/// which is the entire reason the two-phase lifecycle exists. It charges no
/// fee or reward, settles nothing, resets no baseline, does not touch
/// `stored_minimum_borrow_fee`, and does **not** restart the
/// minimum-position-lifetime clock — a top-up is not a size increase.
///
/// A liquidatable owner may use this to rescue the position before a
/// liquidation transaction succeeds. That is deliberate: refusing a rescue
/// would only convert a recoverable position into bad debt.
pub fn add_collateral(env: Env, position_id: u64, amount: i128) {
    require_initialized(&env);
    let mut position = storage::get_position(&env, position_id);
    position.owner.require_auth();
    if amount <= 0 {
        panic_with_error!(&env, PositionManagerError::InvalidAmount);
    }

    let mut ledger = storage::get_ledger(&env);
    let mut market = storage::get_market(&env, &position.market);
    let now = env.ledger().timestamp();
    // The checkpoints run so health is reported against one consistent
    // timestamp, not because this action settles anything.
    borrow::accrue(&env, &mut ledger, now);
    funding::accrue(&env, &mut ledger, &mut market, now);

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
    // Physical cash and the position claim rise by the same amount, so cash
    // LP equity and the rate ordinarily do not move; the refresh is the
    // §4.9 order rather than an expectation of change.
    borrow::refresh_rate(&env, &mut ledger, ledger::physical_cash(&env));
    storage::save_position(&env, &position);
    storage::save_ledger(&env, &ledger);
    events::emit_collateral_added(&env, &position, amount);
}

// ---------------------------------------------------------------------------
// Creation (§7.8, §7.9, §7.10).
// ---------------------------------------------------------------------------

/// The common creation steps: authorize, claim the position's one mutation
/// slot, freeze the commitment cursor, escrow any added cash, and store.
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
    borrow::accrue(env, &mut ledger, now);
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

/// Load the position, its market, and the commitment observation, and claim
/// the position's one mutation slot.
///
/// One feed read and one market read per creation: the cursor stored in the
/// action and any price the caller validates against are then guaranteed to
/// be the same observation.
fn claim_slot(env: &Env, position_id: u64) -> (Position, Market, StampedPrice) {
    require_initialized(env);
    let position = storage::get_position(env, position_id);
    position.owner.require_auth();
    // §8.4 — at most one ordinary pending mutation. The slot is cleared only
    // by execution, terminal failure, or forced-position cleanup.
    if position.pending_mutation_action_id.is_some() {
        panic_with_error!(env, PositionManagerError::MutationPending);
    }
    let market = storage::get_market(env, &position.market);
    let commit_price = snapshot::read_stamped_price(env, &position.market);
    (position, market, commit_price)
}

/// §7.8 — commit an increase.
///
/// The two dust checks run **here**, against the commitment price, and that
/// placement is the point of them. `derive_added_exposure` requires positive
/// added base and positive added risk, and those are `require`s: failing one
/// at settlement reverts, and a position mutation has neither a cancel
/// operation nor an expiry. A size add of one unit on a market priced in the
/// tens of thousands would therefore occupy the position's one mutation slot
/// permanently, leaving it with no increase, decrease, or close for the rest
/// of its life — only an attached trigger or liquidation could still exit
/// it. Rejecting at creation is what makes those requires unreachable rather
/// than merely defensive.
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

/// §7.9 — commit a partial decrease. A decrease carries no escrow: its
/// payment source is the position that already exists.
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

/// §7.10 — commit a full close. No size is stored: it always targets the
/// complete remaining exposure as it stands at settlement, so an increase
/// that lands in between is closed too.
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

// ---------------------------------------------------------------------------
// Settlement.
// ---------------------------------------------------------------------------

/// Everything the three settlement paths need after their shared gates have
/// passed. Assembling it in one place is what guarantees eligibility, the
/// liquidation assessment, the fee snapshot, and the settlement itself all
/// observe the same instant.
struct Eligible {
    action: PendingAction,
    position: Position,
    market: Market,
    ledger: Ledger,
    config: GlobalConfig,
    pending: PendingFees,
    assessment: LiquidationAssessment,
    price: i128,
}

/// The shared front half of `settle_increase`, `settle_decrease`, and
/// `settle_close`: the timing gates, the checkpoints, the risk refresh, and
/// the liquidation precedence check.
///
/// `Err` carries a non-terminal outcome — the action is untouched and
/// nothing is paid.
fn eligible(
    env: &Env,
    keeper_address: &Address,
    action_id: u64,
    kind: ActionKind,
    removes_exposure: bool,
) -> Result<Eligible, ActionOutcome> {
    require_initialized(env);
    keeper_address.require_auth();

    let action = action::load(env, action_id, kind);
    let position = action::load_target(env, &action);
    let now = env.ledger().timestamp();
    let config = storage::get_global_config(env);

    if !action::delay_satisfied(now, action.execute_after) {
        return Err(ActionOutcome::NotReady);
    }
    // §8.5 — the lifetime gate is read from the position at settlement, not
    // frozen at creation, so an increase landing in between moves it
    // forward. It applies to decrease and close, never to increase.
    if removes_exposure && !action::lifetime_satisfied(now, &position, &config) {
        return Err(ActionOutcome::NotReady);
    }
    let fill = snapshot::read_stamped_price(env, &action.market_id);
    if !action::fresh_for_commit(fill.observed_at, action.commit_observed_at) {
        return Err(ActionOutcome::NotReady);
    }

    let mut ledger = storage::get_ledger(env);
    let mut market = storage::get_market(env, &action.market_id);
    borrow::accrue(env, &mut ledger, now);
    funding::accrue(env, &mut ledger, &mut market, now);
    let physical = ledger::physical_cash(env);
    let equity = ledger.cash_lp_equity(env, physical);
    // §6.5 — the side risk state is refreshed from this fill before anything
    // reads a payout factor. Refreshing afterwards would let the first
    // position out of a newly-crossed side settle unscaled.
    risk::evaluate_market_risk(env, &mut ledger, &action.market_id, &mut market, fill.price, equity);

    let pending = funding::pending_fees(env, &ledger, &position, &market);
    let assessment = risk::evaluate_liquidation(env, &ledger, &position, &market, fill.price);
    // §8.12 — liquidation outranks every voluntary mutation. This is the
    // only non-terminal safety exception: the action stays pending and pays
    // nothing, and the liquidation path supersedes it.
    if assessment.liquidatable {
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
    })
}

/// §6.11 — the LP share of a collected fee, which is the *only* part of a
/// distribution that changes cash LP equity: the protocol and referral
/// slices become claims, and leaving the rest in the residual is what
/// credits LPs.
fn lp_share(env: &Env, collected: i128, share_bps: u32) -> i128 {
    math::mul_div_floor(env, collected, share_bps as i128, BPS)
}

// ---------------------------------------------------------------------------
// §7.8 Increase.
// ---------------------------------------------------------------------------

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

/// §7.8 — the increase preflight, against the complete resulting position.
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

    let exposure = math::derive_added_exposure(
        env,
        e.position.is_long,
        e.position.size,
        e.position.risk_units,
        payload.size_added,
        e.price,
        e.market.config.market_risk_factor_bps,
    );
    let opening_fee = fees::calculate_opening_fee(env, payload.size_added, &e.market.config);

    // Project stored collateral through the settlement, in the order
    // `execute_increase` performs it. The completed old window is settled
    // first, from **pre-existing** collateral: an increase must not use new
    // money to cover an obligation the position could not already meet,
    // because that would let a position which should have been liquidated
    // buy its way past the check.
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
    // Every pending obligation is settled by the capitalization above, so
    // effective collateral after the action is the projected stored
    // collateral plus the payable PnL still riding on the resulting
    // exposure (§6.6).
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
    // §7.2's `projected_minimum_borrow`, here against the **full resulting**
    // risk units rather than only the added ones: the increase opens a fresh
    // window for the whole position (§3.3.3), so the floor it will be quoted
    // covers all of it.
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

/// Cash LP equity after an increase settles.
///
/// Only three moves touch it. Collected payer funding returns cash that
/// already backs a receiver claim or is LP revenue outright, so it lands in
/// the residual in full; collected borrow lands there net of the protocol
/// slice; and the opening fee lands there net of the protocol and referral
/// slices. The keeper reward leaves the vault against its own claim and the
/// escrow-to-collateral move relabels one claim as another, so neither
/// changes equity at all.
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

/// §7.8 — the success path, in the specified order: settle the old window,
/// then move escrow in, then the opening fee, then the keeper reward, then
/// add exposure, then reset the baselines and open the new borrow window.
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
    // §4.9 step 7 — reset the opposite stream's distribution carry before
    // this side's size changes.
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

    funding::reset_debts(env, &mut e.position, &e.market);
    funding::refresh_display(env, &mut e.ledger, &mut e.market);
    let physical = ledger::physical_cash(env);
    let equity = e.ledger.cash_lp_equity(env, physical);
    risk::evaluate_market_risk(
        env,
        &mut e.ledger,
        &e.action.market_id,
        &mut e.market,
        e.price,
        equity,
    );
    storage::save_market(env, &e.action.market_id, &e.market);
    // §4.9 step 9, then §4.10 — the rate is refreshed from the resulting
    // risk units, and only then is the replacement window quoted. §3.3.3:
    // nothing of the old window carries forward.
    borrow::refresh_rate(env, &mut e.ledger, physical);
    borrow::initialize_window(env, &e.ledger, &mut e.position);

    e.position.pending_mutation_action_id = None;
    storage::remove_pending_action(env, e.action.action_id);
    storage::save_position(env, &e.position);
    storage::save_ledger(env, &e.ledger);
    events::emit_increased(
        env,
        &e.position,
        payload.size_added,
        exposure.base_added,
        escrow,
        e.price,
        &collected,
    );
}

// ---------------------------------------------------------------------------
// §7.9 Decrease.
// ---------------------------------------------------------------------------

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
        Some(settle::Keeper {
            recipient: &keeper_address,
            reward,
            liquidation: false,
        }),
        ClosingFee::Charged,
    );
    storage::save_ledger(&env, &ledger);
    match &settled {
        settle::Settled::Partial(header) => events::emit_decreased(&env, header),
        // `create_decrease` requires `size_removed < position.size`, so a
        // decrease can never consume the position.
        settle::Settled::Closed(..) => {
            panic_with_error!(&env, PositionManagerError::InvariantViolation)
        }
    }
    ActionOutcome::Executed
}

/// §7.9 — the decrease preflight.
///
/// It projects the settlement's collateral arithmetic in the order
/// `settle::settle` performs it, so the three guards that keep this path
/// from diverging from the terminal path are evaluated *before* anything
/// moves: no uncollectible loss, no unpaid profit, and cash LP equity
/// covering the payable profit to be credited.
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

    // §6.5 — the payment-time cash limit. A surviving position may not
    // realize profit the vault could not pay: it has no result in which to
    // report the shortfall, and its closing fee would be computed from
    // profit that was never credited.
    let equity = e.ledger.cash_lp_equity(env, ledger::physical_cash(env));
    if payable > equity {
        return Some(FailureReason::UnpayableProfit);
    }

    // The §11.4 waterfall, as arithmetic. A surviving position may not carry
    // a loss its collateral could not absorb either, so the whole of it must
    // clear.
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
    // §6.12 — a voluntary settlement that cannot pay for itself must not
    // complete, so the reward is required in full here.
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
    let stored_final = math::sub(env, after_reward, closing.collectible);

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
    // Strictly above: liquidation eligibility is `effective <= threshold`,
    // so a survivor left exactly at it would be liquidatable the instant the
    // decrease committed.
    if math::add(env, stored_final, remaining_pnl) <= threshold {
        return Some(FailureReason::InsufficientCollateral);
    }
    None
}

// ---------------------------------------------------------------------------
// §7.10 Close.
// ---------------------------------------------------------------------------

pub fn settle_close(env: Env, keeper_address: Address, action_id: u64) -> ActionOutcome {
    let mut e = match eligible(&env, &keeper_address, action_id, ActionKind::Close, true) {
        Ok(e) => e,
        Err(outcome) => return outcome,
    };
    let ActionPayload::Close(payload) = e.action.payload.clone() else {
        panic_with_error!(&env, PositionManagerError::WrongActionKind);
    };
    let reward = keeper::reward_for(&e.config, keeper::RewardKind::Close);

    // A close needs no economic preflight beyond its price bound. Terminal
    // settlement may collect less than is owed and report the remainder as
    // bad debt, so there is no shortfall that could make it diverge — which
    // is exactly what a surviving decrease cannot do.
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
        Some(settle::Keeper {
            recipient: &keeper_address,
            reward,
            liquidation: false,
        }),
        ClosingFee::Charged,
    );
    storage::save_ledger(&env, &ledger);
    super::emit_terminal(&env, &settled, CloseReason::VoluntaryClose);
    ActionOutcome::Executed
}
