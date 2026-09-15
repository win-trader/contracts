//! Checked arithmetic cores shared by every protocol contract — §2.1.1.
//!
//! Every function returns `None` on overflow or a domain violation instead of
//! panicking. Contract crates wrap these with a typed panic
//! (`ArithmeticError` in their own error enum) so a failing invocation
//! reports an error code that identifies the source contract. Keeping the
//! cores here guarantees one rounding/sign policy across contracts.
//!
//! **Every product formed on the way to a division is computed in 256-bit**,
//! regardless of how small its operands look (§2.1.1). Three products in the
//! specification exceed `u128` at configured parameter limits — the funding
//! weight outright, LP share minting and the minimum borrow fee by a margin a
//! single parameter change could remove. A uniform 256-bit product removes
//! the whole class of question, and the range-check on narrowing the quotient
//! back to 128 bits is what turns a would-be silent wrap into a revert. It is
//! not optional.
//!
//! The widening uses `soroban_sdk::U256` / `I256`, which are host types: the
//! 256-bit work is a host call rather than a WASM software multiply. The one
//! hot-path caller is §6.14's borrow rate, a single `mul_div_floor` per
//! mutation; the 48-iteration primitives in `crate::fixed` run once per
//! funding window.
//!
//! Sign policy: `mul_div_floor` / `mul_div_ceil` are defined for non-negative
//! `a`/`b` and strictly positive `d` only. `mul_div_trunc` is the signed
//! helper and truncates toward zero, so the magnitude of a signed skew is
//! never overstated in either direction — which floor division would not give
//! for a negative value. No saturating or wrapping operation appears
//! anywhere: an overflow is an unexpected failure under §8.9, it reverts, and
//! it is never a terminal business outcome.

use soroban_sdk::{Env, I256, U256};

/// `a + b`, `None` on overflow. Checked at 128 bits (§2.1.1: additions
/// outside the four helpers are checked and error on overflow).
#[inline]
pub fn add(a: i128, b: i128) -> Option<i128> {
    a.checked_add(b)
}

/// `a - b`, `None` on overflow.
#[inline]
pub fn sub(a: i128, b: i128) -> Option<i128> {
    a.checked_sub(b)
}

/// `a * b`, `None` on overflow. A standalone multiplication, not one on the
/// way to a division — those go through the helpers below.
#[inline]
pub fn mul(a: i128, b: i128) -> Option<i128> {
    a.checked_mul(b)
}

/// Widen a non-negative `i128` to 256-bit. `None` for a negative value: the
/// unsigned helpers are defined on magnitudes only.
#[inline]
pub fn widen(env: &Env, value: i128) -> Option<U256> {
    if value < 0 {
        return None;
    }
    Some(U256::from_u128(env, value as u128))
}

/// Narrow a 256-bit value back to `i128`, `None` if it does not fit. This is
/// the check §2.1.1 calls "what turns a would-be silent wrap into a revert".
#[inline]
pub fn narrow(value: &U256) -> Option<i128> {
    match value.to_u128() {
        Some(v) if v <= i128::MAX as u128 => Some(v as i128),
        _ => None,
    }
}

/// §2.1.1 — `floor(a * b / d)` for `a, b >= 0`, `d > 0`, the product formed
/// in 256-bit and the quotient range-checked on the way back to `i128`.
pub fn mul_div_floor(env: &Env, a: i128, b: i128, denominator: i128) -> Option<i128> {
    if denominator <= 0 {
        return None;
    }
    let product = widen(env, a)?.mul(&widen(env, b)?);
    narrow(&product.div(&widen(env, denominator)?))
}

/// §2.1.1 — `ceil(a * b / d)` for `a, b >= 0`, `d > 0`. At 256-bit the
/// rounding add cannot overflow, so there is no `checked_add` dance: the
/// product is at most `2^254` and `d - 1` is below `2^127`.
pub fn mul_div_ceil(env: &Env, a: i128, b: i128, denominator: i128) -> Option<i128> {
    if denominator <= 0 {
        return None;
    }
    let d = widen(env, denominator)?;
    let product = widen(env, a)?.mul(&widen(env, b)?);
    let rounded = product.add(&d).sub(&U256::from_u32(env, 1));
    narrow(&rounded.div(&d))
}

