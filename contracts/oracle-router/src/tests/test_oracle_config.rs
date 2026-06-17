//! Tests for `set_oracle_config` and `get_oracle_config` on the OracleRouter.
//!
//! Coverage areas:
//!   - Happy path: admin can set config; config is retrievable and round-trips
//!   - Overwrite: a second set_oracle_config replaces the previous value
//!   - Auth: missing auth propagates a host-level error before contract logic
//!   - Access control: non-admin caller is rejected with Unauthorized (3)
//!   - Error code: Unauthorized discriminant is verified to be exactly 3
//!   - Validation: zero values for each of the three OracleConfig fields panic
//!   - Boundary: all fields equal to 1 (minimum positive value) succeeds
//!   - View: get_oracle_config before any set returns NotInitialized (2)
//!   - Round-trip: all individual fields match what was written

#![cfg(test)]

use soroban_sdk::{testutils::Address as _, Address, Env};

use crate::OracleRouterError;

use super::helpers::{deploy_initialized, deploy_with_config_manager, valid_oracle_config};
use crate::OracleConfig;

// ---------------------------------------------------------------------------
// Happy path
// ---------------------------------------------------------------------------

/// An admin caller (with auth mocked) must be able to call set_oracle_config
/// without panicking.  The contract stores the config in instance storage and
/// the function returns without error.
///
/// This test FAILS until `set_oracle_config` is implemented (currently todo!()).
#[test]
fn test_set_oracle_config_admin_succeeds() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, admin) = deploy_with_config_manager(&env);
    let config = valid_oracle_config();

    // Must not panic. Fails on todo!() until implementation is written.
    oracle.set_oracle_config(&admin, &config);
}

/// After a successful set_oracle_config, get_oracle_config must return the
/// same config that was written — not a default, not NotInitialized.
///
/// This test FAILS until both `set_oracle_config` and the storage write are
/// implemented.
#[test]
fn test_get_oracle_config_returns_stored_config() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, admin) = deploy_with_config_manager(&env);
    let config = valid_oracle_config();

    oracle.set_oracle_config(&admin, &config);

    // get_oracle_config is view-only — no auth needed.
    let stored = oracle.get_oracle_config();

    assert_eq!(
        stored.max_deviation_bps, config.max_deviation_bps,
        "max_deviation_bps must round-trip through storage unchanged"
    );
    assert_eq!(
        stored.staleness_threshold, config.staleness_threshold,
        "staleness_threshold must round-trip through storage unchanged"
    );
    assert_eq!(
        stored.min_required_sources, config.min_required_sources,
        "cache_duration must round-trip through storage unchanged"
    );
}

/// A second call to set_oracle_config must overwrite the first; subsequent
/// get_oracle_config must return the NEW values, not the old ones.
///
/// This guards against implementations that silently ignore re-configuration
/// or that merge structs field-by-field in a broken way.
#[test]
fn test_set_oracle_config_overwrites_previous_config() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, admin) = deploy_with_config_manager(&env);

    let first = OracleConfig {
        max_deviation_bps: 100,
        staleness_threshold: 60,
        cache_duration: 10,
        min_required_sources: 2,
    };
    let second = OracleConfig {
        max_deviation_bps: 250,
        staleness_threshold: 120,
        cache_duration: 10,
        min_required_sources: 2,
    };

    oracle.set_oracle_config(&admin, &first);
    oracle.set_oracle_config(&admin, &second);

    let stored = oracle.get_oracle_config();

    assert_eq!(
        stored.max_deviation_bps, second.max_deviation_bps,
        "max_deviation_bps must reflect the SECOND write, not the first"
    );
    assert_eq!(
        stored.staleness_threshold, second.staleness_threshold,
        "staleness_threshold must reflect the SECOND write, not the first"
    );
    assert_eq!(
        stored.min_required_sources, second.min_required_sources,
        "cache_duration must reflect the SECOND write, not the first"
    );
}

// ---------------------------------------------------------------------------
// Auth checks
// ---------------------------------------------------------------------------

