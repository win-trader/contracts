use shared::{ActionKind, FailureReason};
use super::EventHeader;
use soroban_sdk::{contractevent, Address, Env, Symbol};

#[contractevent(topics = ["actfail"], data_format = "map")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionFailed {
    #[topic]
    pub action_id: u64,
    pub header: EventHeader,
    pub owner: Address,
    pub kind: ActionKind,
    pub reason: FailureReason,
    pub keeper: Address,
    pub reward: i128,
    pub refund: i128,
}

#[allow(clippy::too_many_arguments)]
pub fn emit_action_failed(
    env: &Env,
    action_id: u64,
    owner: &Address,
    market: &Symbol,
    kind: ActionKind,
    reason: FailureReason,
    keeper: &Address,
    reward: i128,
    refund: i128,
) {
    ActionFailed {
        action_id,
        header: super::header(env, market, keeper),
        owner: owner.clone(),
        kind,
        reason,
        keeper: keeper.clone(),
        reward,
        refund,
    }
    .publish(env);
}
