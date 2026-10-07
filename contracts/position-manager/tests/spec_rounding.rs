//! §2.11 and §9.15 — "rounding never favours the trader."
//!
//! The consequential entry in that table is the base-exposure conversion,
//! because it is the one a trader can pick the inputs for: choose a size and
//! a price whose division has a remainder, and a conversion that rounded the
//! convenient way would hand out free PnL at the moment of opening, on every
//! position, forever.
//!
//! The partial-decrease split is the table's one documented exception, and it
//! is a conservation rule rather than a direction rule — so it gets tested for
//! conservation instead.
//!
//! Written from `shared`'s interfaces and the specification (§2.6, §2.8,
//! §2.11, §9.15).

mod spec_harness;

use spec_harness::*;

use shared::constants::PRICE_PRECISION;
use shared::{defaults, ActionOutcome};
use soroban_sdk::{testutils::Address as _, Address};

/// `$100,000` of size at `$30,000` is `3.333…` units: `1e19 / 3e11` leaves a
/// remainder, so the two rounding directions are one unit apart and visible.
const AWKWARD: i128 = usd_const(30_000);

fn open_at(p: &Protocol, owner: &Address, is_long: bool, price: i128) -> u64 {
    p.publish(price);
    let mut order = p.open_payload(300);
    order.is_long = is_long;
    order.size = SIZE;
    order.submitted_collateral = SUBMITTED;
    order.acceptable_price = 0;
    let action = p.pm().create_market_open(owner, &p.market, &order);
    p.observe(6, price);
    assert_eq!(
        p.pm().settle_market_open(&p.keeper, &action),
        ActionOutcome::Executed
    );
    p.claim_position_id()
}

/// §2.6 — "long exposure rounds down and short exposure rounds up so
/// conversion rounding cannot give the trader free PnL."
#[test]
fn base_exposure_rounds_against_the_trader_in_both_directions() {
    let p = Protocol::new();
    let c = p.pm();
    let bull = Address::generate(&p.env);
    let bear = Address::generate(&p.env);
    p.mint(&bull, usd(50_000));
    p.mint(&bear, usd(50_000));

    let long = open_at(&p, &bull, true, AWKWARD);
    let short = open_at(&p, &bear, false, AWKWARD);

    let exact = SIZE * PRICE_PRECISION / AWKWARD;
    assert!(
        SIZE * PRICE_PRECISION % AWKWARD != 0,
        "the fixture is only meaningful where the division has a remainder"
    );

    let long_base = c.get_position(&long).base_exposure;
    let short_base = c.get_position(&short).base_exposure;
    assert_eq!(long_base, exact, "§2.6 — long base is floored");
    assert_eq!(short_base, exact + 1, "§2.6 — short base is ceiled");
    assert_eq!(short_base - long_base, 1, "one unit apart, in the vault's favour");
}

/// §2.8 and §2.11 together: marked at the price it opened at, neither
/// direction may show a profit. The conversion is where free PnL would be
/// created, and both roundings have to point the same way for that to hold.
#[test]
fn neither_direction_profits_from_the_conversion_itself() {
    let p = Protocol::new();
    let c = p.pm();
    let bull = Address::generate(&p.env);
    let bear = Address::generate(&p.env);
    p.mint(&bull, usd(50_000));
    p.mint(&bear, usd(50_000));

    let long = c.get_position(&open_at(&p, &bull, true, AWKWARD));
    let short = c.get_position(&open_at(&p, &bear, false, AWKWARD));

    // §2.8's two formulas, at the price the positions were opened at.
    let long_pnl = long.base_exposure * AWKWARD / PRICE_PRECISION - long.size;
    let buyback = short.base_exposure * AWKWARD;
    let short_pnl =
        short.size - (buyback + PRICE_PRECISION - 1) / PRICE_PRECISION;

    assert!(long_pnl < 0, "§2.11 — a long starts at a rounding disadvantage");
    assert!(short_pnl < 0, "§2.11 — and so does a short");
    assert!(
        long_pnl + short_pnl < 0,
        "§9.15 — the pair cannot be combined into a free position either"
    );
    p.assert_conserved("with both directions open at an awkward price");
}

/// §2.11's documented exception — "removed and remaining base always sum to
/// exactly the pre-reduction base ... a final close removes the remainder
/// exactly", so "repeated partial decreases cannot strand exposure or risk."
#[test]
fn repeated_partial_decreases_cannot_strand_exposure_or_risk() {
    let p = Protocol::new();
    let c = p.pm();
    let trader = Address::generate(&p.env);
    p.mint(&trader, usd(50_000));
    let id = open_at(&p, &trader, true, AWKWARD);

    let side_before = c.get_market(&p.market).long;
    assert_eq!(side_before.base_exposure, c.get_position(&id).base_exposure);

    // Three awkward slices, each leaving a sub-unit to be placed somewhere.
    for slice in [usd(31_111), usd(17_777), usd(9_133)] {
        p.observe(defaults::MIN_POSITION_LIFETIME, AWKWARD);
        let before = c.get_position(&id);
        let action = c.create_decrease(&id, &slice, &0);
        p.observe(6, AWKWARD);
        assert_eq!(c.settle_decrease(&p.keeper, &action), ActionOutcome::Executed);

        let after = c.get_position(&id);
        let side = c.get_market(&p.market).long;
        assert_eq!(after.size, before.size - slice, "§9.7 — size comes off exactly");
        assert_eq!(
            side.base_exposure, after.base_exposure,
            "§9.7 — the aggregate still equals the one live position"
        );
        assert_eq!(side.risk_units, after.risk_units);
        assert!(
            after.base_exposure < before.base_exposure,
            "each decrease removes base"
        );
        p.assert_conserved("mid-decrease");
    }

    // The final close takes whatever the three splits left behind.
    p.observe(defaults::MIN_POSITION_LIFETIME, AWKWARD);
    let close = c.create_close(&id, &0);
    p.observe(6, AWKWARD);
    assert_eq!(c.settle_close(&p.keeper, &close), ActionOutcome::Executed);

    let side = c.get_market(&p.market).long;
    assert_eq!(side.size_open_interest, 0, "§2.11 — nothing stranded");
    assert_eq!(side.base_exposure, 0, "§2.11 — not one sub-unit of base");
    assert_eq!(side.risk_units, 0);
    assert_eq!(side.stored_collateral_total, 0);
    assert_eq!(p.snapshot().total_risk_units, 0);
    p.assert_conserved("after three decreases and a close");
}