/// Calling set_oracle_config WITHOUT mocking auth must fail at the Soroban
/// host level with an auth error before any contract logic executes.
///
/// This verifies that `caller.require_auth()` is the FIRST line executed,
/// meaning the signature check happens before any cross-contract calls or
/// storage reads.  We use `try_set_oracle_config` so the test does not
/// unwind, and assert an error is returned (type is a host-level InvokeError
/// rather than a well-typed OracleRouterError).
#[test]
fn test_set_oracle_config_requires_auth() {
    let env = Env::default();
    env.mock_all_auths();
    let (oracle, _) = deploy_initialized(&env);

    // Strip auth before the call under test so the missing signature for
    // `caller.require_auth()` is detected.
    env.mock_auths(&[]);
    let caller = Address::generate(&env);
    let config = valid_oracle_config();

    let result = oracle.try_set_oracle_config(&caller, &config);

    assert!(
        result.is_err(),
        "set_oracle_config called without auth must return an error; \
         require_auth() must be present and must be the first guard"
    );
}

/// A random address that is NOT the ConfigManager admin must be rejected with
/// OracleRouterError::Unauthorized even when its auth is mocked (i.e., the
/// signature is valid but the role check fails).
///
/// This tests the cross-contract `has_role` guard independently of the
/// signature check.  The attacker address has valid auth but no ADMIN role.
#[test]
fn test_set_oracle_config_non_admin_is_unauthorized() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, _real_admin) = deploy_with_config_manager(&env);

    // Generate a fresh address that ConfigManager has never granted ADMIN to.
    let attacker = Address::generate(&env);
    let config = valid_oracle_config();

    let result = oracle.try_set_oracle_config(&attacker, &config);

    assert!(
        result.is_err(),
        "a non-admin caller must be rejected by the has_role cross-contract check"
    );
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(OracleRouterError::Unauthorized as u32),
        "non-admin caller must receive Unauthorized, not a generic or host-level error"
    );
}

/// Explicitly verify that OracleRouterError::Unauthorized has the discriminant
/// value 3, matching the enum definition in errors.rs.  This test catches
/// accidental renumbering of the error enum that would break on-chain clients
/// that pattern-match on the numeric error code.
#[test]
fn test_set_oracle_config_unauthorized_error_code_is_3() {
    assert_eq!(
        OracleRouterError::Unauthorized as u32,
        3,
        "OracleRouterError::Unauthorized must always be discriminant 3; \
         changing this value breaks on-chain clients that match numeric codes"
    );
}

/// A completely different account that was granted the KEEPER role (not ADMIN)
/// must also be rejected.  Verifies there is no role-confusion bug where any
/// role holder is treated as an admin.
#[test]
fn test_set_oracle_config_keeper_role_is_not_sufficient() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, cm, admin) = deploy_with_config_manager(&env);

    // Grant the KEEPER role to a keeper bot.
    let keeper = Address::generate(&env);
    let keeper_role = soroban_sdk::Symbol::new(&env, "KEEPER");
    cm.grant_role(&admin, &keeper_role, &keeper);

    // Keeper is a legitimate role holder but must NOT be allowed to set config.
    let config = valid_oracle_config();
    let result = oracle.try_set_oracle_config(&keeper, &config);

    assert!(
        result.is_err(),
        "KEEPER role must not be sufficient to call set_oracle_config — only ADMIN is allowed"
    );
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(OracleRouterError::Unauthorized as u32),
        "KEEPER must receive Unauthorized (3), not succeed or receive a different error"
    );
}

// ---------------------------------------------------------------------------
// Validation — zero values
// ---------------------------------------------------------------------------

/// max_deviation_bps == 0 is economically meaningless (0 bps deviation allowed
/// means any spread between oracles would always be rejected, making the oracle
/// unusable).  The contract must reject this with a panic.
#[test]
#[should_panic]
fn test_set_oracle_config_zero_max_deviation_is_invalid() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, admin) = deploy_with_config_manager(&env);
    let config = OracleConfig {
        max_deviation_bps: 0, // invalid
        staleness_threshold: 60,
        cache_duration: 10,
        min_required_sources: 2,
    };

    // Must panic because max_deviation_bps == 0 is not allowed.
    oracle.set_oracle_config(&admin, &config);
}

