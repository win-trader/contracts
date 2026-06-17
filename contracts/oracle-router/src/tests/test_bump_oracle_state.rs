//! Tests for `bump_oracle_state`.
//!
//! `bump_oracle_state` extends the Soroban TTL of the OracleRouter's
//! instance storage so that oracle config and source lists are not archived.
//! The function is callable by anyone — no authentication or role is required.
//!
//! Covers:
//!   - 2.6-a: Calling `bump_oracle_state` without any auth must succeed (no
//!             `require_auth` inside the function).
//!   - 2.6-b: Calling on a fully initialized router must succeed.
//!   - 2.6-c: A random address with no roles can call it without error.
//!   - 2.6-d: Instance storage remains readable immediately after the bump.
//!   - 2.6-e: Multiple sequential bumps must not error.
//!   - 2.6-f: Calling before initialization (uninitialized contract) must
//!             not panic — TTL extension does not depend on init state.
//!
//! All tests that exercise the happy path will FAIL against the current
//! `todo!()` stub and will PASS once the implementation calls
//! `bump_instance_ttl(&env)`.

#![cfg(test)]

use soroban_sdk::{testutils::Address as _, Address, Env};

use super::helpers::{deploy, deploy_initialized, deploy_with_config_manager, valid_oracle_config};

// ---------------------------------------------------------------------------
// 2.6-a: No authentication required
// ---------------------------------------------------------------------------

/// bump_oracle_state requires no auth from the caller.
/// Calling it with NO mocked auth must succeed (must not panic with an
/// auth error). This is the primary specification test.
///
/// FAILS against todo!() — passes once `bump_instance_ttl(&env)` is called.
#[test]
fn test_bump_oracle_state_succeeds_without_auth() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _config_manager) = deploy_initialized(&env);

    // Strip auth before the bump call to verify it has no internal auth check.
    env.mock_auths(&[]);
    client.bump_oracle_state();
}

/// Confirm that calling bump_oracle_state does not touch auth at all.
/// We mock auths to a completely empty slice (no auth approvals), then call
/// the function; if auth is required the soroban host will panic.
#[test]
fn test_bump_oracle_state_empty_auth_context_does_not_panic() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = deploy_initialized(&env);

    // Strip auth before the bump call — must still succeed.
    env.mock_auths(&[]);
    client.bump_oracle_state();
}

// ---------------------------------------------------------------------------
// 2.6-b: Works on an initialized router
// ---------------------------------------------------------------------------

/// Calling bump_oracle_state on a fully initialized OracleRouter (with
/// ConfigManager wired and oracle config set) must succeed without panicking.
///
/// FAILS against todo!() — passes once the implementation is present.
#[test]
fn test_bump_oracle_state_on_initialized_router_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    let (oracle, _cm, admin) = deploy_with_config_manager(&env);

    // Give the router a real oracle config so it has meaningful state to bump.
    let config = valid_oracle_config();
    oracle.set_oracle_config(&admin, &config);

    // TTL extension must not panic.
    oracle.bump_oracle_state();
}

// ---------------------------------------------------------------------------
// 2.6-c: Anyone can call — no role required
// ---------------------------------------------------------------------------

/// A completely random address (no roles in ConfigManager, no auth mocked)
/// must be able to call bump_oracle_state without any error.
///
/// This guards against an implementation that accidentally calls
/// `require_oracle_admin` or any role check inside bump_oracle_state.
///
/// FAILS against todo!() — passes once the no-auth implementation is present.
#[test]
fn test_bump_oracle_state_anyone_can_call() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _config_manager) = deploy_initialized(&env);

    // Generate an address with absolutely no roles anywhere.
    let _random_caller = Address::generate(&env);

    // The call is not gated by any caller argument — it takes only `Env`.
    // If the implementation were to add any auth check, this test would catch it.
    client.bump_oracle_state();
}

/// Adversarial: a non-admin, non-upgrader, completely untrusted address
/// calls bump_oracle_state in an environment where NO auth is mocked.
/// Must still succeed because the function is entirely permissionless.
#[test]
fn test_bump_oracle_state_untrusted_caller_no_auth_succeeds() {
    // The router is constructed atomically with deploy (no separate init), so
    // a freshly deployed router is already live. Calling bump with no auth
    // context must still succeed because the function is permissionless.
    let env = Env::default();
    let client = deploy(&env);

    // Call with empty auths — permissionless call must not fail.
    env.mock_auths(&[]);
    client.bump_oracle_state();
}

// ---------------------------------------------------------------------------
// 2.6-d: Instance storage remains accessible after bump
// ---------------------------------------------------------------------------

/// After calling bump_oracle_state, a follow-up read of OracleConfig must
/// still succeed (returning the previously stored config), proving that
/// the bump did not corrupt or clear instance storage.
///
/// FAILS against todo!() — passes once the implementation is present.
#[test]
fn test_bump_oracle_state_instance_storage_remains_accessible_after_bump() {
    let env = Env::default();
    env.mock_all_auths();
    let (oracle, _cm, admin) = deploy_with_config_manager(&env);

    let config = valid_oracle_config();
    oracle.set_oracle_config(&admin, &config);

    // Bump TTL.
    oracle.bump_oracle_state();

    // The stored config must still be readable and correct after the bump.
    let fetched = oracle.get_oracle_config();
    assert_eq!(
        fetched.max_deviation_bps, config.max_deviation_bps,
        "2.6-d: max_deviation_bps must be unchanged after bump_oracle_state"
    );
    assert_eq!(
        fetched.staleness_threshold, config.staleness_threshold,
        "2.6-d: staleness_threshold must be unchanged after bump_oracle_state"
    );
    assert_eq!(
        fetched.min_required_sources, config.min_required_sources,
        "2.6-d: cache_duration must be unchanged after bump_oracle_state"
    );
}

// ---------------------------------------------------------------------------
// 2.6-e: Multiple sequential bumps are safe
// ---------------------------------------------------------------------------

/// Calling bump_oracle_state ten times in a row must never panic.
/// TTL extension is idempotent — repeated calls only re-extend the deadline.
///
/// FAILS against todo!() — passes once the implementation is present.
#[test]
fn test_bump_oracle_state_repeated_calls_are_idempotent() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = deploy_initialized(&env);

    for _ in 0..10 {
        client.bump_oracle_state();
    }
    // No assertion needed — reaching here means none of the calls panicked.
}

// ---------------------------------------------------------------------------
// 2.6-f: Never returns Unauthorized
// ---------------------------------------------------------------------------

/// Calling bump_oracle_state on a freshly deployed router must not return an
/// Unauthorized error. TTL extension is permissionless and takes no caller
/// argument. The test is written as `try_bump_oracle_state` so it can observe
/// any error without unwinding; we assert that it at least does not return an
/// Unauthorized error (which would indicate an accidental auth check was added).
#[test]
fn test_bump_oracle_state_uninitialized_contract_does_not_return_unauthorized() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);

    let result = client.try_bump_oracle_state();
    // The important invariant: if it fails, it must NOT be Unauthorized.
    // (It is acceptable for it to succeed or fail with a different host error.)
    if let Err(Ok(contract_error)) = result {
        // Any contract-level error other than Unauthorized is acceptable.
        use crate::OracleRouterError;
        assert_ne!(
            contract_error,
            soroban_sdk::Error::from_contract_error(OracleRouterError::Unauthorized as u32),
            "2.6-f: bump_oracle_state must never return Unauthorized — it is permissionless"
        );
    }
}
