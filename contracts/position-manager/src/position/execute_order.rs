use crate::{
    auth::{require_auth, require_initialized},
    borrow,
    errors::PositionManagerError,
    events, funding, ledger, settle, snapshot, storage,
};
use soroban_sdk::{panic_with_error, Address, Env};

pub fn execute_order(env: Env, caller: Address, position_id: u64) {
    require_initialized(&env);
    require_auth(&caller);

    let mut position = storage::get_position(&env, position_id);
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

    let mut ledger = storage::get_ledger(&env);
    let budget = position.execution_budget;
    position.execution_budget = 0;
    storage::save_position(&env, &position);
    ledger::payout(
        &env,
        &mut ledger,
        ledger::Bucket::ExecutionBudget,
        &caller,
        budget,
    );

    let mut market = storage::get_market(&env, &position.market);
    let now = env.ledger().timestamp();
    borrow::accrue(&env, &mut ledger, now);
    funding::accrue(&env, &mut ledger, &mut market, now);

    let size = position.size;
    let summary = settle::settle_close(&env, &mut ledger, position, market, size, 0, price, None);
    storage::save_ledger(&env, &ledger);

    events::emit_order_executed(&env, position_id, &caller, budget);
    events::emit_closed(&env, &summary, events::CloseReason::Order);
}
