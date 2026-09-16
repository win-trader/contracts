use super::EventHeader;
use soroban_sdk::{contractevent, Address, Env};

#[contractevent(topics = ["borrowchk"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BorrowCheckpoint {
    pub header: EventHeader,
    pub elapsed: u64,
    pub index_delta: i128,
    pub rate_applied: i128,
    pub index_after: i128,
}

pub fn emit_borrow_checkpoint(
    env: &Env,
    actor: &Address,
    elapsed: u64,
    index_delta: i128,
    rate_applied: i128,
    index_after: i128,
) {
    BorrowCheckpoint {
        header: super::vault_header(env, actor),
        elapsed,
        index_delta,
        rate_applied,
        index_after,
    }
    .publish(env);
}
