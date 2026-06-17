//! Entrypoint guards: initialization state, pause state, role checks,
//! basic input validation. Routing layer (`contract.rs`) calls these before
//! delegating to feature modules.

use soroban_sdk::{panic_with_error, Address, Env};

use crate::errors::PositionManagerError;
use crate::storage;

// ---------------------------------------------------------------------------
// Initialization guards
// ---------------------------------------------------------------------------

/// Panics with `NotInitialized` (error 2) if the contract has not been initialized.
pub fn require_initialized(env: &Env) {
    if !storage::is_initialized(env) {
        panic_with_error!(env, PositionManagerError::NotInitialized);
    }
}

// ---------------------------------------------------------------------------
// Pause guard
// ---------------------------------------------------------------------------

/// Panics with `Paused` (error 3) if the contract is currently paused.
pub fn require_not_paused(env: &Env) {
    if storage::get_paused(env) {
        panic_with_error!(env, PositionManagerError::Paused);
    }
}

// ---------------------------------------------------------------------------
// Role guards (via ConfigManager cross-contract call)
// ---------------------------------------------------------------------------

/// Cross-contract role check + per-contract panic. Panics with
/// `PositionManagerError::Unauthorized` (code 7) on failure so the panic
/// code identifies the source contract.
fn require_role_or_panic(env: &Env, caller: &Address, role: &str) {
    caller.require_auth();
    let config_mgr = storage::get_config_manager(env);
    if !shared::has_role(env, &config_mgr, role, caller) {
        panic_with_error!(env, PositionManagerError::Unauthorized);
    }
}

/// Panics with `Unauthorized` (error 7) if `caller` does not have the KEEPER role.
pub fn require_keeper(env: &Env, caller: &Address) {
    require_role_or_panic(env, caller, shared::constants::ROLE_KEEPER);
}

/// Panics with `Unauthorized` (error 7) if `caller` does not have the PAUSER role.
pub fn require_pauser(env: &Env, caller: &Address) {
    require_role_or_panic(env, caller, shared::constants::ROLE_PAUSER);
}

/// Panics with `Unauthorized` (error 7) if `caller` does not have the ADMIN role.
pub fn require_admin(env: &Env, caller: &Address) {
    require_role_or_panic(env, caller, shared::constants::ROLE_ADMIN);
}

/// Panics with `Unauthorized` (error 7) if `caller` does not have the UPGRADER role.
pub fn require_upgrader(env: &Env, caller: &Address) {
    require_role_or_panic(env, caller, shared::constants::ROLE_UPGRADER);
}

// ---------------------------------------------------------------------------
// Input validation
// ---------------------------------------------------------------------------

pub fn require_positive(env: &Env, value: i128) {
    if value <= 0 {
        panic_with_error!(env, PositionManagerError::ZeroAmount);
    }
}
