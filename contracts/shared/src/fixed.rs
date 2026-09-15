//! Fixed-point transcendental primitives — §2.1.2 and §2.1.3.
//!
//! Funding integration needs `2^-x`; locating a funding sign change needs
//! `log2`. Both are evaluated at `INDEX_PRECISION` and both are fully
//! deterministic. Nothing else in the protocol needs a transcendental — the
//! borrow curve is a plain square (§6.14), so there is no `pow` and `log2`
//! is called once per funding window rather than on every action that moves
//! the borrow rate.
//!
//! Every rounding here truncates, so the error is one-directional and a
//! decay factor is never overstated. Each of the at most 48 multiplications
//! truncates by less than one unit in `1e14`, so the accumulated relative
//! error stays below `48 / 1e14` — comfortably inside `DECAY_TOLERANCE`.

use soroban_sdk::Env;

use crate::constants::INDEX_PRECISION;
use crate::math;

/// §2.1.3 — `floor(INDEX_PRECISION * 2^(-2^-i))` for `i = 1..=48`, indexed
/// from zero here so `HALF_POW[i - 1]` is the spec's `HALF_POW[i]`.
///
/// This is **reference data, not a derivation to redo at a different
/// precision**. Entries 46 through 48 are identical because `ln(2) / 2^i`
/// drops below one unit in `1e14` at `i = 47`; they are kept so the loop
/// bound matches the 46.5 bits of fraction `INDEX_PRECISION` can represent,
/// and so the table is indexable without a special case.
pub const HALF_POW: [i128; 48] = [
    70_710_678_118_654,
    84_089_641_525_371,
    91_700_404_320_467,
    95_760_328_069_857,
    97_857_206_208_770,
    98_922_801_319_397,
    99_459_942_348_363,
    99_729_605_608_547,
    99_864_711_289_097,
    99_932_332_750_265,
    99_966_160_649_624,
    99_983_078_893_192,
    99_991_539_088_661,
    99_995_769_454_843,
    99_997_884_705_049,
    99_998_942_346_931,
    99_999_471_172_067,
    99_999_735_585_684,
    99_999_867_792_754,
    99_999_933_896_355,
    99_999_966_948_172,
    99_999_983_474_084,
    99_999_991_737_042,
    99_999_995_868_520,
    99_999_997_934_260,
    99_999_998_967_130,
    99_999_999_483_565,
    99_999_999_741_782,
    99_999_999_870_891,
    99_999_999_935_445,
    99_999_999_967_722,
    99_999_999_983_861,
    99_999_999_991_930,
    99_999_999_995_965,
    99_999_999_997_982,
    99_999_999_998_991,
    99_999_999_999_495,
    99_999_999_999_747,
    99_999_999_999_873,
    99_999_999_999_936,
    99_999_999_999_968,
    99_999_999_999_984,
    99_999_999_999_992,
    99_999_999_999_996,
    99_999_999_999_998,
    99_999_999_999_999,
    99_999_999_999_999,
    99_999_999_999_999,
];

/// §2.1.2 — `ln(2) * INDEX_PRECISION`, truncated. Required by the §4.6
/// window integral. Replaces the `1/ln 2` ratio the superseded closed form
/// carried.
pub const LN2: i128 = 69_314_718_055_994;

/// §2.1.2 — the declared accuracy bound for the decay primitives, as a
/// relative error: `1e-12`. Expressed as a reciprocal because the contract
/// does no floating point. Used by QA, never by the contract itself.
pub const DECAY_TOLERANCE_RECIPROCAL: i128 = 1_000_000_000_000;

/// §2.1.2 — `2^-x` for `x >= 0`, with `x` and the result scaled by
/// `INDEX_PRECISION`. `None` for a negative `x` or on an arithmetic failure.
///
/// `x` splits into a whole part `n` and a fractional part `f`, where `f` is
/// an `INDEX_PRECISION`-scaled integer representing the fraction `f / 1e14`
/// in `[0, 1)`. The binary digits of *that fraction* — not bits of the
/// integer `f` — come out by repeated doubling, and each set digit folds in
/// one `HALF_POW` entry.
///
/// **The shift is applied last**, after the fractional product, so the
/// multiplications keep full precision and only one truncation is taken at
/// the end. Shifting first would discard `n` bits of the mantissa before any
/// of them were used.
pub fn exp2_neg(env: &Env, x: i128) -> Option<i128> {
    if x < 0 {
        return None;
    }
    let n = x / INDEX_PRECISION;
    let f = x % INDEX_PRECISION;
    if n >= 128 {
        return Some(0);
    }

    let mut result = INDEX_PRECISION;
    let mut rem = f;
    for entry in HALF_POW.iter() {
        rem = math::mul(rem, 2)?;
        if rem >= INDEX_PRECISION {
            rem -= INDEX_PRECISION;
            result = math::mul_div_floor(env, result, *entry, INDEX_PRECISION)?;
        }
    }
    Some(result >> (n as u32))
}

