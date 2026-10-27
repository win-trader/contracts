//! §12.6 — the emitted results.
//!
//! "Events are the only durable record of a terminal state — §5.6 removes the
//! pending record and §5.14 removes the position — so an off-chain consumer
//! that misses an event cannot reconstruct it from state." That makes the
//! event stream part of the interface, and until now nothing asserted a
//! single field of it: six event modules sat at 0% coverage, which is an
//! indexer silently seeing nothing.
//!
//! These decode the published data as a `Map<Symbol, Val>` rather than through
//! the emitting crate's structs, because that is what a consumer actually
//! receives. Reading the fields by name also means the test fails if a field
//! an indexer keys on is renamed, which a typed decode would quietly survive.
//!
//! Written from `shared`'s interfaces and the specification.

mod spec_harness;

use spec_harness::*;

use shared::{defaults, ActionOutcome};
use soroban_sdk::{
    testutils::{Address as _, Events as _},
    Address, Map, Symbol, TryFromVal, Val,
};

/// §12.6 — "`event_version` is bumped whenever a field's meaning changes,
/// never silently reused." Pinned as a literal on purpose: a test that read
/// the constant from the source could never catch an accidental bump.
const EVENT_VERSION: u32 = 1;

/// The published payloads whose first topic is `topic`, decoded the way a
/// consumer receives them.
///
/// `Events::all()` holds only the most recent top-level invocation's events,
/// so this is always read immediately after the call under test — there is no
/// running log to index into.
fn payloads(p: &Protocol, topic: &str) -> Vec<Map<Symbol, Val>> {
    let wanted = Symbol::new(&p.env, topic);
    p.env
        .events()
        .all()
        .iter()
        .filter(|(_, topics, _)| {
            topics
                .first()
                .map(|t| Symbol::try_from_val(&p.env, &t) == Ok(wanted.clone()))
                .unwrap_or(false)
        })
        .map(|(_, _, data)| Map::try_from_val(&p.env, &data).expect("map-format payload"))
        .collect()
}

/// Validate §12.6's envelope on every event the last call published, and
/// return how many carried one.
fn check_envelope(p: &Protocol) -> usize {
    let market = Symbol::new(&p.env, "BTC");
    let vault_wide = Symbol::new(&p.env, "vault");
    let mut seen = 0;
    for (source, _, data) in p.env.events().all().iter() {
        // The collateral token publishes its own transfers; only this
        // protocol's contracts owe the envelope.
        if source != p.pm && source != p.vault && source != p.router {
            continue;
        }
        let Ok(m) = Map::<Symbol, Val>::try_from_val(&p.env, &data) else {
            continue;
        };
        let Some(h) = m.get(Symbol::new(&p.env, "header")) else {
            continue;
        };
        let h = Map::<Symbol, Val>::try_from_val(&p.env, &h).unwrap();
        seen += 1;

        assert_eq!(
            u32::try_from_val(&p.env, &h.get(Symbol::new(&p.env, "event_version")).unwrap()),
            Ok(EVENT_VERSION),
            "§12.6 — consumers pin the version they understand"
        );
        let stamp: u64 =
            u64::try_from_val(&p.env, &h.get(Symbol::new(&p.env, "ledger_timestamp")).unwrap())
                .unwrap();
        assert!(
            stamp > 0 && stamp <= p.now(),
            "§12.6 — the envelope carries the ledger time the event was emitted at"
        );
        let on: Symbol =
            Symbol::try_from_val(&p.env, &h.get(Symbol::new(&p.env, "market")).unwrap()).unwrap();
        assert!(
            on == market || on == vault_wide,
            "§12.6 — market id, or the vault-wide marker"
        );
        assert!(
            h.get(Symbol::new(&p.env, "actor")).is_some(),
            "§12.6 — the caller credited with the action"
        );
    }
    seen
}

fn int(p: &Protocol, m: &Map<Symbol, Val>, field: &str) -> i128 {
    let v = m
        .get(Symbol::new(&p.env, field))
        .unwrap_or_else(|| panic!("§12.6 — required field `{field}` is missing"));
    i128::try_from_val(&p.env, &v).expect("i128 field")
}

