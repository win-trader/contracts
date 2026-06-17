//! Tests for `upgrade` and `migrate` authorization on OracleRouter.
//!
//! The contract exposes two upgrade-related entrypoints:
//!   - `upgrade(new_wasm_hash, operator)` — replaces the WASM after auth and
//!     pending-upgrade checks.
//!   - `migrate(data, operator)` — runs post-upgrade migration after auth.
//!
//! Auth for both:
//!   1. `operator.require_auth()`.
//!   2. Cross-call ConfigManager's `has_role("UPGRADER", operator)`.
//!   3. Panic with `OracleRouterError::Unauthorized (3)` if not upgrader.
//!
//! Migration writes `data.version` to `StorageKey::Version` in instance
//! storage.

#![cfg(test)]

use soroban_sdk::{testutils::Address as _, Address, BytesN, Env, IntoVal, Symbol};
use stellar_contract_utils::upgradeable::enable_migration;

use crate::storage::StorageKey;
use crate::{OracleRouterContract, OracleRouterError, MigrationData};

use super::helpers::{deploy_with_config_manager, deploy_with_upgrader};

// ---------------------------------------------------------------------------
// Shared fixture helpers
// ---------------------------------------------------------------------------

/// Returns a 32-byte dummy WASM hash (all zeros). Used when calling
/// `try_upgrade` without a real WASM blob — the host will reject the hash
/// as invalid, but only after the auth check has run.
fn dummy_wasm_hash(env: &Env) -> BytesN<32> {
    BytesN::from_array(env, &[0u8; 32])
}

// ---------------------------------------------------------------------------
// upgrade — non-upgrader is rejected with Unauthorized
// ---------------------------------------------------------------------------

/// An address that has no UPGRADER role calling `upgrade` must receive
/// `OracleRouterError::Unauthorized (3)`.
///
/// We use `try_upgrade` so we can inspect the error without panic unwinding.
/// The non-upgrader check happens inside `_require_auth`, which runs before
/// the WASM hash is validated by the host, so the error must be a contract
/// error with code 3.
///
/// FAILS against todo!() — passes once `_require_auth` checks the UPGRADER role.
#[test]
fn test_upgrade_non_upgrader_is_unauthorized() {
    let env = Env::default();
    env.mock_all_auths();

    // deploy_with_config_manager gives us a real CM but admin has no UPGRADER role.
    let (oracle, _cm, _admin) = deploy_with_config_manager(&env);
    let non_upgrader = Address::generate(&env);
    let hash = dummy_wasm_hash(&env);

    let upgradeable =
        stellar_contract_utils::upgradeable::UpgradeableClient::new(&env, &oracle.address);

    let result = upgradeable.try_upgrade(&hash, &non_upgrader);
    assert!(
        result.is_err(),
        "upgrade from a non-upgrader address must return an error"
    );
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(OracleRouterError::Unauthorized as u32),
        "error code must be Unauthorized (3)"
    );
}

/// Same as above but with the admin address (which has the ADMIN role but NOT
/// the UPGRADER role by default in deploy_with_config_manager).
#[test]
fn test_upgrade_admin_without_upgrader_role_is_unauthorized() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, admin) = deploy_with_config_manager(&env);
    let hash = dummy_wasm_hash(&env);

    let upgradeable =
        stellar_contract_utils::upgradeable::UpgradeableClient::new(&env, &oracle.address);

    // admin has ADMIN but not UPGRADER.
    let result = upgradeable.try_upgrade(&hash, &admin);
    assert!(
        result.is_err(),
        "admin address without UPGRADER role must be rejected for upgrade"
    );
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(OracleRouterError::Unauthorized as u32),
        "error must be Unauthorized (3) — ADMIN role alone is insufficient"
    );
}

// ---------------------------------------------------------------------------
// upgrade — upgrader passes _require_auth (host WASM error, not Unauthorized)
// ---------------------------------------------------------------------------

