//! Tests for Vault's `_panic_with_upgrade_error` mapping — the
//! `TimelockedUpgradeable` trait's failure modes must surface as
//! `VaultError::{NoPendingUpgrade, UpgradeTimelockNotElapsed,
//! UpgradeHashMismatch}` (codes 14/15/16), not the trait-shared
//! `UpgradeFailure` enum.
//!
//! Mirrors `config-manager/src/tests/test_upgrade_timelock_enforcement.rs`
//! so a future refactor that swaps variants in Vault's `impl
//! TimelockedUpgradeable` is caught here.

#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    Address, BytesN, Env, String, Symbol,
};

use crate::VaultError;

const HASH_A: [u8; 32] = [0xAAu8; 32];
const HASH_B: [u8; 32] = [0xBBu8; 32];

struct UpgradeFixture {
    env: Env,
    upgrader: Address,
    vault_client: crate::VaultContractClient<'static>,
    config_client: config_manager::ConfigManagerClient<'static>,
}

fn setup_upgrade_fixture() -> UpgradeFixture {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let position_manager = Address::generate(&env);

    let token_id = env.register(mock_token::MockToken, ());
    mock_token::MockTokenClient::new(&env, &token_id).initialize(
        &admin,
        &7u32,
        &String::from_str(&env, "USD Coin"),
        &String::from_str(&env, "USDC"),
    );

    let config_id = env.register(config_manager::ConfigManagerContract, (admin.clone(),));
    let config_client = config_manager::ConfigManagerClient::new(&env, &config_id);

    let vault_id = env.register(
        crate::VaultContract,
        (token_id.clone(), config_id.clone(), position_manager.clone()),
    );
    let vault_client = crate::VaultContractClient::new(&env, &vault_id);

    // Grant UPGRADER + PAUSER to a dedicated address so both flows work.
    let upgrader = Address::generate(&env);
    let upgrader_role = Symbol::new(&env, "UPGRADER");
    let pauser_role = Symbol::new(&env, "PAUSER");
    config_client.grant_role(&admin, &upgrader_role, &upgrader);
    config_client.grant_role(&admin, &pauser_role, &upgrader);

    // SAFETY: env lives in the fixture, clients borrow from it.
    let vault_client = unsafe { core::mem::transmute(vault_client) };
    let config_client = unsafe { core::mem::transmute(config_client) };

    UpgradeFixture { env, upgrader, vault_client, config_client }
}

/// `upgrade` without a prior `propose_upgrade` returns `NoPendingUpgrade=14`.
#[test]
fn test_upgrade_without_proposal_errors_no_pending() {
    let f = setup_upgrade_fixture();
    let hash = BytesN::from_array(&f.env, &HASH_A);
    let result = f.vault_client.try_upgrade(&hash, &f.upgrader);
    assert!(result.is_err(), "upgrade without a proposal must fail");
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(VaultError::NoPendingUpgrade as u32),
    );
}

/// `upgrade` before `eta` elapses returns `UpgradeTimelockNotElapsed=15`.
#[test]
fn test_upgrade_before_eta_errors_timelock_not_elapsed() {
    let f = setup_upgrade_fixture();
    f.env.ledger().set_timestamp(1_000_000);
    let hash = BytesN::from_array(&f.env, &HASH_A);
    f.vault_client.propose_upgrade(&f.upgrader, &hash);

    let result = f.vault_client.try_upgrade(&hash, &f.upgrader);
    assert!(result.is_err(), "upgrade before eta must fail");
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(VaultError::UpgradeTimelockNotElapsed as u32),
    );
}

/// `upgrade` with mismatched wasm hash returns `UpgradeHashMismatch=16`.
#[test]
fn test_upgrade_with_mismatched_hash_errors_hash_mismatch() {
    let f = setup_upgrade_fixture();
    f.env.ledger().set_timestamp(1_000_000);
    let proposed = BytesN::from_array(&f.env, &HASH_A);
    let attacker_hash = BytesN::from_array(&f.env, &HASH_B);
    f.vault_client.propose_upgrade(&f.upgrader, &proposed);

    f.env.ledger().set_timestamp(1_000_000 + 10_000_000);
    let result = f.vault_client.try_upgrade(&attacker_hash, &f.upgrader);

    assert!(result.is_err(), "upgrade with mismatched hash must fail");
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(VaultError::UpgradeHashMismatch as u32),
    );
}

/// After `cancel_upgrade`, a subsequent `upgrade` with the previously
/// proposed hash fails with `NoPendingUpgrade=14`.
#[test]
fn test_upgrade_after_cancel_errors_no_pending() {
    let f = setup_upgrade_fixture();
    let hash = BytesN::from_array(&f.env, &HASH_A);
    f.vault_client.propose_upgrade(&f.upgrader, &hash);
    // Avoid unused warning — the fixture's config_client is set up but the
    // pauser-role grant already happened in setup, so no further config
    // mutation is needed.
    let _ = &f.config_client;
    f.vault_client.cancel_upgrade(&f.upgrader);

    let result = f.vault_client.try_upgrade(&hash, &f.upgrader);
    assert!(result.is_err(), "upgrade after cancel must fail");
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(VaultError::NoPendingUpgrade as u32),
    );
}
