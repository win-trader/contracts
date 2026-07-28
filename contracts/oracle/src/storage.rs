use soroban_sdk::{contracttype, Address, Env, Symbol};

#[contracttype]
pub enum StorageKey {
    Initialized,
    ConfigManager,
    /// The single address authorized to push prices into this instance.
    Publisher,
    Price(Symbol),
    LastUpdate(Symbol),
    Version,
}

pub fn is_initialized(env: &Env) -> bool {
    env.storage().instance().has(&StorageKey::Initialized)
}

pub fn set_initialized(env: &Env) {
    env.storage()
        .instance()
        .set(&StorageKey::Initialized, &true);
}

pub fn set_config_manager(env: &Env, addr: &Address) {
    env.storage()
        .instance()
        .set(&StorageKey::ConfigManager, addr);
}

pub fn get_config_manager(env: &Env) -> Address {
    env.storage()
        .instance()
        .get(&StorageKey::ConfigManager)
        .unwrap()
}

pub fn set_publisher(env: &Env, addr: &Address) {
    env.storage().instance().set(&StorageKey::Publisher, addr);
}

pub fn get_publisher(env: &Env) -> Address {
    env.storage()
        .instance()
        .get(&StorageKey::Publisher)
        .unwrap()
}

pub fn set_price(env: &Env, symbol: &Symbol, price: i128) {
    env.storage()
        .instance()
        .set(&StorageKey::Price(symbol.clone()), &price);
}

pub fn get_price(env: &Env, symbol: &Symbol) -> Option<i128> {
    env.storage()
        .instance()
        .get(&StorageKey::Price(symbol.clone()))
}

pub fn set_last_update(env: &Env, symbol: &Symbol, ts: u64) {
    env.storage()
        .instance()
        .set(&StorageKey::LastUpdate(symbol.clone()), &ts);
}

pub fn get_last_update(env: &Env, symbol: &Symbol) -> u64 {
    env.storage()
        .instance()
        .get(&StorageKey::LastUpdate(symbol.clone()))
        .unwrap_or(0)
}

pub fn save_version(env: &Env, version: u32) {
    env.storage().instance().set(&StorageKey::Version, &version);
}
