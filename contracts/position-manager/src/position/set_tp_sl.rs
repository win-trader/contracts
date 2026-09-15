use crate::{auth::require_auth, events, snapshot, storage, validation};
use shared::{Trigger, TriggerInstruction};
use soroban_sdk::Env;

/// §5.5 — attach a trigger, or `None` for the zero sentinel.
///
/// `commit_observed_at` is left at zero: the oracle read still returns a
/// bare price, so there is no observation cursor to record. P7-01 makes the
/// read stamped and P6-22 records a fresh cursor on every attach or
/// replace, which is what stops a trigger from executing on the same
/// observation that armed it.
pub(crate) fn attach_trigger(trigger_price: i128, now: u64, execution_delay: u64) -> Trigger {
    if trigger_price == 0 {
        return Trigger::None;
    }
    Trigger::Attached(TriggerInstruction {
        trigger_price,
        acceptable_price: 0,
        committed_at: now,
        execute_after: now.saturating_add(execution_delay),
        commit_observed_at: 0,
    })
}

pub fn set_tp_sl(env: Env, position_id: u64, take_profit: i128, stop_loss: i128) {
    let mut position = storage::get_position(&env, position_id);
    require_auth(&position.owner);

    let price = snapshot::authenticated_price(&env, &position.market);
    validation::validate_orders(&env, position.is_long, take_profit, stop_loss, price);

    let now = env.ledger().timestamp();
    let delay = storage::get_market(&env, &position.market)
        .config
        .order_execution_delay_seconds;
    position.take_profit = attach_trigger(take_profit, now, delay);
    position.stop_loss = attach_trigger(stop_loss, now, delay);
    storage::save_position(&env, &position);

    events::emit_tp_sl_updated(&env, &position);
}