/// §12.6 — the common envelope, on every event the protocol publishes.
#[test]
fn every_event_carries_the_common_envelope() {
    let p = Protocol::new();
    let c = p.pm();
    let mut seen = 0;

    // A life broad enough to reach the governance, market, and vault-wide
    // emitters as well as the settlement ones. Each call is checked on its
    // own, because the event log only holds the latest invocation.
    let id = p.open_position();
    seen += check_envelope(&p);
    c.add_collateral(&id, &usd(100));
    seen += check_envelope(&p);
    c.update_indices(&p.keeper, &p.market);
    seen += check_envelope(&p);
    c.disable_market(&p.admin, &p.market);
    seen += check_envelope(&p);
    c.enable_market(&p.admin, &p.market);
    seen += check_envelope(&p);
    c.pause(&p.admin);
    seen += check_envelope(&p);
    c.unpause(&p.admin);
    seen += check_envelope(&p);
    let donor = Address::generate(&p.env);
    p.mint(&donor, usd(10));
    c.recapitalize(&donor, &usd(10));
    seen += check_envelope(&p);

    p.observe(defaults::MIN_POSITION_LIFETIME, usd(52_000));
    let close = c.create_close(&id, &0);
    seen += check_envelope(&p);
    p.observe(6, usd(52_000));
    c.settle_close(&p.keeper, &close);
    seen += check_envelope(&p);

    assert!(seen >= 8, "the walk should have reached many emitters, saw {seen}");
}

/// §12.6 — "every terminal outcome emits exactly one structured event", and
/// the two settlement outcomes are distinguishable without inspecting state.
#[test]
fn a_terminal_action_emits_exactly_one_result_event() {
    // Executed.
    let p = Protocol::new();
    let c = p.pm();
    p.publish(COMMIT_PRICE);
    let action = c.create_market_open(&p.trader, &p.market, &p.open_payload(300));
    p.observe(6, FILL);
    assert_eq!(c.settle_market_open(&p.keeper, &action), ActionOutcome::Executed);
    assert_eq!(payloads(&p, "actsettle").len(), 1, "one settled result");
    assert_eq!(payloads(&p, "actfail").len(), 0, "and not also a failure");
    assert_eq!(payloads(&p, "posopen").len(), 1, "§12.6 — PositionOpened");

    // Failed, on an identical commitment.
    let q = Protocol::new();
    let d = q.pm();
    q.publish(COMMIT_PRICE);
    let slipped = d.create_market_open(&q.trader, &q.market, &q.open_payload(300));
    q.observe(6, usd(50_200));
    assert_eq!(d.settle_market_open(&q.keeper, &slipped), ActionOutcome::Failed);
    assert_eq!(payloads(&q, "actfail").len(), 1, "one failed result");
    assert_eq!(payloads(&q, "actsettle").len(), 0);
    assert_eq!(payloads(&q, "posopen").len(), 0, "§9.12 — no position");
}

/// §12.6 — "the sum of an event's parts equals the amount it moved, so §9.1
/// is checkable from the event stream alone." For a failed entry that is
/// §9.11's identity: `escrow = keeper reward + owner refund`.
#[test]
fn a_failed_entry_event_accounts_for_every_unit_of_its_escrow() {
    let p = Protocol::new();
    let c = p.pm();
    p.publish(COMMIT_PRICE);
    let action = c.create_market_open(&p.trader, &p.market, &p.open_payload(300));
    let escrow = c.get_pending_action(&action).escrowed_collateral;

    p.observe(6, usd(50_200));
    let trader_before = p.cash(&p.trader);
    let keeper_before = p.cash(&p.keeper);
    c.settle_market_open(&p.keeper, &action);

    let events = payloads(&p, "actfail");
    assert_eq!(events.len(), 1);
    let e = &events[0];
    let reward = int(&p, e, "reward");
    let refund = int(&p, e, "refund");

    assert_eq!(
        reward + refund,
        escrow,
        "§9.11 — escrow_before = keeper_reward_paid_from_escrow + owner_refund"
    );
    // And the reported parts are what actually moved, not what was intended.
    assert_eq!(reward, p.cash(&p.keeper) - keeper_before);
    assert_eq!(refund, p.cash(&p.trader) - trader_before);
    p.assert_conserved("after the reported failure");
}

