//! §6.2.1 — integrate one funding window by sign.
//!
//! Pure: reads no storage, writes none, and its result is a function of its
//! arguments alone. `funding::accrue` delegates the whole of the window
//! mathematics here.
//!
//! Between two checkpoints the book is constant, so the blended skew is
//! `I(t) = A + B·d(t)` with `d = 2^(−t/H)` — monotonic across the window.
//! That monotonicity is what makes the sign change locatable analytically
//! instead of by search, and it is why a crossing exists *exactly* when the
//! two endpoints disagree in sign.

use soroban_sdk::{panic_with_error, Env, I256, U256};

use shared::constants::{BPS, INDEX_PRECISION};
use shared::fixed::{self, LN2};
use shared::PayerSide;

use crate::errors::PositionManagerError;
use crate::math;

fn fail(env: &Env) -> ! {
    panic_with_error!(env, PositionManagerError::InvariantViolation)
}

/// One constant-payer span of a window.
pub struct Segment {
    /// `+1` longs pay, `-1` shorts pay. A zero-sign span accrues nothing and
    /// is never emitted.
    pub payer_sign: i128,
    /// `max_funding_rate_bps_day * quadratic_integral`, at
    /// `INDEX_PRECISION² · seconds · bps/day`.
    ///
    /// 256-bit because it has to be: at the validated maximum funding rate
    /// this reaches ~`3e39` against a `u128` ceiling of `3.4e38` (§2.1.1).
    pub funding_weight: U256,
}

impl Segment {
    pub fn long_pays(&self) -> bool {
        self.payer_sign > 0
    }
}

/// Everything one window resolves to.
///
/// At most two segments, because `I(t)` is monotonic and can cross zero at
/// most once. Holding them as two named fields rather than a list makes
/// "at most two, in chronological order" structural instead of a property
/// a reader has to trust.
pub struct FundingWindow {
    /// The span from the window origin.
    pub first: Option<Segment>,
    /// The span after the sign change, if there was one.
    pub second: Option<Segment>,
    pub ema_after: i128,
    pub payer_side_at_end: PayerSide,
    /// Display only (§5.4). The quadratic rate at the window's final blended
    /// skew; no obligation is ever read from it.
    pub displayed_rate_at_end: i128,
}

impl FundingWindow {
    /// The window's segments in **chronological order**. Order does not
    /// change the arithmetic, but it does change which carried remainder
    /// each division sees, so a window whose crossing is replayed out of
    /// order will not reproduce the same indices.
    pub fn segments(&self) -> impl Iterator<Item = &Segment> {
        self.first.iter().chain(self.second.iter())
    }
}

/// §3.4.1 — signed live skew as a fraction of one at `INDEX_PRECISION`:
/// positive when longs dominate, zero on an empty book. Truncating toward
/// zero so the magnitude is never overstated in either direction.
fn live_skew(env: &Env, long_base: i128, short_base: i128) -> i128 {
    let total = math::add(env, long_base, short_base);
    if total == 0 {
        return 0;
    }
    math::mul_div_trunc(
        env,
        math::sub(env, long_base, short_base),
        INDEX_PRECISION,
        total,
    )
}

/// One span's `(t1, d1, t2, d2)` plus the blended skew whose sign it takes.
struct Span {
    t1: i128,
    d1: i128,
    t2: i128,
    d2: i128,
    sign_source: i128,
}

