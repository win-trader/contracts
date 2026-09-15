use super::EventHeader;
use soroban_sdk::{contractevent, Address, Env, Symbol};

/// §7.6 — a keeper cleaned an entry at or after its expiry boundary. The
/// expiry reward comes out of escrow and the remainder goes back to the
/// owner frozen in the action.
#[contractevent(topics = ["actexpire"], data_format = "map")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionExpired {
    #[topic]
    pub action_id: u64,
    pub header: EventHeader,
    pub owner: Address,
    pub keeper: Address,
    pub reward: i128,
    pub refund: i128,
}

pub fn emit_action_expired(
    env: &Env,
    action_id: u64,
    owner: &Address,
    market: &Symbol,
    keeper: &Address,
    reward: i128,
    refund: i128,
) {
    ActionExpired {
        action_id,
        header: super::header(env, market, keeper),
        owner: owner.clone(),
        keeper: keeper.clone(),
        reward,
        refund,
    }
    .publish(env);
}
