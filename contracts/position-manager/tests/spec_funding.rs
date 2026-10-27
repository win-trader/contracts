//! §9.3 and §9.4 — funding conservation on a **two-sided** book.
//!
//! Every other suite in this repo runs one side only, which leaves the whole
//! receiver-backed / LP-backed split, the guaranteed receiver liability, and
//! the four carried remainders of §4.5 untouched. That is the part of the
//! accounting where a mistake does not merely misallocate: §9.4 states that
//! omitting the §4.5.1 reset makes a receiver position **unsettleable**, which
//! is stuck money rather than wrong money.
//!
//! Written from `shared`'s interfaces and the specification (§4.3, §4.5,
//! §9.3, §9.4).

mod spec_harness;

use spec_harness::*;

use shared::{defaults, ActionOutcome, PayerSide};
use soroban_sdk::{testutils::Address as _, Address};

/// A dominant long side and a lighter short side, so longs pay and shorts
/// receive. Returns (long position, short position, short owner).
fn two_sided(p: &Protocol) -> (u64, u64, Address) {
    let shorter = Address::generate(&p.env);
    p.mint(&shorter, usd(100_000));
    let long = p.open_position();
    let short = p.open(&shorter, false, usd(40_000), usd(3_000));
    (long, short, shorter)
}

/// §9.3 — "funding is a risk-balancing transfer, not protocol revenue. It
/// never invokes opening-fee, closing-fee, borrow-fee, or keeper
/// distribution."
#[test]
fn funding_moves_from_the_payer_side_to_the_receiver_side_and_nowhere_else() {
    let p = Protocol::new();
    let c = p.pm();
    let (long, short, _) = two_sided(&p);

    p.accrue(43_200, 3_600, FILL);

    let market = c.get_market(&p.market);
    assert_eq!(
        market.current_payer_side,
        PayerSide::Long,
        "§8.1 — the blended skew points at the heavier side once the EMA has run"
    );

    let payer = c.pending_fees(&long, &p.now());
    let receiver = c.pending_fees(&short, &p.now());

    assert!(
        payer.funding_paid_to_receivers + payer.funding_paid_to_lps > 0,
        "§9.3 — the payer owes across both streams"
    );
    assert_eq!(payer.funding_received, 0, "§9.3 — a payer earns no credit");
    assert!(receiver.funding_received > 0, "§9.3 — the receiver earns one");
    assert_eq!(receiver.funding_paid_to_receivers, 0);
    assert_eq!(receiver.funding_paid_to_lps, 0);

    // "no uncollected payer amount is recorded as protocol revenue"
    assert_eq!(
        c.protocol_claimable_total(),
        0,
        "§9.3 — accrued funding is not revenue"
    );
    p.assert_conserved("with funding accrued both ways");
}

/// §9.4 — "once receiver-backed funding accrues, it is a senior explicit
/// liability", and the credits taken can never exceed it.
#[test]
fn receiver_credits_never_exceed_the_recognised_liability() {
    let p = Protocol::new();
    let c = p.pm();
    let (_, short, shorter) = two_sided(&p);
    // A second receiver, so the distribution divides across more than one.
    let other = Address::generate(&p.env);
    p.mint(&other, usd(100_000));
    let short_two = p.open(&other, false, usd(25_000), usd(2_000));

    for _ in 0..8 {
        p.accrue(3_600, 900, FILL);

        let market = c.get_market(&p.market);
        let recognised = market.pending_receiver_funding;
        assert!(recognised >= 0, "§9.4 — the liability never goes negative");
        assert_eq!(
            c.pending_receiver_funding_total(),
            recognised,
            "§9.4 — the global total is the sum of the per-market liabilities"
        );

        let taken = c.pending_fees(&short, &p.now()).funding_received
            + c.pending_fees(&short_two, &p.now()).funding_received;
        assert!(
            taken <= recognised,
            "§9.4 — sum(receiver credits taken) <= sum(receiver_liability_delta recognised); \
             took {taken} against {recognised}"
        );
    }

    let _ = shorter;
    p.assert_conserved("after eight hours of receiver accrual");
}