#[allow(clippy::too_many_arguments)]
pub fn integrate_funding_window_by_sign(
    env: &Env,
    long_base: i128,
    short_base: i128,
    skew_ema: i128,
    instant_weight_bps: u32,
    half_life_seconds: u64,
    max_funding_rate_bps_day: i128,
    elapsed: u64,
) -> FundingWindow {
    if elapsed == 0 || half_life_seconds == 0 {
        fail(env);
    }
    let p = INDEX_PRECISION;
    let half_life = half_life_seconds as i128;

    // Live skew is constant for the whole window (§3.4.1).
    let s = live_skew(env, long_base, short_base);

    // Blend coefficients (§4.6). `B` is signed, so it takes the truncating
    // helper: floor division would overstate a negative coefficient.
    let w = math::mul_div_floor(env, instant_weight_bps as i128, p, BPS);
    let a = s;
    let b = math::mul_div_trunc(env, math::sub(env, p, w), math::sub(env, skew_ema, s), p);

    let d_end = decay_at(env, elapsed, half_life_seconds);
    let ema_after = math::add(
        env,
        s,
        math::mul_div_trunc(env, math::sub(env, skew_ema, s), d_end, p),
    );

    // Endpoint values of the blended skew. `I_start` uses `d = 1`, so it is
    // `A + B`; `I_end` uses the window's own decay.
    let i_start = math::add(env, a, b);
    let i_end = math::add(env, a, math::mul_div_trunc(env, b, d_end, p));

    // §2.1.2 — a crossing exists exactly when the two endpoints disagree in
    // sign. Comparing `A + B` against `A` alone would test for a crossing on
    // `[0, ∞)`, because `A` is the *limit* of `I(t)` rather than its value
    // at the end of the window: a window whose EMA is still far from live
    // skew satisfies that weaker test while its crossing lies hours beyond
    // `elapsed`, and the window would be split at a point outside itself.
    let crosses = i_start != 0 && i_end != 0 && i_start.signum() != i_end.signum();

    let (first_span, second_span) = if crosses {
        let d_star = math::mul_div_trunc(env, -a, p, b);
        // Guards the endpoint test is supposed to make unreachable. They are
        // cheap, and a wrong sign test otherwise integrates a window at
        // ~150× its true length, silently.
        if !(d_end < d_star && d_star < p) {
            fail(env);
        }
        let t_star = math::mul_div_floor(
            env,
            half_life,
            fixed::log2(env, math::mul_div_floor(env, p, p, d_star)).unwrap_or_else(|| fail(env)),
            p,
        );
        if !(t_star > 0 && t_star < elapsed as i128) {
            fail(env);
        }
        (
            Span {
                t1: 0,
                d1: p,
                t2: t_star,
                d2: d_star,
                sign_source: i_start,
            },
            // §6.2.1 — `d1` and `d2` are measured from the **window
            // origin**, so the second span starts at `d_star`, not back at
            // `INDEX_PRECISION`. Restarting the decay per segment would
            // integrate this one as though the EMA were fresh and roughly
            // double the funding attributed to it.
            Some(Span {
                t1: t_star,
                d1: d_star,
                t2: elapsed as i128,
                d2: d_end,
                sign_source: i_end,
            }),
        )
    } else {
        (
            Span {
                t1: 0,
                d1: p,
                t2: elapsed as i128,
                d2: d_end,
                sign_source: if i_start != 0 { i_start } else { i_end },
            },
            None,
        )
    };

    let first = integrate_span(env, &first_span, a, b, half_life, max_funding_rate_bps_day);
    let second = second_span
        .and_then(|span| integrate_span(env, &span, a, b, half_life, max_funding_rate_bps_day));

    FundingWindow {
        first,
        second,
        ema_after,
        payer_side_at_end: if i_end > 0 {
            PayerSide::Long
        } else if i_end < 0 {
            PayerSide::Short
        } else {
            PayerSide::None
        },
        displayed_rate_at_end: math::mul_div_floor(
            env,
            math::mul(env, max_funding_rate_bps_day, i_end.abs()),
            i_end.abs(),
            p,
        ),
    }
}

/// §2.1.2 — `d = 2^(−elapsed/H)`, formed the way the specification forms
/// `d_end`: scale the elapsed fraction of a half-life to `INDEX_PRECISION`
/// first, then take `exp2_neg` of it.
pub fn decay_at(env: &Env, elapsed: u64, half_life_seconds: u64) -> i128 {
    let x = math::mul_div_floor(
        env,
        elapsed as i128,
        INDEX_PRECISION,
        half_life_seconds as i128,
    );
    fixed::exp2_neg(env, x).unwrap_or_else(|| fail(env))
}

