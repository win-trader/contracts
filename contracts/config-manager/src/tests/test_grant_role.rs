//! Tests for `grant_role` — permission assignment by the admin.
//!
//! Covers:
//!   - Happy path: admin can assign KEEPER, PAUSER, and UPGRADER roles (1.3)
//!   - Idempotency: granting the same role twice is a no-op (1.3)
//!   - Auth enforcement: non-admin callers are rejected with Unauthorized (1.3)
//!   - Privilege escalation: keepers cannot grant roles to others (1.3)

use soroban_sdk::{testutils::Address as _, Address, Env};

use crate::ConfigManagerError;

use super::helpers::{deploy_initialized, role_admin, role_keeper, role_pauser, role_upgrader};

/// Admin can grant KEEPER_ROLE to a new address; `has_role` must return `true` afterward.
#[test]
fn test_grant_role_admin_can_assign_keeper_role() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);
    let keeper = Address::generate(&env);

    let keeper_role = role_keeper(&env);
    client.grant_role(&admin, &keeper_role, &keeper);

    assert!(
        client.has_role(&keeper_role, &keeper),
        "has_role must return true for keeper after grant_role"
    );
}

/// Admin can grant PAUSER_ROLE.
#[test]
fn test_grant_role_admin_can_assign_pauser_role() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);
    let pauser = Address::generate(&env);

    let pauser_role = role_pauser(&env);
    client.grant_role(&admin, &pauser_role, &pauser);

    assert!(
        client.has_role(&pauser_role, &pauser),
        "has_role must return true for pauser after grant_role"
    );
}

/// Admin can grant UPGRADER_ROLE.
#[test]
fn test_grant_role_admin_can_assign_upgrader_role() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);
    let upgrader = Address::generate(&env);

    let upgrader_role = role_upgrader(&env);
    client.grant_role(&admin, &upgrader_role, &upgrader);

    assert!(
        client.has_role(&upgrader_role, &upgrader),
        "has_role must return true for upgrader after grant_role"
    );
}

/// Granting a role is idempotent — granting the same role twice must not panic
/// and `has_role` still returns `true`.
#[test]
fn test_grant_role_twice_to_same_address_is_idempotent() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);
    let keeper = Address::generate(&env);

    let keeper_role = role_keeper(&env);
    client.grant_role(&admin, &keeper_role, &keeper);
    client.grant_role(&admin, &keeper_role, &keeper);

    assert!(
        client.has_role(&keeper_role, &keeper),
        "has_role must remain true after granting same role twice"
    );
}

/// Non-admin calling `grant_role` must error with Unauthorized (3).
#[test]
fn test_grant_role_non_admin_caller_errors_unauthorized() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = deploy_initialized(&env);
    let attacker = Address::generate(&env);
    let victim = Address::generate(&env);

    let result = client.try_grant_role(&attacker, &role_keeper(&env), &victim);
    assert!(result.is_err(), "non-admin must not be able to grant_role");
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(ConfigManagerError::Unauthorized as u32),
        "error code must be Unauthorized (3)"
    );
}

/// A keeper cannot escalate privileges by granting DEFAULT_ADMIN_ROLE to any address.
#[test]
fn test_grant_role_keeper_cannot_escalate_to_grant_admin_role() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);
    let keeper = Address::generate(&env);
    let new_address = Address::generate(&env);

    let keeper_role = role_keeper(&env);
    client.grant_role(&admin, &keeper_role, &keeper);

    let result = client.try_grant_role(&keeper, &role_admin(&env), &new_address);
    assert!(
        result.is_err(),
        "keeper must not be able to grant DEFAULT_ADMIN_ROLE to others"
    );
}

/// A keeper cannot use `grant_role` to assign another keeper.
#[test]
fn test_grant_role_keeper_cannot_grant_keeper_role_to_others() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);
    let keeper = Address::generate(&env);
    let target = Address::generate(&env);

    let keeper_role = role_keeper(&env);
    client.grant_role(&admin, &keeper_role, &keeper);

    let result = client.try_grant_role(&keeper, &keeper_role, &target);
    assert!(
        result.is_err(),
        "keeper must not be able to grant KEEPER_ROLE to other addresses"
    );
}
