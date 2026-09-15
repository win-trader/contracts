//! §7.11–7.12 — attached take-profit and stop-loss.
//!
//! A trigger is a standing conditional instruction stored inside the
//! position, not a market-style one-attempt commitment and not a prepaid
//! keeper budget. That difference decides its failure behaviour: a crossed
//! trigger whose exit bound does not pass leaves the instruction **attached**
//! and pays nothing (§8.7), because the trader asked to exit at a price, not
//! to exit once.
//!
//! Both close the position in full. There is no partial take-profit and
//! neither takes a size.

use soroban_sdk::{panic_with_error, Address, Env};

use shared::{ActionOutcome, Trigger, TriggerInstruction};

use crate::auth::require_initialized;
use crate::errors::PositionManagerError;
use crate::events::{self, CloseReason};
use crate::ledger;
use crate::settle::{self, ClosingFee};
use crate::{action, borrow, funding, keeper, risk, snapshot, storage};

/// Build an attached instruction, or the `None` variant for the zero
/// sentinel.
///
/// `commit_observed_at` is the caller's: an entry fill passes its own fill
/// observation (§8.6), and `set_take_profit` passes the observation it read
/// when attaching. Either way the instruction cannot execute on the
/// observation that armed it.
pub(crate) fn attach(
    trigger_price: i128,
    acceptable_price: i128,
    now: u64,
    execution_delay: u64,
    commit_observed_at: u64,
) -> Trigger {
    if trigger_price == 0 {
        return Trigger::None;
    }
    Trigger::Attached(TriggerInstruction {
        trigger_price,
        acceptable_price,
        committed_at: now,
        execute_after: now.saturating_add(execution_delay),
        commit_observed_at,
    })
}

/// §7.11 / §7.12 — attach or replace a trigger. Each attach records a fresh
/// commitment cursor and a fresh delay, so replacing a trigger cannot be
/// used to execute against an observation the trader has already seen.
fn set(env: &Env, position_id: u64, trigger_price: i128, acceptable_price: i128, take_profit: bool) {
    require_initialized(env);
    let mut position = storage::get_position(env, position_id);
    position.owner.require_auth();
    if trigger_price <= 0 || acceptable_price < 0 {
        panic_with_error!(env, PositionManagerError::InvalidOrder);
    }
    let commit = snapshot::read_stamped_price(env, &position.market);
    let now = env.ledger().timestamp();
    let delay = storage::get_market(env, &position.market)
        .config
        .order_execution_delay_seconds;
    let instruction = attach(
        trigger_price,
        acceptable_price,
        now,
        delay,
        commit.observed_at,
    );
    if take_profit {
        position.take_profit = instruction;
    } else {
        position.stop_loss = instruction;
    }
    storage::save_position(env, &position);
    events::emit_tp_sl_updated(env, &position);
}

fn clear(env: &Env, position_id: u64, take_profit: bool) {
    require_initialized(env);
    let mut position = storage::get_position(env, position_id);
    position.owner.require_auth();
    if take_profit {
        position.take_profit = Trigger::None;
    } else {
        position.stop_loss = Trigger::None;
    }
    storage::save_position(env, &position);
    events::emit_tp_sl_updated(env, &position);
}

pub fn set_take_profit(env: Env, position_id: u64, trigger_price: i128, acceptable_price: i128) {
    set(&env, position_id, trigger_price, acceptable_price, true);
}

pub fn clear_take_profit(env: Env, position_id: u64) {
    clear(&env, position_id, true);
}

pub fn set_stop_loss(env: Env, position_id: u64, trigger_price: i128, acceptable_price: i128) {
    set(&env, position_id, trigger_price, acceptable_price, false);
}

pub fn clear_stop_loss(env: Env, position_id: u64) {
    clear(&env, position_id, false);
}

pub fn execute_take_profit(env: Env, keeper_address: Address, position_id: u64) -> ActionOutcome {
    execute(&env, keeper_address, position_id, true)
}

pub fn execute_stop_loss(env: Env, keeper_address: Address, position_id: u64) -> ActionOutcome {
    execute(&env, keeper_address, position_id, false)
}

