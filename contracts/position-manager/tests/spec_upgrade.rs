//! §12.4 and §12.3 — the upgrade timelock, and the admin handover.
//!
//! Four contracts each carry a `propose_upgrade` / `cancel_upgrade` /
//! `upgrade` triple, and none of them had any coverage. The upgrade path is
//! the one place where a single key can replace the whole protocol, so what
//! matters is the guards around it rather than a successful install: who may
//! propose, that the eta is honoured, that the installed hash is the proposed
//! one, and that a cancellation really removes the proposal.
//!
//! These tests are deliberately all negative on the install itself. Actually
//! installing would need a second build's wasm uploaded into the test host,
//! which proves the SDK's deployer works and nothing about this protocol.
//! `state_version` (§12.4's migration guard) is likewise out of reach without
//! a second build and is called out in the handover notes instead.
//!
//! Written from `shared`'s interfaces and the specification.

mod spec_harness;

use spec_harness::*;

use shared::constants::{
    ADMIN_PROPOSAL_TTL_SECS, DEFAULT_UPGRADE_TIMELOCK, MAX_UPGRADE_TIMELOCK_SECS,
    MIN_UPGRADE_TIMELOCK, ROLE_UPGRADER,
};
use shared::{RequestRouterClient, VaultClient};
use soroban_sdk::{testutils::Address as _, Address, BytesN, Symbol};

fn hash(p: &Protocol, byte: u8) -> BytesN<32> {
    BytesN::from_array(&p.env, &[byte; 32])
}

fn cm(p: &Protocol) -> config_manager::ConfigManagerClient<'_> {
    config_manager::ConfigManagerClient::new(&p.env, &p.config_manager)
}

/// `upgrade` is declared on the concrete contract rather than on the shared
/// trait, so it needs the contract's own generated client.
fn pmc(p: &Protocol) -> position_manager::PositionManagerContractClient<'_> {
    position_manager::PositionManagerContractClient::new(&p.env, &p.pm)
}

fn upgrader(p: &Protocol) -> Address {
    let who = Address::generate(&p.env);
    cm(p).grant_role(&p.admin, &Symbol::new(&p.env, ROLE_UPGRADER), &who);
    who
}

/// §12.3 — an upgrade proposal is an authority action on every contract that
/// has one. An outsider must not be able to start the clock anywhere.
#[test]
fn only_the_upgrade_authority_may_propose_on_any_contract() {
    let p = Protocol::new();
    let outsider = Address::generate(&p.env);
    let h = hash(&p, 1);

    assert!(p.pm().try_propose_upgrade(&outsider, &h).is_err(), "position manager");
    assert!(
        VaultClient::new(&p.env, &p.vault)
            .try_propose_upgrade(&outsider, &h)
            .is_err(),
        "vault"
    );
    assert!(
        RequestRouterClient::new(&p.env, &p.router)
            .try_propose_upgrade(&outsider, &h)
            .is_err(),
        "request router"
    );
    assert!(cm(&p).try_propose_upgrade(&outsider, &h).is_err(), "config manager");

    // And the authority itself may.
    let auth = upgrader(&p);
    p.pm().propose_upgrade(&auth, &h);
    VaultClient::new(&p.env, &p.vault).propose_upgrade(&auth, &h);
    RequestRouterClient::new(&p.env, &p.router).propose_upgrade(&auth, &h);
    cm(&p).propose_upgrade(&auth, &h);
}

/// §12.4 — the proposal carries `eta`, and `upgrade` "refuses to install
/// unless `pending.wasm_hash` matches the supplied hash and `now >= eta`".
#[test]
fn an_upgrade_honours_its_eta_and_its_hash() {
    let p = Protocol::new();
    let auth = upgrader(&p);
    let proposed = hash(&p, 7);
    let other = hash(&p, 8);

    assert_eq!(
        cm(&p).get_upgrade_timelock(),
        DEFAULT_UPGRADE_TIMELOCK,
        "§12.4 — 24h by default"
    );
    p.pm().propose_upgrade(&auth, &proposed);

    // Before the eta, with the right hash.
    assert!(
        pmc(&p).try_upgrade(&proposed, &auth).is_err(),
        "§12.4 — the timelock has not elapsed"
    );

    // After the eta, with the wrong hash. This must fail for the hash and not
    // quietly install something else.
    p.wait(DEFAULT_UPGRADE_TIMELOCK + 1);
    assert!(
        pmc(&p).try_upgrade(&other, &auth).is_err(),
        "§12.4 — the supplied hash is not the proposed one"
    );

    // The proposal is still standing after both refusals.
    p.pm().cancel_upgrade(&p.admin);
}

