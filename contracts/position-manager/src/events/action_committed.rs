use shared::{ActionKind, PendingAction};
use super::EventHeader;
use soroban_sdk::{contractevent, Address, Env};

#[contractevent(topics = ["actcommit"], data_format = "map")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionCommitted {
    #[topic]
    pub action_id: u64,
    pub header: EventHeader,
    pub owner: Address,
    pub kind: ActionKind,
    pub created_at: u64,
    pub execute_after: u64,
    pub commit_observed_at: u64,
    pub escrowed_collateral: i128,
    pub expires_at: u64,
}

pub fn emit_action_committed(env: &Env, action: &PendingAction) {
    ActionCommitted {
        action_id: action.action_id,
        header: super::header(env, &action.market_id, &action.owner),
        owner: action.owner.clone(),
        kind: action.kind,
        created_at: action.created_at,
        execute_after: action.execute_after,
        commit_observed_at: action.commit_observed_at,
        escrowed_collateral: action.escrowed_collateral,
        expires_at: action.payload.open().map(|o| o.expires_at).unwrap_or(0),
    }
    .publish(env);
}
