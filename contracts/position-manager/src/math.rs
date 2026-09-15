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
use shared::fixed::{self, LN2};

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

/// §9.1 — risk units opened by `size` USD notional.
pub fn risk_added(env: &Env, size: i128, factor_bps: u32) -> i128 {
    mul_div_floor(env, size, factor_bps as i128, BPS)
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

/// Everything one §8.1 funding window resolves to.
pub struct FundingWindow {
    /// `∫ rate dt` over the window — `INDEX_PRECISION`-scaled bps·seconds;
    /// divide by `BPS × SECONDS_PER_DAY` for the per-unit-size index delta.
    pub weight: i128,
    /// Sign of `∫ I dt`: +1 longs pay, −1 shorts pay, 0 nobody.
    pub payer_sign: i128,
    /// The skew EMA at the window's end.
    pub ema_after: i128,
    /// The blended integral skew at the window's end (signed).
    pub integral_now: i128,
}

/// §8.1 — resolve one funding window over a constant book, in closed form:
/// `E(t) = S + (E₀−S)·d`, `I(t) = A + B·d` with `A = S`,
/// `B = (BPS−w)(E₀−S)/BPS`, and
/// `∫ rate dt = max_rate × (A²Δt + 2AB·J₁ + B²·J₂)` where
/// `J₁ = H/ln2·(1−d)` and `J₂ = H/(2ln2)·(1−d²)`. Exact integration means
/// checkpoint frequency cannot change accrued value beyond `d`'s
/// quantization (§3, tolerance-bounded).
#[allow(clippy::too_many_arguments)]
pub fn funding_window(
    env: &Env,
    long_base: i128,
    short_base: i128,
    ema: i128,
    instant_weight_bps: u32,
    half_life: u64,
    max_rate_bps_day: i128,
    elapsed: u64,
) -> FundingWindow {
    let p = INDEX_PRECISION;
    let s = skew_frac(env, long_base, short_base);
    let a = s;
    let b = mul_div_trunc(
        env,
        sub(env, BPS, instant_weight_bps as i128),
        sub(env, ema, s),
        BPS,
    );
    let dt = elapsed as i128;
    let h = half_life as i128;
    let d = decay(env, elapsed, half_life);
    // `H / ln 2` at INDEX_PRECISION, from the §2.1.2 constant: dividing by
    // `LN2` (which carries the IP factor) expresses the same quantity the
    // deleted `INV_LN2_NUM / INV_LN2_DEN` ratio did.
    let h_scaled = mul(env, h, p);
    let j1 = mul_div_floor(env, sub(env, p, d), h_scaled, LN2);
    let d2 = mul_div_floor(env, d, d, p);
    let j2 = mul_div_floor(env, sub(env, p, d2), h_scaled, mul(env, 2, LN2));
    let term1 = mul(env, mul_div_trunc(env, a, a, p), dt);
    let term2 = mul(env, 2, mul_div_trunc(env, b, mul_div_trunc(env, a, j1, p), p));
    let term3 = mul_div_trunc(env, b, mul_div_trunc(env, b, j2, p), p);
    let sum = core::cmp::max(add(env, add(env, term1, term2), term3), 0);
    let linear = add(env, mul(env, a, dt), mul_div_trunc(env, b, j1, p));
    FundingWindow {
        weight: mul(env, max_rate_bps_day, sum),
        payer_sign: linear.signum(),
        ema_after: add(env, s, mul_div_trunc(env, sub(env, ema, s), d, p)),
        integral_now: add(env, a, mul_div_trunc(env, b, d, p)),
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

/// §2.1.2 — the window decay factor `d = 2^(−elapsed/H)`, formed the way the
/// specification forms `d_end`: scale the elapsed fraction of a half-life to
/// `INDEX_PRECISION` first, then take `exp2_neg` of it. The primitive itself
/// no longer takes an `(elapsed, half_life)` pair.
pub fn decay(env: &Env, elapsed: u64, half_life: u64) -> i128 {
    let x = mul_div_floor(env, elapsed as i128, INDEX_PRECISION, half_life as i128);
    fixed::exp2_neg(env, x).unwrap_or_else(|| fail(env))
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

/// §11.5 — pro-rata remaining value after a partial close; the final close
/// removes the complete remainder so nothing strands (§7.1).
pub fn remaining(env: &Env, value: i128, old_size: i128, new_size: i128) -> i128 {
    if new_size == 0 {
        0
    } else {
        mul_div_floor(env, value, new_size, old_size)
    }
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
    fn funding_window_degenerates_to_constant_rate_at_full_instant_weight() {
        let e = env();
        // w = BPS → I == S: the weight is exactly max_rate × S² × Δt / P,
        // linear in Δt, so splitting an interval is exact.
        let (long, short) = (75, 25); // S = P/2
        let day = 86_400u64;
        let w = funding_window(&e, long, short, 0, BPS as u32, 43_200, 100, day);
        assert_eq!(w.payer_sign, 1);
        assert_eq!(w.weight, 100 * (INDEX_PRECISION / 4) * day as i128);
        assert_eq!(w.integral_now, INDEX_PRECISION / 2);
    }

    #[test]
    fn funding_window_ema_decays_toward_the_instant_skew() {
        let e = env();
        let h = 43_200u64;
        // Book flipped hard short (S = −P) with a fully-long memory (E = +P):
        // after one half-life the EMA sits at the midpoint, zero.
        let w = funding_window(&e, 0, 1_000, INDEX_PRECISION, 3_000, h, 100, h);
        assert_eq!(w.ema_after, 0);
        // Blend at the window end: (0.3×(−P) + 0.7×0) = −0.3P.
        assert_eq!(w.integral_now, -(INDEX_PRECISION * 3 / 10));
        // Early in the flip the longs (per the memory) still pay: the whole
        // first half-life's integral must stay long-pays only if the linear
        // part is positive — here w=0.3 pulls it short quickly, so the sign
        // follows ∫I dt, not the old book.
        let early = funding_window(&e, 0, 1_000, INDEX_PRECISION, 0, h, 100, 60);
        assert_eq!(
            early.payer_sign, 1,
            "with w=0 the memory alone decides the early payer"
        );
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
    fn remaining_is_pro_rata_and_final_close_removes_all() {
        let e = env();
        assert_eq!(remaining(&e, 100, 30, 10), 33);
        assert_eq!(remaining(&e, 100, 30, 0), 0);
        // base/risk conservation: removed = old - remaining, so a 1/3 close
        // of an odd value strands nothing at the end.
        let after = remaining(&e, 101, 3, 2);
        assert_eq!(after, 67);
        assert_eq!(remaining(&e, after, 2, 0), 0);
    }

    #[test]
    fn index_value_rounding_directions() {
        let e = env();
        let index = INDEX_PRECISION / 3; // 0.333... fee per unit
        assert_eq!(index_value_ceil(&e, 1, index), 1);
        assert_eq!(index_value_floor(&e, 1, index), 0);
    }












}
