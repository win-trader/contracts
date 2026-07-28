#![no_std]

use soroban_sdk::{contractclient, Address, Env, Symbol};

pub mod constants;

use constants::{INSTANCE_BUMP, INSTANCE_THRESHOLD};

/// Extend instance storage TTL to prevent archival.
pub fn bump_instance_ttl(env: &Env) {
    env.storage()
        .instance()
        .extend_ttl(INSTANCE_THRESHOLD, INSTANCE_BUMP);
}

// ---------------------------------------------------------------------------
// Access control — cross-contract role checking via ConfigManager
//
// Uses a minimal contractclient trait (NOT the full config-manager crate) so
// shared has zero dependency on any protocol contract, preventing circular deps.
// ---------------------------------------------------------------------------

/// Minimal ConfigManager interface — only the has_role selector is needed.
#[contractclient(name = "AccessControlClient")]
pub trait AccessControlInterface {
    fn has_role(env: Env, role: Symbol, account: Address) -> bool;
}

/// Return true if `caller` holds `role` in the given ConfigManager contract.
///
/// Cross-contract auth primitive — does NOT call `require_auth` and does NOT
/// panic. Callers compose this with `caller.require_auth()` and a typed panic
/// using their own contract-local `Unauthorized` error so failures point to
/// the source contract via its error code.
pub fn has_role(env: &Env, config_manager: &Address, role: &str, caller: &Address) -> bool {
    AccessControlClient::new(env, config_manager).has_role(&Symbol::new(env, role), caller)
}
