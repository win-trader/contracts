//! Tests for OracleRouter's `_panic_with_upgrade_error` mapping — the
//! `TimelockedUpgradeable` trait's failure modes must surface as
//! `OracleRouterError::{NoPendingUpgrade, UpgradeTimelockNotElapsed,
//! UpgradeHashMismatch}` (codes 12/13/14), not the trait-shared
//! `UpgradeFailure` enum.
//!
//! Mirrors `config-manager/src/tests/test_upgrade_timelock_enforcement.rs`
//! so a future refactor that swaps variants in OracleRouter's `impl
//! TimelockedUpgradeable` is caught here.

#![cfg(test)]

use soroban_sdk::{testutils::Ledger as _, BytesN, Env, Symbol};
use stellar_contract_utils::upgradeable::UpgradeableClient;

use crate::OracleRouterError;

use super::helpers::{deploy_with_upgrader, role_upgrader};

const HASH_A: [u8; 32] = [0xAAu8; 32];
const HASH_B: [u8; 32] = [0xBBu8; 32];

/// `upgrade` without a prior `propose_upgrade` returns `NoPendingUpgrade=12`.
#[test]
fn test_upgrade_without_proposal_errors_no_pending() {
    let env = Env::default();
    env.mock_all_auths();
    let (oracle, _cm, admin) = deploy_with_upgrader(&env);

    let hash = BytesN::from_array(&env, &HASH_A);
    let upgrade_client = UpgradeableClient::new(&env, &oracle.address);
    let result = upgrade_client.try_upgrade(&hash, &admin);

    assert!(result.is_err(), "upgrade without a proposal must fail");
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(OracleRouterError::NoPendingUpgrade as u32),
    );
}

/// `upgrade` before `eta` elapses returns `UpgradeTimelockNotElapsed=13`.
#[test]
fn test_upgrade_before_eta_errors_timelock_not_elapsed() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000_000);
    let (oracle, _cm, admin) = deploy_with_upgrader(&env);

    let hash = BytesN::from_array(&env, &HASH_A);
    oracle.propose_upgrade(&admin, &hash);

    let upgrade_client = UpgradeableClient::new(&env, &oracle.address);
    let result = upgrade_client.try_upgrade(&hash, &admin);

    assert!(result.is_err(), "upgrade before eta must fail");
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(
            OracleRouterError::UpgradeTimelockNotElapsed as u32
        ),
    );
}

/// `upgrade` with mismatched wasm hash returns `UpgradeHashMismatch=14`.
#[test]
fn test_upgrade_with_mismatched_hash_errors_hash_mismatch() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000_000);
    let (oracle, _cm, admin) = deploy_with_upgrader(&env);

    let proposed = BytesN::from_array(&env, &HASH_A);
    let attacker_hash = BytesN::from_array(&env, &HASH_B);
    oracle.propose_upgrade(&admin, &proposed);

    env.ledger().set_timestamp(1_000_000 + 10_000_000);

    let upgrade_client = UpgradeableClient::new(&env, &oracle.address);
    let result = upgrade_client.try_upgrade(&attacker_hash, &admin);

    assert!(result.is_err(), "upgrade with mismatched hash must fail");
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(OracleRouterError::UpgradeHashMismatch as u32),
    );
}

/// `cancel_upgrade` clears the pending upgrade — a subsequent `upgrade` with
/// the same hash must fail with `NoPendingUpgrade=12`.
#[test]
fn test_upgrade_after_cancel_errors_no_pending() {
    let env = Env::default();
    env.mock_all_auths();
    let (oracle, cm, admin) = deploy_with_upgrader(&env);

    // Grant PAUSER to admin so the same address can cancel.
    let pauser_role = Symbol::new(&env, "PAUSER");
    cm.grant_role(&admin, &pauser_role, &admin);
    cm.grant_role(&admin, &role_upgrader(&env), &admin);

    let hash = BytesN::from_array(&env, &HASH_A);
    oracle.propose_upgrade(&admin, &hash);
    oracle.cancel_upgrade(&admin);

    let upgrade_client = UpgradeableClient::new(&env, &oracle.address);
    let result = upgrade_client.try_upgrade(&hash, &admin);

    assert!(result.is_err(), "upgrade after cancel must fail");
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(OracleRouterError::NoPendingUpgrade as u32),
    );
}
