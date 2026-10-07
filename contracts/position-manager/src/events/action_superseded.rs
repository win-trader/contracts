use super::EventHeader;
use soroban_sdk::{contractevent, Address, Env, Symbol};

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
        header: super::header(env, market, actor),
        position_id,
        owner: owner.clone(),
        refund,
    }
    .publish(env);
}
