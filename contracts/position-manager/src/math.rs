//! Typed wrappers over `shared::math` plus the protocol's fee/rate formulas.
//!
//! Every function panics with `PositionManagerError::ArithmeticError` on
//! overflow or domain violation so failures carry this contract's error code.
//!
//! Units (see `shared::constants` for the scale table):
//! - cash amounts and USD notionals: token units at `PRICE_PRECISION`
//! - prices: USD at `PRICE_PRECISION`
//! - rates: bps/day scaled by `INDEX_PRECISION`
//! - indices: fee per unit of fee base, scaled by `INDEX_PRECISION`

use soroban_sdk::{panic_with_error, Env, U256};

use shared::constants::{BPS, INDEX_PRECISION, PRICE_PRECISION};

use crate::errors::PositionManagerError;

fn fail(env: &Env) -> ! {
    panic_with_error!(env, PositionManagerError::ArithmeticError)
}

pub fn add(env: &Env, a: i128, b: i128) -> i128 {
    shared::math::add(a, b).unwrap_or_else(|| fail(env))
}

pub fn sub(env: &Env, a: i128, b: i128) -> i128 {
    shared::math::sub(a, b).unwrap_or_else(|| fail(env))
}

pub fn mul(env: &Env, a: i128, b: i128) -> i128 {
    shared::math::mul(a, b).unwrap_or_else(|| fail(env))
}

pub fn mul_div_floor(env: &Env, a: i128, b: i128, denominator: i128) -> i128 {
    shared::math::mul_div_floor(env, a, b, denominator).unwrap_or_else(|| fail(env))
}

pub fn mul_div_ceil(env: &Env, a: i128, b: i128, denominator: i128) -> i128 {
    shared::math::mul_div_ceil(env, a, b, denominator).unwrap_or_else(|| fail(env))
}

/// §2.1.1 — signed `a × b / d`, truncating toward zero. Replaces the
/// deleted local `smul_div` (P1-09/P2-03): the sign handling moves into the
/// 256-bit helper instead of being recomposed from magnitudes here.
pub fn mul_div_trunc(env: &Env, a: i128, b: i128, denominator: i128) -> i128 {
    shared::math::mul_div_trunc(env, a, b, denominator).unwrap_or_else(|| fail(env))
}

/// §2.1.1 — one accumulator advance with its remainder carried. The
/// numerator is 256-bit; see `shared::math::carried_div`.
pub fn carried_div(env: &Env, numerator: &U256, divisor: i128, remainder: i128) -> (i128, i128) {
    shared::math::carried_div(env, numerator, divisor, remainder).unwrap_or_else(|| fail(env))
}

/// Widen a non-negative `i128` for a `carried_div` numerator.
pub fn widen(env: &Env, value: i128) -> U256 {
    shared::math::widen(env, value).unwrap_or_else(|| fail(env))
}

/// §2.1.1 — `floor(n × b / d)` for a numerator that is already 256-bit and
/// a result that is still one. §6.2 splits a funding weight in proportion to
/// base exposure before dividing it into an index; narrowing it at that
/// point would overflow.
pub fn wide_mul_div_floor(env: &Env, n: &U256, b: i128, d: i128) -> U256 {
    shared::math::wide_mul_div_floor(env, n, b, d).unwrap_or_else(|| fail(env))
}

/// §2.1.1 — `ceil(a × b × c / d)` with the triple product formed at 256
/// bits. §6.7's minimum borrow fee is the one formula with three
/// multiplicands on the way to a division.
pub fn mul_mul_div_ceil(env: &Env, a: i128, b: i128, c: i128, d: i128) -> i128 {
    shared::math::mul_mul_div_ceil(env, a, b, c, d).unwrap_or_else(|| fail(env))
}

/// The 256-bit product `a × b`, for a `carried_div` numerator. §2.1.1
/// requires every product on the way to a division to be formed at 256 bits;
/// this is the form for the ones whose quotient is taken by `carried_div`
/// rather than by a `mul_div_*` helper.
pub fn widen_mul(env: &Env, a: i128, b: i128) -> U256 {
    widen(env, a).mul(&widen(env, b))
}

/// §7.1 — base exposure bought by `size` USD notional at `price`. Longs
/// floor (marginally less exposure per dollar); shorts ceil (marginally
/// more buyback owed) — rounding never favors the trader (§16). A floored
/// short base would hand the trader up to one base unit of free PnL at
/// close.
pub fn base_added(env: &Env, size: i128, price: i128, is_long: bool) -> i128 {
    if is_long {
        mul_div_floor(env, size, PRICE_PRECISION, price)
    } else {
        mul_div_ceil(env, size, PRICE_PRECISION, price)
    }
}

