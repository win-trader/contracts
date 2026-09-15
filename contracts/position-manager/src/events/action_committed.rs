use shared::{ActionKind, PendingAction};
use soroban_sdk::{contractevent, Address, Env, Symbol};

/// §12.6 — a trader committed a price-sensitive action. Nothing economic
/// has happened yet: no capacity is reserved, no price is chosen, no fee is
/// collected. The two cursors are what settlement is held to.
#[contractevent(topics = ["actcommit"], data_format = "map")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionCommitted {
    #[topic]
    pub action_id: u64,
    pub owner: Address,
    pub market: Symbol,
    pub kind: ActionKind,
    pub created_at: u64,
    pub execute_after: u64,
    /// The observation cursor a fill must be strictly newer than (§8.6).
    pub commit_observed_at: u64,
    pub escrowed_collateral: i128,
    /// `0` for a mutation; the entry's expiry boundary otherwise.
    pub expires_at: u64,
}

pub fn emit_action_committed(env: &Env, action: &PendingAction) {
    ActionCommitted {
        action_id: action.action_id,
        owner: action.owner.clone(),
        market: action.market_id.clone(),
        kind: action.kind,
        created_at: action.created_at,
        execute_after: action.execute_after,
        commit_observed_at: action.commit_observed_at,
        escrowed_collateral: action.escrowed_collateral,
        expires_at: action.payload.open().map(|o| o.expires_at).unwrap_or(0),
    }
    .publish(env);
}
