use super::EventHeader;
use soroban_sdk::{contractevent, Address, Env, Symbol};

#[contractevent(topics = ["actcancel"], data_format = "map")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionCancelled {
    #[topic]
    pub action_id: u64,
    pub header: EventHeader,
    pub owner: Address,
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
        header: super::header(env, market, owner),
        owner: owner.clone(),
        refund,
    }
    .publish(env);
}
