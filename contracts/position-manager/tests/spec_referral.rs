//! §3.6, §7.15, §7.16 — referral revenue.
//!
//! A fee carve-out plus a pull-claim path: two things that move money, and
//! neither had a single line of coverage. The property that matters is where
//! the share comes from — §3.6 says it "is carved entirely from the protocol
//! portion and never reduces the LP share", which is a claim about three
//! labels at once and cannot be checked by looking at the referrer's balance
//! alone. So the central test runs the same close twice, with and without a
//! referrer, and compares all three.
//!
//! Written from `shared`'s interfaces and the specification.

mod spec_harness;

use spec_harness::*;

use shared::{defaults, ActionOutcome};
use soroban_sdk::{testutils::Address as _, Address, Symbol};

/// 2 BTC marked at $55,000 against $100,000 of size is $10,000 of profit, so
/// §3.2's PnL component dominates and the collected fee is large enough that
/// a 250 bps slice of it is unambiguous.
const EXIT: i128 = usd_const(55_000);

fn code(p: &Protocol, text: &str) -> Symbol {
    Symbol::new(&p.env, text)
}

/// Open, hold, and close one profitable position. Returns the protocol claim,
/// the referrer's balance, and cash LP equity afterwards.
fn profitable_close(referred: bool) -> (i128, i128, i128) {
    let p = Protocol::new();
    let c = p.pm();
    let referrer = Address::generate(&p.env);
    let id = p.open_position();

    if referred {
        c.register_referral_code(&referrer, &code(&p, "ALICE"));
        c.set_referrer(&p.trader, &code(&p, "ALICE"));
    }

    p.wait(defaults::MIN_POSITION_LIFETIME);
    let close = c.create_close(&id, &0);
    p.observe(6, EXIT);
    assert_eq!(c.settle_close(&p.keeper, &close), ActionOutcome::Executed);

    p.assert_conserved("after the profitable close");
    (
        c.protocol_claimable_total(),
        c.referral_balance(&referrer),
        p.snapshot().cash_lp_equity,
    )
}

/// §3.6 — "carved entirely from the protocol portion and never reduces the LP
/// share." Two otherwise identical worlds, so the only difference is the
/// referrer.
#[test]
fn a_referrers_share_is_carved_from_protocol_and_never_from_the_lps() {
    let (protocol_alone, none, equity_alone) = profitable_close(false);
    let (protocol_shared, referral, equity_shared) = profitable_close(true);

    assert_eq!(none, 0, "no referrer, no balance");
    assert!(referral > 0, "§3.6 — a profitable close pays the referrer");
    assert_eq!(
        protocol_shared,
        protocol_alone - referral,
        "§3.6 — the slice comes out of the protocol portion, exactly"
    );
    assert_eq!(
        equity_shared, equity_alone,
        "§3.6 — and the LP share is untouched by it"
    );
}

/// §7.15 — "register an immutable code owner ... require code is valid and
/// unregistered", and "one address may own several codes".
#[test]
fn a_referral_code_is_first_come_and_its_owner_immutable() {
    let p = Protocol::new();
    let c = p.pm();
    let first = Address::generate(&p.env);
    let squatter = Address::generate(&p.env);

    c.register_referral_code(&first, &code(&p, "TAKEN"));
    assert_eq!(c.referral_code_owner(&code(&p, "TAKEN")), Some(first.clone()));

    assert!(
        c.try_register_referral_code(&squatter, &code(&p, "TAKEN")).is_err(),
        "§7.15 — the code is already owned, and ownership is immutable"
    );
    assert!(
        c.try_register_referral_code(&first, &code(&p, "TAKEN")).is_err(),
        "even for its own owner"
    );
    assert_eq!(c.referral_code_owner(&code(&p, "TAKEN")), Some(first.clone()));

    // One address may own several.
    c.register_referral_code(&first, &code(&p, "SECOND"));
    assert_eq!(c.referral_code_owner(&code(&p, "SECOND")), Some(first));
    assert_eq!(c.referral_code_owner(&code(&p, "NOBODY")), None);
}

/// §7.15 — "require referrer exists" and "require referrer != trader".
#[test]
fn a_trader_cannot_refer_themselves_or_point_at_nothing() {
    let p = Protocol::new();
    let c = p.pm();

    c.register_referral_code(&p.trader, &code(&p, "SELF"));
    assert!(
        c.try_set_referrer(&p.trader, &code(&p, "SELF")).is_err(),
        "§7.15 — self-referral is rejected"
    );
    assert!(
        c.try_set_referrer(&p.trader, &code(&p, "GHOST")).is_err(),
        "§7.15 — an unregistered code names no referrer"
    );
    assert_eq!(c.get_referrer(&p.trader), None);
}

