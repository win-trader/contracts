use soroban_sdk::{contractevent, Address, Symbol, Vec};

#[contractevent(topics = ["price"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PriceFetch {
    #[topic]
    pub symbol: Symbol,
    pub price: i128,
    pub timestamp: u64,
}

/// Emitted by `set_oracle_config` whenever the global safety thresholds
/// change. Mirrors every field of the on-chain `OracleConfig` struct.
#[contractevent(topics = ["orccfg"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OracleConfigUpdate {
    pub staleness: u64,
    pub deviation: i128,
    pub cache_duration: u64,
    pub min_required_sources: u32,
}

/// Emitted by `set_oracle_sources` so off-chain monitoring can detect every
/// rotation of the source set.
#[contractevent(topics = ["orcsrc"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OracleSourcesUpdate {
    #[topic]
    pub symbol: Symbol,
    pub sources: Vec<Address>,
}

/// Emitted by `publish_round` — the push signal for anything waiting on a
/// canonical round (the FIFO LP request queue in particular).
#[contractevent(topics = ["roundpub"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoundPublished {
    pub id: u64,
    pub timestamp: u64,
    pub previous_id: u64,
}

// Upgrade events live in `shared::events` — the
// `TimelockedUpgradeable` trait's default methods emit them, so no
// per-contract definition is needed here.