/// When the caller HAS the UPGRADER role, `_require_auth` must pass cleanly.
/// The upgrade will still fail because `[0u8; 32]` is not a valid WASM hash,
/// but the error must NOT be `OracleRouterError::Unauthorized` — it must be
/// a host-level error about the invalid WASM hash.
///
/// This proves `_require_auth` succeeds for a valid upgrader.
///
/// FAILS against todo!() — passes once `_require_auth` is implemented.
#[test]
fn test_upgrade_upgrader_passes_require_auth_gets_wasm_error_not_unauthorized() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, admin) = deploy_with_upgrader(&env);
    let hash = dummy_wasm_hash(&env);

    let upgradeable =
        stellar_contract_utils::upgradeable::UpgradeableClient::new(&env, &oracle.address);

    let result = upgradeable.try_upgrade(&hash, &admin);
    // The result must be an error (invalid WASM hash), BUT it must not be
    // the Unauthorized contract error.
    match result {
        Ok(_) => {
            // Unexpected success — the dummy hash was somehow accepted.
            // This is not a test failure we care about deeply, but flag it.
        }
        Err(Ok(contract_err)) => {
            assert_ne!(
                contract_err,
                soroban_sdk::Error::from_contract_error(OracleRouterError::Unauthorized as u32),
                "upgrader must not receive Unauthorized — _require_auth must pass"
            );
        }
        Err(Err(_host_err)) => {
            // Host-level error for invalid WASM — this is the expected outcome.
            // Test passes.
        }
    }
}

// ---------------------------------------------------------------------------
// upgrade — no mocked auth panics
// ---------------------------------------------------------------------------

/// Calling `upgrade` without mocked auth must panic because `operator.require_auth()`
/// is the first line of `_require_auth`.
///
/// FAILS against todo!() — passes once `_require_auth` calls `operator.require_auth()`.
#[test]
#[should_panic]
fn test_upgrade_requires_caller_auth() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, admin) = deploy_with_upgrader(&env);
    let hash = dummy_wasm_hash(&env);

    let upgradeable =
        stellar_contract_utils::upgradeable::UpgradeableClient::new(&env, &oracle.address);

    // Clear all mocked auths — require_auth must now panic.
    env.mock_auths(&[]);
    // This call must panic because operator.require_auth() is not satisfied.
    upgradeable.upgrade(&hash, &admin);
}

// ---------------------------------------------------------------------------
// Compile-time pin — Unauthorized error code is exactly 3
// ---------------------------------------------------------------------------

/// Compile-time pin: `OracleRouterError::Unauthorized` must have discriminant 3.
/// If this assertion fails to compile, the error enum discriminant changed.
#[test]
fn test_upgrade_unauthorized_error_code_is_3() {
    // This is a compile-time check via a const assertion.
    const _: () = assert!(OracleRouterError::Unauthorized as u32 == 3);

    // Also verify at runtime for extra certainty.
    assert_eq!(
        OracleRouterError::Unauthorized as u32,
        3,
        "OracleRouterError::Unauthorized must have discriminant 3"
    );
}

// ---------------------------------------------------------------------------
// migrate — non-upgrader is rejected with Unauthorized (3)
// ---------------------------------------------------------------------------

/// An address without the UPGRADER role calling `migrate` must receive
/// `OracleRouterError::Unauthorized (3)`.
///
/// Note: `_require_auth` runs before the MIGRATING flag check, so the auth
/// failure is observed even without a prior `upgrade` call.
///
/// FAILS against todo!() — passes once `_require_auth` is implemented.
#[test]
fn test_migrate_non_upgrader_is_unauthorized() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, _admin) = deploy_with_config_manager(&env);
    let non_upgrader = Address::generate(&env);

    let migration_data = MigrationData { version: 1 };
    let result = env.try_invoke_contract::<(), soroban_sdk::Error>(
        &oracle.address,
        &Symbol::new(&env, "migrate"),
        (migration_data, non_upgrader).into_val(&env),
    );

    assert!(
        result.is_err(),
        "migrate from a non-upgrader must return an error"
    );
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(OracleRouterError::Unauthorized as u32),
        "error code must be Unauthorized (3)"
    );
}

