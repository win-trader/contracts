use soroban_sdk::{contractclient, Address, BytesN, Env, Symbol};

use crate::types::{GlobalConfig, MarketConfig};

/// Owns configuration changes: proposals, timelocks, and the conservative fast
/// path. Installs into the PositionManager, which keeps the live config.
#[contractclient(name = "MarketGovernorClient")]
pub trait MarketGovernor {
    /// Propose a global config (ADMIN). Conservative changes install immediately.
    fn propose_global_config(env: Env, caller: Address, config: GlobalConfig);

    /// Install a due global proposal. Permissionless; expires one timelock after it is due.
    fn apply_global_config(env: Env, caller: Address);

    /// Withdraw the pending global proposal (ADMIN).
    fn cancel_global_config(env: Env, caller: Address);

    /// Register a new market immediately, or propose a change to a known one (ADMIN).
    fn propose_market_config(env: Env, caller: Address, market_symbol: Symbol, config: MarketConfig);

    /// Install a due market proposal. Permissionless.
    fn apply_market_config(env: Env, caller: Address, market_symbol: Symbol);

    /// Withdraw a pending market proposal (ADMIN).
    fn cancel_market_config(env: Env, caller: Address, market_symbol: Symbol);

    /// Propose a replacement price feed (ORACLE). Applies after `config_timelock_seconds`.
    fn propose_price_feed(env: Env, caller: Address, price_feed: Address);

    /// Install a due price-feed proposal. Permissionless.
    fn apply_price_feed(env: Env, caller: Address);

    /// Withdraw a pending price-feed proposal (ORACLE or ADMIN).
    fn cancel_price_feed(env: Env, caller: Address);

    /// Remove an empty market from the registry (ADMIN).
    fn deregister_market(env: Env, caller: Address, market_symbol: Symbol);

    /// The PositionManager this governor configures.
    fn position_manager(env: Env) -> Address;

    /// Propose a WASM upgrade (UPGRADER).
    fn propose_upgrade(env: Env, caller: Address, wasm_hash: BytesN<32>);

    /// Cancel a pending upgrade (PAUSER).
    fn cancel_upgrade(env: Env, caller: Address);
}