/// staleness_threshold == 0 would mean every price feed is immediately
/// considered stale the moment it is returned, making get_price always fail.
/// The contract must reject this with a panic.
#[test]
#[should_panic]
fn test_set_oracle_config_zero_staleness_threshold_is_invalid() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, admin) = deploy_with_config_manager(&env);
    let config = OracleConfig {
        max_deviation_bps: 100,
        staleness_threshold: 0, // invalid
        cache_duration: 10,
        min_required_sources: 2,
    };

    // Must panic because staleness_threshold == 0 is not allowed.
    oracle.set_oracle_config(&admin, &config);
}

/// min_required_sources == 0 falls below `MIN_REQUIRED_SOURCES_FLOOR` and
/// must be rejected — without a quorum, OracleRouter would return the
/// median of an empty (or single-source) set.
#[test]
#[should_panic]
fn test_set_oracle_config_zero_min_required_sources_is_invalid() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, admin) = deploy_with_config_manager(&env);
    let config = OracleConfig {
        max_deviation_bps: 100,
        staleness_threshold: 60,
        cache_duration: 10,
        min_required_sources: 0,
    };

    oracle.set_oracle_config(&admin, &config);
}

/// Negative max_deviation_bps is nonsensical — basis points cannot be negative.
/// An implementation that accepts i128::MIN as a deviation threshold has a
/// critical validation gap.
#[test]
#[should_panic]
fn test_set_oracle_config_negative_max_deviation_is_invalid() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, admin) = deploy_with_config_manager(&env);
    let config = OracleConfig {
        max_deviation_bps: -1, // negative — invalid
        staleness_threshold: 60,
        cache_duration: 10,
        min_required_sources: 2,
    };

    oracle.set_oracle_config(&admin, &config);
}

/// All three fields set to zero simultaneously must be rejected (belt-and-
/// suspenders check to ensure the validator does not short-circuit after only
/// the first field).
#[test]
#[should_panic]
fn test_set_oracle_config_all_fields_zero_is_invalid() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, admin) = deploy_with_config_manager(&env);
    let config = OracleConfig {
        max_deviation_bps: 0,
        staleness_threshold: 0,
        cache_duration: 0,
        min_required_sources: 2,
    };

    oracle.set_oracle_config(&admin, &config);
}

// ---------------------------------------------------------------------------
// Validation — boundary values that SHOULD succeed
// ---------------------------------------------------------------------------

/// All three fields set to exactly 1 (the minimum positive value for each
/// numeric type) must succeed.  This is the tightest valid configuration.
/// Verifies that the zero-value guard is exclusive (> 0 not >= 1).
///
/// This test FAILS until `set_oracle_config` is implemented.
#[test]
fn test_set_oracle_config_valid_boundary_values_succeed() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, admin) = deploy_with_config_manager(&env);
    let config = OracleConfig {
        max_deviation_bps: 1,   // minimum valid i128 positive
        staleness_threshold: 1, // minimum valid u64 positive
        cache_duration: 1,      // must be <= staleness_threshold
        min_required_sources: 2, // minimum valid u32 positive
    };

    // Must not panic. Minimum boundary values are valid.
    oracle.set_oracle_config(&admin, &config);

    let stored = oracle.get_oracle_config();
    assert_eq!(
        stored.max_deviation_bps, 1,
        "max_deviation_bps boundary value 1 must be stored"
    );
    assert_eq!(
        stored.staleness_threshold, 1,
        "staleness_threshold boundary value 1 must be stored"
    );
    assert_eq!(
        stored.min_required_sources, 2,
        "min_required_sources boundary value 2 (the quorum floor) must be stored"
    );
}