/// §7.15 — "changing the mapping affects only fees collected afterward. It
/// does not move or recalculate previously accrued referral balances."
#[test]
fn changing_a_referrer_leaves_already_accrued_balances_alone() {
    let p = Protocol::new();
    let c = p.pm();
    let first = Address::generate(&p.env);
    let second = Address::generate(&p.env);
    c.register_referral_code(&first, &code(&p, "FIRST"));
    c.register_referral_code(&second, &code(&p, "SECOND"));

    let one = p.open_position();
    let two = p.open(&p.trader.clone(), true, SIZE, SUBMITTED);
    c.set_referrer(&p.trader, &code(&p, "FIRST"));

    p.wait(defaults::MIN_POSITION_LIFETIME);
    let close = c.create_close(&one, &0);
    p.observe(6, EXIT);
    c.settle_close(&p.keeper, &close);
    let earned = c.referral_balance(&first);
    assert!(earned > 0);

    // Switch, then collect another fee.
    c.set_referrer(&p.trader, &code(&p, "SECOND"));
    p.wait(defaults::MIN_POSITION_LIFETIME);
    let close = c.create_close(&two, &0);
    p.observe(6, EXIT);
    c.settle_close(&p.keeper, &close);

    assert_eq!(
        c.referral_balance(&first),
        earned,
        "§7.15 — the previous referrer keeps exactly what was already accrued"
    );
    assert!(c.referral_balance(&second) > 0, "and the new one earns the next fee");
    assert_eq!(
        c.referral_claimable_total(),
        c.referral_balance(&first) + c.referral_balance(&second),
        "§9.1 — referral_claimable_total = sum(referral_balance)"
    );
    p.assert_conserved("with two referrers holding balances");
}

/// §7.16 — the claim zeroes the balance, reduces the claim total, and moves
/// the cash. §12.2 adds that it stays available while paused: "referral
/// balances are ordinary user funds and are not withheld."
#[test]
fn referral_revenue_is_claimable_and_survives_a_pause() {
    let p = Protocol::new();
    let c = p.pm();
    let referrer = Address::generate(&p.env);
    c.register_referral_code(&referrer, &code(&p, "ALICE"));
    c.set_referrer(&p.trader, &code(&p, "ALICE"));

    let id = p.open_position();
    p.wait(defaults::MIN_POSITION_LIFETIME);
    let close = c.create_close(&id, &0);
    p.observe(6, EXIT);
    c.settle_close(&p.keeper, &close);

    let owed = c.referral_balance(&referrer);
    assert!(owed > 0);
    assert_eq!(c.referral_claimable_total(), owed);

    // §12.2 — "claim referral revenue: allowed" while paused; protocol is not.
    c.pause(&p.admin);
    assert!(c.try_claim_protocol(&p.admin, &p.admin, &1).is_err());
    c.claim_referral(&referrer);

    assert_eq!(p.cash(&referrer), owed, "§7.16 — the cash reaches the referrer");
    assert_eq!(c.referral_balance(&referrer), 0, "§7.16 — the balance is zeroed");
    assert_eq!(
        c.referral_claimable_total(),
        0,
        "§7.16 — and leaves the claim total with it"
    );
    assert!(
        c.try_claim_referral(&referrer).is_err(),
        "§7.16 — require amount > 0; a second claim finds nothing"
    );
    p.assert_conserved("after the referral claim");
}

/// §3.6 — "there is no referral reward when the applicable fee is zero,
/// waived, or uncollected." A losing close collects no closing fee (§9.13),
/// and the default `open_fee_bps` is zero, so nothing accrues either way.
#[test]
fn a_losing_close_pays_the_referrer_nothing() {
    let p = Protocol::new();
    let c = p.pm();
    let referrer = Address::generate(&p.env);
    c.register_referral_code(&referrer, &code(&p, "ALICE"));
    c.set_referrer(&p.trader, &code(&p, "ALICE"));

    let id = p.open_position();
    assert_eq!(
        c.referral_balance(&referrer),
        0,
        "§3.6 — open_fee_bps is zero, so the open carries no referral either"
    );

    p.wait(defaults::MIN_POSITION_LIFETIME);
    let close = c.create_close(&id, &0);
    p.observe(6, usd(49_000));
    assert_eq!(c.settle_close(&p.keeper, &close), ActionOutcome::Executed);

    assert_eq!(
        c.referral_balance(&referrer),
        0,
        "§3.6 — no fee collected, no referral revenue"
    );
    p.assert_conserved("after a losing close by a referred trader");
}
