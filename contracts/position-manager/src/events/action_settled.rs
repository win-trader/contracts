use super::EventHeader;
use shared::ActionKind;
use soroban_sdk::{contractevent, Address, Env, Symbol};

#[contractevent(topics = ["actsettle"], data_format = "map")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionSettled {
    #[topic]
    pub action_id: u64,
    pub header: EventHeader,
    pub owner: Address,
    pub kind: ActionKind,
    pub position_id: u64,
    pub fill_price: i128,
    pub fill_observed_at: u64,
    pub keeper_reward: i128,
}

#[allow(clippy::too_many_arguments)]
pub fn emit_action_settled(
    env: &Env,
    market: &Symbol,
    keeper: &Address,
    action_id: u64,
    owner: &Address,
    kind: ActionKind,
    position_id: u64,
    fill_price: i128,
    fill_observed_at: u64,
    keeper_reward: i128,
) {
    ActionSettled {
        action_id,
        header: super::header(env, market, keeper),
        owner: owner.clone(),
        kind,
        position_id,
        fill_price,
        fill_observed_at,
        keeper_reward,
    }
    .publish(env);
}
