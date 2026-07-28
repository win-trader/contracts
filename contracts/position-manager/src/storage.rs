//! Storage layout.
//!
//! Instance storage holds the wiring addresses, configs, pause flag, and the
//! one `Ledger` aggregate (all global accounting lives inside it — business
//! logic never reads a bare accounting key). Positions and markets are
//! persistent entries with explicit TTL extension; anyone can re-extend a
//! position via `bump_position`.

use shared::constants::{SHARED_BUMP, SHARED_THRESHOLD};
use shared::{GlobalConfig, Market, Position};
use soroban_sdk::{contracttype, panic_with_error, Address, Env, Symbol, Vec};

use crate::errors::PositionManagerError;
use crate::ledger::Ledger;

#[contracttype]
#[derive(Clone)]
pub enum Key {
    ConfigManager,
    OracleRouter,
    Vault,
    GlobalConfig,
    Initialized,
    Paused,
    NextPositionId,
    ActiveMarkets,
    Ledger,
    Version,
    Position(u64),
    Market(Symbol),
    MarketDisabled(Symbol),
}

pub fn set<T: soroban_sdk::IntoVal<Env, soroban_sdk::Val> + Clone>(
    env: &Env,
    key: &Key,
    value: &T,
) {
    env.storage().instance().set(key, value);
}

pub fn get<T: soroban_sdk::TryFromVal<Env, soroban_sdk::Val>>(env: &Env, key: &Key) -> Option<T> {
    env.storage().instance().get(key)
}

pub fn ledger(env: &Env) -> Ledger {
    get(env, &Key::Ledger)
        .unwrap_or_else(|| panic_with_error!(env, PositionManagerError::NotInitialized))
}

pub fn save_ledger(env: &Env, ledger: &Ledger) {
    set(env, &Key::Ledger, ledger);
}

pub fn is_paused(env: &Env) -> bool {
    get(env, &Key::Paused).unwrap_or(false)
}

pub fn is_market_disabled(env: &Env, market: &Symbol) -> bool {
    get(env, &Key::MarketDisabled(market.clone())).unwrap_or(false)
}

pub fn position(env: &Env, id: u64) -> Option<Position> {
    env.storage().persistent().get(&Key::Position(id))
}

pub fn save_position(env: &Env, position: &Position) {
    let key = Key::Position(position.id);
    env.storage().persistent().set(&key, position);
    env.storage()
        .persistent()
        .extend_ttl(&key, SHARED_THRESHOLD, SHARED_BUMP);
    shared::bump_instance_ttl(env);
}

pub fn remove_position(env: &Env, id: u64) {
    env.storage().persistent().remove(&Key::Position(id));
}

pub fn market(env: &Env, symbol: &Symbol) -> Option<Market> {
    env.storage().persistent().get(&Key::Market(symbol.clone()))
}

pub fn save_market(env: &Env, symbol: &Symbol, market: &Market) {
    let key = Key::Market(symbol.clone());
    env.storage().persistent().set(&key, market);
    env.storage()
        .persistent()
        .extend_ttl(&key, SHARED_THRESHOLD, SHARED_BUMP);
}

pub fn config_manager(env: &Env) -> Address {
    get(env, &Key::ConfigManager)
        .unwrap_or_else(|| panic_with_error!(env, PositionManagerError::NotInitialized))
}

pub fn oracle_router(env: &Env) -> Address {
    get(env, &Key::OracleRouter)
        .unwrap_or_else(|| panic_with_error!(env, PositionManagerError::NotInitialized))
}

pub fn vault(env: &Env) -> Address {
    get(env, &Key::Vault)
        .unwrap_or_else(|| panic_with_error!(env, PositionManagerError::NotInitialized))
}

pub fn global_config(env: &Env) -> GlobalConfig {
    get(env, &Key::GlobalConfig)
        .unwrap_or_else(|| panic_with_error!(env, PositionManagerError::NotInitialized))
}

pub fn active_markets(env: &Env) -> Vec<Symbol> {
    get(env, &Key::ActiveMarkets).unwrap_or(Vec::new(env))
}

pub fn save_version(env: &Env, version: u32) {
    set(env, &Key::Version, &version);
}