/// §2.1.1 — signed `a * b / d`, **truncating toward zero**, for `d > 0`.
/// The helper for signed quantities: the signed skew of §3.4.1, the blend
/// coefficient `B` of §4.6, and every decayed value derived from them.
/// `I256::to_i128` performs the narrowing range-check.
pub fn mul_div_trunc(env: &Env, a: i128, b: i128, denominator: i128) -> Option<i128> {
    if denominator <= 0 {
        return None;
    }
    let product = I256::from_i128(env, a).mul(&I256::from_i128(env, b));
    product.div(&I256::from_i128(env, denominator)).to_i128()
}

/// §2.1.1 — `floor(n * b / d)` where the numerator is **already** a 256-bit
/// value and the result is still one.
///
/// §6.2 splits a funding weight — a quantity the specification puts near
/// `1e40` — in proportion to base exposure before dividing it into an index.
/// Narrowing it to `i128` at that point would overflow; the split has to
/// happen at full width and stay there until `carried_div` takes the
/// quotient.
pub fn wide_mul_div_floor(env: &Env, n: &U256, b: i128, d: i128) -> Option<U256> {
    if d <= 0 {
        return None;
    }
    Some(n.mul(&widen(env, b)?).div(&widen(env, d)?))
}

/// §2.1.1 — `ceil(a * b * c / d)` with the **triple** product formed in
/// 256-bit.
///
/// §6.7's minimum borrow fee is the one formula with three multiplicands on
/// the way to a division. Its note — "the grouped expression does not
/// authorize an overflowing intermediate" — is exactly this: writing it as
/// `mul_div_ceil(a * b, c, d)` forms `a * b` at 128 bits first.
pub fn mul_mul_div_ceil(env: &Env, a: i128, b: i128, c: i128, d: i128) -> Option<i128> {
    if d <= 0 {
        return None;
    }
    let divisor = widen(env, d)?;
    let product = widen(env, a)?.mul(&widen(env, b)?).mul(&widen(env, c)?);
    let rounded = product.add(&divisor).sub(&U256::from_u32(env, 1));
    narrow(&rounded.div(&divisor))
}

