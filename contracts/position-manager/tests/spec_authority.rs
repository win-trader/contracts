//! §12.3 — the authority gates, and the contract-only entry points.
//!
//! `env.mock_all_auths()` runs in every test in this repo, which does not
//! merely leave authorization untested: it **masks** it, because every
//! `require_auth` is satisfied for free. What that leaves visible is the layer
//! §12.3 actually specifies — the ConfigManager role checks and the
//! caller-is-that-contract checks — and those are the ones a wrong wiring
//! turns into a drained vault rather than a failed transaction. The last test
//! drops the mock to cover the other half.
//!
//! Written from `shared`'s interfaces and the specification (§12.3, §7.0,
//! and the vault's own module contract).

mod spec_harness;

use spec_harness::*;

use shared::constants::{ROLE_PAUSER, ROLE_UNPAUSER};
use shared::{defaults, VaultClient};
use soroban_sdk::{testutils::Address as _, Address, Symbol};

fn granted(p: &Protocol, role: &str) -> Address {
    let who = Address::generate(&p.env);
    config_manager::ConfigManagerClient::new(&p.env, &p.config_manager).grant_role(
        &p.admin,
        &Symbol::new(&p.env, role),
        &who,
    );
    who
}

/// §12.3 — every authority-gated entry point refuses an address holding no
/// role, and every entry point the specification calls permissionless accepts
/// that same address.
#[test]
fn the_authority_gates_refuse_an_outsider_and_the_open_ones_do_not() {
    let p = Protocol::new();
    let c = p.pm();
    let outsider = Address::generate(&p.env);
    p.open_position();

    assert!(c.try_pause(&outsider).is_err(), "§12.3 — pause_authority");
    assert!(c.try_unpause(&outsider).is_err(), "§12.3 — unpause_authority");
    assert!(
        p.gov().try_propose_global_config(&outsider, &defaults::global_config()).is_err(),
        "§12.3 — configuration_authority"
    );
    assert!(p.gov()
        .try_propose_market_config(&outsider, &p.market, &defaults::market_config())
        .is_err());
    assert!(p.gov().try_deregister_market(&outsider, &p.market).is_err());
    assert!(c.try_disable_market(&outsider, &p.market).is_err());
    assert!(c.try_enable_market(&outsider, &p.market).is_err());
    assert!(
        p.gov().try_propose_price_feed(&outsider, &p.feed).is_err(),
        "§12.3 — oracle_authority"
    );
    assert!(p.gov().try_cancel_price_feed(&outsider).is_err());
    assert!(p.gov().try_cancel_global_config(&outsider).is_err());
    assert!(p.gov().try_cancel_market_config(&outsider, &p.market).is_err());
    assert!(
        c.try_claim_protocol(&outsider, &outsider, &1).is_err(),
        "§12.3 — protocol_recipient"
    );
    assert!(c.try_set_vault(&outsider, &p.vault).is_err());

    // §7.0 — "a checkpoint pays no reward and moves no value between parties,
    // so there is nothing for an allowlist to protect."
    c.update_indices(&outsider, &p.market);
    p.assert_conserved("after an outsider exercised the open entry points");
}

/// §12.3 — "pausing and unpausing are split on purpose ... `pause_authority`
/// may set `paused`; it may not clear it." A single key holding both is the
/// configuration this test exists to catch.
#[test]
fn pausing_and_unpausing_are_separate_keys() {
    let p = Protocol::new();
    let c = p.pm();
    let pauser = granted(&p, ROLE_PAUSER);
    let unpauser = granted(&p, ROLE_UNPAUSER);

    c.pause(&pauser);
    assert!(
        c.try_pause(&unpauser).is_err(),
        "§12.3 — the unpause key is not a pause key"
    );
    assert!(
        c.try_unpause(&pauser).is_err(),
        "§12.3 — pausing is fast, unpausing re-admits risk and is not the same key"
    );
    c.unpause(&unpauser);

    // And the pause it set was real.
    c.pause(&pauser);
    assert!(c
        .try_create_market_open(&p.trader, &p.market, &p.open_payload(300))
        .is_err());
}

