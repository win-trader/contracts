use super::EventHeader;
use soroban_sdk::{contractevent, Address, Env};

/// §12.6 — the global borrow index advanced.
///
/// `rate_applied` is the **stored** rate the elapsed window was priced at,
/// which is what §9.6 requires: the rate is re-derived only *after* the
/// mutation that changed it, so a state change can never reprice time that
/// already passed.
///
/// §12.6 also lists "rate after". It is deliberately absent. At checkpoint
/// time the resulting rate is not yet known — `refresh_borrow_rate` runs at
/// the end of the same transaction, from the post-mutation risk units and
/// equity — so a value emitted here would be the *old* rate wearing the new
/// rate's name. The post-refresh figure is carried by `MarketCheckpoint`.
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
