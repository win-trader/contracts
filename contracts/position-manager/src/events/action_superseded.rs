use super::EventHeader;
use soroban_sdk::{contractevent, Address, Env, Symbol};

/// §8.12 step 4-5 — a forced safety action removed the position out from
/// under a pending voluntary mutation. The mutation pays **no** reward of
/// its own (the forced action's caller receives only the forced reward) and
/// its complete added-collateral escrow goes back to the owner.
#[contractevent(topics = ["actsuper"], data_format = "map")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionSuperseded {
    #[topic]
    pub action_id: u64,
    pub header: EventHeader,
    pub position_id: u64,
    pub owner: Address,
    pub refund: i128,
}

pub fn emit_action_superseded(
    env: &Env,
    market: &Symbol,
    actor: &Address,
    action_id: u64,
    position_id: u64,
    owner: &Address,
    refund: i128,
) {
    ActionSuperseded {
        action_id,
        // The actor is the forced action's caller, not the owner of the
        // mutation being superseded: §8.11 credits one keeper action.
        header: super::header(env, market, actor),
        position_id,
        owner: owner.clone(),
        refund,
    }
    .publish(env);
}