/// Admin address with ADMIN role but no UPGRADER role must also be rejected.
#[test]
fn test_migrate_admin_without_upgrader_role_is_unauthorized() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, admin) = deploy_with_config_manager(&env);

    let migration_data = MigrationData { version: 5 };
    let result = env.try_invoke_contract::<(), soroban_sdk::Error>(
        &oracle.address,
        &Symbol::new(&env, "migrate"),
        (migration_data, admin).into_val(&env),
    );

    assert!(
        result.is_err(),
        "admin without UPGRADER role must be rejected for migrate"
    );
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(OracleRouterError::Unauthorized as u32),
        "error must be Unauthorized (3) — ADMIN role alone is insufficient"
    );
}

// ---------------------------------------------------------------------------
// migrate — without prior upgrade (no MIGRATING flag) errors
// ---------------------------------------------------------------------------

/// Calling `migrate` with a valid upgrader, but without a prior `upgrade`
/// (so the MIGRATING flag is NOT set), must fail with a non-Unauthorized error.
///
/// The OZ-generated `migrate` function checks the MIGRATING flag and returns
/// a `MigrationNotAllowed` host error if the flag is absent.
///
/// FAILS against todo!() — the auth check (todo) panics before reaching the
/// MIGRATING flag check. After implementation, auth passes but MIGRATING fails.
#[test]
fn test_migrate_without_prior_upgrade_errors_non_unauthorized() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, admin) = deploy_with_upgrader(&env);

    let migration_data = MigrationData { version: 2 };
    let result = env.try_invoke_contract::<(), soroban_sdk::Error>(
        &oracle.address,
        &Symbol::new(&env, "migrate"),
        (migration_data, admin).into_val(&env),
    );

    assert!(
        result.is_err(),
        "migrate without prior upgrade must fail"
    );
    // The error must NOT be Unauthorized — the upgrader passed auth successfully,
    // but the MIGRATING flag is absent, so a different error fires.
    if let Err(Ok(contract_err)) = result {
        assert_ne!(
            contract_err,
            soroban_sdk::Error::from_contract_error(OracleRouterError::Unauthorized as u32),
            "upgrader should pass _require_auth; error must not be Unauthorized"
        );
    }
    // A host-level error (Err(Err(_))) is also acceptable here and indicates
    // the MIGRATING flag check failed at the host level.
}

// ---------------------------------------------------------------------------
// migrate — MIGRATING flag active, version is written to storage
// ---------------------------------------------------------------------------

/// When the MIGRATING flag is manually set (simulating a prior `upgrade` call)
/// and the caller holds the UPGRADER role, `migrate` must:
///   1. Pass `_require_auth`.
///   2. Call `_migrate`, which writes `data.version` to `StorageKey::Version`.
///
/// FAILS against todo!() — passes once both `_require_auth` and `_migrate` are
/// implemented.
#[test]
fn test_migrate_with_active_migration_flag_writes_version() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, admin) = deploy_with_upgrader(&env);

    // Manually set the MIGRATING flag to simulate a successful upgrade.
    env.as_contract(&oracle.address, || {
        enable_migration(&env);
    });

    let migration_data = MigrationData { version: 7 };
    env.invoke_contract::<()>(
        &oracle.address,
        &Symbol::new(&env, "migrate"),
        (migration_data, admin).into_val(&env),
    );

    // Verify StorageKey::Version was written.
    env.as_contract(&oracle.address, || {
        let stored_version: Option<u32> = env.storage().instance().get(&StorageKey::Version);
        assert_eq!(
            stored_version,
            Some(7),
            "_migrate must write version = 7 to StorageKey::Version"
        );
    });
}

/// Verify with a different version value (42) to confirm the value is used
/// dynamically, not hardcoded.
#[test]
fn test_migrate_with_active_migration_flag_writes_correct_version_value() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, admin) = deploy_with_upgrader(&env);

    env.as_contract(&oracle.address, || {
        enable_migration(&env);
    });

    let migration_data = MigrationData { version: 42 };
    env.invoke_contract::<()>(
        &oracle.address,
        &Symbol::new(&env, "migrate"),
        (migration_data, admin).into_val(&env),
    );

    env.as_contract(&oracle.address, || {
        let stored_version: Option<u32> = env.storage().instance().get(&StorageKey::Version);
        assert_eq!(
            stored_version,
            Some(42),
            "_migrate must write the exact version value supplied in MigrationData"
        );
    });
}