/// §2.1.1 — one accumulator advance: `(n + r) / d` with the remainder
/// carried, returning `(quotient, remainder)`.
///
/// `n` is the one numerator in this module that is **not** bounded by 128
/// bits — §6.2 passes it a funding weight the specification puts near
/// `1e40`, already the output of a 256-bit product. Only `d`, the quotient,
/// and the remainder are declared at 128 bits, and all three are
/// range-checked. Typing the numerator as `i128` to match the other helpers
/// overflows on the first funding checkpoint of a busy market.
pub fn carried_div(
    env: &Env,
    numerator: &U256,
    divisor: i128,
    remainder: i128,
) -> Option<(i128, i128)> {
    if divisor <= 0 {
        return None;
    }
    let d = widen(env, divisor)?;
    let total = numerator.add(&widen(env, remainder)?);
    Some((narrow(&total.div(&d))?, narrow(&total.rem_euclid(&d))?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env() -> Env {
        Env::default()
    }

    #[test]
    fn floor_and_ceil_agree_on_exact_division() {
        let e = env();
        assert_eq!(mul_div_floor(&e, 10, 4, 2), Some(20));
        assert_eq!(mul_div_ceil(&e, 10, 4, 2), Some(20));
    }

    #[test]
    fn ceil_rounds_up_floor_rounds_down() {
        let e = env();
        assert_eq!(mul_div_floor(&e, 10, 3, 4), Some(7)); // 30/4 = 7.5
        assert_eq!(mul_div_ceil(&e, 10, 3, 4), Some(8));
    }

    #[test]
    fn zero_product_is_zero_both_directions() {
        let e = env();
        assert_eq!(mul_div_floor(&e, 0, 5, 3), Some(0));
        assert_eq!(mul_div_ceil(&e, 0, 5, 3), Some(0));
    }

    #[test]
    fn negative_inputs_are_domain_violations() {
        let e = env();
        assert_eq!(mul_div_floor(&e, -1, 5, 3), None);
        assert_eq!(mul_div_floor(&e, 5, -1, 3), None);
        assert_eq!(mul_div_floor(&e, 5, 1, 0), None);
        assert_eq!(mul_div_ceil(&e, -1, 5, 3), None);
        assert_eq!(mul_div_ceil(&e, 5, -1, 3), None);
        assert_eq!(mul_div_ceil(&e, 5, 1, -3), None);
        assert_eq!(mul_div_trunc(&e, 5, 1, 0), None);
        assert_eq!(mul_div_trunc(&e, 5, 1, -3), None);
    }

    #[test]
    fn the_product_is_formed_in_256_bit_not_128() {
        let e = env();
        // `i128::MAX * 2 / 2` wraps at 128 bits and is exact at 256.
        assert_eq!(mul_div_floor(&e, i128::MAX, 2, 2), Some(i128::MAX));
        assert_eq!(mul_div_ceil(&e, i128::MAX, 2, 2), Some(i128::MAX));
        assert_eq!(mul_div_trunc(&e, i128::MAX, 2, 2), Some(i128::MAX));
        assert_eq!(mul_div_trunc(&e, i128::MIN, 2, 2), Some(i128::MIN));
    }

    #[test]
    fn a_quotient_that_does_not_fit_128_bits_is_none_not_a_wrap() {
        let e = env();
        assert_eq!(mul_div_floor(&e, i128::MAX, 4, 2), None);
        assert_eq!(mul_div_ceil(&e, i128::MAX, 4, 2), None);
        assert_eq!(mul_div_trunc(&e, i128::MAX, 4, 2), None);
        assert_eq!(mul_div_trunc(&e, i128::MIN, 4, 2), None);
    }

    #[test]
    fn trunc_rounds_toward_zero_in_both_directions() {
        let e = env();
        // -7/2 is -3.5: truncation gives -3, floor would give -4.
        assert_eq!(mul_div_trunc(&e, -7, 1, 2), Some(-3));
        assert_eq!(mul_div_trunc(&e, 7, 1, 2), Some(3));
        assert_eq!(mul_div_trunc(&e, 7, -1, 2), Some(-3));
        assert_eq!(mul_div_trunc(&e, -7, -1, 2), Some(3));
    }

    #[test]
    fn carried_div_carries_the_remainder_exactly() {
        let e = env();
        let n = widen(&e, 10).unwrap();
        assert_eq!(carried_div(&e, &n, 3, 0), Some((3, 1)));
        // The carried remainder is what makes the next advance whole.
        assert_eq!(carried_div(&e, &n, 3, 1), Some((3, 2)));
        assert_eq!(carried_div(&e, &n, 3, 2), Some((4, 0)));
        assert_eq!(carried_div(&e, &n, 0, 0), None);
        assert_eq!(carried_div(&e, &n, 3, -1), None);
    }

    #[test]
    fn carried_div_takes_a_numerator_wider_than_128_bits() {
        let e = env();
        // ~1e40: the scale §6.2 puts a busy market's funding weight at. An
        // i128 numerator cannot represent it at all.
        let big = U256::from_u128(&e, 10u128.pow(20)).mul(&U256::from_u128(&e, 10u128.pow(20)));
        let (q, r) = carried_div(&e, &big, 10i128.pow(18), 0).unwrap();
        assert_eq!(q, 10i128.pow(22));
        assert_eq!(r, 0);
        // A quotient that still does not fit 128 bits is refused, not wrapped.
        assert_eq!(carried_div(&e, &big, 1, 0), None);
    }

    #[test]
    fn checked_128_bit_helpers_are_unchanged() {
        assert_eq!(add(i128::MAX, 1), None);
        assert_eq!(sub(i128::MIN, 1), None);
        assert_eq!(mul(i128::MAX, 2), None);
        assert_eq!(add(2, 3), Some(5));
    }
}
