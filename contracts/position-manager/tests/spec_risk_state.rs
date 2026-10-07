//! §6.5 and §6.16 — the side risk states, and the hard-cap payout factor.
//!
//! The property worth proving here is order-independence. §6.5 makes the
//! payout factor a **stored snapshot** rather than a live ratio for one
//! reason: every settlement moves both LP equity and the side's aggregate
//! PnL, so a factor derived per settlement would measure each position
//! against a denominator the previous one shrank. That is extractable — it
//! would pay whoever settles first more than whoever settles second, out of
//! the same pool — and it is invisible in any single-position test.
//!
//! Written from `shared`'s interfaces and the specification (§6.5, §6.16,
//! §6.16.1, §2.8).

mod spec_harness;

use spec_harness::*;

use shared::constants::{INDEX_PRECISION, PRICE_PRECISION};
use shared::{defaults, ActionOutcome, RiskState};
use soroban_sdk::{testutils::Address as _, Address};

/// Drive the long side into `HardCap`: two `$100,000` longs at 4 BTC of base
/// against `$1,000,000` of LP equity, marked at `$85,000`, is `$140,000` of
/// side profit — 1,400 bps against a 600 bps cap.
const SPIKE: i128 = usd_const(85_000);

/// §6.5 — "no position's recognized profit depends on when it settles
/// relative to another's."
#[test]
fn the_hard_cap_payout_is_the_same_whichever_position_settles_first() {
    // `first_out` names which of the two identical longs closes first.
    let payouts = |first_out: usize| -> (i128, i128) {
        let p = Protocol::new();
        let c = p.pm();
        let a = Address::generate(&p.env);
        let b = Address::generate(&p.env);
        p.mint(&a, usd(50_000));
        p.mint(&b, usd(50_000));

        let pos_a = p.open(&a, true, SIZE, SUBMITTED);
        let pos_b = p.open(&b, true, SIZE, SUBMITTED);
        p.latch_risk_state(SPIKE);
        assert_eq!(
            c.get_market(&p.market).long.risk_state,
            RiskState::HardCap,
            "the side is latched before either position exits"
        );

        p.wait(defaults::MIN_POSITION_LIFETIME);
        let close_a = c.create_close(&pos_a, &0);
        let close_b = c.create_close(&pos_b, &0);
        // One observation for both settlements, so the two positions face
        // identical borrow and funding conditions and only the order differs.
        p.observe(6, SPIKE);

        let before_a = p.cash(&a);
        let before_b = p.cash(&b);
        let order = if first_out == 0 {
            [close_a, close_b]
        } else {
            [close_b, close_a]
        };
        for action in order {
            assert_eq!(c.settle_close(&p.keeper, &action), ActionOutcome::Executed);
        }
        p.assert_conserved("after both hard-capped positions settled");
        (p.cash(&a) - before_a, p.cash(&b) - before_b)
    };

    let (a_first, b_first) = payouts(0);
    let (a_second, b_second) = payouts(1);

    assert_eq!(
        a_first, a_second,
        "§6.5 — the first position out is paid the same as when it goes second"
    );
    assert_eq!(
        b_first, b_second,
        "§6.5 — and so is the second; the factor is read, not recomputed"
    );
    assert!(a_first > 0 && b_first > 0, "both positions were profitable");
}

/// §6.5 — "the factor is read, not recomputed. It is snapshotted once when
/// the side enters `HardCap` and stays fixed for as long as the side remains
/// there." §6.16 adds that a side whose profit *falls* keeps its factor,
/// because lowering the denominator would pay later exits more than earlier
/// ones.
#[test]
fn the_payout_factor_is_a_snapshot_and_not_a_live_ratio() {
    let p = Protocol::new();
    let c = p.pm();
    p.open_position();
    p.open(&p.trader.clone(), true, SIZE, SUBMITTED);
    p.latch_risk_state(SPIKE);

    let side = c.get_market(&p.market).long;
    assert_eq!(side.risk_state, RiskState::HardCap);
    assert!(
        side.hard_cap_payout_factor < INDEX_PRECISION,
        "§6.5 — a latched side pays out at less than one whole unit"
    );
    assert!(side.hard_cap_payout_factor > 0);
    assert!(
        side.hard_cap_reference_pnl > 0,
        "§6.16 — the denominator it was measured against is recorded"
    );
    let latched = side.hard_cap_payout_factor;
    let reference = side.hard_cap_reference_pnl;

    // Profit falls but the side stays latched: the factor must not move. A
    // close refreshes the side's state from its own price snapshot, which is
    // the reachable trigger — §14 refuses a new LP request while a side is
    // restricted, so `latch_risk_state` cannot be used a second time.
    p.wait(defaults::MIN_POSITION_LIFETIME);
    let close = c.create_close(&1, &0);
    p.observe(6, SPIKE);
    assert_eq!(c.settle_close(&p.keeper, &close), ActionOutcome::Executed);

    let side = c.get_market(&p.market).long;
    assert_eq!(side.risk_state, RiskState::HardCap, "still latched");
    assert_eq!(
        side.hard_cap_payout_factor, latched,
        "§6.16 — a falling profit does not re-latch; that would pay later exits more"
    );
    assert_eq!(side.hard_cap_reference_pnl, reference);

    // §2.8 — and the surviving position still faces that same factor.
    let survivor = c.get_position(&2);
    let raw = survivor.base_exposure * SPIKE / PRICE_PRECISION - survivor.size;
    assert!(raw > 0, "the survivor is profitable and will be scaled by the factor");
    p.assert_conserved("with the side latched");
}

