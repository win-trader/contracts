//! Tests for PositionManager's `_panic_with_upgrade_error` mapping — the
//! `TimelockedUpgradeable` trait's failure modes must surface as
//! `PositionManagerError::{NoPendingUpgrade, UpgradeTimelockNotElapsed,
//! UpgradeHashMismatch}` (codes 23/24/25), not the trait-shared
//! `UpgradeFailure` enum.
//!
//! Mirrors `config-manager/src/tests/test_upgrade_timelock_enforcement.rs`
//! so a future refactor that swaps variants in PositionManager's `impl
//! TimelockedUpgradeable` is caught here.

#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    Address, BytesN, Env, Symbol,
};

use crate::{PositionManagerContract, PositionManagerError, PositionManagerClient};

const HASH_A: [u8; 32] = [0xAAu8; 32];
const HASH_B: [u8; 32] = [0xBBu8; 32];

struct UpgradeFixture {
    env: Env,
    upgrader: Address,
    pm_client: PositionManagerClient<'static>,
}

fn setup_upgrade_fixture() -> UpgradeFixture {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);

    // Real ConfigManager so cross-call role checks resolve.
    let config_id = env.register(config_manager::ConfigManagerContract, (admin.clone(),));
    let config_client = config_manager::ConfigManagerClient::new(&env, &config_id);

    // Upgrade paths only cross-call ConfigManager for role checks; the
    // OracleRouter link is stored but never invoked here, so any address
    // satisfies the constructor. No Vault is wired — `set_vault` is unneeded.
    let pm_id = env.register(PositionManagerContract, (config_id.clone(), oracle.clone()));
    let pm_client = PositionManagerClient::new(&env, &pm_id);

    let upgrader = Address::generate(&env);
    let upgrader_role = Symbol::new(&env, "UPGRADER");
    let pauser_role = Symbol::new(&env, "PAUSER");
    config_client.grant_role(&admin, &upgrader_role, &upgrader);
    config_client.grant_role(&admin, &pauser_role, &upgrader);

    // SAFETY: env lives in the fixture, client borrows from it.
    let pm_client = unsafe { core::mem::transmute(pm_client) };

    UpgradeFixture { env, upgrader, pm_client }
}

/// `upgrade` without a prior `propose_upgrade` returns `NoPendingUpgrade=23`.
#[test]
fn test_upgrade_without_proposal_errors_no_pending() {
    let f = setup_upgrade_fixture();
    let hash = BytesN::from_array(&f.env, &HASH_A);
    let upgrade_client =
        stellar_contract_utils::upgradeable::UpgradeableClient::new(&f.env, &f.pm_client.address);
    let result = upgrade_client.try_upgrade(&hash, &f.upgrader);
    assert!(result.is_err(), "upgrade without a proposal must fail");
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(PositionManagerError::NoPendingUpgrade as u32),
    );
}

/// `upgrade` before `eta` elapses returns `UpgradeTimelockNotElapsed=24`.
#[test]
fn test_upgrade_before_eta_errors_timelock_not_elapsed() {
    let f = setup_upgrade_fixture();
    f.env.ledger().set_timestamp(1_000_000);
    let hash = BytesN::from_array(&f.env, &HASH_A);
    f.pm_client.propose_upgrade(&f.upgrader, &hash);

    let upgrade_client =
        stellar_contract_utils::upgradeable::UpgradeableClient::new(&f.env, &f.pm_client.address);
    let result = upgrade_client.try_upgrade(&hash, &f.upgrader);
    assert!(result.is_err(), "upgrade before eta must fail");
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(
            PositionManagerError::UpgradeTimelockNotElapsed as u32
        ),
    );
}

/// `upgrade` with mismatched wasm hash returns `UpgradeHashMismatch=25`.
#[test]
fn test_upgrade_with_mismatched_hash_errors_hash_mismatch() {
    let f = setup_upgrade_fixture();
    f.env.ledger().set_timestamp(1_000_000);
    let proposed = BytesN::from_array(&f.env, &HASH_A);
    let attacker_hash = BytesN::from_array(&f.env, &HASH_B);
    f.pm_client.propose_upgrade(&f.upgrader, &proposed);

    f.env.ledger().set_timestamp(1_000_000 + 10_000_000);
    let upgrade_client =
        stellar_contract_utils::upgradeable::UpgradeableClient::new(&f.env, &f.pm_client.address);
    let result = upgrade_client.try_upgrade(&attacker_hash, &f.upgrader);

    assert!(result.is_err(), "upgrade with mismatched hash must fail");
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(PositionManagerError::UpgradeHashMismatch as u32),
    );
}

/// After `cancel_upgrade`, a subsequent `upgrade` with the previously
/// proposed hash fails with `NoPendingUpgrade=23`.
#[test]
fn test_upgrade_after_cancel_errors_no_pending() {
    let f = setup_upgrade_fixture();
    let hash = BytesN::from_array(&f.env, &HASH_A);
    f.pm_client.propose_upgrade(&f.upgrader, &hash);
    f.pm_client.cancel_upgrade(&f.upgrader);

    let upgrade_client =
        stellar_contract_utils::upgradeable::UpgradeableClient::new(&f.env, &f.pm_client.address);
    let result = upgrade_client.try_upgrade(&hash, &f.upgrader);
    assert!(result.is_err(), "upgrade after cancel must fail");
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(PositionManagerError::NoPendingUpgrade as u32),
    );
}
