use super::EventHeader;
use soroban_sdk::{contractevent, Address, Env, Symbol};

#[contractevent(topics = ["baddebt"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BadDebt {
    pub header: EventHeader,
    pub position_id: u64,
    pub amount: i128,
}

pub fn emit_bad_debt(
    env: &Env,
    market: &Symbol,
    actor: &Address,
    position_id: u64,
    amount: i128,
) {
    BadDebt {
        header: super::header(env, market, actor),
        position_id,
        amount,
    }
    .publish(env);
}
