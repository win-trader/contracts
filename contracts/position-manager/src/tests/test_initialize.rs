// ---------------------------------------------------------------------------
// Tests for: the Soroban constructor (`__constructor`), the `set_vault`
// entrypoint that wires the trusted Vault after deploy, and the
// initialization / pause guards in `guards.rs`.
//
// The constructor binds the ConfigManager and OracleRouter atomically with
// deploy, so there is no "uninitialized contract" reachable through the
// generated client — a registered PositionManager is always initialized.
// `set_vault` is the one remaining post-deploy wiring step and is ADMIN-gated
// + one-shot.
// ---------------------------------------------------------------------------

use soroban_sdk::{
    contract, symbol_short, testutils::Address as _, Address, Env,
};

use config_manager::ConfigManagerContract;

use crate::contract::PositionManagerContract;
use crate::guards;
use crate::storage;
use crate::PositionManagerClient;

// ===========================================================================
// Helpers
// ===========================================================================

/// Constructor-free host contract used to exercise the `guards` against raw
/// storage. The real `PositionManagerContract` seeds `Initialized` in its
/// constructor, which would defeat the "not initialized" assertions; a bare
/// contract gives a clean storage context keyed by the same `StorageKey` enum.
#[contract]
struct GuardHost;

/// Register the bare host and run a closure inside its storage context.
fn with_contract<F: FnOnce(&Env, &Address)>(f: F) {
    let env = Env::default();
    let contract_id = env.register(GuardHost, ());
    env.as_contract(&contract_id, || f(&env, &contract_id));
}

/// Deploy a real ConfigManager (admin holds ADMIN) plus a PositionManager
/// bound to it. Returns (env, pm_client, config_id, oracle, admin). The Vault
/// is intentionally NOT wired so `set_vault` tests start from a clean slate.
fn deploy_pm() -> (
    Env,
    PositionManagerClient<'static>,
    Address,
    Address,
    Address,
) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    // Real ConfigManager: its constructor grants ADMIN to `admin`, so the
    // cross-contract role check in `set_vault` resolves.
    let config_id = env.register(ConfigManagerContract, (admin.clone(),));

    // The OracleRouter link is stored but never invoked by these tests, so any
    // address satisfies the constructor.
    let oracle = Address::generate(&env);

    let pm_id = env.register(PositionManagerContract, (config_id.clone(), oracle.clone()));
    let pm_client = PositionManagerClient::new(&env, &pm_id);

    // SAFETY: env lives in the fixture, client borrows from it.
    let pm_client = unsafe { core::mem::transmute(pm_client) };

    (env, pm_client, config_id, oracle, admin)
}

// ===========================================================================
// Unit tests for guards::require_initialized
// ===========================================================================

#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn test_require_initialized_panics_when_not_initialized() {
    // On a context that has never had the Initialized flag set, the guard must
    // panic with PositionManagerError::NotInitialized (error code 2).
    with_contract(|env, _| {
        guards::require_initialized(env);
    });
}

#[test]
fn test_require_initialized_passes_after_init() {
    // After setting the Initialized flag in storage, the guard should pass.
    with_contract(|env, _| {
        storage::set_initialized(env);
        guards::require_initialized(env);
    });
}

// ===========================================================================
// Unit tests for guards::require_not_paused
// ===========================================================================

