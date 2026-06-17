//! Cross-contract event shapes for the upgrade flow. Defining the events
//! here (instead of redeclaring them in each contract's `events.rs`) means
//! off-chain consumers parse one shape across four contracts.
//!
//! Topic strings are part of the on-chain event identity — they MUST be the
//! literals listed below so an indexer keyed on `topic0` can dispatch
//! contract-agnostically.

use soroban_sdk::{contractevent, Address, BytesN};

/// Emitted by `propose_upgrade`. Off-chain monitoring records the proposed
/// `wasm_hash` + `eta` and flags any subsequent `upgrade()` call whose hash
/// diverges or that fires before `eta`.
#[contractevent(topics = ["upgprp"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpgradeProposed {
    pub wasm_hash: BytesN<32>,
    pub eta: u64,
}

/// Emitted by `cancel_upgrade` (PAUSER veto path).
#[contractevent(topics = ["upgcan"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpgradeCancelled {
    pub caller: Address,
}