/// §4.5.1 — "when a market side's `size_open_interest` changes, reset the
/// `receiver_distribution_remainder` of the **opposite** side's payer stream
/// to zero." The long-payer stream distributes to short receivers, so a change
/// on the short side is what must clear it.
#[test]
fn a_receiver_side_size_change_clears_the_opposite_payer_streams_carry() {
    let p = Protocol::new();
    let c = p.pm();
    let (_, _, shorter) = two_sided(&p);

    // Accrue in slices until the long-payer distribution carry is non-zero.
    let mut carried = 0;
    for _ in 0..12 {
        p.accrue(600, 300, FILL);
        carried = c
            .get_market(&p.market)
            .long_payer_remainders
            .distribution_remainder;
        if carried > 0 {
            break;
        }
    }
    assert!(
        carried > 0,
        "§4.5 — the receiver distribution should carry a sub-unit fraction"
    );

    // Change the receiving side's open interest.
    let third = Address::generate(&p.env);
    p.mint(&third, usd(100_000));
    p.open(&third, false, usd(10_000), usd(1_500));

    assert_eq!(
        c.get_market(&p.market)
            .long_payer_remainders
            .distribution_remainder,
        0,
        "§4.5.1 — the carry is void once its divisor moved"
    );
    let _ = shorter;
    p.assert_conserved("after the receiver side changed size");
}

/// §9.4's stated failure mode, end to end: "if an implementation omits the
/// §4.5.1 reset ... the sufficiency check in `credit_received_funding`
/// reverts, which would prevent a receiver position from being settled."
///
/// So churn the receiving side hard, then settle a receiver. It must close,
/// and its credit must actually reach the payout.
#[test]
fn a_receiver_position_stays_settleable_after_the_receiving_side_churns() {
    let p = Protocol::new();
    let c = p.pm();
    let (_, short, shorter) = two_sided(&p);

    // Repeatedly move the receiving side's size while funding accrues, which
    // is exactly the sequence that invalidates a carried divisor.
    for round in 0..4 {
        p.accrue(1_800, 450, FILL);
        let extra = Address::generate(&p.env);
        p.mint(&extra, usd(100_000));
        let id = p.open(&extra, false, usd(8_000) + usd(round * 1_000), usd(1_500));
        p.accrue(1_800, 450, FILL);
        p.wait(defaults::MIN_POSITION_LIFETIME);
        let close = c.create_close(&id, &0);
        p.observe(6, FILL);
        assert_eq!(
            c.settle_close(&p.keeper, &close),
            ActionOutcome::Executed,
            "a receiver opened and closed mid-stream settles"
        );
    }

    p.accrue(3_600, 600, FILL);
    let credit = c.pending_fees(&short, &p.now()).funding_received;
    assert!(credit > 0, "the surviving receiver has earned a credit");

    let before = p.cash(&shorter);
    let stored = c.get_position(&short).stored_collateral;
    p.wait(defaults::MIN_POSITION_LIFETIME);
    let close = c.create_close(&short, &0);
    p.observe(6, FILL);
    let fees = c.pending_fees(&short, &p.now());
    assert_eq!(
        c.settle_close(&p.keeper, &close),
        ActionOutcome::Executed,
        "§9.4 — the receiver must remain settleable; a revert here is the \
         omitted §4.5.1 reset"
    );

    let paid = p.cash(&shorter) - before;
    assert!(
        paid > stored - fees.borrow - REWARD,
        "§9.4 — the earned credit reaches the payout rather than being stranded"
    );
    assert!(
        c.pending_receiver_funding_total() >= 0,
        "§9.4 — and the liability does not underflow as it is drawn down"
    );
    p.assert_conserved("after the churned receiver settled");
}

/// §4.5 — "this makes many short checkpoints converge to the same result as
/// one long checkpoint." The carried-division pattern exists to make that
/// true; without it, frequent checkpointing would shave a sub-unit off every
/// slice and a keeper could change what a trader owes by choosing how often
/// to call `update_indices`.
#[test]
fn checkpoint_frequency_does_not_change_what_is_owed() {
    let owed = |step: u64| {
        let p = Protocol::new();
        let (long, _, _) = two_sided(&p);
        p.accrue(7_200, step, FILL);
        let f = p.pm().pending_fees(&long, &p.now());
        (f.funding_paid_to_receivers + f.funding_paid_to_lps, f.borrow)
    };

    let (funding_coarse, borrow_coarse) = owed(7_200);
    let (funding_fine, borrow_fine) = owed(300);

    assert_eq!(
        funding_fine, funding_coarse,
        "§4.5 — twenty-four checkpoints owe what one does"
    );
    assert_eq!(
        borrow_fine, borrow_coarse,
        "§4.5 — the global borrow carry behaves the same way"
    );
}
