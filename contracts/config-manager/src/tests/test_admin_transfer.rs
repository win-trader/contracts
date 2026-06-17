//! Two-step admin transfer: propose_admin → accept_admin (+ cancel_admin_proposal).
//!
//! A single-step admin transfer with no event would be unrecoverable on a
//! typo and silently undetectable off-chain. The two-step flow requires
//! the new admin to actively `accept_admin`, preventing accidental bricking,
//! and the role transition is emitted as `RoleChange` events so off-chain
//! monitoring can detect admin rotations in real time.

use soroban_sdk::{
    testutils::{Address as _, Events as _},
    Address, Env, Symbol, TryIntoVal,
};

use crate::ConfigManagerError;

use super::helpers::{deploy, deploy_initialized, role_admin, role_keeper};

// ---------------------------------------------------------------------------
// construction — genesis admin grant must emit a RoleChange event
// ---------------------------------------------------------------------------

/// construction emits RoleChange { role: "ADMIN", account: admin, is_grant: true }
/// so off-chain indexers see the genesis admin without scanning instance storage.
#[test]
fn test_initialize_emits_admin_role_change() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let client = deploy(&env, &admin);

    let cm_id = client.address.clone();
    let admin_role = role_admin(&env);
    let mut saw = false;
    for (contract, topics, data) in env.events().all() {
        if contract != cm_id || topics.len() == 0 {
            continue;
        }
        let topic0: Symbol = match topics.get(0).unwrap().try_into_val(&env) {
            Ok(s) => s,
            Err(_) => continue,
        };
        if topic0 != Symbol::new(&env, "role") {
            continue;
        }
        let parsed: (Symbol, Address, bool) = match data.try_into_val(&env) {
            Ok(t) => t,
            Err(_) => continue,
        };
        if parsed.0 == admin_role && parsed.1 == admin && parsed.2 {
            saw = true;
            break;
        }
    }
    assert!(saw, "initialize must emit RoleChange(ADMIN, admin, is_grant=true)");
}

// ---------------------------------------------------------------------------
// propose_admin
// ---------------------------------------------------------------------------

/// propose_admin stores the pending admin and emits AdminProposed.
#[test]
fn test_propose_admin_happy_path() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);
    let candidate = Address::generate(&env);

    client.propose_admin(&admin, &candidate);

    assert_eq!(client.get_pending_admin(), Some(candidate));
}

/// propose_admin emits AdminProposed(proposer, new_admin) so off-chain
/// monitoring can detect pending admin transfers before `accept_admin`.
#[test]
fn test_propose_admin_emits_admin_proposed_event() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);
    let candidate = Address::generate(&env);

    client.propose_admin(&admin, &candidate);

    let cm_id = client.address.clone();
    let topic_admin_prop = Symbol::new(&env, "adminprop");
    let mut saw = false;
    for (contract, topics, data) in env.events().all() {
        if contract != cm_id || topics.len() == 0 {
            continue;
        }
        let topic0: Symbol = match topics.get(0).unwrap().try_into_val(&env) {
            Ok(s) => s,
            Err(_) => continue,
        };
        if topic0 != topic_admin_prop {
            continue;
        }
        let parsed: (Address, Address) = match data.try_into_val(&env) {
            Ok(t) => t,
            Err(_) => continue,
        };
        if parsed.0 == admin && parsed.1 == candidate {
            saw = true;
            break;
        }
    }
    assert!(saw, "propose_admin must emit AdminProposed(proposer, new_admin)");
}

/// propose_admin with caller == new_admin reverts with InvalidAdminProposal —
/// avoids a degenerate "transfer to self" that silently no-ops.
#[test]
fn test_propose_admin_self_reverts() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let result = client.try_propose_admin(&admin, &admin);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(
            ConfigManagerError::InvalidAdminProposal as u32
        ),
    );
}

/// Only the current admin can propose a new one.
#[test]
fn test_propose_admin_non_admin_reverts() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = deploy_initialized(&env);
    let attacker = Address::generate(&env);
    let target = Address::generate(&env);

    let result = client.try_propose_admin(&attacker, &target);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(ConfigManagerError::Unauthorized as u32),
    );
}

/// Re-proposing overwrites a prior pending admin. Only the latest candidate
/// can accept.
#[test]
fn test_propose_admin_overwrites_previous() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);
    let first = Address::generate(&env);
    let second = Address::generate(&env);

    client.propose_admin(&admin, &first);
    client.propose_admin(&admin, &second);

    assert_eq!(client.get_pending_admin(), Some(second.clone()));

    // The first candidate calling accept_admin must be rejected.
    let result = client.try_accept_admin(&first);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(ConfigManagerError::NotPendingAdmin as u32),
    );
}

// ---------------------------------------------------------------------------
// accept_admin
// ---------------------------------------------------------------------------

/// accept_admin migrates admin role from old to new and emits both RoleChange
/// events. After acceptance, the pending slot is cleared.
#[test]
fn test_accept_admin_happy_path() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, old_admin) = deploy_initialized(&env);
    let new_admin = Address::generate(&env);
    client.propose_admin(&old_admin, &new_admin);

    client.accept_admin(&new_admin);

    let admin_role = role_admin(&env);
    assert!(client.has_role(&admin_role, &new_admin));
    assert!(!client.has_role(&admin_role, &old_admin));
    assert_eq!(client.get_pending_admin(), None);
}

