//! Tests for admin split-brain consistency.
//!
//! The contract maintains two parallel representations of "is X the admin?":
//!   (a) The OZ AccessControl admin pointer in instance storage (read by
//!       `require_admin` via `oz_get_admin`)
//!   (b) The OZ AccessControl role-membership entry for `ADMIN` (queried by
//!       `has_role`)
//!
//! These tests verify both representations remain consistent across all operations.

use soroban_sdk::{testutils::Address as _, Address, Env};

use crate::ConfigManagerError;

use super::helpers::{deploy_initialized, role_admin, role_keeper};

/// After granting KEEPER to another address, the admin must still hold ADMIN role.
#[test]
fn test_has_role_admin_consistent_after_grant_role() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);
    let keeper = Address::generate(&env);

    let admin_role = role_admin(&env);
    let keeper_role = role_keeper(&env);

    client.grant_role(&admin, &keeper_role, &keeper);

    assert!(
        client.has_role(&admin_role, &admin),
        "admin must still hold ADMIN role after granting KEEPER role to another address"
    );
}

/// After revoking KEEPER from another address, the admin must still hold ADMIN role.
#[test]
fn test_has_role_admin_consistent_after_revoke_role() {
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
        "admin must still hold ADMIN role after revoking KEEPER role from another address"
    );
}

/// Admin role persists across a full grant → revoke → re-grant sequence on other accounts.
#[test]
fn test_has_role_admin_consistent_after_operations() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);
    let keeper = Address::generate(&env);

    let admin_role = role_admin(&env);
    let keeper_role = role_keeper(&env);

    client.grant_role(&admin, &keeper_role, &keeper);
    assert!(client.has_role(&admin_role, &admin), "after grant_role: admin must still hold ADMIN role");

    client.revoke_role(&admin, &keeper_role, &keeper);
    assert!(client.has_role(&admin_role, &admin), "after revoke_role: admin must still hold ADMIN role");

    client.grant_role(&admin, &keeper_role, &keeper);
    assert!(client.has_role(&admin_role, &admin), "after re-grant: admin must still hold ADMIN role");
}

/// Self-revoke of ADMIN must leave both stores consistent — no split-brain.
///
/// The contract may either reject the revoke (Err) or accept it (Ok).
/// In either case, has_role and require_admin must agree on the result.
#[test]
fn test_revoking_admin_role_from_admin_no_split_brain() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);
    let keeper = Address::generate(&env);

    let admin_role = role_admin(&env);
    let keeper_role = role_keeper(&env);

    let revoke_result = client.try_revoke_role(&admin, &admin_role, &admin);

    match revoke_result {
        Err(_) => {
            // Case A: revoke was rejected — admin must still be operational in both stores.
            assert!(
                client.has_role(&admin_role, &admin),
                "Case A: revoke was rejected but has_role(ADMIN, admin) returned false — split-brain"
            );
            let grant_result = client.try_grant_role(&admin, &keeper_role, &keeper);
            assert!(
                grant_result.is_ok(),
                "Case A: admin must still be able to grant_role after failed self-revoke"
            );
        }
        Ok(_) => {
            // Case B: revoke was accepted — persistent store must agree.
            assert!(
                !client.has_role(&admin_role, &admin),
                "Case B: revoke succeeded but has_role(ADMIN, admin) returned true — split-brain"
            );
            let grant_result = client.try_grant_role(&admin, &keeper_role, &keeper);
            assert!(
                grant_result.is_err(),
                "Case B: after self-revoke the former admin must not be able to grant_role"
            );
        }
    }
}

/// has_role must match require_admin after initialize — no split-brain between stores.
#[test]
fn test_admin_has_role_matches_require_admin_after_initialize() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);
    let keeper = Address::generate(&env);

    let admin_role = role_admin(&env);
    let keeper_role = role_keeper(&env);

    // Persistent-store view: has_role must report admin holds ADMIN.
    assert!(
        client.has_role(&admin_role, &admin),
        "has_role must return true for admin immediately after initialize"
    );

    // Instance-store view: require_admin must accept admin (evidenced by grant_role succeeding).
    let grant_result = client.try_grant_role(&admin, &keeper_role, &keeper);
    assert!(
        grant_result.is_ok(),
        "admin must be accepted by require_admin after initialize — no split-brain"
    );
}

/// An impostor must be rejected by both has_role and require_admin.
#[test]
fn test_impostor_has_no_admin_privileges_in_either_store() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = deploy_initialized(&env);
    let impostor = Address::generate(&env);
    let victim = Address::generate(&env);

    let admin_role = role_admin(&env);
    let keeper_role = role_keeper(&env);

    // Persistent-store view: impostor must not hold ADMIN.
    assert!(
        !client.has_role(&admin_role, &impostor),
        "impostor must not hold ADMIN role in persistent store"
    );

    // Instance-store view: require_admin must reject impostor.
    let grant_result = client.try_grant_role(&impostor, &keeper_role, &victim);
    assert!(
        grant_result.is_err(),
        "impostor must be rejected by require_admin — not in instance store"
    );
    assert_eq!(
        grant_result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(ConfigManagerError::Unauthorized as u32),
        "impostor grant_role attempt must return Unauthorized (3)"
    );
}

/// Granting an extra role to the admin address must not corrupt the instance store.
#[test]
fn test_granting_extra_role_to_admin_does_not_corrupt_instance_store() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);
    let victim = Address::generate(&env);

    let keeper_role = role_keeper(&env);
    let admin_role = role_admin(&env);

    // Grant the admin address KEEPER as well — must not clobber the instance-store Admin entry.
    client.grant_role(&admin, &keeper_role, &admin);

    // Persistent store: admin still holds ADMIN.
    assert!(
        client.has_role(&admin_role, &admin),
        "admin must still hold ADMIN role after being granted KEEPER as well"
    );

    // Instance store: admin is still accepted by require_admin.
    let grant_result = client.try_grant_role(&admin, &keeper_role, &victim);
    assert!(
        grant_result.is_ok(),
        "admin must still be accepted by require_admin after receiving an extra role"
    );
}
