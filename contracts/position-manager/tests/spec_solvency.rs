//! §3.9, §9.8, §9.10 — the vault under stress.
//!
//! Three claims the specification makes that only bind when the vault is
//! small enough for a test to actually hurt it, which is why they need a
//! thinly-seeded deployment rather than the comfortable default:
//!
//! - §9.10: liquidation "completes regardless" — the keeper reward degrades
//!   before the mechanism does. The earlier formulation turned an unpayable
//!   reward into a revert, "which would have blocked liquidation precisely in
//!   the state where an unliquidated position is most dangerous."
//! - §3.9: bad debt is reported explicitly and the position is removed; it
//!   "does not leave a negative-collateral position open."
//! - §9.8: no pending action reserves capacity, so two orders that each looked
//!   fillable are resolved by whichever settles first.
//!
//! The last test is the closest thing here to fuzzing: a deterministic
//! pseudo-random walk through the operation set, asserting §9.1 after every
//! step.
//!
//! Written from `shared`'s interfaces and the specification.

mod spec_harness;

use spec_harness::*;

use shared::{defaults, ActionOutcome};
use soroban_sdk::{testutils::Address as _, Address};

/// One `$100,000` position is `$10,000` of risk units, which needs about
/// `$11,765` of cash LP equity at the default `risk_capacity_limit_bps` of
/// 8,500. Twelve thousand admits exactly one and refuses the second.
const THIN: i128 = usd_const(12_000);

/// §9.10 — "liquidation therefore pays from remaining position collateral
/// first and uses LP residual equity only for the reward gap ... a keeper can
/// receive less than the configured amount, or nothing, when both sources are
/// empty. The liquidation completes regardless."
#[test]
fn liquidation_completes_even_when_nothing_is_left_to_pay_the_keeper() {
    let p = Protocol::with_lp_seed(THIN);
    let c = p.pm();
    let id = p.open_position();
    let keeper_before = p.cash(&p.keeper);

    // 2 BTC against $5,099.75 of collateral, marked at $40,000: a $20,000
    // loss. Collateral is gone and the hole is larger than the thin vault.
    p.observe(1, usd(40_000));
    assert!(
        c.try_liquidate_position(&p.keeper, &id).is_ok(),
        "§9.10 — the risk-removal path stays open at all times"
    );

    assert!(
        c.try_get_position(&id).is_err(),
        "§3.9 — the position is removed, not left open with negative collateral"
    );
    let earned = p.cash(&p.keeper) - keeper_before;
    assert!(
        earned <= defaults::KEEPER_REWARD,
        "§9.10 — at most the configured fixed reward, never a percentage"
    );
    assert!(earned >= 0, "§9.10 — and never negative");

    // §9.7 — nothing of the position survives in the aggregates.
    let side = c.get_market(&p.market).long;
    assert_eq!(side.size_open_interest, 0);
    assert_eq!(side.base_exposure, 0);
    assert_eq!(side.risk_units, 0);
    p.assert_conserved("after liquidating into a thin vault");
}

/// §9.1 — "it must not conceal the deficit with a negative unsigned balance,
/// a fabricated cash counter, or an untracked receivable", and §15.2:
/// recapitalization is the cure.
#[test]
fn a_shortfall_is_reported_and_recapitalisation_clears_it() {
    let p = Protocol::with_lp_seed(THIN);
    let c = p.pm();
    let id = p.open_position();

    p.observe(1, usd(35_000));
    c.liquidate_position(&p.keeper, &id);

    let s = p.snapshot();
    if s.cash_shortfall > 0 {
        assert_eq!(s.cash_lp_equity, 0, "§9.1 — a shortfall zeroes LP equity");
        assert_eq!(s.cash_shortfall, s.non_lp_claims - s.physical_cash);
        // §12.2 and §7.16 — an outgoing claim is blocked while short.
        assert!(c.try_claim_protocol(&p.admin, &p.admin, &1).is_err());

        // §15.2 — "transfer cash into the vault without minting shares ...
        // the cure for a cash shortfall." Open to anyone.
        let rescuer = Address::generate(&p.env);
        p.mint(&rescuer, usd(100_000));
        c.recapitalize(&rescuer, &usd(100_000));
        let after = p.snapshot();
        assert_eq!(after.cash_shortfall, 0, "§15.2 — the deficit is cured");
        assert!(after.cash_lp_equity > 0);
    }
    p.assert_conserved("after the shortfall was handled");
}

/// §9.8 and §8.8 — "no pending action reserves capacity. Competing actions
/// are checked in atomic settlement order, so a later action can fail even if
/// it appeared fillable when created."
#[test]
fn two_orders_that_each_fit_are_resolved_by_whichever_settles_first() {
    let p = Protocol::with_lp_seed(THIN);
    let c = p.pm();
    let first = Address::generate(&p.env);
    let second = Address::generate(&p.env);
    p.mint(&first, usd(20_000));
    p.mint(&second, usd(20_000));

    // Both commit while the vault can still back either one of them.
    p.publish(COMMIT_PRICE);
    let a = c.create_market_open(&first, &p.market, &p.open_payload(300));
    let b = c.create_market_open(&second, &p.market, &p.open_payload(300));
    assert_eq!(
        p.snapshot().total_risk_units,
        0,
        "§9.8 — neither commitment reserved anything"
    );

    p.observe(6, FILL);
    assert_eq!(
        c.settle_market_open(&p.keeper, &a),
        ActionOutcome::Executed,
        "the first through consumes the capacity"
    );
    assert_eq!(
        c.settle_market_open(&p.keeper, &b),
        ActionOutcome::Failed,
        "§8.8 — insufficient capacity is an expected terminal failure"
    );

    // §8.10 — the loser is refunded everything but the action reward.
    assert_eq!(p.cash(&second), usd(20_000) - usd(5_100) + usd(5_100) - REWARD);
    assert!(c.try_get_position(&2).is_err(), "§9.12 — and no position was made");
    assert_eq!(p.snapshot().open_position_count, 1);
    p.assert_conserved("after the capacity race");
}

