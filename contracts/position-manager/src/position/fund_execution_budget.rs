use crate::{
    auth::require_auth, borrow, errors::PositionManagerError, events, ledger, math, storage,
};
use soroban_sdk::{panic_with_error, Env};

pub fn fund_execution_budget(env: Env, position_id: u64, amount: i128) {
    if amount <= 0 {
        panic_with_error!(&env, PositionManagerError::InvalidAmount);
    }

    let mut position = storage::get_position(&env, position_id);
    require_auth(&position.owner);

    let mut ledger = storage::get_ledger(&env);
    borrow::accrue(&env, &mut ledger, env.ledger().timestamp());

    ledger::receive(&env, &position.owner, amount);
    position.execution_budget = math::add(&env, position.execution_budget, amount);
    ledger.credit(&env, ledger::Bucket::ExecutionBudget, amount);
    storage::save_position(&env, &position);
    borrow::refresh_rate(&env, &mut ledger, ledger::physical_cash(&env));
    storage::save_ledger(&env, &ledger);

    events::emit_budget_funded(&env, position_id, amount);
}
