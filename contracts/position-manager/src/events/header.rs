use soroban_sdk::{contracttype, Address, Env, Symbol};

/// §12.6 — the envelope every emitted result carries.
///
/// Events are the **only** durable record of a terminal outcome: §5.6 removes
/// the pending record and §5.14 removes the position, so a consumer that
/// misses an event cannot reconstruct it from state. That makes the envelope
/// part of the interface rather than decoration.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventHeader {
    /// Bumped whenever a field's meaning changes, never silently reused.
    /// Consumers pin the version they understand.
    pub event_version: u32,
    pub ledger_timestamp: u64,
    /// The market this result belongs to. **Empty** for a vault-wide event —
    /// §12.6 calls it absent, and an empty symbol is how a fixed-shape
    /// envelope says so without a second nullable type.
    pub market: Symbol,
    /// The caller credited with the action: the keeper for a settlement, the
    /// owner for a commitment, the authority for a configuration change.
    pub actor: Address,
}

/// The version every event in this build emits. One number for the whole
/// set, because a consumer pins "the shape of this protocol's events", not
/// the shape of each event independently.
pub const EVENT_VERSION: u32 = 1;

/// Envelope for a market-scoped result.
pub fn header(env: &Env, market: &Symbol, actor: &Address) -> EventHeader {
    EventHeader {
        event_version: EVENT_VERSION,
        ledger_timestamp: env.ledger().timestamp(),
        market: market.clone(),
        actor: actor.clone(),
    }
}

/// Envelope for a vault-wide result — configuration, pause, protocol
/// revenue, recapitalization, referrals.
pub fn vault_header(env: &Env, actor: &Address) -> EventHeader {
    EventHeader {
        event_version: EVENT_VERSION,
        ledger_timestamp: env.ledger().timestamp(),
        market: Symbol::new(env, "vault"),
        actor: actor.clone(),
    }
}