/// §2.7 — risk units for `size` USD notional.
pub fn risk_units_for(env: &Env, size: i128, factor_bps: u32) -> i128 {
    mul_div_floor(env, size, factor_bps as i128, BPS)
}

/// §6.13 — what adding `size_added` at `price` contributes.
#[derive(Clone, Copy, Debug)]
pub struct AddedExposure {
    pub base_added: i128,
    pub risk_added: i128,
}

/// §6.13 `derive_added_exposure` — re-derive risk from the **complete
/// resulting size**, not by accumulating independently rounded tranches.
///
/// `risk_units` is a function of size; deriving each increase's contribution
/// separately and summing lets the stored value drift from the value the
/// resulting size implies, and §5.11's aggregate equality is what that
/// drift breaks.
///
/// The two positivity requirements are what make a dust increase impossible
/// rather than merely unprofitable: a size add too small to move either
/// quantity would otherwise consume the position's one pending-mutation slot
/// for nothing.
pub fn derive_added_exposure(
    env: &Env,
    is_long: bool,
    current_size: i128,
    current_risk_units: i128,
    size_added: i128,
    price: i128,
    factor_bps: u32,
) -> AddedExposure {
    if size_added <= 0 || price <= 0 {
        fail(env);
    }
    let base_added = base_added(env, size_added, price, is_long);
    let resulting_size = add(env, current_size, size_added);
    let risk_after = risk_units_for(env, resulting_size, factor_bps);
    if risk_after < current_risk_units {
        fail(env);
    }
    let risk_added = sub(env, risk_after, current_risk_units);
    if base_added <= 0 || risk_added <= 0 {
        fail(env);
    }
    AddedExposure {
        base_added,
        risk_added,
    }
}

// ---------------------------------------------------------------------------
// §8.1 EMA funding: signed skew, half-life decay, and the exact closed-form
// window integral of the quadratic rate. Between two checkpoints the book is
// constant, so the blended integral skew is `I(t) = A + B·d(t)` with
// `d = 2^(−t/H)` — every window quantity below follows from that.
// ---------------------------------------------------------------------------

/// §8.1 — signed skew as a fraction of one at `INDEX_PRECISION` scale:
/// positive when longs dominate, zero on an empty book.
pub fn skew_frac(env: &Env, long_base: i128, short_base: i128) -> i128 {
    let total = add(env, long_base, short_base);
    if total == 0 {
        return 0;
    }
    let diff = sub(env, long_base, short_base);
    let magnitude = mul_div_floor(env, diff.abs(), INDEX_PRECISION, total);
    if diff < 0 {
        -magnitude
    } else {
        magnitude
    }
}

/// §8.1 — the blended integral skew right now: `(w·S + (BPS−w)·E) / BPS`.
pub fn integral_skew(env: &Env, skew: i128, ema: i128, instant_weight_bps: u32) -> i128 {
    let w = instant_weight_bps as i128;
    add(
        env,
        mul_div_trunc(env, skew, w, BPS),
        mul_div_trunc(env, ema, sub(env, BPS, w), BPS),
    )
}

/// §8.1 — instantaneous payer rate from a signed integral skew:
/// `max_rate × I²`, `INDEX_PRECISION`-scaled bps/day.
pub fn rate_from_integral(env: &Env, max_rate: i128, integral: i128) -> i128 {
    let magnitude = integral.abs();
    let max_scaled = mul(env, max_rate, INDEX_PRECISION);
    let first = mul_div_floor(env, max_scaled, magnitude, INDEX_PRECISION);
    mul_div_floor(env, first, magnitude, INDEX_PRECISION)
}

/// §6.2 — vault-wide utilization in bps, capped at `BPS`. Zero risk is zero
/// utilization even with zero equity; nonzero risk on zero equity is `BPS`.
/// Callerless between P1-07 and P4-02, which feeds it the fixed square.
#[allow(dead_code)]
pub fn utilization_bps(env: &Env, risk: i128, equity: i128) -> i128 {
    if risk == 0 {
        0
    } else if equity <= 0 {
        BPS
    } else {
        core::cmp::min(mul_div_floor(env, risk, BPS, equity), BPS)
    }
}

