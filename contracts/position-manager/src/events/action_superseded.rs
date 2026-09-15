use soroban_sdk::{contractevent, Address, Env};

/// §8.12 step 4-5 — a forced safety action removed the position out from
/// under a pending voluntary mutation. The mutation pays **no** reward of
/// its own (the forced action's caller receives only the forced reward) and
/// its complete added-collateral escrow goes back to the owner.
#[contractevent(topics = ["actsuper"], data_format = "map")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionSuperseded {
    #[topic]
    pub action_id: u64,
    pub position_id: u64,
    pub owner: Address,
    pub refund: i128,
}

pub fn emit_action_superseded(
    env: &Env,
    action_id: u64,
    position_id: u64,
    owner: &Address,
    refund: i128,
) {
    ActionSuperseded {
        action_id,
        position_id,
        owner: owner.clone(),
        refund,
    }
    .publish(env);
}
