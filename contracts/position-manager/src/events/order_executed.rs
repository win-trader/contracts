use soroban_sdk::{contractevent, Address, Env};

#[contractevent(topics = ["ordexec"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrderExecuted {
    pub position_id: u64,
    pub executor: Address,
    pub budget_paid: i128,
}

pub fn emit_order_executed(env: &Env, position_id: u64, executor: &Address, budget_paid: i128) {
    OrderExecuted {
        position_id,
        executor: executor.clone(),
        budget_paid,
    }
    .publish(env);
}
