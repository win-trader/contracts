use interfaces::LpConfig;
use soroban_sdk::{contracttype, Address, Env};

#[contracttype]
#[derive(Clone)]
pub enum Key {
    Initialized,
    ConfigManager,
    PositionManager,
    RequestRouter,
    LpConfig,
    Paused,
    Version,
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

pub fn config_manager(env: &Env) -> Address {
    get(env, &Key::ConfigManager).unwrap()
}

pub fn position_manager(env: &Env) -> Address {
    get(env, &Key::PositionManager).unwrap()
}

pub fn request_router(env: &Env) -> Address {
    get(env, &Key::RequestRouter).unwrap()
}

pub fn lp_config(env: &Env) -> LpConfig {
    get(env, &Key::LpConfig).unwrap()
}

pub fn save_version(env: &Env, version: u32) {
    set(env, &Key::Version, &version);
}
