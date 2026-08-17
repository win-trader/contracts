use crate::{
    auth::require_auth, checkpoint, errors::PositionManagerError, events, ledger, math, risk,
    storage,
};
use shared::VaultClient;
use soroban_sdk::{panic_with_error, Env};

pub fn fund_execution_budget(env: Env, position_id: u64, amount: i128) {
    if amount <= 0 {
        panic_with_error!(&env, PositionManagerError::InvalidAmount);
    }

    let mut position = storage::get_position(&env, position_id);
    require_auth(&position.owner);

    let mut ledger = storage::get_ledger(&env);
    checkpoint::checkpoint_global(&env, &mut ledger, env.ledger().timestamp());

    VaultClient::new(&env, &storage::get_vault(&env)).receive_collateral(
        &env.current_contract_address(),
        &position.owner,
        &amount,
    );
    position.execution_budget = math::add(&env, position.execution_budget, amount);
    ledger.execution_budget_total = math::add(&env, ledger.execution_budget_total, amount);
    storage::save_position(&env, &position);
    risk::refresh_rate(&env, &mut ledger, ledger::physical_cash(&env));
    storage::save_ledger(&env, &ledger);

    events::emit_budget_funded(&env, position_id, amount);
}