/// §2.1.2 — `log2(y)` at `INDEX_PRECISION` for `y >= INDEX_PRECISION`, by
/// the mirror construction to `exp2_neg`: the integer part from the bit
/// length, then one fraction bit at a time by repeated squaring of the
/// normalized mantissa, for 48 iterations. `None` below the domain.
///
/// One caller: locating the funding sign change `t_star` in §6.2.1. This is
/// a different function from the deleted `neg_log2`, with a different
/// domain; the borrow curve no longer needs a logarithm at all.
pub fn log2(env: &Env, y: i128) -> Option<i128> {
    if y < INDEX_PRECISION {
        return None;
    }
    let n = (128 - ((y / INDEX_PRECISION) as u128).leading_zeros()) - 1;
    let mut m = y >> n;
    let mut result = math::mul(n as i128, INDEX_PRECISION)?;

    for i in 1..=48u32 {
        m = math::mul_div_floor(env, m, m, INDEX_PRECISION)?;
        if m >= 2 * INDEX_PRECISION {
            m /= 2;
            result = math::add(result, INDEX_PRECISION >> i)?;
        }
    }
    Some(result)
}

// ---------------------------------------------------------------------------
// §2.1.3 conformance vectors.
//
// These are the output of the algorithms exactly as specified, not the
// mathematically exact values. Where they differ the truncating algorithm is
// normative and an implementation must reproduce these integers bit for bit.
// They live here as data so the QA phase asserts them without re-deriving
// them at a different precision.
// ---------------------------------------------------------------------------

/// `(x, exp2_neg(x))`. The `0.5` and `3.25` rows happen to agree with the
/// exact values to all 14 places; the others do not, and that is expected.
pub const EXP2_NEG_VECTORS: [(i128, i128); 7] = [
    (0, 100_000_000_000_000),
    (50_000_000_000_000, 70_710_678_118_654),
    (100_000_000_000_000, 50_000_000_000_000),
    (200_000_000_000_000, 25_000_000_000_000),
    (325_000_000_000_000, 10_511_205_190_671),
    (1_000_000_000_000_000, 97_656_250_000),
    (12_700_000_000_000_000, 0),
];

/// `(y, log2(y))`. `log2(3)` is expected to return `158_496_250_072_105`
/// against an exact `158_496_250_072_115` — low by ten units in `1.6e14`, or
/// `6e-14` relative, inside `DECAY_TOLERANCE` and in the safe direction.
pub const LOG2_VECTORS: [(i128, i128); 6] = [
    (100_000_000_000_000, 0),
    (200_000_000_000_000, 100_000_000_000_000),
    (300_000_000_000_000, 158_496_250_072_105),
    (400_000_000_000_000, 200_000_000_000_000),
    (420_000_000_000_000, 207_038_932_789_131),
    (1_000_000_000_000_000, 332_192_809_488_729),
];