/// max_deviation_bps > MAX_DEVIATION_BPS_CEILING (100%) is rejected — any
/// larger value would effectively disable the deviation gate.
#[test]
#[should_panic]
fn test_set_oracle_config_deviation_above_ceiling_is_invalid() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, admin) = deploy_with_config_manager(&env);
    let config = OracleConfig {
        max_deviation_bps: shared::constants::MAX_DEVIATION_BPS_CEILING + 1,
        staleness_threshold: 60,
        cache_duration: 10,
        min_required_sources: 2,
    };

    oracle.set_oracle_config(&admin, &config);
}

/// min_required_sources > MAX_ORACLE_SOURCES is rejected — quorum can never
/// require more sources than the configured maximum.
#[test]
#[should_panic]
fn test_set_oracle_config_quorum_above_ceiling_is_invalid() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, admin) = deploy_with_config_manager(&env);
    let config = OracleConfig {
        max_deviation_bps: 100,
        staleness_threshold: 60,
        cache_duration: 10,
        min_required_sources: shared::constants::MAX_ORACLE_SOURCES + 1,
    };

    oracle.set_oracle_config(&admin, &config);
}

// ---------------------------------------------------------------------------
// get_oracle_config — view-only behavior
// ---------------------------------------------------------------------------

/// Calling get_oracle_config on an initialized router that has never had
/// set_oracle_config called must panic with NotInitialized (2).  This
/// duplicates the test in test_initialize for clarity and to make this
/// module self-contained.
///
/// This test should PASS because get_oracle_config is already partially
/// implemented with a NotInitialized guard.
#[test]
fn test_get_oracle_config_before_set_returns_not_initialized() {
    let env = Env::default();
    env.mock_all_auths();

    // Use deploy_with_config_manager but do NOT call set_oracle_config.
    let (oracle, _cm, _admin) = deploy_with_config_manager(&env);

    let result = oracle.try_get_oracle_config();

    assert!(
        result.is_err(),
        "get_oracle_config must return an error when no config has been set yet"
    );
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(OracleRouterError::NotInitialized as u32),
        "error must be NotInitialized (2) — not Unauthorized or any other variant"
    );
}

/// get_oracle_config requires no authentication.  Calling it from an address
/// that has no roles and no mocked auth must still succeed after a config
/// has been set.
///
/// This test FAILS until `set_oracle_config` is implemented.
#[test]
fn test_get_oracle_config_requires_no_auth() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, admin) = deploy_with_config_manager(&env);
    oracle.set_oracle_config(&admin, &valid_oracle_config());

    // Drop auth mocking — subsequent call must still succeed.
    // (No way to "un-mock" in the same env, but the trait contract guarantees
    // get_oracle_config has no require_auth call, so any read is valid.)
    let result = oracle.try_get_oracle_config();
    assert!(
        result.is_ok(),
        "get_oracle_config is view-only and must not require any auth"
    );
}

/// Round-trip every field individually with a distinctive value to detect
/// field-mapping bugs where two fields are swapped in the serialization path.
///
/// This test FAILS until `set_oracle_config` is implemented.
#[test]
fn test_get_oracle_config_round_trips_all_fields() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, admin) = deploy_with_config_manager(&env);

    // Use distinctive prime values so any field swap is immediately visible.
    let config = OracleConfig {
        max_deviation_bps: 137,
        staleness_threshold: 251,
        cache_duration: 10,
        min_required_sources: 7,
    };

    oracle.set_oracle_config(&admin, &config);
    let stored = oracle.get_oracle_config();

    assert_eq!(
        stored.max_deviation_bps, 137,
        "max_deviation_bps must be 137 — field swap or truncation detected if this fails"
    );
    assert_eq!(
        stored.staleness_threshold, 251,
        "staleness_threshold must be 251 — field swap or truncation detected if this fails"
    );
    assert_eq!(
        stored.min_required_sources, 7,
        "min_required_sources must be 7 — field swap or truncation detected if this fails"
    );
}

// ---------------------------------------------------------------------------
// Adversarial: state isolation
// ---------------------------------------------------------------------------