/// §7.11 / §7.12 — execute a crossed trigger.
///
/// Every gate here is non-terminal. A trigger that is early, unfunded by a
/// newer observation, uncrossed, or outside its exit bound simply stays
/// attached; none of those is an execution attempt and none pays a reward.
fn execute(
    env: &Env,
    keeper_address: Address,
    position_id: u64,
    take_profit: bool,
) -> ActionOutcome {
    require_initialized(env);
    keeper_address.require_auth();

    let position = storage::get_position(env, position_id);
    let instruction = match if take_profit {
        &position.take_profit
    } else {
        &position.stop_loss
    } {
        Trigger::Attached(i) => i.clone(),
        Trigger::None => panic_with_error!(env, PositionManagerError::TriggerNotAttached),
    };

    let now = env.ledger().timestamp();
    let config = storage::get_global_config(env);
    if !action::delay_satisfied(now, instruction.execute_after) {
        return ActionOutcome::NotReady;
    }
    // §8.5 — the minimum lifetime gates all four exits, triggers included.
    // A size increase re-locks the position against its own stop-loss;
    // exempting stop-loss alone would leave take-profit as the obvious
    // churn bypass, and exempting both would turn open / take-profit just
    // above entry / exit into a way around the rule entirely.
    if !action::lifetime_satisfied(now, &position, &config) {
        return ActionOutcome::NotReady;
    }
    let fill = snapshot::read_stamped_price(env, &position.market);
    if !action::fresh_for_commit(fill.observed_at, instruction.commit_observed_at) {
        return ActionOutcome::NotReady;
    }
    if !action::exit_trigger_crossed(
        position.is_long,
        take_profit,
        fill.price,
        instruction.trigger_price,
    ) {
        return ActionOutcome::Pending;
    }
    // §8.7 — a crossed trigger outside its bound stays attached and may
    // execute on a later qualifying observation. This is the one place a
    // failed price check is *not* terminal, and the reason is that the
    // instruction is standing rather than single-attempt.
    if !action::exit_price_allowed(position.is_long, fill.price, instruction.acceptable_price) {
        return ActionOutcome::Pending;
    }

    let mut ledger = storage::get_ledger(env);
    let mut market = storage::get_market(env, &position.market);
    borrow::accrue(env, &mut ledger, Some(&keeper_address), now);
    funding::accrue(env, &mut ledger, &position.market, Some(&keeper_address), &mut market, now);
    let physical = ledger::physical_cash(env);
    let equity = ledger.cash_lp_equity(env, physical);
    // §6.5 — refresh the side risk state from this fill before anything
    // reads a payout factor.
    risk::evaluate_market_risk(
        env,
        &mut ledger,
        &position.market,
        &keeper_address,
        &mut market,
        fill.price,
        equity,
    );

    // §8.12 — liquidation outranks every voluntary exit, triggers included.
    // The instruction stays attached; the liquidation path removes the
    // position.
    if risk::evaluate_liquidation(env, &ledger, &position, &market, fill.price).liquidatable {
        return ActionOutcome::RequiresLiquidation;
    }

    let reward = keeper::reward_for(
        &config,
        if take_profit {
            keeper::RewardKind::TakeProfit
        } else {
            keeper::RewardKind::StopLoss
        },
    );
    let size = position.size;
    let settled = settle::settle(
        env,
        &mut ledger,
        position,
        market,
        size,
        fill.price,
        settle::Keeper {
            recipient: &keeper_address,
            reward,
            liquidation: false,
        },
        // A losing stop-loss pays no closing fee because there is no profit
        // to charge it against, not because the path waives it; a stop-loss
        // moved above entry closes in profit and pays the normal fee.
        ClosingFee::Charged,
    );
    storage::save_ledger(env, &ledger);
    super::emit_terminal(
        env,
        &keeper_address,
        &settled,
        if take_profit {
            CloseReason::TakeProfit
        } else {
            CloseReason::StopLoss
        },
    );
    ActionOutcome::Executed
}
