//! Storage layout.
//!
//! Instance storage holds the wiring addresses, configs, pause flag, and the
//! one `Ledger` aggregate (all global accounting lives inside it — business
//! logic never reads a bare accounting key). Positions and markets are
//! persistent entries with explicit TTL extension; anyone can re-extend a
//! position via `bump_position`.

use shared::constants::{SHARED_BUMP, SHARED_THRESHOLD};
use shared::{EntryOrder, GlobalConfig, Market, Position};
use soroban_sdk::{contracttype, panic_with_error, Address, Env, Symbol, Vec};

use crate::errors::PositionManagerError;
use crate::ledger::Ledger;

#[contracttype]
#[derive(Clone)]
pub enum StorageKey {
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
    EntryOrder(u64),
    NextEntryOrderId,
}

pub fn is_paused(env: &Env) -> bool {
    env.storage()
        .instance()
        .get(&StorageKey::Paused)
        .unwrap_or(false)
}

pub fn save_paused(env: &Env, paused: bool) {
    env.storage().instance().set(&StorageKey::Paused, &paused);
}

// POSITION

pub fn get_position(env: &Env, id: u64) -> Position {
    env.storage()
        .persistent()
        .get(&StorageKey::Position(id))
        .unwrap_or_else(|| panic_with_error!(env, PositionManagerError::PositionNotFound))
}

pub fn save_position(env: &Env, position: &Position) {
    let key = StorageKey::Position(position.id);
    env.storage().persistent().set(&key, position);
    env.storage()
        .persistent()
        .extend_ttl(&key, SHARED_THRESHOLD, SHARED_BUMP);
}

pub fn remove_position(env: &Env, id: u64) {
    env.storage().persistent().remove(&StorageKey::Position(id));
}

// ENTRY ORDER

pub fn get_entry_order(env: &Env, id: u64) -> EntryOrder {
    env.storage()
        .persistent()
        .get(&StorageKey::EntryOrder(id))
        .unwrap_or_else(|| panic_with_error!(env, PositionManagerError::OrderNotFound))
}

pub fn save_entry_order(env: &Env, order: &EntryOrder) {
    let key = StorageKey::EntryOrder(order.id);
    env.storage().persistent().set(&key, order);
    env.storage()
        .persistent()
        .extend_ttl(&key, SHARED_THRESHOLD, SHARED_BUMP);
}

pub fn remove_entry_order(env: &Env, id: u64) {
    env.storage()
        .persistent()
        .remove(&StorageKey::EntryOrder(id));
}

pub fn get_next_entry_order_id(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get(&StorageKey::NextEntryOrderId)
        .unwrap_or(1)
}

pub fn save_next_entry_order_id(env: &Env, id: u64) {
    env.storage()
        .instance()
        .set(&StorageKey::NextEntryOrderId, &id);
}

pub fn update_entry_order_id(env: &Env) {
    save_next_entry_order_id(env, get_next_entry_order_id(env) + 1);
}

// MARKET

pub fn get_market(env: &Env, symbol: &Symbol) -> Market {
    try_get_market(env, symbol)
        .unwrap_or_else(|| panic_with_error!(env, PositionManagerError::MarketNotConfigured))
}

/// Non-panicking market lookup — for callers (config admin) where absence is
/// a legal state, not an error.
pub fn try_get_market(env: &Env, symbol: &Symbol) -> Option<Market> {
    env.storage()
        .persistent()
        .get(&StorageKey::Market(symbol.clone()))
}

pub fn save_market(env: &Env, symbol: &Symbol, market: &Market) {
    let key = StorageKey::Market(symbol.clone());
    env.storage().persistent().set(&key, market);
    env.storage()
        .persistent()
        .extend_ttl(&key, SHARED_THRESHOLD, SHARED_BUMP);
}

pub fn is_market_disabled(env: &Env, market: &Symbol) -> bool {
    env.storage()
        .instance()
        .get(&StorageKey::MarketDisabled(market.clone()))
        .unwrap_or(false)
}

pub fn set_market_disabled(env: &Env, market: &Symbol, disabled: bool) {
    env.storage()
        .instance()
        .set(&StorageKey::MarketDisabled(market.clone()), &disabled);
}

// todo do we have 1 pos manager for all or per market?
pub fn get_active_markets(env: &Env) -> Vec<Symbol> {
    env.storage()
        .instance()
        .get(&StorageKey::ActiveMarkets)
        .unwrap_or(Vec::new(env))
}

pub fn save_active_markets(env: &Env, markets: &Vec<Symbol>) {
    env.storage()
        .instance()
        .set(&StorageKey::ActiveMarkets, markets);
}

// LEDGER

pub fn get_ledger(env: &Env) -> Ledger {
    env.storage()
        .instance()
        .get(&StorageKey::Ledger)
        .unwrap_or_else(|| panic_with_error!(env, PositionManagerError::NotInitialized))
}

pub fn save_ledger(env: &Env, ledger: &Ledger) {
    env.storage().instance().set(&StorageKey::Ledger, ledger);
}

pub fn get_config_manager(env: &Env) -> Address {
    env.storage()
        .instance()
        .get(&StorageKey::ConfigManager)
        .unwrap_or_else(|| panic_with_error!(env, PositionManagerError::NotInitialized))
}

pub fn save_config_manager(env: &Env, config_manager: &Address) {
    env.storage()
        .instance()
        .set(&StorageKey::ConfigManager, config_manager);
}

pub fn get_oracle_router(env: &Env) -> Address {
    env.storage()
        .instance()
        .get(&StorageKey::OracleRouter)
        .unwrap_or_else(|| panic_with_error!(env, PositionManagerError::NotInitialized))
}

pub fn save_oracle_router(env: &Env, oracle_router: &Address) {
    env.storage()
        .instance()
        .set(&StorageKey::OracleRouter, oracle_router);
}

pub fn get_vault(env: &Env) -> Address {
    try_get_vault(env)
        .unwrap_or_else(|| panic_with_error!(env, PositionManagerError::NotInitialized))
}

/// Non-panicking vault lookup — `set_vault` uses absence as "not wired yet".
pub fn try_get_vault(env: &Env) -> Option<Address> {
    env.storage().instance().get(&StorageKey::Vault)
}

pub fn save_vault(env: &Env, vault: &Address) {
    env.storage().instance().set(&StorageKey::Vault, vault);
}

pub fn get_global_config(env: &Env) -> GlobalConfig {
    env.storage()
        .instance()
        .get(&StorageKey::GlobalConfig)
        .unwrap_or_else(|| panic_with_error!(env, PositionManagerError::NotInitialized))
}

pub fn save_global_config(env: &Env, config: &GlobalConfig) {
    env.storage()
        .instance()
        .set(&StorageKey::GlobalConfig, config);
}

pub fn get_initialized(env: &Env) -> bool {
    env.storage()
        .instance()
        .get(&StorageKey::Initialized)
        .unwrap_or_else(|| panic_with_error!(env, PositionManagerError::NotInitialized))
}

pub fn save_initialized(env: &Env) {
    env.storage()
        .instance()
        .set(&StorageKey::Initialized, &true);
}

pub fn save_version(env: &Env, version: u32) {
    env.storage().instance().set(&StorageKey::Version, &version);
}

pub fn get_next_position_id(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get(&StorageKey::NextPositionId)
        .unwrap_or(1)
}

pub fn save_next_position_id(env: &Env, id: u64) {
    env.storage()
        .instance()
        .set(&StorageKey::NextPositionId, &id);
}

pub fn update_position_id(env: &Env) {
    save_next_position_id(env, get_next_position_id(env) + 1);
}