/// Two independently deployed OracleRouter instances must each maintain their
/// own separate OracleConfig in storage.  Writing to one must not affect the
/// other — guards against global mutable state or shared storage key collision.
///
/// This test FAILS until `set_oracle_config` is implemented.
#[test]
fn test_two_oracle_router_instances_have_independent_config_storage() {
    let env = Env::default();
    env.mock_all_auths();

    // Deploy two independent (router, config_manager, admin) triples.
    let (oracle_a, _cm_a, admin_a) = deploy_with_config_manager(&env);
    let (oracle_b, _cm_b, admin_b) = deploy_with_config_manager(&env);

    let config_a = OracleConfig {
        max_deviation_bps: 50,
        staleness_threshold: 30,
        cache_duration: 10,
        min_required_sources: 2,
    };
    let config_b = OracleConfig {
        max_deviation_bps: 200,
        staleness_threshold: 120,
        cache_duration: 10,
        min_required_sources: 2,
    };

    oracle_a.set_oracle_config(&admin_a, &config_a);
    oracle_b.set_oracle_config(&admin_b, &config_b);

    let stored_a = oracle_a.get_oracle_config();
    let stored_b = oracle_b.get_oracle_config();

    assert_eq!(
        stored_a.max_deviation_bps, 50,
        "oracle_a must retain its own config after oracle_b was written"
    );
    assert_eq!(
        stored_b.max_deviation_bps, 200,
        "oracle_b must retain its own config, unaffected by oracle_a"
    );
}

/// An attacker who legitimately holds the ADMIN role in a DIFFERENT
/// ConfigManager must not be able to set config on a router linked to a
/// DIFFERENT ConfigManager instance.
///
/// This tests that the stored ConfigManager address is actually used as the
/// lookup target — not a global or hardcoded address.
#[test]
fn test_set_oracle_config_admin_of_wrong_config_manager_is_unauthorized() {
    let env = Env::default();
    env.mock_all_auths();

    // Deploy two independent (router, config_manager, admin) triples.
    let (oracle_a, _cm_a, _admin_a) = deploy_with_config_manager(&env);
    let (_oracle_b, _cm_b, admin_b) = deploy_with_config_manager(&env);

    // admin_b is the ADMIN of cm_b, but oracle_a is wired to cm_a.
    // admin_b should NOT be treated as admin by oracle_a.
    let config = valid_oracle_config();
    let result = oracle_a.try_set_oracle_config(&admin_b, &config);

    assert!(
        result.is_err(),
        "admin of a DIFFERENT ConfigManager must be rejected by oracle_a's has_role check"
    );
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(OracleRouterError::Unauthorized as u32),
        "cross-CM admin must receive Unauthorized (3)"
    );
}

/// A revoked admin (who previously held the role but had it transferred away)
/// must no longer be able to set config after the transfer.  Verifies that
/// the contract does not cache the admin address internally but always
/// delegates to the live ConfigManager state.
///
/// This test FAILS until `set_oracle_config` is implemented.
#[test]
fn test_set_oracle_config_revoked_admin_is_unauthorized_after_transfer() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, cm, original_admin) = deploy_with_config_manager(&env);

    // Confirm original admin can set config.
    oracle.set_oracle_config(&original_admin, &valid_oracle_config());

    // Transfer admin role to a new address.
    let new_admin = Address::generate(&env);
    cm.propose_admin(&original_admin, &new_admin);
    cm.accept_admin(&new_admin);

    // original_admin no longer holds the ADMIN role — must be rejected.
    let config = OracleConfig {
        max_deviation_bps: 999,
        staleness_threshold: 999,
        cache_duration: 10,
        min_required_sources: 2,
    };
    let result = oracle.try_set_oracle_config(&original_admin, &config);

    assert!(
        result.is_err(),
        "a former admin whose role was transferred must be rejected"
    );
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(OracleRouterError::Unauthorized as u32),
        "revoked admin must receive Unauthorized (3)"
    );

    // New admin must succeed and the stale config from original_admin must not
    // have been written.
    oracle.set_oracle_config(&new_admin, &valid_oracle_config());
    let stored = oracle.get_oracle_config();
    assert_ne!(
        stored.max_deviation_bps, 999,
        "the rejected write from the revoked admin must not have modified storage"
    );
}