/// Payer obligations round up at the position boundary (§16).
pub fn index_value_ceil(env: &Env, base: i128, index: i128) -> i128 {
    mul_div_ceil(env, base, index, INDEX_PRECISION)
}

/// Receiver credits round down at the position boundary (§16).
pub fn index_value_floor(env: &Env, base: i128, index: i128) -> i128 {
    mul_div_floor(env, base, index, INDEX_PRECISION)
}

/// §7.2 — signed price PnL for `size`/`base` at `price`. Long profit rounds
/// down, short loss rounds up: rounding never favors the trader.
pub fn pnl(env: &Env, is_long: bool, size: i128, base: i128, price: i128) -> i128 {
    if is_long {
        sub(env, mul_div_floor(env, base, price, PRICE_PRECISION), size)
    } else {
        sub(env, size, mul_div_ceil(env, base, price, PRICE_PRECISION))
    }
}

/// §11.1 — closing fee as a share of the payable price profit, rounded
/// up (§16). Callerless between P1-06 and P5-12, which replaces it with
/// `max(size component, PnL component)`.
#[allow(dead_code)]
pub fn closing_fee(env: &Env, payable_profit: i128, bps: u32) -> i128 {
    mul_div_ceil(env, payable_profit, bps as i128, BPS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::constants::{BPS, INDEX_PRECISION, PRICE_PRECISION};

    fn env() -> Env {
        Env::default()
    }

    const UNIT: i128 = PRICE_PRECISION;


    #[test]
    fn rate_from_integral_is_quadratic() {
        let e = env();
        let max_rate = 100; // bps/day
        assert_eq!(rate_from_integral(&e, max_rate, 0), 0);
        assert_eq!(
            rate_from_integral(&e, max_rate, INDEX_PRECISION),
            max_rate * INDEX_PRECISION
        );
        assert_eq!(
            rate_from_integral(&e, max_rate, -INDEX_PRECISION),
            max_rate * INDEX_PRECISION,
            "the rate is unsigned; the sign picks the payer side"
        );
        // Half skew → quarter rate.
        assert_eq!(
            rate_from_integral(&e, max_rate, INDEX_PRECISION / 2),
            max_rate * INDEX_PRECISION / 4
        );
    }

    #[test]
    fn skew_frac_is_signed() {
        let e = env();
        assert_eq!(skew_frac(&e, 0, 0), 0);
        assert_eq!(skew_frac(&e, 75, 25), INDEX_PRECISION / 2);
        assert_eq!(skew_frac(&e, 25, 75), -INDEX_PRECISION / 2);
        assert_eq!(skew_frac(&e, 1_000, 0), INDEX_PRECISION);
        assert_eq!(skew_frac(&e, 0, 1_000), -INDEX_PRECISION);
    }






    #[test]
    fn utilization_edges() {
        let e = env();
        assert_eq!(utilization_bps(&e, 0, 0), 0);
        assert_eq!(utilization_bps(&e, 1, 0), BPS);
        assert_eq!(utilization_bps(&e, 50, 100), BPS / 2);
        // Capped at BPS even when risk exceeds equity.
        assert_eq!(utilization_bps(&e, 200, 100), BPS);
    }

    #[test]
    fn pnl_rounding_never_favors_the_trader() {
        let e = env();
        // 3 base units at price 1/3 above entry parity: long floor, short ceil.
        let size = 1;
        let base = 1;
        let price = UNIT + UNIT / 3;
        let long = pnl(&e, true, size, base, price);
        let short = pnl(&e, false, size, base, price);
        // long value floor(base*price/P) = 1 (dust profit floored away)
        assert_eq!(long, 0);
        // short owes ceil → loss of 1 recognized
        assert_eq!(short, -1);
    }

    #[test]
    fn closing_fee_rounds_up() {
        let e = env();
        // 1 unit at 10 bps → ceil(10_000_000 * 10 / 10_000) = 10_000 exact
        assert_eq!(closing_fee(&e, UNIT, 10), 10_000);
        // 1 stroop at 10 bps → ceil(10/10_000) = 1, never 0
        assert_eq!(closing_fee(&e, 1, 10), 1);
    }


    #[test]
    fn index_value_rounding_directions() {
        let e = env();
        let index = INDEX_PRECISION / 3; // 0.333... fee per unit
        assert_eq!(index_value_ceil(&e, 1, index), 1);
        assert_eq!(index_value_floor(&e, 1, index), 0);
    }












}