/// After acceptance, the former admin loses grant_role; the new admin gains it.
#[test]
fn test_accept_admin_role_handover() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, old_admin) = deploy_initialized(&env);
    let new_admin = Address::generate(&env);
    client.propose_admin(&old_admin, &new_admin);
    client.accept_admin(&new_admin);

    let keeper_role = role_keeper(&env);
    let victim = Address::generate(&env);

    let old_result = client.try_grant_role(&old_admin, &keeper_role, &victim);
    assert!(old_result.is_err());
    let new_result = client.try_grant_role(&new_admin, &keeper_role, &victim);
    assert!(new_result.is_ok());
}

/// accept_admin emits two RoleChange events: revoke from old, grant to new.
#[test]
fn test_accept_admin_emits_role_change_events() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, old_admin) = deploy_initialized(&env);
    let new_admin = Address::generate(&env);
    client.propose_admin(&old_admin, &new_admin);

    client.accept_admin(&new_admin);

    let cm_id = client.address.clone();
    let admin_role = role_admin(&env);
    let mut saw_revoke = false;
    let mut saw_grant = false;
    for (contract, topics, data) in env.events().all() {
        if contract != cm_id || topics.len() == 0 {
            continue;
        }
        let topic0: Symbol = match topics.get(0).unwrap().try_into_val(&env) {
            Ok(s) => s,
            Err(_) => continue,
        };
        if topic0 != Symbol::new(&env, "role") {
            continue;
        }
        let parsed: (Symbol, Address, bool) = match data.try_into_val(&env) {
            Ok(t) => t,
            Err(_) => continue,
        };
        if parsed.0 == admin_role && parsed.1 == old_admin && !parsed.2 {
            saw_revoke = true;
        }
        if parsed.0 == admin_role && parsed.1 == new_admin && parsed.2 {
            saw_grant = true;
        }
    }
    assert!(saw_revoke, "expected RoleChange(ADMIN, old_admin, false)");
    assert!(saw_grant, "expected RoleChange(ADMIN, new_admin, true)");
}

/// accept_admin without a pending proposal reverts with NoPendingAdmin.
#[test]
fn test_accept_admin_with_no_pending_reverts() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = deploy_initialized(&env);
    let candidate = Address::generate(&env);

    let result = client.try_accept_admin(&candidate);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(ConfigManagerError::NoPendingAdmin as u32),
    );
}

/// accept_admin called by an address other than the pending one reverts with
/// NotPendingAdmin — even though the auth signature is mocked, the contract
/// must verify the caller IS the pending admin.
#[test]
fn test_accept_admin_by_non_pending_reverts() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);
    let pending = Address::generate(&env);
    let impostor = Address::generate(&env);
    client.propose_admin(&admin, &pending);

    let result = client.try_accept_admin(&impostor);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(ConfigManagerError::NotPendingAdmin as u32),
    );
}

// ---------------------------------------------------------------------------
// cancel_admin_proposal
// ---------------------------------------------------------------------------

/// cancel_admin_proposal clears the pending slot. The previously-pending
/// candidate can no longer accept.
#[test]
fn test_cancel_admin_proposal_happy_path() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);
    let candidate = Address::generate(&env);
    client.propose_admin(&admin, &candidate);

    client.cancel_admin_proposal(&admin);

    assert_eq!(client.get_pending_admin(), None);
    let result = client.try_accept_admin(&candidate);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(ConfigManagerError::NoPendingAdmin as u32),
    );
}

/// Only the current admin can cancel a proposal.
#[test]
fn test_cancel_admin_proposal_non_admin_reverts() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);
    let candidate = Address::generate(&env);
    let attacker = Address::generate(&env);
    client.propose_admin(&admin, &candidate);

    let result = client.try_cancel_admin_proposal(&attacker);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(ConfigManagerError::Unauthorized as u32),
    );
}

/// Calling cancel_admin_proposal with no pending proposal is a no-op (idempotent).
#[test]
fn test_cancel_admin_proposal_no_pending_is_noop() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    client.cancel_admin_proposal(&admin);
    assert_eq!(client.get_pending_admin(), None);
}

/// cancel_admin_proposal emits AdminProposalCancelled so off-chain monitoring
/// can correlate cancellations with prior proposals.
#[test]
fn test_cancel_admin_proposal_emits_event() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);
    let candidate = Address::generate(&env);
    client.propose_admin(&admin, &candidate);

    client.cancel_admin_proposal(&admin);

    let cm_id = client.address.clone();
    let topic_cxl = Symbol::new(&env, "admincxl");
    let mut saw = false;
    for (contract, topics, data) in env.events().all() {
        if contract != cm_id || topics.len() == 0 {
            continue;
        }
        let topic0: Symbol = match topics.get(0).unwrap().try_into_val(&env) {
            Ok(s) => s,
            Err(_) => continue,
        };
        if topic0 != topic_cxl {
            continue;
        }
        let parsed: (Address,) = match data.try_into_val(&env) {
            Ok(t) => t,
            Err(_) => continue,
        };
        if parsed.0 == admin {
            saw = true;
            break;
        }
    }
    assert!(saw, "cancel_admin_proposal must emit AdminProposalCancelled(canceller)");
}

// ---------------------------------------------------------------------------
// get_pending_admin view
// ---------------------------------------------------------------------------

/// Fresh contract has no pending admin.
#[test]
fn test_get_pending_admin_initially_none() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = deploy_initialized(&env);
    assert_eq!(client.get_pending_admin(), None);
}
