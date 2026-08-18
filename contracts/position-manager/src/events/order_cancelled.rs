use soroban_sdk::{contractevent, contracttype, Env};

/// Why an entry order was removed before filling.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CancelReason {
    /// The owner cancelled it.
    Owner,
    /// It passed its expiry and was swept.
    Expired,
    /// A fill attempt could not pull the collateral (allowance/balance gone).
    PullFailed,
}

/// An entry order was removed before filling.
#[contractevent(topics = ["ordcancel"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrderCancelled {
    pub order_id: u64,
    pub reason: CancelReason,
}

pub fn emit_order_cancelled(env: &Env, order_id: u64, reason: CancelReason) {
    OrderCancelled { order_id, reason }.publish(env);
}
