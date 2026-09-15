//! §7.1–7.6 — the entry lifecycle: market and limit opens as commit then
//! settle.
//!
//! Creation performs structural and collateral checks and escrows the
//! trader's cash. It reserves no capacity, chooses no price, collects no fee,
//! pays no keeper, and creates no position — every economic decision belongs
//! to settlement, against an observation that did not exist when the trader
//! committed.
//!
//! Settlement returns an outcome. An eligible attempt that fails an expected
//! check is **terminal**: it pays the action's reward from escrow, refunds
//! the remainder to the owner, and consumes the record. Reverting instead
//! would give the trader a free retry after seeing the price, which is
//! exactly the option the two-phase lifecycle exists to remove.

use soroban_sdk::{panic_with_error, Address, Env, Symbol};

use shared::constants::BPS;
use shared::{
    ActionKind, ActionOutcome, ActionPayload, FailureReason, GlobalConfig, Market, OpenPayload,
    PendingAction, Position, TriggerCondition,
};

use crate::auth::{require_initialized, require_market_active};
use crate::errors::PositionManagerError;
use crate::events::{self, FeeSource};
use crate::ledger::{self, Ledger};
use crate::math::AddedExposure;
use crate::{action, borrow, fees, funding, keeper, math, risk, snapshot, storage, validation};

use super::trigger;

/// §7.1 / §7.3 — the two kinds differ in four values and nothing else.
struct EntryKind {
    kind: ActionKind,
    reward: keeper::RewardKind,
    /// `true` for a limit entry: bounded by `max_order_lifetime_seconds`
    /// rather than the shorter market-entry cap, because a resting order is
    /// meant to rest (§8.5).
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

// ---------------------------------------------------------------------------
// Creation (§7.1, §7.3).
// ---------------------------------------------------------------------------

/// §7.1 — commit a market open. Binding: there is no cancel operation, only
/// settlement or expiry.
pub fn create_market_open(env: Env, owner: Address, market: Symbol, request: OpenPayload) -> u64 {
    create_entry(&env, owner, market, request, 0, &MARKET)
}

/// §7.3 — commit a limit open. Cancellable by the owner until expiry.
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
    require_initialized(env);
    owner.require_auth();
    require_market_active(env, &market_symbol);
    if request.size <= 0 || request.submitted_collateral <= 0 {
        panic_with_error!(env, PositionManagerError::InvalidAmount);
    }

    let market = storage::get_market(env, &market_symbol);
    let config = storage::get_global_config(env);
    let now = env.ledger().timestamp();
    let commit = snapshot::read_stamped_price(env, &market_symbol);

    // §7.1 — the side must be able to take new exposure *now*. The pause
    // folds into this predicate (§6.16.1), so a paused vault accepts no new
    // commitments rather than accumulating a queue for an unpause that may
    // never come.
    if !risk::side_accepts_new_exposure(env, market.side(request.is_long)) {
        panic_with_error!(env, PositionManagerError::RiskStateBlocked);
    }

    // A limit entry fills at roughly its trigger, so its attached exits are
    // validated against the trigger; a market entry against the price it is
    // committing at.
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

    // The dust guard §7.8 states for an increase applies here for the same
    // reason, one step weaker. A size too small to move base or risk units
    // makes `derive_added_exposure` revert at settlement (§6.13), and a
    // revert leaves the entry pending — so the trader holds a free retry
    // against every observation until expiry instead of the single binding
    // attempt they committed to. Rejecting it against the commitment price
    // is what makes those requires unreachable.
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
    // §9.11 — the escrow must cover the settlement charges *and* the expiry
    // reward, so both terminal paths remain payable from it however the
    // order ends. Entry rewards are read from live configuration rather than
    // frozen, and §10.3.1's `every reward <= min_collateral` bound is what
    // keeps a later configuration change from stranding one.
    if after_charges < config.min_collateral
        || request.submitted_collateral < config.keeper_rewards.expiry
        || after_charges < risk::initial_requirement(env, request.size, &market.config)
    {
        panic_with_error!(env, PositionManagerError::InsufficientCollateral);
    }

    let mut ledger = storage::get_ledger(env);
    borrow::accrue(env, &mut ledger, now);
    ledger::escrow_in(env, &mut ledger, &owner, request.submitted_collateral);

