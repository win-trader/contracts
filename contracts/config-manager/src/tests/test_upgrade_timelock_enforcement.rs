//! Tests for `upgrade` timelock + wasm-hash match enforcement.

use soroban_sdk::{testutils::{Address as _, Ledger as _}, Address, BytesN, Env};
use stellar_contract_utils::upgradeable::UpgradeableClient;

use crate::ConfigManagerError;

use super::helpers::{deploy_initialized, role_upgrader};

const HASH_A: [u8; 32] = [0xAAu8; 32];
const HASH_B: [u8; 32] = [0xBBu8; 32];

/// `upgrade` without a prior `propose_upgrade` returns `NoPendingUpgrade`.
#[test]
fn test_upgrade_without_proposal_errors_no_pending() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);
    let upgrader = Address::generate(&env);
    client.grant_role(&admin, &role_upgrader(&env), &upgrader);

    let hash = BytesN::from_array(&env, &HASH_A);
    let upgrade_client = UpgradeableClient::new(&env, &client.address);
    let result = upgrade_client.try_upgrade(&hash, &upgrader);

    assert!(result.is_err(), "upgrade without a proposal must fail");
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(ConfigManagerError::NoPendingUpgrade as u32),
    );
}

/// `upgrade` before `eta` elapses returns `UpgradeTimelockNotElapsed`.
#[test]
fn test_upgrade_before_eta_errors_timelock_not_elapsed() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000_000);
    let (client, admin) = deploy_initialized(&env);
    let upgrader = Address::generate(&env);
    client.grant_role(&admin, &role_upgrader(&env), &upgrader);

    let hash = BytesN::from_array(&env, &HASH_A);
    client.propose_upgrade(&upgrader, &hash);

    // Still at proposal time — eta is in the future.
    let upgrade_client = UpgradeableClient::new(&env, &client.address);
    let result = upgrade_client.try_upgrade(&hash, &upgrader);

    assert!(result.is_err(), "upgrade before eta must fail");
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(
            ConfigManagerError::UpgradeTimelockNotElapsed as u32
        ),
    );
}

/// `upgrade` with a wasm hash different from the proposed one returns
/// `UpgradeHashMismatch`.
#[test]
fn test_upgrade_with_mismatched_hash_errors_hash_mismatch() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000_000);
    let (client, admin) = deploy_initialized(&env);
    let upgrader = Address::generate(&env);
    client.grant_role(&admin, &role_upgrader(&env), &upgrader);

    let proposed = BytesN::from_array(&env, &HASH_A);
    let attacker_hash = BytesN::from_array(&env, &HASH_B);
    client.propose_upgrade(&upgrader, &proposed);

    // Advance well past eta so timelock check passes.
    env.ledger().set_timestamp(1_000_000 + 10_000_000);

    let upgrade_client = UpgradeableClient::new(&env, &client.address);
    let result = upgrade_client.try_upgrade(&attacker_hash, &upgrader);

    assert!(result.is_err(), "upgrade with mismatched hash must fail");
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(ConfigManagerError::UpgradeHashMismatch as u32),
    );
}

/// After `cancel_upgrade`, attempting to upgrade with the previously-proposed
/// hash returns `NoPendingUpgrade` — the cancellation must truly clear state.
#[test]
fn test_upgrade_after_cancel_errors_no_pending() {
    use super::helpers::role_pauser;

    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);
    let upgrader = Address::generate(&env);
    let pauser = Address::generate(&env);
    client.grant_role(&admin, &role_upgrader(&env), &upgrader);
    client.grant_role(&admin, &role_pauser(&env), &pauser);

    let hash = BytesN::from_array(&env, &HASH_A);
    client.propose_upgrade(&upgrader, &hash);
    client.cancel_upgrade(&pauser);

    let upgrade_client = UpgradeableClient::new(&env, &client.address);
    let result = upgrade_client.try_upgrade(&hash, &upgrader);

    assert!(result.is_err(), "upgrade after cancel must fail");
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(ConfigManagerError::NoPendingUpgrade as u32),
    );
}

/// `upgrade` with a proposal that has matching hash AND elapsed eta gets
/// past all the protocol-level checks. The all-zero hash is not a real
/// uploaded WASM, so the host will reject at install time — but the rejection
/// must NOT be one of our protocol errors. This proves the protocol checks
/// passed and the call reached `update_current_contract_wasm`.
#[test]
fn test_upgrade_with_valid_proposal_passes_protocol_checks() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000_000);
    let (client, admin) = deploy_initialized(&env);
    let upgrader = Address::generate(&env);
    client.grant_role(&admin, &role_upgrader(&env), &upgrader);

    let hash = BytesN::from_array(&env, &HASH_A);
    client.propose_upgrade(&upgrader, &hash);

    env.ledger().set_timestamp(1_000_000 + 10_000_000);

    let upgrade_client = UpgradeableClient::new(&env, &client.address);
    let result = upgrade_client.try_upgrade(&hash, &upgrader);
    assert!(result.is_err(), "host must reject install of a non-uploaded hash");
    let err = result.unwrap_err().unwrap();
    let no_pending = soroban_sdk::Error::from_contract_error(
        ConfigManagerError::NoPendingUpgrade as u32,
    );
    let not_elapsed = soroban_sdk::Error::from_contract_error(
        ConfigManagerError::UpgradeTimelockNotElapsed as u32,
    );
    let mismatch = soroban_sdk::Error::from_contract_error(
        ConfigManagerError::UpgradeHashMismatch as u32,
    );
    assert_ne!(err, no_pending);
    assert_ne!(err, not_elapsed);
    assert_ne!(err, mismatch);
}
