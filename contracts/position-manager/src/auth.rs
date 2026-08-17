use soroban_sdk::{panic_with_error, Address, Env, Symbol};

use crate::{errors::PositionManagerError, storage};

pub fn require_auth(caller: &Address) {
    caller.require_auth();
}

pub fn require_role(env: &Env, caller: &Address, role: &str) {
    require_auth(caller);
    if !shared::has_role(env, &storage::get_config_manager(env), role, caller) {
        panic_with_error!(env, PositionManagerError::Unauthorized);
    }
    shared::bump_instance_ttl(env);
}

pub fn require_initialized(env: &Env) {
    if storage::get_initialized(env) != true {
        panic_with_error!(env, PositionManagerError::NotInitialized);
    }
}

pub fn require_vault(env: &Env, caller: &Address) {
    require_auth(caller);
    if &storage::get_vault(env) != caller {
        panic_with_error!(env, PositionManagerError::InvalidCaller);
    }
}

pub fn require_not_paused(env: &Env) {
    if storage::is_paused(env) {
        panic_with_error!(env, PositionManagerError::Paused);
    }
}

pub fn require_market_active(env: &Env, market_symbol: &Symbol) {
    if storage::is_market_disabled(&env, &market_symbol) {
        panic_with_error!(&env, PositionManagerError::MarketDisabled);
    }
}