/// §12.4 — a cancellation clears the proposal, so a later install has nothing
/// to stand on. `upgrade` with no pending proposal is `UpgradeNoPending`.
#[test]
fn a_cancelled_proposal_leaves_nothing_to_install() {
    let p = Protocol::new();
    let auth = upgrader(&p);
    let h = hash(&p, 3);
    let outsider = Address::generate(&p.env);

    assert!(
        pmc(&p).try_upgrade(&h, &auth).is_err(),
        "nothing proposed yet"
    );

    p.pm().propose_upgrade(&auth, &h);
    assert!(
        p.pm().try_cancel_upgrade(&outsider).is_err(),
        "§12.3 — cancelling is an authority action too"
    );
    // Proposing and cancelling are separate keys, the same split §12.3 makes
    // between pausing and unpausing: the key that can start an upgrade is not
    // the key that can call one off.
    assert!(
        p.pm().try_cancel_upgrade(&auth).is_err(),
        "§12.3 — the proposer's key does not also cancel"
    );
    p.pm().cancel_upgrade(&p.admin);

    p.wait(DEFAULT_UPGRADE_TIMELOCK + 1);
    assert!(
        pmc(&p).try_upgrade(&h, &auth).is_err(),
        "§12.4 — the cancelled proposal cannot be installed after its eta"
    );
}

/// §12.4 — "the admin can raise but not lower below `MIN_UPGRADE_TIMELOCK`",
/// and the ceiling exists so an oversized value cannot push every eta past
/// the horizon and block all upgrades.
#[test]
fn the_upgrade_timelock_stays_inside_its_bounds() {
    let p = Protocol::new();
    let c = cm(&p);
    let outsider = Address::generate(&p.env);

    assert!(
        c.try_set_upgrade_timelock(&p.admin, &(MIN_UPGRADE_TIMELOCK - 1)).is_err(),
        "below the floor"
    );
    assert!(
        c.try_set_upgrade_timelock(&p.admin, &(MAX_UPGRADE_TIMELOCK_SECS + 1)).is_err(),
        "above the ceiling"
    );
    assert!(
        c.try_set_upgrade_timelock(&outsider, &MIN_UPGRADE_TIMELOCK).is_err(),
        "and it is the admin's to set"
    );

    c.set_upgrade_timelock(&p.admin, &(MIN_UPGRADE_TIMELOCK * 2));
    assert_eq!(c.get_upgrade_timelock(), MIN_UPGRADE_TIMELOCK * 2);
    c.set_upgrade_timelock(&p.admin, &MIN_UPGRADE_TIMELOCK);
    assert_eq!(c.get_upgrade_timelock(), MIN_UPGRADE_TIMELOCK, "raising is not one-way");
}

/// The admin handover is two-phase for the same reason the upgrade is: the
/// receiving key must prove it exists before it holds the protocol.
#[test]
fn the_admin_handover_needs_the_new_key_to_accept() {
    let p = Protocol::new();
    let c = cm(&p);
    let successor = Address::generate(&p.env);
    let impostor = Address::generate(&p.env);

    assert_eq!(c.get_pending_admin(), None);
    assert!(
        c.try_propose_admin(&impostor, &impostor).is_err(),
        "an outsider cannot nominate themselves"
    );

    c.propose_admin(&p.admin, &successor);
    assert_eq!(c.get_pending_admin(), Some(successor.clone()));
    assert!(
        c.try_accept_admin(&impostor).is_err(),
        "only the nominated address may accept"
    );

    c.accept_admin(&successor);
    assert_eq!(c.get_pending_admin(), None);
    assert!(
        c.try_propose_admin(&p.admin, &p.admin).is_err(),
        "the previous admin no longer holds the authority"
    );
    c.propose_admin(&successor, &p.admin);
    c.cancel_admin_proposal(&successor);
    assert_eq!(c.get_pending_admin(), None, "a cancelled handover leaves nothing");
}

/// A forgotten proposal is not a standing capability held by the proposed
/// key: `accept_admin` rejects one older than `ADMIN_PROPOSAL_TTL_SECS`.
#[test]
fn a_forgotten_admin_proposal_expires_rather_than_waiting_forever() {
    let p = Protocol::new();
    let c = cm(&p);
    let successor = Address::generate(&p.env);

    c.propose_admin(&p.admin, &successor);
    p.wait(ADMIN_PROPOSAL_TTL_SECS + 1);

    assert!(
        c.try_accept_admin(&successor).is_err(),
        "a week-old nomination is not a key someone still holds"
    );
    assert!(
        c.try_propose_admin(&successor, &successor).is_err(),
        "and it conferred nothing in the meantime"
    );
}