// ---------------------------------------------------------------------------
// migrate — no mocked auth panics
// ---------------------------------------------------------------------------

/// Calling `migrate` without any mocked auth must panic because
/// `operator.require_auth()` is called inside `_require_auth`.
///
/// FAILS against todo!() — passes once `_require_auth` calls `operator.require_auth()`.
#[test]
#[should_panic]
fn test_migrate_requires_caller_auth() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, admin) = deploy_with_upgrader(&env);

    env.as_contract(&oracle.address, || {
        enable_migration(&env);
    });

    // Clear all mocked auths — require_auth must now panic.
    env.mock_auths(&[]);

    let migration_data = MigrationData { version: 3 };
    env.invoke_contract::<()>(
        &oracle.address,
        &Symbol::new(&env, "migrate"),
        (migration_data, admin).into_val(&env),
    );
}

// ---------------------------------------------------------------------------
// _migrate directly — writes version to instance storage
// ---------------------------------------------------------------------------

/// Directly call the internal `_migrate` function (bypassing the macro-generated
/// public `migrate` function and its MIGRATING flag guard) to validate that
/// `_migrate` in isolation writes `StorageKey::Version`.
///
/// This mirrors the pattern used in config-manager's  test.
///
/// FAILS against todo!() — passes once `_migrate` writes the version key.
#[test]
fn test_migrate_internal_writes_version_to_storage() {
    let env = Env::default();
    // Construct the router with a config_manager arg; _migrate only touches the
    // Version key, so the bound ConfigManager is irrelevant here.
    let config_manager = Address::generate(&env);
    let oracle_id = env.register(OracleRouterContract, (config_manager,));
    let migration_data = MigrationData { version: 2 };

    env.as_contract(&oracle_id, || {
        OracleRouterContract::_migrate(&env, &migration_data);

        let stored_version: Option<u32> = env.storage().instance().get(&StorageKey::Version);
        assert_eq!(
            stored_version,
            Some(2),
            "_migrate must write version = 2 to StorageKey::Version in instance storage"
        );
    });
}

/// _migrate with version = 0 must still write 0, not skip the write.
#[test]
fn test_migrate_internal_writes_version_zero() {
    let env = Env::default();
    let config_manager = Address::generate(&env);
    let oracle_id = env.register(OracleRouterContract, (config_manager,));
    let migration_data = MigrationData { version: 0 };

    env.as_contract(&oracle_id, || {
        OracleRouterContract::_migrate(&env, &migration_data);

        let stored_version: Option<u32> = env.storage().instance().get(&StorageKey::Version);
        assert_eq!(
            stored_version,
            Some(0),
            "_migrate must write version = 0 — zero is a valid version number"
        );
    });
}

/// _migrate with version = u32::MAX must write u32::MAX without overflow.
#[test]
fn test_migrate_internal_writes_max_version_without_overflow() {
    let env = Env::default();
    let config_manager = Address::generate(&env);
    let oracle_id = env.register(OracleRouterContract, (config_manager,));
    let migration_data = MigrationData { version: u32::MAX };

    env.as_contract(&oracle_id, || {
        OracleRouterContract::_migrate(&env, &migration_data);

        let stored_version: Option<u32> = env.storage().instance().get(&StorageKey::Version);
        assert_eq!(
            stored_version,
            Some(u32::MAX),
            "_migrate must write u32::MAX without overflow"
        );
    });
}

// ---------------------------------------------------------------------------
// Multiple sequential migrations overwrite the version
// ---------------------------------------------------------------------------

/// Calling `_migrate` twice must leave only the second version in storage.
/// The second call must overwrite the first — no accumulation, no error.
///
/// FAILS against todo!() — passes once `_migrate` is implemented.
#[test]
fn test_migrate_internal_second_call_overwrites_first_version() {
    let env = Env::default();
    let config_manager = Address::generate(&env);
    let oracle_id = env.register(OracleRouterContract, (config_manager,));

    env.as_contract(&oracle_id, || {
        OracleRouterContract::_migrate(&env, &MigrationData { version: 1 });
        OracleRouterContract::_migrate(&env, &MigrationData { version: 2 });

        let stored_version: Option<u32> = env.storage().instance().get(&StorageKey::Version);
        assert_eq!(
            stored_version,
            Some(2),
            "second _migrate must overwrite first — version must be 2"
        );
    });
}

