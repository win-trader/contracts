use crate::{auth::require_auth, events, snapshot, storage, validation};
use soroban_sdk::Env;

pub fn set_tp_sl(env: Env, position_id: u64, take_profit: i128, stop_loss: i128) {
    let mut position = storage::get_position(&env, position_id);
    require_auth(&position.owner);

    let price = snapshot::authenticated_price(&env, &position.market);
    validation::validate_orders(&env, position.is_long, take_profit, stop_loss, price);

    position.take_profit = take_profit;
    position.stop_loss = stop_loss;
    storage::save_position(&env, &position);

    events::emit_tp_sl_updated(&env, &position);
}
