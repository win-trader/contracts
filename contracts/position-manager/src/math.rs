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

use soroban_sdk::{panic_with_error, Env};

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
    shared::math::mul_div_floor(a, b, denominator).unwrap_or_else(|| fail(env))
}

pub fn mul_div_ceil(env: &Env, a: i128, b: i128, denominator: i128) -> i128 {
    shared::math::mul_div_ceil(a, b, denominator).unwrap_or_else(|| fail(env))
}

/// §7.1 — base exposure bought by `size` USD notional at `price`.
pub fn base_added(env: &Env, size: i128, price: i128) -> i128 {
    mul_div_floor(env, size, PRICE_PRECISION, price)
}

/// §9.1 — risk units opened by `size` USD notional.
pub fn risk_added(env: &Env, size: i128, factor_bps: u32) -> i128 {
    mul_div_floor(env, size, factor_bps as i128, BPS)
}

/// §11.1 — unsigned distance from balance in bps: 0 balanced, `BPS`
/// one-sided. The fee tier wants magnitude only; funding uses the signed
/// `skew_frac` below.
pub fn skew_abs(env: &Env, long_base: i128, short_base: i128) -> i128 {
    let total = add(env, long_base, short_base);
    if total == 0 {
        return 0;
    }
    mul_div_floor(env, (long_base - short_base).abs(), BPS, total)
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
    let diff = long_base - short_base;
    let magnitude = mul_div_floor(env, diff.abs(), INDEX_PRECISION, total);
    if diff < 0 {
        -magnitude
    } else {
        magnitude
    }
}

/// Signed `a × b / d`, truncated toward zero. `shared::math` operates on
/// magnitudes only (§16); the sign is recomposed here. `d` must be positive.
pub fn smul_div(env: &Env, a: i128, b: i128, d: i128) -> i128 {
    let magnitude = mul_div_floor(env, a.abs(), b.abs(), d);
    if (a < 0) != (b < 0) {
        -magnitude
    } else {
        magnitude
    }
}

/// `round(INDEX_PRECISION × 2^(−2^(−i)))` for `i = 1..=47` — the
/// square-and-multiply constants behind `exp2_neg`. Entry `i` satisfies
/// `T[i]² ≈ T[i−1] × INDEX_PRECISION`; both properties are unit-tested.
const EXP2_FRAC: [i128; 47] = [
    70_710_678_118_655,
    84_089_641_525_371,
    91_700_404_320_467,
    95_760_328_069_857,
    97_857_206_208_770,
    98_922_801_319_398,
    99_459_942_348_363,
    99_729_605_608_547,
    99_864_711_289_097,
    99_932_332_750_265,
    99_966_160_649_624,
    99_983_078_893_193,
    99_991_539_088_661,
    99_995_769_454_843,
    99_997_884_705_049,
    99_998_942_346_931,
    99_999_471_172_067,
    99_999_735_585_684,
    99_999_867_792_755,
    99_999_933_896_355,
    99_999_966_948_172,
    99_999_983_474_085,
    99_999_991_737_042,
    99_999_995_868_521,
    99_999_997_934_260,
    99_999_998_967_130,
    99_999_999_483_565,
    99_999_999_741_783,
    99_999_999_870_891,
    99_999_999_935_446,
    99_999_999_967_723,
    99_999_999_983_861,
    99_999_999_991_931,
    99_999_999_995_965,
    99_999_999_997_983,
    99_999_999_998_991,
    99_999_999_999_496,
    99_999_999_999_748,
    99_999_999_999_874,
    99_999_999_999_937,
    99_999_999_999_968,
    99_999_999_999_984,
    99_999_999_999_992,
    99_999_999_999_996,
    99_999_999_999_998,
    99_999_999_999_999,
    100_000_000_000_000,
];

/// `1/ln 2` as a fixed ratio, for the closed-form decay integrals.
const INV_LN2_NUM: i128 = 14_426_950_408_889_634;
const INV_LN2_DEN: i128 = 10_000_000_000_000_000;

/// §8.1 — `2^(−elapsed/half_life)` at `INDEX_PRECISION` scale: whole
/// half-lives as a right shift, the fractional part by square-and-multiply
/// over `EXP2_FRAC`. Splitting an interval multiplies out to the same value
/// up to table quantization (~1e-13 relative).
pub fn exp2_neg(env: &Env, elapsed: u64, half_life: u64) -> i128 {
    let whole = elapsed / half_life;
    if whole >= 47 {
        return 0;
    }
    let mut acc = INDEX_PRECISION;
    let mut r = (elapsed % half_life) as i128;
    let h = half_life as i128;
    for entry in EXP2_FRAC.iter() {
        if r == 0 || acc == 0 {
            break;
        }
        r = mul(env, r, 2);
        if r >= h {
            r -= h;
            acc = mul_div_floor(env, acc, *entry, INDEX_PRECISION);
        }
    }
    acc >> (whole as u32)
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
    let b = smul_div(
        env,
        sub(env, BPS, instant_weight_bps as i128),
        sub(env, ema, s),
        BPS,
    );
    let d = exp2_neg(env, elapsed, half_life);
    let dt = elapsed as i128;
    let h = half_life as i128;
    let j1 = mul_div_floor(env, sub(env, p, d), mul(env, h, INV_LN2_NUM), INV_LN2_DEN);
    let d2 = mul_div_floor(env, d, d, p);
    let j2 = mul_div_floor(
        env,
        sub(env, p, d2),
        mul(env, h, INV_LN2_NUM),
        mul(env, 2, INV_LN2_DEN),
    );
    let term1 = mul(env, smul_div(env, a, a, p), dt);
    let term2 = mul(env, 2, smul_div(env, b, smul_div(env, a, j1, p), p));
    let term3 = smul_div(env, b, smul_div(env, b, j2, p), p);
    let sum = core::cmp::max(add(env, add(env, term1, term2), term3), 0);
    let linear = add(env, mul(env, a, dt), smul_div(env, b, j1, p));
    FundingWindow {
        weight: mul(env, max_rate_bps_day, sum),
        payer_sign: linear.signum(),
        ema_after: add(env, s, smul_div(env, sub(env, ema, s), d, p)),
        integral_now: add(env, a, smul_div(env, b, d, p)),
    }
}

