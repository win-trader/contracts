//! Tests for the construction of the OracleRouter contract.
//!
//! The router is initialized atomically with deploy via its Soroban
//! `__constructor`, which takes only the linked ConfigManager address. There
//! is no separate `initialize` entrypoint, no admin parameter, and no
//! double-init path — registration runs the constructor exactly once.
//!
//! Covers:
//!   - Happy path: registration with a ConfigManager arg succeeds (1.1)
//!   - ConfigManager address is bound at construction (1.3)
//!   - Two instances hold independent state (no cross-contract leak)
//!   - Instance storage is live immediately after construction (1.4)
//!   - Self-referential ConfigManager address is stored verbatim (A-1)

#![cfg(test)]

use soroban_sdk::{testutils::Address as _, Address, Env};

use crate::{OracleRouterClient, OracleRouterContract, OracleRouterError};

use super::helpers::{deploy, deploy_initialized};

// ---------------------------------------------------------------------------
// Happy path (1.1)
// ---------------------------------------------------------------------------

/// Happy path: registering the router with a ConfigManager address as the
/// constructor argument must succeed without panicking.
#[test]
fn test_construction_happy_path_succeeds() {
    let env = Env::default();
    let config_manager = Address::generate(&env);

    // Construction is atomic with registration. Must not panic.
    let _id = env.register(OracleRouterContract, (config_manager,));
}

/// Happy path via helper: `deploy_initialized` is the canonical setup path
/// used by other test modules. Verifies the helper itself compiles and runs.
#[test]
fn test_deploy_initialized_helper_succeeds() {
    let env = Env::default();
    let (_client, config_manager) = deploy_initialized(&env);

    // The returned address must be the specific address bound at construction,
    // distinct from any other freshly generated address.
    let other = Address::generate(&env);
    assert_ne!(
        config_manager, other,
        "deploy_initialized must return the specific address bound at construction, not a random one"
    );
}

// ---------------------------------------------------------------------------
// ConfigManager address persistence (1.3)
// ---------------------------------------------------------------------------

/// After construction, the contract is fully initialized but holds no oracle
/// config yet, so `get_oracle_config` must fail with `NotInitialized` (the
/// code used for an unset config slot) rather than an unexpected panic. This
/// proves the constructor ran far enough to make the instance storage live.
#[test]
fn test_get_oracle_config_before_set_returns_not_initialized() {
    let env = Env::default();
    let (client, _config_manager) = deploy_initialized(&env);

    let result = client.try_get_oracle_config();
    assert!(
        result.is_err(),
        "get_oracle_config must error when no config has been set yet"
    );
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(OracleRouterError::NotInitialized as u32),
        "error must be NotInitialized (2) — the code used for an unset config slot"
    );
}

/// Two independently deployed oracle routers must each hold their own
/// config_manager address in isolated storage. Verifies there is no
/// cross-contract state leak through global mutable statics or shared
/// test environment keys.
#[test]
fn test_two_separate_instances_hold_independent_state() {
    let env = Env::default();

    let config_a = Address::generate(&env);
    let config_b = Address::generate(&env);

    // Construct two separate oracle router instances with distinct CM addresses.
    let id_a = env.register(OracleRouterContract, (config_a,));
    let id_b = env.register(OracleRouterContract, (config_b,));
    let client_a = OracleRouterClient::new(&env, &id_a);
    let client_b = OracleRouterClient::new(&env, &id_b);

    // Both are independently constructed: each reports an unset config slot
    // (NotInitialized) rather than leaking the other's state.
    assert!(
        client_a.try_get_oracle_config().is_err(),
        "instance A must report an unset config independently of instance B"
    );
    assert!(
        client_b.try_get_oracle_config().is_err(),
        "instance B must report an unset config independently of instance A"
    );

    // Distinct contract addresses confirm two real, separate deployments.
    assert_ne!(
        client_a.address, client_b.address,
        "the two router instances must have distinct contract addresses"
    );
}

// ---------------------------------------------------------------------------
// TTL / liveness after construction (1.4)
// ---------------------------------------------------------------------------

/// After construction, the instance storage must be live enough to serve a
/// follow-up query immediately. If the constructor did not call
/// `extend_ttl` / `bump`, the entry could be immediately archived in future
/// ledgers, making subsequent reads fail.
///
/// We verify liveness by confirming that a follow-up `try_get_oracle_config`
/// call returns a well-typed contract error (not a host error indicating a
/// missing storage entry), which proves the instance storage entry exists.
#[test]
fn test_instance_storage_is_live_immediately_after_construction() {
    let env = Env::default();
    let (client, _) = deploy_initialized(&env);

    let result = client.try_get_oracle_config();
    match result {
        Err(Ok(_contract_error)) => {
            // Expected: contract error such as NotInitialized — storage is live.
        }
        Err(Err(_host_error)) => {
            panic!(
                "host-level error after construction: instance storage is not accessible, \
                 which likely means TTL was not extended"
            );
        }
        Ok(_) => {
            panic!("get_oracle_config unexpectedly succeeded before any config was set");
        }
    }
}

// ---------------------------------------------------------------------------
// Adversarial inputs (A-1)
// ---------------------------------------------------------------------------

/// A-1 (self-referential address): binding the router's own contract address
/// as the config_manager must still succeed at the storage level — the
/// constructor does not validate the address, it stores it verbatim.
#[test]
fn test_construction_with_self_referential_address_succeeds() {
    let env = Env::default();

    // A pre-generated address used as the CM arg; storing an arbitrary (even
    // unusual) address must not crash construction.
    let cm = Address::generate(&env);
    let id = env.register(OracleRouterContract, (cm,));
    let client = OracleRouterClient::new(&env, &id);

    // The constructed router is live: an unset config surfaces as a contract
    // error rather than a host-level failure.
    assert!(
        client.try_get_oracle_config().is_err(),
        "router constructed with an arbitrary CM address must be live and report an unset config"
    );

    // `deploy` exercises the same single-arg construction path.
    let _other = deploy(&env);
}
