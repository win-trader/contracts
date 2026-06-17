use soroban_sdk::{contractclient, Address, BytesN, Env, Symbol, Vec};

use crate::types::OracleConfig;

/// OracleRouter contract interface.
/// SEP-40 median aggregation with a per-symbol price cache.
#[contractclient(name = "OracleRouterClient")]
pub trait OracleRouter {
    // Initialization is the contract `__constructor(config_manager_address)` —
    // atomic with deploy, closing the first-caller front-running window. Not a
    // trait method (Soroban constructors are inherent). The router holds no
    // admin of its own; all role checks cross-call the linked ConfigManager.

    /// Return the validated median price for `symbol` (scaled by 1e7).
    /// Returns a cached value if the last fetch is within `cache_duration`;
    /// otherwise queries sources fresh and refreshes the cache.
    fn get_price(env: Env, symbol: Symbol) -> i128;

    /// Add or replace the flat SEP-40 oracle source list for `symbol`.
    /// Sources form a single equally-weighted pool (no primary/secondary
    /// tiering). Source count capped at MAX_ORACLE_SOURCES.
    /// Callable only by ADMIN role (via ConfigManager).
    fn set_oracle_sources(env: Env, caller: Address, symbol: Symbol, sources: Vec<Address>);

    /// Update the global oracle safety thresholds.
    /// Callable only by ADMIN role (via ConfigManager).
    fn set_oracle_config(env: Env, caller: Address, config: OracleConfig);

    /// Returns the current oracle configuration.
    fn get_oracle_config(env: Env) -> OracleConfig;

    /// Extends the Soroban TTL of the OracleRouter's instance storage.
    fn bump_oracle_state(env: Env);

    /// Propose a WASM upgrade. UPGRADER role only.
    fn propose_upgrade(env: Env, caller: Address, wasm_hash: BytesN<32>);

    /// PAUSER veto of a pending upgrade.
    fn cancel_upgrade(env: Env, caller: Address);
}
