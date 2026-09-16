//! §7.14 — automatic deleveraging.
//!
//! ADL and liquidation are the only two paths that take a position without
//! its owner's consent, and ADL is the one that takes a **winning** position.
//! Until now nothing in this repo had ever executed one successfully: every
//! reference to `execute_adl` asserted that it was refused.
//!
//! What actually bounds the mechanism is the state gate, re-evaluated from
//! the current book on every call — not an allowlist and not a ranking of
//! candidates. So the gate is what these tests spend their effort on.
//!
//! Written from `shared`'s interfaces and the specification (§7.14, §8.12,
//! §9.13, §3.6).

mod spec_harness;

use spec_harness::*;

use shared::constants::{INDEX_PRECISION, PRICE_PRECISION};
use shared::{defaults, ActionOutcome, RiskState};
use soroban_sdk::{testutils::Address as _, Address, Symbol};

/// Two `$100,000` longs are 4 BTC of base against `$200,000` of size. Marked
/// at `$63,750` that is `$55,000` of side profit on `$1,000,000` of LP
/// equity — 550 bps, inside `adl_pnl_factor_bps` (500) and below
/// `hard_cap_pnl_factor_bps` (600), so the side deleverages at an unscaled
/// payout factor.
const ADL_MARK: i128 = usd_const(63_750);

fn restricted_side(p: &Protocol) -> (u64, u64, Address, Address) {
    let a = Address::generate(&p.env);
    let b = Address::generate(&p.env);
    p.mint(&a, usd(50_000));
    p.mint(&b, usd(50_000));
    let first = p.open(&a, true, SIZE, SUBMITTED);
    let second = p.open(&b, true, SIZE, SUBMITTED);
    p.latch_risk_state(ADL_MARK);
    assert_eq!(
        p.pm().get_market(&p.market).long.risk_state,
        RiskState::Adl,
        "the side is in ADL, and not yet in HardCap"
    );
    (first, second, a, b)
}

/// §7.14 — the happy path, and §9.13: "liquidation and ADL never charge"
/// closing fees. The payout is therefore the waterfall with no fee term.
#[test]
fn adl_removes_a_profitable_position_and_charges_no_closing_fee() {
    let p = Protocol::new();
    let c = p.pm();
    let (target, survivor, owner, _) = restricted_side(&p);

    let pos = c.get_position(&target);
    let side_before = c.get_market(&p.market).long;
    let owner_before = p.cash(&owner);
    let keeper_before = p.cash(&p.keeper);
    let protocol_before = c.protocol_claimable_total();

    p.observe(1, ADL_MARK);
    let fees = c.pending_fees(&target, &p.now());
    let raw = pos.base_exposure * ADL_MARK / PRICE_PRECISION - pos.size;
    assert!(raw > 0, "§7.14 — ADL requires positive raw PnL");
    assert_eq!(
        side_before.hard_cap_payout_factor, INDEX_PRECISION,
        "§6.5 — an ADL side is not scaled; only HardCap is"
    );

    assert_eq!(
        c.execute_adl(&p.keeper, &target),
        ActionOutcome::Executed,
        "§7.14 — a profitable position on a restricted side is a candidate"
    );

    // "ADL pays no liquidation or close reward in addition to its own fixed
    //  reward."
    assert_eq!(
        p.cash(&p.keeper) - keeper_before,
        defaults::KEEPER_REWARD,
        "§8.11 — exactly one ADL reward"
    );

    // §9.13 — the waterfall without a closing fee.
    let expected = pos.stored_collateral + raw + fees.funding_received
        - fees.funding_paid_to_receivers
        - fees.funding_paid_to_lps
        - fees.borrow
        - defaults::KEEPER_REWARD;
    assert_eq!(
        p.cash(&owner) - owner_before,
        expected,
        "§9.13 — ADL charges no closing fee, so nothing is deducted for one"
    );

    // §9.7 — "remove complete exposure and position state".
    assert!(c.try_get_position(&target).is_err());
    let side = c.get_market(&p.market).long;
    assert_eq!(side.size_open_interest, side_before.size_open_interest - pos.size);
    assert_eq!(side.base_exposure, side_before.base_exposure - pos.base_exposure);
    assert_eq!(side.risk_units, side_before.risk_units - pos.risk_units);
    assert_eq!(c.get_position(&survivor).size, SIZE, "only the candidate went");

    // The closing-fee absence again, from the revenue side: the only protocol
    // accrual is borrow's share (§9.2 — borrow has no referral component).
    assert!(c.protocol_claimable_total() >= protocol_before);
    p.assert_conserved("after the ADL");
}