/// §8.1 — the blended integral skew right now: `(w·S + (BPS−w)·E) / BPS`.
pub fn integral_skew(env: &Env, skew: i128, ema: i128, instant_weight_bps: u32) -> i128 {
    let w = instant_weight_bps as i128;
    add(
        env,
        smul_div(env, skew, w, BPS),
        smul_div(env, ema, sub(env, BPS, w), BPS),
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

/// §9.2 — quadratic borrow rate: `base + variable * utilization² / BPS²`,
/// scaled by `INDEX_PRECISION` (bps/day inputs).
pub fn borrow_rate(env: &Env, base: i128, variable: i128, utilization: i128) -> i128 {
    let base_scaled = mul(env, base, INDEX_PRECISION);
    let variable_scaled = mul(env, variable, INDEX_PRECISION);
    let first = mul_div_floor(env, variable_scaled, utilization, BPS);
    add(
        env,
        base_scaled,
        mul_div_floor(env, first, utilization, BPS),
    )
}

/// §6.2 — vault-wide utilization in bps, capped at `BPS`. Zero risk is zero
/// utilization even with zero equity; nonzero risk on zero equity is `BPS`.
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

/// §11.1 — closing fee on removed size, rounded up (§16).
pub fn closing_fee(env: &Env, size: i128, bps: u32) -> i128 {
    mul_div_ceil(env, size, bps as i128, BPS)
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
    fn skew_is_zero_when_balanced_and_bps_when_one_sided() {
        let e = env();
        assert_eq!(skew_abs(&e, 0, 0), 0);
        assert_eq!(skew_abs(&e, 500, 500), 0);
        assert_eq!(skew_abs(&e, 1_000, 0), BPS);
        assert_eq!(skew_abs(&e, 0, 1_000), BPS);
        // 75/25 split → |50| * BPS / 100 = 5_000
        assert_eq!(skew_abs(&e, 75, 25), 5_000);
    }

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
    fn exp2_frac_table_matches_its_defining_recurrences() {
        let e = env();
        // T[0] = 2^(-1/2): squaring it recovers 1/2 within 1 ulp.
        let half = mul_div_floor(&e, EXP2_FRAC[0], EXP2_FRAC[0], INDEX_PRECISION);
        assert!((half - INDEX_PRECISION / 2).abs() <= 1, "T[1]^2 != 1/2");
        // Telescoping: T[i]^2 == T[i-1] within rounding, for every entry.
        for i in 1..EXP2_FRAC.len() {
            let squared = mul_div_floor(&e, EXP2_FRAC[i], EXP2_FRAC[i], INDEX_PRECISION);
            assert!(
                (squared - EXP2_FRAC[i - 1]).abs() <= 2,
                "table entry {} breaks the telescoping recurrence",
                i
            );
        }
    }

    #[test]
    fn exp2_neg_halves_per_half_life_and_splits_cleanly() {
        let e = env();
        let h = 43_200u64;
        assert_eq!(exp2_neg(&e, 0, h), INDEX_PRECISION);
        assert_eq!(exp2_neg(&e, h, h), INDEX_PRECISION / 2);
        assert_eq!(exp2_neg(&e, 2 * h, h), INDEX_PRECISION / 4);
        assert_eq!(exp2_neg(&e, 47 * h, h), 0, "underflow clamps to zero");
        // Quarter half-life: 2^(-1/4) = 0.8408964...
        let q = exp2_neg(&e, h / 4, h);
        assert!((q - 84_089_641_525_371).abs() <= 5, "2^(-1/4) off: {q}");
        // Split-invariance up to quantization: d(a)·d(b) ≈ d(a+b).
        let a = 10_000u64;
        let b = 25_000u64;
        let joined = exp2_neg(&e, a + b, h);
        let split = mul_div_floor(&e, exp2_neg(&e, a, h), exp2_neg(&e, b, h), INDEX_PRECISION);
        assert!(
            (joined - split).abs() <= 50,
            "split {split} vs joined {joined}"
        );
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
    fn borrow_rate_is_base_plus_quadratic_variable() {
        let e = env();
        assert_eq!(borrow_rate(&e, 100, 900, 0), 100 * INDEX_PRECISION);
        assert_eq!(
            borrow_rate(&e, 100, 900, BPS),
            (100 + 900) * INDEX_PRECISION
        );
        assert_eq!(
            borrow_rate(&e, 100, 900, BPS / 2),
            100 * INDEX_PRECISION + 900 * INDEX_PRECISION / 4
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