/// `(utilization_bps, u, u_squared, current_borrow_rate)` for the §6.14
/// curve at the initial rate parameters (base 25, max variable 250 bps/day).
/// Every entry is exact: `u` is a multiple of `1e10` and its square a
/// multiple of `1e6`, so the division by `INDEX_PRECISION` has no remainder
/// at any reachable utilization. A test may assert these exactly.
pub const BORROW_CURVE_VECTORS: [(i128, i128, i128, i128); 6] = [
    (0, 0, 0, 2_500_000_000_000_000),
    (2_500, 25_000_000_000_000, 6_250_000_000_000, 4_062_500_000_000_000),
    (5_000, 50_000_000_000_000, 25_000_000_000_000, 8_750_000_000_000_000),
    (7_500, 75_000_000_000_000, 56_250_000_000_000, 16_562_500_000_000_000),
    (8_500, 85_000_000_000_000, 72_250_000_000_000, 20_562_500_000_000_000),
    (
        10_000,
        100_000_000_000_000,
        100_000_000_000_000,
        27_500_000_000_000_000,
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn env() -> Env {
        Env::default()
    }

    #[test]
    fn exp2_neg_reproduces_every_conformance_vector() {
        let e = env();
        for (x, want) in EXP2_NEG_VECTORS {
            assert_eq!(exp2_neg(&e, x), Some(want), "exp2_neg({x})");
        }
    }

    #[test]
    fn log2_reproduces_every_conformance_vector() {
        let e = env();
        for (y, want) in LOG2_VECTORS {
            assert_eq!(log2(&e, y), Some(want), "log2({y})");
        }
    }

    /// §2.1.2 property 1, which an implementation "must test directly".
    #[test]
    fn exp2_neg_is_monotonically_non_increasing_and_exact_at_zero() {
        let e = env();
        assert_eq!(exp2_neg(&e, 0), Some(INDEX_PRECISION));
        let mut previous = INDEX_PRECISION;
        let mut x = 0i128;
        while x <= 8 * INDEX_PRECISION {
            let current = exp2_neg(&e, x).unwrap();
            assert!(current <= previous, "exp2_neg rose at x = {x}");
            previous = current;
            x += INDEX_PRECISION / 16;
        }
    }

    #[test]
    fn exp2_neg_rejects_a_negative_exponent_and_underflows_to_zero() {
        let e = env();
        assert_eq!(exp2_neg(&e, -1), None);
        assert_eq!(exp2_neg(&e, 128 * INDEX_PRECISION), Some(0));
        assert_eq!(exp2_neg(&e, 127 * INDEX_PRECISION), Some(0));
    }

    #[test]
    fn log2_is_outside_its_domain_below_one() {
        let e = env();
        assert_eq!(log2(&e, INDEX_PRECISION - 1), None);
        assert_eq!(log2(&e, 0), None);
        assert_eq!(log2(&e, -1), None);
    }

    #[test]
    fn log2_and_exp2_neg_invert_each_other_at_powers_of_two() {
        let e = env();
        for k in 1..=8i128 {
            let y = INDEX_PRECISION << k;
            assert_eq!(log2(&e, y), Some(k * INDEX_PRECISION));
            assert_eq!(exp2_neg(&e, k * INDEX_PRECISION), Some(INDEX_PRECISION >> k));
        }
    }

    /// `HALF_POW[1]` squared is 1/2 and every later entry squared is its
    /// predecessor — up to the truncation each entry carries. The deviation
    /// is asserted as a *signed* band `[-2, 0]`, not an absolute tolerance:
    /// flooring can only understate a decay factor, so a positive deviation
    /// anywhere would mean an entry was rounded rather than truncated, which
    /// is the exact defect the superseded rounded table had.
    #[test]
    fn the_table_telescopes_and_never_overstates_a_decay() {
        let e = env();
        let half = math::mul_div_floor(&e, HALF_POW[0], HALF_POW[0], INDEX_PRECISION).unwrap();
        let first = half - INDEX_PRECISION / 2;
        assert!((-2..=0).contains(&first), "HALF_POW[1]^2 off by {first}");
        for i in 1..HALF_POW.len() {
            let squared =
                math::mul_div_floor(&e, HALF_POW[i], HALF_POW[i], INDEX_PRECISION).unwrap();
            let deviation = squared - HALF_POW[i - 1];
            assert!(
                (-2..=0).contains(&deviation),
                "entry {} deviates by {deviation}",
                i + 1
            );
        }
    }

    #[test]
    fn every_table_entry_truncates_rather_than_rounds() {
        // The superseded table rounded to nearest and its last entry was
        // exactly INDEX_PRECISION — a no-op multiply that contributed
        // nothing. Every entry here is strictly below INDEX_PRECISION.
        for (i, entry) in HALF_POW.iter().enumerate() {
            assert!(*entry < INDEX_PRECISION, "entry {} is not a decay", i + 1);
            assert!(*entry > 0);
        }
        assert_eq!(HALF_POW[45], HALF_POW[46]);
        assert_eq!(HALF_POW[46], HALF_POW[47]);
    }
}
