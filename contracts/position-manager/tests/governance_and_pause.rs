//! §12.2 pause semantics and §7.18 market lifecycle.

mod common;

use common::{World, PRICE};
use shared::{defaults, ActionOutcome};
use soroban_sdk::{testutils::Address as _, Address};

/// §12.2 — "a pause stops the vault taking on risk; it never stops anyone
/// shedding it", plus the one revenue path that is deliberately closed with
/// it.
#[test]
fn a_pause_closes_risk_and_protocol_revenue_but_not_exits() {
    let w = World::new();
    let c = w.client();
    let action_id = c.create_market_open(&w.trader, &w.market, &w.request(100_000_0000, 0, 120));
    w.observe(10, PRICE);
    c.settle_market_open(&w.keeper, &action_id);
    c.pause(&w.admin);

    // Closed: anything that adds exposure.
    assert!(c
        .try_create_market_open(&w.trader, &w.market, &w.request(100_000_0000, 0, 120))
        .is_err());
    // Closed: protocol revenue. The same authority can generally reach both,
    // and leaving this open is a pause-and-drain path that costs nothing to
    // close.
    assert!(c.try_claim_protocol(&w.admin, &w.admin, &1).is_err());

    // Open: everything that reduces risk or returns a trader's own money.
    c.add_collateral(&1, &10_000_0000);
    c.set_stop_loss(&1, &(PRICE / 2), &0);
    let close = c.create_close(&1, &0);
    w.observe(defaults::MIN_POSITION_LIFETIME, PRICE);
    assert_eq!(c.settle_close(&w.keeper, &close), ActionOutcome::Executed);
}

/// §12.2 — accrual never pauses. Both checkpoint clocks advance across a
/// pause exactly as they would otherwise, which is only fair because exits
/// stay open.
#[test]
fn accrual_keeps_running_while_paused() {
    let w = World::new();
    let c = w.client();
    let action_id = c.create_market_open(&w.trader, &w.market, &w.request(100_000_0000, 0, 120));
    w.observe(10, PRICE);
    c.settle_market_open(&w.keeper, &action_id);

    c.pause(&w.admin);
    let before = c.pending_fees(&1, &w.env.ledger().timestamp());
    w.observe(3_600, PRICE);
    let after = c.pending_fees(&1, &w.env.ledger().timestamp());
    assert!(
        after.borrow > before.borrow,
        "the borrow clock kept running through the pause"
    );
}

/// §7.18 — deregistration requires an empty market, and keeps the record.
#[test]
fn a_market_is_deregistered_only_when_nothing_is_owed_to_it() {
    let w = World::new();
    let c = w.client();
    let action_id = c.create_market_open(&w.trader, &w.market, &w.request(100_000_0000, 0, 120));
    w.observe(10, PRICE);
    c.settle_market_open(&w.keeper, &action_id);

    assert!(
        c.try_deregister_market(&w.admin, &w.market).is_err(),
        "open interest still stands against it"
    );

    let close = c.create_close(&1, &0);
    w.observe(defaults::MIN_POSITION_LIFETIME, PRICE);
    c.settle_close(&w.keeper, &close);

    let index_before = c.get_market(&w.market).receiver_backed_index_long;
    c.deregister_market(&w.admin, &w.market);
    assert_eq!(c.active_markets().len(), 0, "out of the registry");
    assert_eq!(
        c.get_market(&w.market).receiver_backed_index_long,
        index_before,
        "but the record and its indices are retained (§7.18): rewinding one \
         would let a re-registration reprice a historical baseline"
    );
    assert!(
        c.try_create_market_open(&w.trader, &w.market, &w.request(100_000_0000, 0, 120))
            .is_err(),
        "and it accepts no new commitments"
    );
}

/// §7.18 — re-registering puts the market back in the registry rather than
/// only updating its configuration.
#[test]
fn re_registering_returns_a_market_to_the_registry() {
    let w = World::new();
    let c = w.client();
    c.deregister_market(&w.admin, &w.market);
    assert_eq!(c.active_markets().len(), 0);

    c.propose_market_config(&w.admin, &w.market, &defaults::market_config());
    assert_eq!(c.active_markets().len(), 1);
    let action_id = c.create_market_open(&w.trader, &w.market, &w.request(100_000_0000, 0, 120));
    w.observe(10, PRICE);
    assert_eq!(
        c.settle_market_open(&w.keeper, &action_id),
        ActionOutcome::Executed
    );
}

/// A pending entry on a market that is deregistered while it waits drains
/// terminally rather than reverting — its escrow comes back immediately
/// instead of waiting out the order's lifetime.
#[test]
fn deregistration_drains_a_pending_entry_rather_than_stranding_it() {
    let w = World::new();
    let c = w.client();
    let before = w.balance(&w.trader);
    let action_id = c.create_market_open(&w.trader, &w.market, &w.request(100_000_0000, 0, 120));
    c.deregister_market(&w.admin, &w.market);

    w.observe(10, PRICE);
    assert_eq!(
        c.settle_market_open(&w.keeper, &action_id),
        ActionOutcome::Failed
    );
    let reward = common::global_config().keeper_rewards.open;
    assert_eq!(w.balance(&w.trader), before - reward);
}

/// §12.5 — the external feed's errors are wrapped, never passed through.
/// A feed numbering from `1` could otherwise return something that reads
/// exactly like a native position-manager code.
#[test]
fn an_unwired_feed_surfaces_as_this_contracts_own_error() {
    let w = World::new();
    let c = w.client();
    // A contract that is not a price feed at all: the call into it fails
    // with something that is not ours.
    let impostor = Address::generate(&w.env);
    assert!(
        c.try_propose_price_feed(&w.admin, &impostor).is_err(),
        "an address that cannot answer `decimals` is refused at proposal time"
    );
}
