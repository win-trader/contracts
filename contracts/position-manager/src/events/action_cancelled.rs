use soroban_sdk::{contractevent, Address, Env, Symbol};

/// §7.5 — the owner cancelled a pending limit entry before its expiry. No
/// fee, no keeper reward, complete refund.
#[contractevent(topics = ["actcancel"], data_format = "map")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionCancelled {
    #[topic]
    pub action_id: u64,
    pub owner: Address,
    pub market: Symbol,
    pub refund: i128,
}

pub fn emit_action_cancelled(
    env: &Env,
    action_id: u64,
    owner: &Address,
    market: &Symbol,
    refund: i128,
) {
    ActionCancelled {
        action_id,
        owner: owner.clone(),
        market: market.clone(),
        refund,
    }
    .publish(env);
}
