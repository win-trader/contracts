use shared::{PendingGlobalConfig, PendingMarketConfig, PendingPriceFeed};
use soroban_sdk::{contracttype, Address, Env, Symbol};

#[contracttype]
#[derive(Clone)]
pub enum Key {
    ConfigManager,
    PositionManager,
    PendingGlobalConfig,
    PendingMarketConfig(Symbol),
    PendingPriceFeed,
    Version,
}

fn get<T: soroban_sdk::TryFromVal<Env, soroban_sdk::Val>>(env: &Env, key: &Key) -> Option<T> {
    env.storage().instance().get(key)
}

fn set<T: soroban_sdk::IntoVal<Env, soroban_sdk::Val>>(env: &Env, key: &Key, value: &T) {
    env.storage().instance().set(key, value);
}

fn remove(env: &Env, key: &Key) {
    env.storage().instance().remove(key);
}

pub fn config_manager(env: &Env) -> Address {
    get(env, &Key::ConfigManager).unwrap()
}

pub fn position_manager(env: &Env) -> Address {
    get(env, &Key::PositionManager).unwrap()
}

pub fn init(env: &Env, config_manager: &Address, position_manager: &Address) {
    set(env, &Key::ConfigManager, config_manager);
    set(env, &Key::PositionManager, position_manager);
}

pub fn pending_global(env: &Env) -> Option<PendingGlobalConfig> {
    get(env, &Key::PendingGlobalConfig)
}

pub fn save_pending_global(env: &Env, pending: &PendingGlobalConfig) {
    set(env, &Key::PendingGlobalConfig, pending);
}

pub fn clear_pending_global(env: &Env) {
    remove(env, &Key::PendingGlobalConfig);
}

pub fn pending_market(env: &Env, market: &Symbol) -> Option<PendingMarketConfig> {
    get(env, &Key::PendingMarketConfig(market.clone()))
}

pub fn save_pending_market(env: &Env, market: &Symbol, pending: &PendingMarketConfig) {
    set(env, &Key::PendingMarketConfig(market.clone()), pending);
}

pub fn clear_pending_market(env: &Env, market: &Symbol) {
    remove(env, &Key::PendingMarketConfig(market.clone()));
}

pub fn pending_price_feed(env: &Env) -> Option<PendingPriceFeed> {
    get(env, &Key::PendingPriceFeed)
}

pub fn save_pending_price_feed(env: &Env, pending: &PendingPriceFeed) {
    set(env, &Key::PendingPriceFeed, pending);
}

pub fn clear_pending_price_feed(env: &Env) {
    remove(env, &Key::PendingPriceFeed);
}

pub fn save_version(env: &Env, version: u32) {
    set(env, &Key::Version, &version);
}