#[test]
fn test_require_not_paused_passes_when_unpaused() {
    // Default state is unpaused. Guard should pass.
    with_contract(|env, _| {
        guards::require_not_paused(env);
    });
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn test_require_not_paused_panics_when_paused() {
    // When IsPaused is set to true, require_not_paused must panic with
    // Paused (error code 3).
    with_contract(|env, _| {
        storage::set_paused(env, true);
        guards::require_not_paused(env);
    });
}

#[test]
fn test_require_not_paused_passes_after_unpause() {
    // Contract was paused then unpaused. Guard should pass again.
    with_contract(|env, _| {
        storage::set_paused(env, true);
        storage::set_paused(env, false);
        guards::require_not_paused(env);
    });
}

// ===========================================================================
// Constructor: stores config_manager + oracle_router, leaves contract unpaused
// ===========================================================================

#[test]
fn test_constructor_stores_addresses() {
    // The constructor binds the ConfigManager and OracleRouter atomically with
    // deploy and marks the contract initialized. The Vault link is NOT set yet.
    let (env, pm_client, config_id, oracle, _admin) = deploy_pm();

    env.as_contract(&pm_client.address, || {
        assert!(
            storage::is_initialized(&env),
            "Initialized flag must be true after deploy"
        );
        assert_eq!(
            storage::get_config_manager(&env),
            config_id,
            "ConfigManager address must match constructor arg"
        );
        assert_eq!(
            storage::get_oracle_router(&env),
            oracle,
            "OracleRouter address must match constructor arg"
        );
        assert!(
            !storage::has_vault_address(&env),
            "Vault must be unset until set_vault is called"
        );
    });
}

#[test]
fn test_constructor_sets_paused_to_false() {
    // A freshly deployed contract must not be paused.
    let (env, pm_client, _config_id, _oracle, _admin) = deploy_pm();

    env.as_contract(&pm_client.address, || {
        assert_eq!(
            storage::get_paused(&env),
            false,
            "Contract must not be paused after deploy"
        );
    });
}

// ===========================================================================
// set_vault: happy path
// ===========================================================================

#[test]
fn test_set_vault_stores_vault_and_makes_pm_usable() {
    // After wiring the Vault via set_vault, the address resolves and the
    // (ADMIN-gated) contract surface is usable.
    let (env, pm_client, _config_id, _oracle, admin) = deploy_pm();
    let vault = Address::generate(&env);

    pm_client.set_vault(&admin, &vault);

    env.as_contract(&pm_client.address, || {
        assert!(
            storage::has_vault_address(&env),
            "Vault must be set after set_vault"
        );
        assert_eq!(
            storage::get_vault_address(&env),
            vault,
            "Stored vault must match the wired address"
        );
    });

    // The contract is usable: an ADMIN-gated config call round-trips.
    pm_client.set_max_leverage(&admin, &symbol_short!("BTC"), &100_i128);
    assert_eq!(
        pm_client.get_max_leverage(&symbol_short!("BTC")),
        100_i128,
        "set_max_leverage must persist after set_vault"
    );
}

// ===========================================================================
// set_vault: one-shot — second call reverts AlreadyInitialized
// ===========================================================================

#[test]
fn test_set_vault_second_call_reverts_already_initialized() {
    // The trusted Vault link is immutable after deploy. A second set_vault must
    // revert with AlreadyInitialized (error code 1).
    let (env, pm_client, _config_id, _oracle, admin) = deploy_pm();
    let vault = Address::generate(&env);
    let evil_vault = Address::generate(&env);

    pm_client.set_vault(&admin, &vault);

    let result = pm_client.try_set_vault(&admin, &evil_vault);
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(
            crate::PositionManagerError::AlreadyInitialized as u32
        ),
        "second set_vault must revert AlreadyInitialized"
    );

    // The original Vault link must survive the failed re-wire attempt.
    env.as_contract(&pm_client.address, || {
        assert_eq!(storage::get_vault_address(&env), vault);
    });
}

// ===========================================================================
// set_vault: non-admin caller reverts Unauthorized
// ===========================================================================

#[test]
fn test_set_vault_non_admin_reverts_unauthorized() {
    // A caller lacking the ADMIN role cannot wire the Vault. Must revert with
    // Unauthorized (error code 7). `mock_all_auths` satisfies require_auth, so
    // the failure is the ConfigManager role check, not a missing signature.
    let (env, pm_client, _config_id, _oracle, _admin) = deploy_pm();
    let stranger = Address::generate(&env);
    let vault = Address::generate(&env);

    let result = pm_client.try_set_vault(&stranger, &vault);
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(
            crate::PositionManagerError::Unauthorized as u32
        ),
        "set_vault by a non-admin must revert Unauthorized"
    );

    // No partial state: the Vault must remain unset.
    env.as_contract(&pm_client.address, || {
        assert!(!storage::has_vault_address(&env));
    });
}
