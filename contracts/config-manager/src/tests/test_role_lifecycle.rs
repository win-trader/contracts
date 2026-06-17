//! Integration tests covering the full role lifecycle and cross-cutting
//! isolation properties.
//!
//! Covers:
//!   - Full grant → revoke → re-grant cycle (1.3)
//!   - Multiple keepers coexisting independently (1.3)
//!   - Revoking one keeper does not affect others (1.3)
//!   - Admin role is unaffected by operations on other accounts (1.3)
//!   - Role isolation: holding one role does not grant others (1.3)
//!   - Auth is required: grant_role panics when no auth is mocked (C-1)

use soroban_sdk::{testutils::Address as _, Address, Env};

use super::helpers::{deploy, deploy_initialized, role_admin, role_keeper, role_pauser, role_upgrader};

/// Full lifecycle: init → grant KEEPER → has_role true → revoke → has_role false → re-grant → true.
#[test]
fn test_full_role_lifecycle_grant_revoke_grant() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);
    let keeper = Address::generate(&env);

    let keeper_role = role_keeper(&env);

    client.grant_role(&admin, &keeper_role, &keeper);
    assert!(client.has_role(&keeper_role, &keeper), "after grant: must be true");

    client.revoke_role(&admin, &keeper_role, &keeper);
    assert!(!client.has_role(&keeper_role, &keeper), "after revoke: must be false");

    client.grant_role(&admin, &keeper_role, &keeper);
    assert!(client.has_role(&keeper_role, &keeper), "after re-grant: must be true");
}

/// Multiple distinct addresses can independently hold KEEPER_ROLE.
#[test]
fn test_multiple_keepers_can_coexist() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);
    let keeper_a = Address::generate(&env);
    let keeper_b = Address::generate(&env);
    let keeper_c = Address::generate(&env);

    let keeper_role = role_keeper(&env);
    client.grant_role(&admin, &keeper_role, &keeper_a);
    client.grant_role(&admin, &keeper_role, &keeper_b);
    client.grant_role(&admin, &keeper_role, &keeper_c);

    assert!(client.has_role(&keeper_role, &keeper_a), "keeper_a must hold role");
    assert!(client.has_role(&keeper_role, &keeper_b), "keeper_b must hold role");
    assert!(client.has_role(&keeper_role, &keeper_c), "keeper_c must hold role");
}

/// Revoking one keeper's role must not affect other keepers.
#[test]
fn test_revoking_one_keeper_does_not_affect_others() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);
    let keeper_a = Address::generate(&env);
    let keeper_b = Address::generate(&env);

    let keeper_role = role_keeper(&env);
    client.grant_role(&admin, &keeper_role, &keeper_a);
    client.grant_role(&admin, &keeper_role, &keeper_b);

    client.revoke_role(&admin, &keeper_role, &keeper_a);

    assert!(
        !client.has_role(&keeper_role, &keeper_a),
        "keeper_a must not hold role after revoke"
    );
    assert!(
        client.has_role(&keeper_role, &keeper_b),
        "keeper_b must still hold role — unaffected by keeper_a revoke"
    );
}

/// Admin's DEFAULT_ADMIN_ROLE persists after operations on other accounts.
#[test]
fn test_admin_role_unaffected_by_operations_on_other_accounts() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);
    let keeper = Address::generate(&env);

    let admin_role = role_admin(&env);
    let keeper_role = role_keeper(&env);

    client.grant_role(&admin, &keeper_role, &keeper);
    client.revoke_role(&admin, &keeper_role, &keeper);

    assert!(
        client.has_role(&admin_role, &admin),
        "admin must still hold DEFAULT_ADMIN_ROLE after operating on other accounts"
    );
}

/// Role isolation: holding KEEPER_ROLE does not imply PAUSER_ROLE, UPGRADER_ROLE, or ADMIN.
#[test]
fn test_role_isolation_keeper_does_not_gain_pauser_role() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);
    let keeper = Address::generate(&env);

    let keeper_role = role_keeper(&env);
    client.grant_role(&admin, &keeper_role, &keeper);

    assert!(!client.has_role(&role_pauser(&env), &keeper), "KEEPER must not automatically gain PAUSER_ROLE");
    assert!(!client.has_role(&role_upgrader(&env), &keeper), "KEEPER must not automatically gain UPGRADER_ROLE");
    assert!(!client.has_role(&role_admin(&env), &keeper), "KEEPER must not automatically gain DEFAULT_ADMIN_ROLE");
}

/// Auth is required: when auth is NOT mocked, `grant_role` must panic.
#[test]
#[should_panic]
fn test_grant_role_requires_caller_auth_without_mock() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let keeper = Address::generate(&env);

    let client = deploy(&env, &admin);

    // Clear all mocked auths — subsequent calls requiring auth will panic.
    env.mock_auths(&[]);

    client.grant_role(&admin, &role_keeper(&env), &keeper);
}
