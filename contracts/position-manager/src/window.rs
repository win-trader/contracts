use soroban_sdk::{panic_with_error, Env, I256, U256};

use shared::constants::{BPS, INDEX_PRECISION};
use shared::fixed::{self, LN2};
use shared::PayerSide;

use crate::errors::PositionManagerError;
use crate::math;

fn fail(env: &Env) -> ! {
    panic_with_error!(env, PositionManagerError::InvariantViolation)
}

pub struct Segment {
    pub payer_sign: i128,
    pub funding_weight: U256,
}

impl Segment {
    pub fn long_pays(&self) -> bool {
        self.payer_sign > 0
    }
}

pub struct FundingWindow {
    pub first: Option<Segment>,
    pub second: Option<Segment>,
    pub ema_after: i128,
    pub payer_side_at_end: PayerSide,
    pub displayed_rate_at_end: i128,
}

impl FundingWindow {
    pub fn segments(&self) -> impl Iterator<Item = &Segment> {
        self.first.iter().chain(self.second.iter())
    }
}

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

struct Span {
    t1: i128,
    d1: i128,
    t2: i128,
    d2: i128,
    sign_source: i128,
}

enum Crossing {
    Inside { t_star: i128, d_star: i128 },
    AtStart,
    AtEnd,
}

// Must never revert: inputs are stored state, so a failed checkpoint would fail forever.
fn locate_crossing(
    env: &Env,
    a: i128,
    b: i128,
    d_end: i128,
    half_life: i128,
    elapsed_scaled: i128,
) -> Crossing {
    let p = INDEX_PRECISION;
    if a == 0 || b == 0 {
        return Crossing::AtEnd;
    }
    // B is negative on an upward crossing, so divide magnitudes.
    let d_star = math::mul_div_floor(env, a.abs(), p, b.abs());
    if d_star >= p {
        return Crossing::AtStart;
    }
    if d_star <= d_end {
        return Crossing::AtEnd;
    }
    let Some(log) = fixed::log2(env, math::mul_div_floor(env, p, p, d_star)) else {
        return Crossing::AtStart;
    };
    let t_star = math::mul(env, half_life, log);
    if t_star <= 0 {
        Crossing::AtStart
    } else if t_star >= elapsed_scaled {
        Crossing::AtEnd
    } else {
        Crossing::Inside { t_star, d_star }
    }
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
    let elapsed_scaled = math::mul(env, elapsed as i128, p);

    let s = live_skew(env, long_base, short_base);

    let w = math::mul_div_floor(env, instant_weight_bps as i128, p, BPS);
    let a = s;
    let b = math::mul_div_trunc(env, math::sub(env, p, w), math::sub(env, skew_ema, s), p);

    let d_end = decay_at(env, elapsed, half_life_seconds);
    let ema_after = math::add(
        env,
        s,
        math::mul_div_trunc(env, math::sub(env, skew_ema, s), d_end, p),
    );

    let i_start = math::add(env, a, b);
    let i_end = math::add(env, a, math::mul_div_trunc(env, b, d_end, p));

    let crosses = i_start != 0 && i_end != 0 && i_start.signum() != i_end.signum();
    let whole = |sign_source: i128| Span {
        t1: 0,
        d1: p,
        t2: elapsed_scaled,
        d2: d_end,
        sign_source,
    };

    let (first_span, second_span) = if !crosses {
        (whole(if i_start != 0 { i_start } else { i_end }), None)
    } else {
        match locate_crossing(env, a, b, d_end, half_life, elapsed_scaled) {
            Crossing::Inside { t_star, d_star } => (
                Span {
                    t1: 0,
                    d1: p,
                    t2: t_star,
                    d2: d_star,
                    sign_source: i_start,
                },
                Some(Span {
                    t1: t_star,
                    d1: d_star,
                    t2: elapsed_scaled,
                    d2: d_end,
                    sign_source: i_end,
                }),
            ),
            Crossing::AtStart => (whole(i_end), None),
            Crossing::AtEnd => (whole(i_start), None),
        }
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

pub fn decay_at(env: &Env, elapsed: u64, half_life_seconds: u64) -> i128 {
    let x = math::mul_div_floor(
        env,
        elapsed as i128,
        INDEX_PRECISION,
        half_life_seconds as i128,
    );
    fixed::exp2_neg(env, x).unwrap_or_else(|| fail(env))
}

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
        return None;
    }
    let p = INDEX_PRECISION;

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

    let a256 = I256::from_i128(env, a);
    let b256 = I256::from_i128(env, b);
    let p256 = I256::from_i128(env, p);
    let term1 = a256
        .mul(&a256)
        .mul(&I256::from_i128(env, math::sub(env, span.t2, span.t1)));
    let term2 = I256::from_i128(env, 2)
        .mul(&a256)
        .mul(&b256)
        .mul(&I256::from_i128(env, j1));
    let term3 = b256.mul(&b256).mul(&I256::from_i128(env, j2));
    let quadratic_scaled = term1.add(&term2).add(&term3);

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

    #[test]
    fn the_11_12_conformance_window_splits_without_reverting() {
        let e = env();
        let w = integrate_funding_window_by_sign(
            &e,
            1_100_000_000,
            900_000_000,
            -50_000_000_000_000,
            3_000,
            HALF_LIFE,
            MAX_RATE,
            172_800,
        );
        let first = w.first.as_ref().expect("shorts pay before the crossing");
        let second = w.second.as_ref().expect("longs pay after it");
        assert_eq!(first.payer_sign, -1);
        assert_eq!(second.payer_sign, 1);
        assert_eq!(w.ema_after, 6_250_000_000_000);
        assert_eq!(w.payer_side_at_end, PayerSide::Long);

        let rate = MAX_RATE as u128;
        assert_eq!(
            first.funding_weight.to_u128().unwrap(),
            20_910_371_643_640_164_186_000_000_000_000 * rate
        );
        assert_eq!(
            second.funding_weight.to_u128().unwrap(),
            2_044_559_468_136_672_399_480_000_000_000 * rate
        );
        let whole: u128 = 22_954_931_111_776_836_585_480_000_000_000 * rate;
        let split = first.funding_weight.to_u128().unwrap() + second.funding_weight.to_u128().unwrap();
        assert_eq!(split, whole, "§4.6 — the segments sum to the one-window integral");
    }

    #[test]
    fn a_crossing_inside_the_first_second_does_not_revert() {
        let e = env();
        for elapsed in [1u64, 5, 3_600, 86_400] {
            let w = integrate_funding_window_by_sign(
                &e,
                20_000_000,
                60_000_000,
                21_429_510_104_365,
                3_000,
                HALF_LIFE,
                MAX_RATE,
                elapsed,
            );
            assert_eq!(w.payer_side_at_end, PayerSide::Short, "elapsed {elapsed}");
        }
    }

    #[test]
    fn a_window_that_does_not_cross_has_one_segment() {
        let e = env();
        let w = integrate_funding_window_by_sign(&e, 750, 250, P / 2, 3_000, HALF_LIFE, MAX_RATE, 3_600);
        assert!(w.first.is_some());
        assert!(w.second.is_none(), "an uncrossed window must not split");
        assert_eq!(w.first.as_ref().unwrap().payer_sign, 1);
        assert_eq!(w.payer_side_at_end, PayerSide::Long);
    }

    #[test]
    fn a_window_that_crosses_zero_splits_and_attributes_each_side() {
        let e = env();
        let w = integrate_funding_window_by_sign(
            &e, 0, 1_000, P, 0, HALF_LIFE, MAX_RATE, 4 * HALF_LIFE,
        );
        let first = w.first.as_ref().expect("first segment");
        let second = w.second.as_ref().expect("window must split at the crossing");
        assert_eq!(first.payer_sign, 1, "longs pay before the crossing");
        assert_eq!(second.payer_sign, -1, "shorts pay after it");
        assert_eq!(w.payer_side_at_end, PayerSide::Short);
    }

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
        let part2 = integrate_funding_window_by_sign(
            &e, long, short, part1.ema_after, w_bps, HALF_LIFE, MAX_RATE, 3_600,
        );
        let total = whole.first.as_ref().unwrap().funding_weight.to_u128().unwrap();
        let split = part1.first.as_ref().unwrap().funding_weight.to_u128().unwrap()
            + part2.first.as_ref().unwrap().funding_weight.to_u128().unwrap();
        let drift = total.abs_diff(split);
        assert!(
            drift * 1_000_000_000 < total,
            "split {split} vs whole {total}, drift {drift}"
        );
    }

    #[test]
    fn a_balanced_book_with_no_memory_accrues_nothing() {
        let e = env();
        let w = integrate_funding_window_by_sign(&e, 500, 500, 0, 3_000, HALF_LIFE, MAX_RATE, 3_600);
        assert!(w.first.is_none(), "zero skew must emit no segment");
        assert!(w.second.is_none());
        assert_eq!(w.payer_side_at_end, PayerSide::None);
        assert_eq!(w.displayed_rate_at_end, 0);
    }

    #[test]
    fn the_ema_halves_its_distance_to_live_skew_per_half_life() {
        let e = env();
        let w = integrate_funding_window_by_sign(&e, 0, 1_000, P, 3_000, HALF_LIFE, MAX_RATE, HALF_LIFE);
        assert_eq!(w.ema_after, 0);
    }
}
