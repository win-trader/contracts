use soroban_sdk::{panic_with_error, Address, Env, Symbol};
use stellar_access::access_control::{
    get_admin as oz_get_admin, grant_role_no_auth as oz_grant_role_no_auth,
    has_role as oz_has_role, revoke_role_no_auth as oz_revoke_role_no_auth,
    set_admin as oz_set_admin,
};

use crate::errors::ConfigManagerError;

pub use shared::bump_instance_ttl;

pub fn require_admin(env: &Env, caller: &Address) {
    let admin = match oz_get_admin(env) {
        Some(a) => a,
        None => panic_with_error!(env, ConfigManagerError::Unauthorized),
    };
    if *caller != admin {
        panic_with_error!(env, ConfigManagerError::Unauthorized);
    }
}

pub fn require_admin_with_auth(env: &Env, caller: &Address) {
    caller.require_auth();
    require_admin(env, caller);
}

pub fn admin_role_symbol(env: &Env) -> Symbol {
    Symbol::new(env, shared::constants::ROLE_ADMIN)
}

pub fn init_admin(env: &Env, admin: &Address) {
    oz_set_admin(env, admin);
}

pub fn rotate_admin(env: &Env, new_admin: &Address) {
    env.storage()
        .instance()
        .remove(&stellar_access::access_control::AccessControlStorageKey::Admin);
    oz_set_admin(env, new_admin);
}

pub fn load_admin(env: &Env) -> Address {
    oz_get_admin(env).unwrap_or_else(|| panic_with_error!(env, ConfigManagerError::NotInitialized))
}

pub fn grant_role_internal(env: &Env, role: &Symbol, account: &Address, caller: &Address) -> bool {
    if has_role_local(env, role, account) {
        return false;
    }
    oz_grant_role_no_auth(env, account, role, caller);
    true
}

pub fn revoke_role_internal(env: &Env, role: &Symbol, account: &Address, caller: &Address) -> bool {
    if !has_role_local(env, role, account) {
        return false;
    }
    oz_revoke_role_no_auth(env, account, role, caller);
    true
}

pub fn has_role_local(env: &Env, role: &Symbol, account: &Address) -> bool {
    oz_has_role(env, account, role).is_some()
}
