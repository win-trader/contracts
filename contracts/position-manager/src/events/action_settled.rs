use super::EventHeader;
use shared::ActionKind;
use soroban_sdk::{contractevent, Address, Env, Symbol};

/// §12.6 — a committed action executed.
///
/// It is what ties an action id to the position it produced or changed.
/// `PositionOpened` and the change events say what happened to the position;
/// this says which commitment caused it, at which observation, and who was
/// paid for settling it. Without it an indexer cannot close the loop between
/// a trader's commitment and its outcome, because §5.6 removes the pending
/// record on the way out.
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
    /// The observation the fill was priced at — strictly newer than the
    /// action's `commit_observed_at` (§8.6).
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