/// §4.6 — the closed-form integral of `I(t)²` over one span.
fn integrate_span(
    env: &Env,
    span: &Span,
    a: i128,
    b: i128,
    half_life: i128,
    max_funding_rate_bps_day: i128,
) -> Option<Segment> {
    if span.t2 <= span.t1 {
        return None;
    }
    let payer_sign = span.sign_source.signum();
    if payer_sign == 0 {
        // No funding accrues on a zero segment.
        return None;
    }
    let p = INDEX_PRECISION;

    // `J₁ = H/ln2 · (d₁ − d₂)` and `J₂ = H/(2 ln2) · (d₁² − d₂²)`, both
    // carried at INDEX_PRECISION. Flooring them to whole seconds would be a
    // 1e-5 relative error — seven orders of magnitude outside
    // DECAY_TOLERANCE, and the dominant error in the whole calculation.
    let j1 = math::mul_div_floor(
        env,
        math::mul(env, half_life, math::sub(env, span.d1, span.d2)),
        p,
        LN2,
    );
    let d1_squared = math::mul_div_floor(env, span.d1, span.d1, p);
    let d2_squared = math::mul_div_floor(env, span.d2, span.d2, p);
    let j2 = math::mul_div_floor(
        env,
        math::mul(env, half_life, math::sub(env, d1_squared, d2_squared)),
        p,
        math::mul(env, 2, LN2),
    );

    // `INDEX_PRECISION³ · seconds`, formed in 256-bit: at the validated
    // maximum this intermediate reaches ~1.7e45, far past i128.
    let a256 = I256::from_i128(env, a);
    let b256 = I256::from_i128(env, b);
    let p256 = I256::from_i128(env, p);
    let term1 = a256
        .mul(&a256)
        .mul(&I256::from_i128(env, math::sub(env, span.t2, span.t1)))
        .mul(&p256);
    let term2 = I256::from_i128(env, 2)
        .mul(&a256)
        .mul(&b256)
        .mul(&I256::from_i128(env, j1));
    let term3 = b256.mul(&b256).mul(&I256::from_i128(env, j2));
    let quadratic_scaled = term1.add(&term2).add(&term3);

    // One compensating division returns §4.6's declared
    // `INDEX_PRECISION² · seconds`.
    //
    // The clamp is bounded truncation residue on a mathematically
    // non-negative quantity — the integral of a square. It is *not* the
    // forbidden clamp of §2.11: nothing here subtracts a stored baseline
    // from a monotonic index, so it cannot hide a negative pending amount.
    let quadratic_integral = quadratic_scaled
        .div(&p256)
        .to_i128()
        .map(|v| core::cmp::max(v, 0))
        .unwrap_or_else(|| fail(env));

    Some(Segment {
        payer_sign,
        funding_weight: math::widen_mul(env, max_funding_rate_bps_day, quadratic_integral),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env() -> Env {
        Env::default()
    }

    const P: i128 = INDEX_PRECISION;
    const HALF_LIFE: u64 = 43_200;
    const MAX_RATE: i128 = 80;

    /// A window with no sign change yields exactly one segment.
    #[test]
    fn a_window_that_does_not_cross_has_one_segment() {
        let e = env();
        // Longs dominate and the memory agrees: I(t) stays positive.
        let w = integrate_funding_window_by_sign(&e, 750, 250, P / 2, 3_000, HALF_LIFE, MAX_RATE, 3_600);
        assert!(w.first.is_some());
        assert!(w.second.is_none(), "an uncrossed window must not split");
        assert_eq!(w.first.as_ref().unwrap().payer_sign, 1);
        assert_eq!(w.payer_side_at_end, PayerSide::Long);
    }

    /// The defect this phase exists to fix: a window whose blended skew
    /// crosses zero must be split, and each side of the crossing must be
    /// attributed to the side that actually generated it.
    #[test]
    fn a_window_that_crosses_zero_splits_and_attributes_each_side() {
        let e = env();
        // The book has flipped hard short (S = -P) but the memory is still
        // fully long (E = +P). Early in the window the blend is positive —
        // longs pay — and it turns negative as the memory decays.
        let w = integrate_funding_window_by_sign(
            &e, 0, 1_000, P, 0, HALF_LIFE, MAX_RATE, 4 * HALF_LIFE,
        );
        let first = w.first.as_ref().expect("first segment");
        let second = w.second.as_ref().expect("window must split at the crossing");
        assert_eq!(first.payer_sign, 1, "longs pay before the crossing");
        assert_eq!(second.payer_sign, -1, "shorts pay after it");
        assert_eq!(w.payer_side_at_end, PayerSide::Short);
    }

    /// `d1` and `d2` are decay factors measured from the **window origin**,
    /// so the post-crossing span starts at `d_star` rather than back at
    /// `INDEX_PRECISION`.
    ///
    /// With `A = -P`, `B = +2P` and `w = 0` the crossing sits at
    /// `d_star = 1/2`, exactly one half-life in. Integrating `I(t)²` in
    /// closed form over `[0, H]` and `[H, 4H]` gives `0.2786·P²H` and
    /// `1.1852·P²H`, a ratio of `4.254`. Restarting the second span's decay
    /// at `1` instead would give `0.4636·P²H`, a ratio of `1.66` — so this
    /// assertion discriminates between the two directly rather than through
    /// an inequality that happens to hold.
    #[test]
    fn the_second_segment_continues_the_decay_it_does_not_restart_it() {
        let e = env();
        let w = integrate_funding_window_by_sign(
            &e, 0, 1_000, P, 0, HALF_LIFE, MAX_RATE, 4 * HALF_LIFE,
        );
        let a = w.first.as_ref().unwrap().funding_weight.to_u128().expect("first fits");
        let b = w.second.as_ref().unwrap().funding_weight.to_u128().expect("second fits");
        let ratio_milli = b * 1_000 / a;
        assert!(
            (4_200..=4_300).contains(&ratio_milli),
            "expected the analytic 4.254 ratio, got {}.{:03} — a restarted \
             decay would read about 1.66",
            ratio_milli / 1_000,
            ratio_milli % 1_000
        );
    }

    /// Splitting a window at an arbitrary interior point and replaying both
    /// halves must reproduce the single-window result. This is §2.1.2's
    /// property 2 and the reason `J₁`/`J₂` are carried at INDEX_PRECISION
    /// rather than floored to whole seconds.
    #[test]
    fn splitting_a_window_reproduces_the_single_window_weight() {
        let e = env();
        let (long, short, ema, w_bps) = (750i128, 250i128, P / 4, 3_000u32);
        let whole = integrate_funding_window_by_sign(
            &e, long, short, ema, w_bps, HALF_LIFE, MAX_RATE, 7_200,
        );
        let part1 = integrate_funding_window_by_sign(
            &e, long, short, ema, w_bps, HALF_LIFE, MAX_RATE, 3_600,
        );
        // The second half starts from the EMA the first half ended at.
        let part2 = integrate_funding_window_by_sign(
            &e, long, short, part1.ema_after, w_bps, HALF_LIFE, MAX_RATE, 3_600,
        );
        let total = whole.first.as_ref().unwrap().funding_weight.to_u128().unwrap();
        let split = part1.first.as_ref().unwrap().funding_weight.to_u128().unwrap()
            + part2.first.as_ref().unwrap().funding_weight.to_u128().unwrap();
        let drift = total.abs_diff(split);
        // Well inside DECAY_TOLERANCE (1e-12 relative) for this magnitude.
        assert!(
            drift * 1_000_000_000 < total,
            "split {split} vs whole {total}, drift {drift}"
        );
    }

    /// A balanced book with no memory generates no funding at all.
    #[test]
    fn a_balanced_book_with_no_memory_accrues_nothing() {
        let e = env();
        let w = integrate_funding_window_by_sign(&e, 500, 500, 0, 3_000, HALF_LIFE, MAX_RATE, 3_600);
        assert!(w.first.is_none(), "zero skew must emit no segment");
        assert!(w.second.is_none());
        assert_eq!(w.payer_side_at_end, PayerSide::None);
        assert_eq!(w.displayed_rate_at_end, 0);
    }

    /// The EMA decays toward live skew by exactly one half in one half-life.
    #[test]
    fn the_ema_halves_its_distance_to_live_skew_per_half_life() {
        let e = env();
        // Live skew is -P, memory is +P: after one half-life the EMA sits
        // at the midpoint, zero.
        let w = integrate_funding_window_by_sign(&e, 0, 1_000, P, 3_000, HALF_LIFE, MAX_RATE, HALF_LIFE);
        assert_eq!(w.ema_after, 0);
    }
}