    let payload = if entry.resting {
        ActionPayload::LimitOpen(
            request.clone(),
            TriggerCondition {
                trigger_price,
                // Frozen here against the authenticated commit price rather
                // than re-derived at settlement, so a price that has since
                // crossed cannot silently flip the order's meaning.
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
    // Escrow raises physical cash and claims by the same amount, so cash LP
    // equity is unchanged and the rate does not move — but the ledger is
    // written either way, and refreshing from the post-change book is the
    // §4.9 order rather than a special case.
    borrow::refresh_rate(env, &mut ledger, ledger::physical_cash(env));
    storage::save_ledger(env, &ledger);
    events::emit_action_committed(env, &action);
    action.action_id
}

// ---------------------------------------------------------------------------
// Settlement (§7.2, §7.4).
// ---------------------------------------------------------------------------

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
    require_initialized(env);
    // §7.0 — "keeper authorization" is the caller authenticating the address
    // that receives the reward. There is no allowlist; execution is
    // permissionless.
    keeper_address.require_auth();

    let mut action = action::load(env, action_id, entry.kind);
    let open = action
        .payload
        .open()
        .unwrap_or_else(|| panic_with_error!(env, PositionManagerError::WrongActionKind))
        .clone();
    let now = env.ledger().timestamp();

    // The three non-terminal gates, in the §7.2 order. None of them is an
    // execution attempt: none consumes the action, none pays a reward, and
    // none may revert.
    if action::expired(now, open.expires_at) {
        return ActionOutcome::Expired;
    }
    if !action::delay_satisfied(now, action.execute_after) {
        return ActionOutcome::NotReady;
    }
    let fill = snapshot::read_stamped_price(env, &action.market_id);
    if !action::fresh_for_commit(fill.observed_at, action.commit_observed_at) {
        return ActionOutcome::NotReady;
    }
    if let ActionPayload::LimitOpen(_, condition) = &action.payload {
        // §8.3 — an untriggered observation is not an attempt. The order
        // keeps resting; only a crossed trigger makes the attempt terminal.
        if !action::trigger_crossed(condition.trigger_above, fill.price, condition.trigger_price) {
            return ActionOutcome::Pending;
        }
    }

    let mut ledger = storage::get_ledger(env);
    let mut market = storage::get_market(env, &action.market_id);
    borrow::accrue(env, &mut ledger, now);
    funding::accrue(env, &mut ledger, &mut market, now);
    let physical = ledger::physical_cash(env);
    let equity = ledger.cash_lp_equity(env, physical);
    // §6.5 — the side risk state is refreshed from this fill before anything
    // reads a payout factor or asks whether the side accepts exposure.
    risk::evaluate_market_risk(env, &mut ledger, &action.market_id, &mut market, fill.price, equity);

    let config = storage::get_global_config(env);
    let reward = keeper::reward_for(&config, entry.reward);
    let opening_fee = fees::calculate_opening_fee(env, open.size, &market.config);
    let after_charges = math::sub(
        env,
        math::sub(env, action.escrowed_collateral, reward),
        opening_fee,
    );
    let exposure = math::derive_added_exposure(
        env,
        open.is_long,
        0,
        0,
        open.size,
        fill.price,
        market.config.market_risk_factor_bps,
    );

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
            // §6.11 — only the LP share of a collected fee reaches cash LP
            // equity. The protocol and referral slices become claims of
            // their own, and the keeper reward leaves the vault against the
            // escrow claim that funded it, so neither moves equity at all.
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

/// §7.2 — the complete preflight, against the hypothetical post-settlement
/// state. Every check here is an **expected** deterministic condition that
/// may legitimately have changed since commitment, so each returns a reason
/// rather than reverting (§8.9).
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
    // §9.11 — a market disabled while the entry waited is not a reason to
    // hold the escrow. The action drains: it terminates, pays, and refunds.
    if storage::is_market_disabled(env, market_symbol) {
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

    // §7.2 `projected_minimum_borrow`. The position's borrow window opens
    // after the health checks, and its monetary minimum is part of pending
    // borrow from the window's first second — so without this term a
    // position could be admitted exactly at initial margin and be under it
    // the moment its window exists. Both inputs come from the projected
    // post-settlement book, never the pre-action one.
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

    // §8.8 — capacity is evaluated from the post-fee, post-reward,
    // post-mutation state. Pending entries reserve nothing, so two orders
    // can each look fillable and the first to settle consumes the room.
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

/// §7.2 — the success path: pay, charge, create, and open the borrow window,
/// in that order.
///
/// Escrow is distributed exactly once and reaches zero before the record is
/// removed, satisfying §9.11's identity `escrow_before = keeper_reward +
/// opening_fee + initial_position_collateral`.
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

    // §6.8 — the opening fee, collected only now that every execution check
    // has passed (§9.12). It leaves escrow for the LP residual, and the
    // distribution then carves the protocol and referral shares out of it.
    if opening_fee > 0 {
        action.escrowed_collateral = math::sub(env, action.escrowed_collateral, opening_fee);
        ledger::spend_escrow(env, ledger, opening_fee);
        fees::distribute_open_close_revenue(
            env,
            ledger,
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
        borrow_debt: 0,
        // §3.3.2 — quoted by `initialize_window` below, after the rate
        // refresh, never here.
        stored_minimum_borrow_fee: 0,
        funding_paid_to_receivers_debt: 0,
        funding_paid_to_lps_debt: 0,
        funding_received_debt: 0,
        opened_at: now,
        last_size_increase_at: now,
        pending_mutation_action_id: None,
        // §8.6 — the fill's observation becomes the commitment cursor for
        // both attached exits, so neither can close the position on the same
        // observation that opened it.
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
    // §4.9 step 7 — this side's size is about to change, so the opposite
    // payer stream's receiver-distribution carry is no longer valid under
    // the new divisor.
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
    funding::reset_debts(env, &mut position, market);
    funding::refresh_display(env, ledger, market);
    // §7.2 — refresh the market display **and** risk state from the
    // resulting book. A position opened at the fill carries no PnL of its
    // own, so this ordinarily changes nothing; it runs because the side's
    // state is a function of the book as it now stands, not as it stood
    // before the preflight.
    let physical_after = ledger::physical_cash(env);
    let equity_after = ledger.cash_lp_equity(env, physical_after);
    risk::evaluate_market_risk(env, ledger, &action.market_id, market, price, equity_after);
    storage::save_market(env, &action.market_id, market);
    // §4.9 step 9, then §4.10 — the rate is refreshed from the resulting
    // risk units and claims, and only then is the borrow window quoted.
    borrow::refresh_rate(env, ledger, physical_after);
    borrow::initialize_window(env, ledger, &mut position);
    storage::save_position(env, &position);
    storage::remove_pending_action(env, action.action_id);
    storage::save_ledger(env, ledger);
    events::emit_opened(env, &position, price);
}

// ---------------------------------------------------------------------------
// Cancellation and expiry (§7.5, §7.6).
// ---------------------------------------------------------------------------

/// §7.5 — the owner withdraws a resting limit entry. Complete refund, no
/// fee, no keeper reward. A market entry does not expose this operation.
pub fn cancel_limit_open(env: Env, action_id: u64) -> i128 {
    require_initialized(&env);
    let mut action = action::load(&env, action_id, ActionKind::LimitOpen);
    action.owner.require_auth();
    let expires_at = action
        .payload
        .open()
        .unwrap_or_else(|| panic_with_error!(&env, PositionManagerError::WrongActionKind))
        .expires_at;
    // At or past expiry only the cleanup path is valid (§8.13), so both
    // cannot consume the same record.
    if action::expired(env.ledger().timestamp(), expires_at) {
        panic_with_error!(&env, PositionManagerError::InvalidOrder);
    }

    let mut ledger = storage::get_ledger(&env);
    borrow::accrue(&env, &mut ledger, env.ledger().timestamp());
    let refund = action.escrowed_collateral;
    action.escrowed_collateral = 0;
    ledger::refund_escrow(&env, &mut ledger, &action.owner, refund);
    storage::remove_pending_action(&env, action_id);
    borrow::refresh_rate(&env, &mut ledger, ledger::physical_cash(&env));
    storage::save_ledger(&env, &ledger);
    events::emit_action_cancelled(&env, action_id, &action.owner, &action.market_id, refund);
    refund
}

/// §7.6 — permissionless cleanup of an expired entry. Pays
/// `keeper_expiry_reward` from escrow and refunds the remainder to the
/// owner. Creation guaranteed the escrow covers this reward.
pub fn clean_expired_entry(env: Env, keeper_address: Address, action_id: u64) {
    require_initialized(&env);
    keeper_address.require_auth();
    let mut action = action::load_entry(&env, action_id);
    let expires_at = action.payload.open().unwrap().expires_at;
    let now = env.ledger().timestamp();
    if !action::expired(now, expires_at) {
        panic_with_error!(&env, PositionManagerError::TooEarly);
    }

    let mut ledger = storage::get_ledger(&env);
    borrow::accrue(&env, &mut ledger, now);
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
