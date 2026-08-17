use soroban_sdk::{contractevent, Env};

#[contractevent(topics = ["budgetout"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionBudgetWithdrawn {
    pub position_id: u64,
    pub amount: i128,
}

pub fn emit_budget_withdrawn(env: &Env, position_id: u64, amount: i128) {
    ExecutionBudgetWithdrawn {
        position_id,
        amount,
    }
    .publish(env);
}
