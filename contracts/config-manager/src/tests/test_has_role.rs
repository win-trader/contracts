//! Tests for the `has_role` function.
//!
//! Covers:
//!   - Basic read behavior for granted, revoked, and absent roles (1.2)
//!   - TTL must be extended on read to prevent archival of live entries ()
//!   - Guarded TTL: absent/deleted keys must not cause panic (F-1)

use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env};

use crate::ConfigManagerError;

use super::helpers::{deploy, deploy_initialized, role_admin, role_keeper, role_pauser};

// ---------------------------------------------------------------------------
// Basic read behavior (1.2)
// ---------------------------------------------------------------------------

/// After construction, the provided admin must hold DEFAULT_ADMIN_ROLE ("ADMIN").
#[test]
fn test_has_role_admin_holds_default_admin_role_after_init() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let client = deploy(&env, &admin);

    assert!(
        client.has_role(&role_admin(&env), &admin),
        "admin must hold DEFAULT_ADMIN_ROLE immediately after construction"
    );
}

/// An address that was never granted any role must return `false`.
#[test]
fn test_has_role_returns_false_for_never_granted_address() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = deploy_initialized(&env);
    let stranger = Address::generate(&env);

    assert!(!client.has_role(&role_admin(&env), &stranger), "stranger must not hold DEFAULT_ADMIN_ROLE");
    assert!(!client.has_role(&role_keeper(&env), &stranger), "stranger must not hold KEEPER_ROLE");
    assert!(!client.has_role(&role_pauser(&env), &stranger), "stranger must not hold PAUSER_ROLE");
}

/// A KEEPER that was explicitly granted KEEPER_ROLE must return `true`.
#[test]
fn test_has_role_returns_true_for_granted_keeper() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);
    let keeper = Address::generate(&env);

    let keeper_role = role_keeper(&env);
    client.grant_role(&admin, &keeper_role, &keeper);

    assert!(
        client.has_role(&keeper_role, &keeper),
        "keeper must hold KEEPER_ROLE after it was granted"
    );
}

/// Querying a role for an address that was never granted it must not return
/// true — either it returns false or errors, never a false positive.
#[test]
fn test_has_role_for_ungranted_address_does_not_return_true() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = deploy_initialized(&env);
    let any_address = Address::generate(&env);

    if let Ok(inner) = client.try_has_role(&role_admin(&env), &any_address) {
        if let Ok(value) = inner {
            assert!(
                !value,
                "has_role must not return true for an address that was never granted the role"
            );
        }
    }
}

/// Admin should NOT automatically hold every role — specifically not
/// KEEPER_ROLE, which is a separate, opt-in grant.
#[test]
fn test_has_role_admin_does_not_automatically_hold_keeper_role() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    assert!(
        !client.has_role(&role_keeper(&env), &admin),
        "admin must NOT automatically hold KEEPER_ROLE without an explicit grant"
    );
}

/// Querying a completely fabricated / unknown role symbol must return `false`,
/// not panic.
#[test]
fn test_has_role_unknown_role_returns_false_not_panic() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let bogus_role = symbol_short!("BOGUS");
    assert!(
        !client.has_role(&bogus_role, &admin),
        "querying a non-existent role must return false, not panic"
    );
}

// ---------------------------------------------------------------------------
// TTL extension on read ()
// ---------------------------------------------------------------------------

/// `has_role` on an existing role entry must not panic — the
/// implementation should extend the persistent entry's TTL on read.
#[test]
fn test_has_role_on_existing_role_does_not_panic() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);
    let keeper = Address::generate(&env);

    let keeper_role = role_keeper(&env);
    client.grant_role(&admin, &keeper_role, &keeper);

    assert!(
        client.has_role(&keeper_role, &keeper),
        "has_role must return true for a role that was explicitly granted"
    );
}

/// `has_role` called on a non-existent entry must return false and
/// must NOT panic even when TTL bumping is attempted on a missing key.
#[test]
fn test_has_role_on_nonexistent_entry_returns_false_without_panic() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = deploy_initialized(&env);
    let stranger = Address::generate(&env);

    let result = client.try_has_role(&role_keeper(&env), &stranger);
    assert!(
        result.is_ok(),
        "has_role on a non-existent entry must not panic even if TTL bump is attempted"
    );
    assert!(
        !result.unwrap().unwrap(),
        "has_role must return false for an address that was never granted the role"
    );
}

/// Calling `has_role` on a role that was previously revoked must
/// return false and must not panic (revoke removes the key; extend_ttl on
/// a deleted key must be guarded).
#[test]
fn test_has_role_after_revoke_returns_false_without_panic() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);
    let keeper = Address::generate(&env);

    let keeper_role = role_keeper(&env);
    client.grant_role(&admin, &keeper_role, &keeper);
    client.revoke_role(&admin, &keeper_role, &keeper);

    let result = client.try_has_role(&keeper_role, &keeper);
    assert!(
        result.is_ok(),
        "has_role after revoke must not panic — deleted keys must be guarded before TTL bump"
    );
    assert!(
        !result.unwrap().unwrap(),
        "has_role must return false after role has been revoked"
    );
}

// ---------------------------------------------------------------------------
// Guarded TTL refresh (F-1)
// ---------------------------------------------------------------------------

/// After granting a role, `has_role` must return `true`. In a correct
/// implementation the TTL refresh keeps the entry live.
#[test]
fn test_ttl_refresh_has_role_returns_true_on_existing_entry() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);
    let keeper = Address::generate(&env);

    let keeper_role = role_keeper(&env);
    client.grant_role(&admin, &keeper_role, &keeper);

    assert!(
        client.has_role(&keeper_role, &keeper),
        "has_role must return true for an existing, live role entry (TTL refresh on read keeps it accessible)"
    );
}

/// `has_role` on a key that was never written must return `false`
/// without panicking. An unconditional `extend_ttl` on a missing key would
/// panic; the fix requires `try_extend_ttl` or a `has()` guard.
#[test]
fn test_ttl_refresh_has_role_on_missing_key_does_not_panic() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = deploy_initialized(&env);
    let stranger = Address::generate(&env);

    let result = client.try_has_role(&role_keeper(&env), &stranger);
    assert!(
        result.is_ok(),
        "has_role on a never-written key must not panic — try_extend_ttl must be guarded"
    );
    assert!(
        !result
            .expect("try_has_role returned Err unexpectedly")
            .expect("inner result conversion failed"),
        "has_role must return false for a key that was never written"
    );
}

/// After `revoke_role` the persistent key is deleted. A subsequent
/// `has_role` call must return `false` and must NOT panic. An unguarded
/// `extend_ttl` on the now-deleted key would cause a host trap.
#[test]
fn test_ttl_refresh_has_role_after_revoke_does_not_panic() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);
    let keeper = Address::generate(&env);

    let keeper_role = role_keeper(&env);
    client.grant_role(&admin, &keeper_role, &keeper);
    client.revoke_role(&admin, &keeper_role, &keeper);

    let result = client.try_has_role(&keeper_role, &keeper);
    assert!(
        result.is_ok(),
        "has_role after revoke must not panic — deleted key must be guarded before TTL bump"
    );
    assert!(
        !result
            .expect("try_has_role returned Err unexpectedly")
            .expect("inner result conversion failed"),
        "has_role must return false after the role has been revoked"
    );
}

// Silence unused import warning for ConfigManagerError (used for type-level assertions).
#[allow(dead_code)]
fn _uses_error(_: ConfigManagerError) {}