/// §7.14 — "each execution that removes profitable exposure lowers the side's
/// PnL factor, and once it falls below `adl_pnl_factor_bps` no further ADL is
/// permitted on that side. A keeper cannot keep taking positions after the
/// condition has cleared."
///
/// This is the whole bound on an unranked, permissionless mechanism.
#[test]
fn the_state_gate_closes_behind_the_first_deleverage() {
    let p = Protocol::new();
    let c = p.pm();
    let (first, second, _, _) = restricted_side(&p);

    p.observe(1, ADL_MARK);
    assert_eq!(c.execute_adl(&p.keeper, &first), ActionOutcome::Executed);

    // One position's profit was removed; the side is no longer over its gate.
    let paid = p.cash(&p.keeper);
    assert!(
        c.try_execute_adl(&p.keeper, &second).is_err(),
        "§7.14 — the gate is re-evaluated from the current book on every call"
    );
    assert_eq!(p.cash(&p.keeper), paid, "a refused ADL pays nothing");
    assert_eq!(c.get_position(&second).size, SIZE, "and takes nothing");
    assert_ne!(c.get_market(&p.market).long.risk_state, RiskState::Adl);
    p.assert_conserved("after the gate closed");
}

/// §7.14 — `require position_raw_pnl > 0`. A losing position on a restricted
/// side is not a candidate: deleveraging it would remove no liability.
#[test]
fn adl_refuses_a_losing_position_even_on_a_restricted_side() {
    let p = Protocol::new();
    let c = p.pm();
    let (_, _, _, _) = restricted_side(&p);

    // A short opened at the marked price is under water as the longs profit.
    let bear = Address::generate(&p.env);
    p.mint(&bear, usd(50_000));
    let losing = p.open(&bear, false, usd(50_000), usd(5_000));

    p.observe(1, ADL_MARK);
    assert!(
        c.try_execute_adl(&p.keeper, &losing).is_err(),
        "§7.14 — a position with non-positive raw PnL is not a candidate"
    );
    assert!(c.try_get_position(&losing).is_ok());
    p.assert_conserved("after refusing a losing candidate");
}

/// §8.12 — when a forced action removes a position it must "remove the
/// ordinary pending mutation, if present", "refund its complete added-
/// collateral escrow", and "pay only the forced action's keeper reward".
/// Covered for liquidation already; this is the ADL twin.
#[test]
fn adl_supersedes_a_pending_mutation_and_refunds_its_escrow() {
    let p = Protocol::new();
    let c = p.pm();
    let (target, _, owner, _) = restricted_side(&p);

    p.wait(defaults::MIN_POSITION_LIFETIME);
    let added = usd(1_000);
    let increase = c.create_increase(&target, &usd(10_000), &added, &0);
    assert_eq!(
        c.get_position(&target).pending_mutation_action_id,
        Some(increase)
    );
    let owner_before = p.cash(&owner);
    let keeper_before = p.cash(&p.keeper);

    p.observe(1, ADL_MARK);
    assert_eq!(c.execute_adl(&p.keeper, &target), ActionOutcome::Executed);

    assert!(
        c.try_get_pending_action(&increase).is_err(),
        "§8.12 — the superseded mutation is removed"
    );
    assert!(
        p.cash(&owner) - owner_before >= added,
        "§8.10 — the complete added-collateral escrow comes back"
    );
    assert_eq!(
        p.cash(&p.keeper) - keeper_before,
        defaults::KEEPER_REWARD,
        "§9.14 — only the forced action's reward, not the increase reward too"
    );
    p.assert_conserved("after ADL superseded the increase");
}

/// §3.6 — "borrow, funding, keeper rewards, liquidation, and ADL do not
/// generate referral revenue."
#[test]
fn adl_generates_no_referral_revenue() {
    let p = Protocol::new();
    let c = p.pm();
    let (target, _, owner, _) = restricted_side(&p);

    let referrer = Address::generate(&p.env);
    let code = Symbol::new(&p.env, "ADLCODE");
    c.register_referral_code(&referrer, &code);
    c.set_referrer(&owner, &code);
    assert_eq!(c.get_referrer(&owner), Some(referrer.clone()));

    p.observe(1, ADL_MARK);
    assert_eq!(c.execute_adl(&p.keeper, &target), ActionOutcome::Executed);

    assert_eq!(
        c.referral_balance(&referrer),
        0,
        "§3.6 — a forced deleverage pays the referrer nothing"
    );
    assert_eq!(c.referral_claimable_total(), 0);
    p.assert_conserved("after an ADL on a referred trader");
}
