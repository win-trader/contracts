use crate::{
    auth::require_auth, checkpoint, errors::PositionManagerError, events, ledger, math, risk,
    storage,
};
use soroban_sdk::{panic_with_error, Env};

pub fn withdraw_execution_budget(env: Env, position_id: u64, amount: i128) {
    if amount <= 0 {
        panic_with_error!(&env, PositionManagerError::InvalidAmount);
    }

    let mut position = storage::get_position(&env, position_id);
    require_auth(&position.owner);

    let mut ledger = storage::get_ledger(&env);
    checkpoint::checkpoint_global(&env, &mut ledger, env.ledger().timestamp());

    if amount > position.execution_budget {
        panic_with_error!(&env, PositionManagerError::InsufficientExecutionBudget);
    }
    position.execution_budget = math::sub(&env, position.execution_budget, amount);
    storage::save_position(&env, &position);
    ledger::payout_checked(
        &env,
        &mut ledger,
        ledger::Bucket::ExecutionBudget,
        &position.owner,
        amount,
    );
    risk::refresh_rate(&env, &mut ledger, ledger::physical_cash(&env));
    storage::save_ledger(&env, &ledger);

    events::emit_budget_withdrawn(&env, position_id, amount);
}