/// The vault "moves cash only on instruction from the PositionManager (claim
/// transfers) or the RequestRouter (LP settlement)". Every one of those entry
/// points is a direct path to the vault's balance, so each must admit exactly
/// its one caller.
#[test]
fn the_vaults_cash_moving_entry_points_admit_only_their_one_caller() {
    let p = Protocol::new();
    let v = VaultClient::new(&p.env, &p.vault);
    let thief = Address::generate(&p.env);
    p.mint(&thief, usd(1_000));
    let held = p.physical();

    assert!(
        v.try_transfer_claim(&thief, &thief, &usd(100), &0).is_err(),
        "PositionManager only"
    );
    assert!(
        v.try_transfer_safety_claim(&thief, &thief, &usd(100)).is_err(),
        "PositionManager only — and this one skips the conservation check"
    );
    assert!(v.try_receive_collateral(&thief, &thief, &usd(100)).is_err());
    assert!(
        v.try_settle_deposit(&thief, &thief, &usd(100)).is_err(),
        "RequestRouter only"
    );
    assert!(v
        .try_settle_withdrawal(&thief, &thief, &usd(100), &thief)
        .is_err());
    assert!(v.try_set_request_router(&thief, &thief).is_err());
    assert!(v.try_set_lp_config(&thief, &v.get_lp_config()).is_err());

    assert_eq!(p.physical(), held, "not one unit of vault cash moved");
    assert_eq!(p.cash(&thief), usd(1_000));
    p.assert_conserved("after the vault refused every impostor");
}

/// The mirror: the position manager's LP-settlement surface is the vault's
/// alone. `prepare_lp_snapshot` persists risk-state transitions (§7.17), so an
/// open caller could latch a side at a moment of their choosing.
#[test]
fn the_position_managers_vault_only_entry_points_admit_only_the_vault() {
    let p = Protocol::new();
    let c = p.pm();
    let outsider = Address::generate(&p.env);
    let physical = p.physical();

    assert!(c.try_prepare_lp_snapshot(&outsider, &physical).is_err());
    assert!(c.try_refresh_borrow_rate(&outsider, &physical).is_err());

    // The read-only sibling is open to anyone, which is the distinction: it
    // persists nothing.
    let _ = c.accounting_snapshot(&physical);
    p.assert_conserved("after an outsider probed the LP surface");
}

/// One-time wiring stays one-time. A second `set_vault` would repoint the
/// protocol's cash at an attacker's contract.
#[test]
fn the_protocol_wiring_is_one_shot() {
    let p = Protocol::new();
    let impostor = Address::generate(&p.env);

    assert!(
        p.pm().try_set_vault(&p.admin, &impostor).is_err(),
        "AlreadyInitialized — even for the admin"
    );
    assert!(VaultClient::new(&p.env, &p.vault)
        .try_set_request_router(&p.admin, &impostor)
        .is_err());
}

/// The half `mock_all_auths` hides: a position's operations are gated on the
/// owner's authorization, not on the caller's identity being passed in. With
/// the mock dropped, a call nobody authorized must fail.
#[test]
fn a_position_operation_without_its_owners_authorization_fails() {
    let p = Protocol::new();
    let c = p.pm();
    let id = p.open_position();
    p.wait(defaults::MIN_POSITION_LIFETIME);

    // Stop approving everything, then ask for a mutation with an empty auth
    // list. §7.0's keeper rule authenticates the reward recipient; the owner
    // rule here has nobody behind it.
    p.env.set_auths(&[]);
    assert!(
        c.try_create_close(&id, &0).is_err(),
        "an unauthorized mutation must not reach the position"
    );
    assert!(c.try_add_collateral(&id, &usd(100)).is_err());
    assert!(c.try_set_stop_loss(&id, &usd(45_000), &0).is_err());

    p.env.mock_all_auths();
    let close = c.create_close(&id, &0);
    assert_eq!(c.get_position(&id).pending_mutation_action_id, Some(close));
}