/// §12.6 — "every event that changes cash ownership carries enough to
/// reproduce the change." The close event must let a consumer rebuild the
/// §3.8 waterfall and land on the payout that actually happened.
#[test]
fn a_close_event_reproduces_the_cash_it_moved() {
    let p = Protocol::new();
    let c = p.pm();
    let id = p.open_position();
    let stored = c.get_position(&id).stored_collateral;

    p.observe(defaults::MIN_POSITION_LIFETIME, usd(55_000));
    let close = c.create_close(&id, &0);
    p.observe(6, usd(55_000));
    let trader_before = p.cash(&p.trader);
    assert_eq!(c.settle_close(&p.keeper, &close), ActionOutcome::Executed);

    let events = payloads(&p, "posclose");
    assert_eq!(events.len(), 1, "§12.6 — exactly one PositionClosed");
    let e = &events[0];

    let payout = int(&p, e, "collateral_payout");
    assert_eq!(
        payout,
        p.cash(&p.trader) - trader_before,
        "§12.6 — the reported payout is the cash that moved"
    );

    // The waterfall, rebuilt from the event's own fields.
    let rebuilt = stored + int(&p, e, "payable_pnl") + int(&p, e, "funding_received")
        - int(&p, e, "receiver_funding_paid")
        - int(&p, e, "lp_funding_paid")
        - int(&p, e, "borrow_paid")
        - int(&p, e, "closing_fee")
        - int(&p, e, "keeper_reward");
    assert_eq!(
        rebuilt, payout,
        "§12.6 — the parts sum to the movement, so §9.1 is checkable from the stream"
    );
    p.assert_conserved("after the reported close");
}

/// §12.6 — "amounts are emitted as **collected**, never as nominal. A waived
/// closing fee ... reported as what it was, zero collected, with the waived
/// amount separate." §11.3 is the case: a nominal fee larger than what the
/// senior items leave collectible.
#[test]
fn a_closing_fee_is_reported_as_collected_and_never_as_nominal() {
    let p = Protocol::new();
    let c = p.pm();
    let id = p.open_position();

    // A thin profit, so §3.2's nominal fee exceeds what remains after the
    // senior items — the §11.3 shape.
    p.observe(defaults::MIN_POSITION_LIFETIME, usd(50_015));
    let close = c.create_close(&id, &0);
    p.observe(6, usd(50_015));
    c.settle_close(&p.keeper, &close);

    let e = &payloads(&p, "posclose")[0];
    let fee = int(&p, e, "closing_fee");
    let payable = int(&p, e, "payable_pnl");

    assert!(payable > 0, "the fixture needs a profitable close");
    assert!(
        fee <= payable,
        "§9.13 — 0 <= collected_closing_fee <= payable_price_pnl, and the \
         event reports the collected figure"
    );
    assert!(fee >= 0, "§9.13 — never negative");

    // And a loser pays none of it at all.
    let q = Protocol::new();
    let d = q.pm();
    let other = q.open_position();
    q.observe(defaults::MIN_POSITION_LIFETIME, usd(49_000));
    let close = d.create_close(&other, &0);
    q.observe(6, usd(49_000));
    d.settle_close(&q.keeper, &close);
    let e = &payloads(&q, "posclose")[0];
    assert_eq!(
        int(&q, e, "closing_fee"),
        0,
        "§9.13 — negative payable PnL collects no closing fee"
    );
    assert!(int(&q, e, "payable_pnl") < 0);
}

/// §9.10 — "any unpaid remainder is reported in the liquidation result as
/// forgone keeper revenue; it creates no claim, no receivable, and no bad
/// debt." And §3.9: the shortfall is reported explicitly.
#[test]
fn a_liquidation_event_reports_the_reward_split_and_the_bad_debt() {
    let p = Protocol::with_lp_seed(usd(12_000));
    let c = p.pm();
    let id = p.open_position();

    p.observe(1, usd(40_000));
    let keeper_before = p.cash(&p.keeper);
    c.liquidate_position(&p.keeper, &id);

    let events = payloads(&p, "posclose");
    assert_eq!(events.len(), 1, "§12.6 — one terminal event for the liquidation");
    let e = &events[0];

    let reward = int(&p, e, "keeper_reward");
    let from_backstop = int(&p, e, "keeper_from_lp_backstop");
    let unpaid = int(&p, e, "keeper_unpaid");

    assert_eq!(
        reward,
        p.cash(&p.keeper) - keeper_before,
        "§12.6 — the reported reward is the cash the keeper actually received"
    );
    assert_eq!(
        reward + unpaid,
        defaults::KEEPER_REWARD,
        "§9.10 — what was paid plus what was forgone is the configured amount"
    );
    assert!(
        from_backstop <= reward,
        "§9.10 — the backstop covers a gap, it does not add to the reward"
    );
    assert!(
        int(&p, e, "bad_debt") >= 0,
        "§3.9 — the shortfall is reported, never negative"
    );
    assert!(
        int(&p, e, "liquidation_threshold") > 0,
        "§12.6 — a consumer can check the eligibility decision itself"
    );
    p.assert_conserved("after the reported liquidation");
}


