use soroban_sdk::{panic_with_error, Address, Env};

use shared::{ActionOutcome, Trigger, TriggerInstruction};

use crate::errors::PositionManagerError;
use crate::events::{self, CloseReason};
use crate::ledger;
use crate::settle::{self, ClosingFee};
use crate::{action, borrow, funding, keeper, risk, snapshot, storage};

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

fn set(env: &Env, position_id: u64, trigger_price: i128, acceptable_price: i128, take_profit: bool) {
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

fn execute(
    env: &Env,
    keeper_address: Address,
    position_id: u64,
    take_profit: bool,
) -> ActionOutcome {
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
    if !action::lifetime_satisfied(now, &position, &config) {
        return ActionOutcome::NotReady;
    }
    let fill = snapshot::read_stamped_price(env, &position.market);
    if !action::fresh_for_commit(
        fill.observed_at,
        instruction.commit_observed_at,
        instruction.committed_at,
    ) {
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
    if !action::exit_price_allowed(position.is_long, fill.price, instruction.acceptable_price) {
        return ActionOutcome::Pending;
    }

    let mut ledger = storage::get_ledger(env);
    let mut market = storage::get_market(env, &position.market);
    borrow::accrue(env, &mut ledger, Some(&keeper_address), now);
    funding::accrue(env, &mut ledger, &position.market, Some(&keeper_address), &mut market, now);
    let physical = ledger::physical_cash(env);
    let equity = ledger.cash_lp_equity(env, physical);
    risk::evaluate_market_risk(
        env,
        &mut ledger,
        &position.market,
        &keeper_address,
        &mut market,
        fill.price,
        equity,
    );

    if risk::evaluate_liquidation(env, &ledger, &position, &market, fill.price).liquidatable {
        action::persist_accrual(env, &mut ledger, &position.market, &market, physical);
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