/// A deterministic walk through the operation set, checking §9.1 after every
/// step. Not a fuzzer — the sequence is fixed, so a failure reproduces — but
/// it reaches interleavings no hand-written scenario would think to write.
#[test]
fn a_pseudo_random_sequence_of_operations_conserves_cash() {
    let p = Protocol::new();
    let c = p.pm();
    let traders: [Address; 3] = [
        Address::generate(&p.env),
        Address::generate(&p.env),
        Address::generate(&p.env),
    ];
    for t in &traders {
        p.mint(t, usd(500_000));
    }

    // A small LCG, so the walk is reproducible from this constant alone.
    let mut seed: u64 = 0x5DEE_CE66_D1CE_B00D;
    let mut next = move || {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        (seed >> 33) as u32
    };

    let mut live: Vec<u64> = Vec::new();
    let prices = [
        usd(50_000),
        usd(46_000),
        usd(58_000),
        usd(41_000),
        usd(63_000),
        usd(52_500),
    ];

    for step in 0..60u32 {
        let r = next();
        let price = prices[(r as usize / 7) % prices.len()];

        match r % 7 {
            0 | 1 => {
                // Open, long or short, on whichever trader comes up.
                let who = traders[(r as usize / 3) % traders.len()].clone();
                let is_long = r % 3 == 0;
                p.publish(COMMIT_PRICE);
                let mut order = p.open_payload(300);
                order.is_long = is_long;
                order.acceptable_price = 0;
                order.size = usd(20_000 + (r % 5) as i128 * 10_000);
                order.submitted_collateral = usd(4_000);
                if let Ok(Ok(action)) =
                    c.try_create_market_open(&who, &p.market, &order)
                {
                    p.observe(6, price);
                    if c.try_settle_market_open(&p.keeper, &action)
                        == Ok(Ok(ActionOutcome::Executed))
                    {
                        live.push(p.claim_position_id());
                    }
                }
            }
            2 => {
                if let Some(&id) = live.first() {
                    let _ = c.try_add_collateral(&id, &usd(250));
                }
            }
            3 => {
                // Try to close the oldest live position.
                if let Some(&id) = live.first() {
                    // The feed has to keep publishing across the lifetime
                    // wait, or the next read is older than
                    // `max_price_age_seconds` and every call refuses (§8.6).
                    p.observe(defaults::MIN_POSITION_LIFETIME, price);
                    if let Ok(Ok(action)) = c.try_create_close(&id, &0) {
                        p.observe(6, price);
                        match c.try_settle_close(&p.keeper, &action) {
                            Ok(Ok(ActionOutcome::Executed))
                            | Ok(Ok(ActionOutcome::Failed)) => {
                                live.remove(0);
                            }
                            _ => {}
                        }
                    }
                }
            }
            4 => {
                // Sweep anything the price move made liquidatable.
                let snapshot: Vec<u64> = live.clone();
                p.observe(1, price);
                for id in snapshot {
                    if c.try_liquidate_position(&p.keeper, &id).is_ok() {
                        live.retain(|&x| x != id);
                    }
                }
            }
            5 => {
                p.observe(300, price);
                c.update_indices(&p.keeper, &p.market);
            }
            _ => {
                p.observe(60, price);
            }
        }

        p.assert_conserved(&format!("step {step} (op {})", r % 7));
        assert!(
            c.pending_receiver_funding_total() >= 0,
            "§9.4 — at step {step}"
        );
        assert!(
            c.referral_claimable_total() == 0 && c.protocol_claimable_total() >= 0,
            "§9.2 — at step {step}"
        );
    }

    // §9.7 — unwind everything that survived and check nothing is stranded.
    for id in live.clone() {
        p.observe(defaults::MIN_POSITION_LIFETIME, usd(50_000));
        if let Ok(Ok(action)) = c.try_create_close(&id, &0) {
            p.observe(6, usd(50_000));
            let _ = c.try_settle_close(&p.keeper, &action);
        }
        let _ = c.try_liquidate_position(&p.keeper, &id);
    }
    p.assert_conserved("after unwinding the walk");

    let market = c.get_market(&p.market);
    if p.snapshot().open_position_count == 0 {
        assert_eq!(market.long.size_open_interest, 0, "§9.7 — no stranded long size");
        assert_eq!(market.short.size_open_interest, 0, "§9.7 — nor short");
        assert_eq!(market.long.base_exposure, 0);
        assert_eq!(market.short.base_exposure, 0);
        assert_eq!(market.long.risk_units, 0);
        assert_eq!(market.short.risk_units, 0);
    }
}
