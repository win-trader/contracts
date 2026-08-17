use soroban_sdk::{contractevent, Env};

#[contractevent(topics = ["baddebt"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BadDebt {
    pub position_id: u64,
    pub amount: i128,
}

pub fn emit_bad_debt(env: &Env, position_id: u64, amount: i128) {
    BadDebt {
        position_id,
        amount,
    }
    .publish(env);
}