/// §6.16.1 — `ADL` and `HardCap` block new exposure **on that side only**.
/// "The opposite side is never restricted by this side's state, because
/// opening against a restricted side reduces skew ... the trade that resolves
/// the condition."
#[test]
fn a_latched_side_is_closed_to_new_exposure_and_the_other_side_is_not() {
    let p = Protocol::new();
    let c = p.pm();
    p.open_position();
    p.open(&p.trader.clone(), true, SIZE, SUBMITTED);
    p.latch_risk_state(SPIKE);
    assert_eq!(c.get_market(&p.market).long.risk_state, RiskState::HardCap);
    assert_eq!(
        c.get_market(&p.market).short.risk_state,
        RiskState::Normal,
        "§6.16.1 — the short side is untouched by the long side's state"
    );

    let newcomer = Address::generate(&p.env);
    p.mint(&newcomer, usd(50_000));

    // §7.1 — creation itself carries `require side_accepts_new_exposure`, so
    // an order onto a side that is already latched never becomes a commitment.
    p.publish(SPIKE);
    let mut long_order = p.open_payload(300);
    long_order.acceptable_price = 0;
    long_order.size = usd(20_000);
    long_order.submitted_collateral = usd(2_000);
    assert!(
        c.try_create_market_open(&newcomer, &p.market, &long_order).is_err(),
        "§7.1 — a HardCap side does not accept new exposure at creation"
    );

    // The opposite side still takes the trade that resolves the condition.
    let short = p.open(&newcomer, false, usd(20_000), usd(2_000));
    assert_eq!(c.get_position(&short).size, usd(20_000));
    p.assert_conserved("after the latched side refused and the other accepted");
}

/// The other half of the same rule. §6.16.1's predicate is evaluated at
/// settlement too, "after the risk state has been refreshed from the action's
/// own price snapshot" — so a side that latches *between* commitment and fill
/// drains the order terminally (§8.9) rather than holding it for a recovery
/// that may never come.
#[test]
fn a_side_that_latches_after_the_commitment_drains_the_order() {
    let p = Protocol::new();
    let c = p.pm();
    p.open_position();
    p.open(&p.trader.clone(), true, SIZE, SUBMITTED);

    // Committed while the side is still Normal.
    let newcomer = Address::generate(&p.env);
    p.mint(&newcomer, usd(50_000));
    p.publish(FILL);
    let mut order = p.open_payload(300);
    order.acceptable_price = 0;
    order.size = usd(20_000);
    order.submitted_collateral = usd(2_000);
    let committed = c.create_market_open(&newcomer, &p.market, &order);
    assert_eq!(c.get_market(&p.market).long.risk_state, RiskState::Normal);

    p.latch_risk_state(SPIKE);
    assert_eq!(c.get_market(&p.market).long.risk_state, RiskState::HardCap);

    let keeper_before = p.cash(&p.keeper);
    p.observe(6, SPIKE);
    assert_eq!(
        c.settle_market_open(&p.keeper, &committed),
        ActionOutcome::Failed,
        "§8.9 — a side in ADL or HardCap is an expected terminal failure"
    );
    assert_eq!(
        p.cash(&p.keeper) - keeper_before,
        REWARD,
        "§8.11 — the drain still pays its one action reward"
    );
    assert_eq!(
        p.cash(&newcomer),
        usd(50_000) - REWARD,
        "§9.12 — no opening fee, and the rest of the escrow comes back"
    );
    p.assert_conserved("after the latched side drained the order");
}

/// §6.16.1 — "`Warning` restricts nothing by itself: it makes recovery
/// sticky." And §5.2: the two counters are reported separately so a
/// withdrawal is not frozen by four sides sitting just over the line.
#[test]
fn warning_latches_without_restricting_anything() {
    let p = Protocol::new();
    let c = p.pm();
    p.open_position();
    // 2 BTC marked at $72,000 is $44,000 of side profit against $1,000,000 of
    // LP equity: 440 bps, over the 400 bps warning line and under ADL's 500.
    p.latch_risk_state(usd(72_000));

    assert_eq!(c.get_market(&p.market).long.risk_state, RiskState::Warning);
    let s = p.snapshot();
    assert_eq!(s.restricted_side_count, 1, "§5.2 — Warning counts as restricted");
    assert_eq!(
        s.deleveraging_side_count, 0,
        "§5.2 — but it is not a side the withdrawal gate is stated over"
    );

    // New exposure on the warned side is still accepted.
    let newcomer = Address::generate(&p.env);
    p.mint(&newcomer, usd(50_000));
    let id = p.open(&newcomer, true, usd(20_000), usd(2_000));
    assert_eq!(c.get_position(&id).size, usd(20_000));
    p.assert_conserved("with a side in Warning");
}

/// §7.14 — ADL is "permissionless: the state gate bounds the mechanism, not
/// an allowlist", which also means the gate has to actually hold on a side
/// that is merely warned.
#[test]
fn adl_is_refused_while_the_side_is_below_its_gate() {
    let p = Protocol::new();
    let c = p.pm();
    let id = p.open_position();
    p.latch_risk_state(usd(72_000));
    assert_eq!(c.get_market(&p.market).long.risk_state, RiskState::Warning);

    let anyone = Address::generate(&p.env);
    assert!(
        c.try_execute_adl(&anyone, &id).is_err(),
        "§7.14 — the state gate is the whole of the authorization"
    );
    assert!(c.try_get_position(&id).is_ok(), "the position is untouched");
    p.assert_conserved("after the refused ADL");
}
