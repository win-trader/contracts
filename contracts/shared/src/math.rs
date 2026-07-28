//! Checked i128 arithmetic cores shared by every protocol contract.
//!
//! Every function returns `None` on overflow or a domain violation instead of
//! panicking. Contract crates wrap these with a typed panic
//! (`ArithmeticError` in their own error enum) so a failing invocation
//! reports an error code that identifies the source contract. Keeping the
//! cores here guarantees one rounding/sign policy across contracts — the
//! vault and position-manager previously carried divergent copies.
//!
//! Sign policy: `mul_div_floor` / `mul_div_ceil` are defined for
//! non-negative `a`/`b` and strictly positive `denominator` only. All fee,
//! index, and conversion math in the protocol operates on magnitudes;
//! signed PnL is handled by the callers before conversion.

/// `a + b`, `None` on overflow.
#[inline]
pub fn add(a: i128, b: i128) -> Option<i128> {
    a.checked_add(b)
}

/// `a - b`, `None` on overflow.
#[inline]
pub fn sub(a: i128, b: i128) -> Option<i128> {
    a.checked_sub(b)
}

/// `a * b`, `None` on overflow.
#[inline]
pub fn mul(a: i128, b: i128) -> Option<i128> {
    a.checked_mul(b)
}

/// `floor(a * b / denominator)` for `a, b >= 0`, `denominator > 0`.
/// `None` on overflow or domain violation.
pub fn mul_div_floor(a: i128, b: i128, denominator: i128) -> Option<i128> {
    if a < 0 || b < 0 || denominator <= 0 {
        return None;
    }
    a.checked_mul(b)?.checked_div(denominator)
}

/// `ceil(a * b / denominator)` for `a, b >= 0`, `denominator > 0`.
/// `None` on overflow or domain violation.
pub fn mul_div_ceil(a: i128, b: i128, denominator: i128) -> Option<i128> {
    if a < 0 || b < 0 || denominator <= 0 {
        return None;
    }
    let product = a.checked_mul(b)?;
    if product == 0 {
        return Some(0);
    }
    product
        .checked_add(denominator - 1)?
        .checked_div(denominator)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floor_and_ceil_agree_on_exact_division() {
        assert_eq!(mul_div_floor(10, 4, 2), Some(20));
        assert_eq!(mul_div_ceil(10, 4, 2), Some(20));
    }

    #[test]
    fn ceil_rounds_up_floor_rounds_down() {
        assert_eq!(mul_div_floor(10, 3, 4), Some(7)); // 30/4 = 7.5
        assert_eq!(mul_div_ceil(10, 3, 4), Some(8));
    }

    #[test]
    fn zero_product_is_zero_both_directions() {
        assert_eq!(mul_div_floor(0, 5, 3), Some(0));
        assert_eq!(mul_div_ceil(0, 5, 3), Some(0));
    }

    #[test]
    fn negative_inputs_are_domain_violations() {
        assert_eq!(mul_div_floor(-1, 5, 3), None);
        assert_eq!(mul_div_floor(5, -1, 3), None);
        assert_eq!(mul_div_floor(5, 1, 0), None);
        assert_eq!(mul_div_ceil(-1, 5, 3), None);
        assert_eq!(mul_div_ceil(5, -1, 3), None);
        assert_eq!(mul_div_ceil(5, 1, -3), None);
    }

    #[test]
    fn overflow_is_none_not_panic() {
        assert_eq!(mul_div_floor(i128::MAX, 2, 1), None);
        assert_eq!(mul_div_ceil(i128::MAX, 2, 1), None);
        assert_eq!(add(i128::MAX, 1), None);
        assert_eq!(sub(i128::MIN, 1), None);
        assert_eq!(mul(i128::MAX, 2), None);
    }

    #[test]
    fn ceil_near_max_does_not_overflow_the_rounding_add() {
        // The rounding add (`product + denominator - 1`) is checked too: it
        // can overflow even when the true ceil would fit. Returning None
        // (typed panic upstream) beats wrapping.
        assert_eq!(mul_div_ceil(i128::MAX, 1, 2), None);
        assert_eq!(mul_div_ceil(i128::MAX, 1, 1), Some(i128::MAX));
        // Exact division at the boundary stays exact.
        assert_eq!(mul_div_ceil(i128::MAX - 1, 1, 2), Some((i128::MAX - 1) / 2));
    }
}
