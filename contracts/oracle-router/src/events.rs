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

// Upgrade events live in `interfaces::events` — the
// `TimelockedUpgradeable` trait's default methods emit them, so no
// per-contract definition is needed here.
