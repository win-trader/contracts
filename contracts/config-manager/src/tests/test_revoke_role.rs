//! Tests for `revoke_role` — permission removal by the admin.
//!
//! Covers:
//!   - Happy path: admin can revoke KEEPER_ROLE (1.3)
//!   - No-op: revoking a role not held must not panic (1.3)
//!   - Auth enforcement: non-admin callers are rejected with Unauthorized (1.3)
//!   - Griefing: keeper cannot revoke another keeper's role (1.3)
//!   - Self-revoke: keeper cannot self-revoke via the admin-only revoke_role (1.3)
//!   - Double-revoke: revoking twice remains consistent (1.3)

use soroban_sdk::{testutils::Address as _, Address, Env};

use crate::ConfigManagerError;

use super::helpers::{deploy_initialized, role_keeper};

/// Admin can revoke KEEPER_ROLE from a holder; `has_role` must return `false` afterward.
#[test]
fn test_revoke_role_admin_can_remove_keeper_role() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);
    let keeper = Address::generate(&env);

    let keeper_role = role_keeper(&env);
    client.grant_role(&admin, &keeper_role, &keeper);
    assert!(
        client.has_role(&keeper_role, &keeper),
        "precondition: keeper must hold KEEPER_ROLE before revoke"
    );

    client.revoke_role(&admin, &keeper_role, &keeper);

    assert!(
        !client.has_role(&keeper_role, &keeper),
        "has_role must return false after revoke_role"
    );
}

/// Revoking a role from an address that never held it must be a no-op — must NOT panic.
#[test]
fn test_revoke_role_from_address_without_role_is_noop() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);
    let non_keeper = Address::generate(&env);

    let result = client.try_revoke_role(&admin, &role_keeper(&env), &non_keeper);
    assert!(
        result.is_ok(),
        "revoking a role from an address that never held it must be a no-op (Ok), not panic"
    );
}

/// Non-admin calling `revoke_role` must error with Unauthorized (3).
#[test]
fn test_revoke_role_non_admin_caller_errors_unauthorized() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);
    let keeper = Address::generate(&env);
    let attacker = Address::generate(&env);

    let keeper_role = role_keeper(&env);
    client.grant_role(&admin, &keeper_role, &keeper);

    let result = client.try_revoke_role(&attacker, &keeper_role, &keeper);
    assert!(result.is_err(), "non-admin must not be able to revoke_role");
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(ConfigManagerError::Unauthorized as u32),
        "error code must be Unauthorized (3)"
    );
}

/// A keeper cannot revoke another keeper's role (griefing vector).
#[test]
fn test_revoke_role_keeper_cannot_revoke_another_keeper() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);
    let keeper_a = Address::generate(&env);
    let keeper_b = Address::generate(&env);

    let keeper_role = role_keeper(&env);
    client.grant_role(&admin, &keeper_role, &keeper_a);
    client.grant_role(&admin, &keeper_role, &keeper_b);

    let result = client.try_revoke_role(&keeper_a, &keeper_role, &keeper_b);
    assert!(
        result.is_err(),
        "keeper_a must not be able to revoke keeper_b's role"
    );
}

/// A keeper cannot self-revoke via `revoke_role` (admin-only function).
#[test]
fn test_revoke_role_keeper_cannot_self_revoke_via_revoke_role() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);
    let keeper = Address::generate(&env);

    let keeper_role = role_keeper(&env);
    client.grant_role(&admin, &keeper_role, &keeper);

    let result = client.try_revoke_role(&keeper, &keeper_role, &keeper);
    assert!(
        result.is_err(),
        "keeper must not be able to self-revoke via the admin revoke_role function"
    );
}

/// Revoking a role twice must remain a no-op on the second call.
#[test]
fn test_revoke_role_twice_remains_consistent() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);
    let keeper = Address::generate(&env);

    let keeper_role = role_keeper(&env);
    client.grant_role(&admin, &keeper_role, &keeper);
    client.revoke_role(&admin, &keeper_role, &keeper);

    let result = client.try_revoke_role(&admin, &keeper_role, &keeper);
    assert!(
        result.is_ok(),
        "second revoke on an address that no longer holds the role must be a no-op"
    );
    assert!(
        !client.has_role(&keeper_role, &keeper),
        "has_role must remain false after double-revoke"
    );
}