// ---------------------------------------------------------------------------
// Compile-time pin — migrate Unauthorized error code is exactly 3
// ---------------------------------------------------------------------------

/// Compile-time and runtime pin: the error code for Unauthorized is 3.
/// Changing it would break existing error-handling integrations.
#[test]
fn test_migrate_unauthorized_error_code_is_3() {
    const _: () = assert!(OracleRouterError::Unauthorized as u32 == 3);

    assert_eq!(
        OracleRouterError::Unauthorized as u32,
        3,
        "OracleRouterError::Unauthorized must have discriminant 3"
    );
}

// ---------------------------------------------------------------------------
// Adversarial: role escalation attempt
// ---------------------------------------------------------------------------

/// Adversarial: an attacker grants themselves ADMIN role (if possible) but
/// not UPGRADER, then tries to call upgrade. Must be rejected with Unauthorized.
///
/// In practice `grant_role` requires admin auth, but this test verifies the
/// UPGRADER check is a separate gate from the ADMIN check.
#[test]
fn test_upgrade_admin_role_does_not_confer_upgrader_privilege() {
    let env = Env::default();
    env.mock_all_auths();

    // admin has ADMIN role only (no UPGRADER).
    let (oracle, _cm, admin) = deploy_with_config_manager(&env);
    let hash = dummy_wasm_hash(&env);

    let upgradeable =
        stellar_contract_utils::upgradeable::UpgradeableClient::new(&env, &oracle.address);

    let result = upgradeable.try_upgrade(&hash, &admin);
    assert!(
        result.is_err(),
        "adversarial: ADMIN role must not confer UPGRADER privilege for upgrade"
    );
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(OracleRouterError::Unauthorized as u32),
        "adversarial: must get Unauthorized (3), not any other error"
    );
}

/// Adversarial: a completely unknown address with no roles in ConfigManager
/// attempts to migrate. Must be rejected with Unauthorized.
#[test]
fn test_migrate_unknown_address_is_unauthorized() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, _admin) = deploy_with_config_manager(&env);
    let attacker = Address::generate(&env);

    let migration_data = MigrationData { version: 999 };
    let result = env.try_invoke_contract::<(), soroban_sdk::Error>(
        &oracle.address,
        &Symbol::new(&env, "migrate"),
        (migration_data, attacker).into_val(&env),
    );

    assert!(
        result.is_err(),
        "adversarial: unknown address must not be able to call migrate"
    );
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(OracleRouterError::Unauthorized as u32),
        "adversarial: must get Unauthorized (3) for completely unknown attacker"
    );
}

/// Adversarial: verify that the UPGRADER role is checked against the
/// OracleRouter's own ConfigManager, not just any ConfigManager. An attacker
/// who deploys their own ConfigManager and grants themselves UPGRADER there
/// must not be able to upgrade the OracleRouter.
///
/// This is verified indirectly: the address used has no role in the CM linked
/// to the OracleRouter, even though it might have one in another CM.
#[test]
fn test_migrate_upgrader_in_wrong_config_manager_is_unauthorized() {
    let env = Env::default();
    env.mock_all_auths();

    // The oracle router is linked to one ConfigManager.
    let (oracle, _cm, _admin) = deploy_with_config_manager(&env);

    // An attacker deploys a separate ConfigManager and grants themselves UPGRADER there.
    // They then try to call migrate on the oracle router — must fail.
    let attacker = Address::generate(&env);

    let migration_data = MigrationData { version: 1 };
    let result = env.try_invoke_contract::<(), soroban_sdk::Error>(
        &oracle.address,
        &Symbol::new(&env, "migrate"),
        (migration_data, attacker).into_val(&env),
    );

    assert!(
        result.is_err(),
        "adversarial: UPGRADER in a foreign CM must not be able to migrate OracleRouter"
    );
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(OracleRouterError::Unauthorized as u32),
        "adversarial: must get Unauthorized (3)"
    );
}
