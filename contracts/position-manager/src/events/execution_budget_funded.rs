use soroban_sdk::{contractevent, Env};

#[contractevent(topics = ["budgetin"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionBudgetFunded {
    pub position_id: u64,
    pub amount: i128,
}

pub fn emit_budget_funded(env: &Env, position_id: u64, amount: i128) {
    ExecutionBudgetFunded {
        position_id,
        amount,
    }
    .publish(env);
}
