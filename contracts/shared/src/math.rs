use soroban_sdk::{Env, I256, U256};

#[inline]
pub fn add(a: i128, b: i128) -> Option<i128> {
    a.checked_add(b)
}

#[inline]
pub fn sub(a: i128, b: i128) -> Option<i128> {
    a.checked_sub(b)
}

#[inline]
pub fn mul(a: i128, b: i128) -> Option<i128> {
    a.checked_mul(b)
}

#[inline]
pub fn widen(env: &Env, value: i128) -> Option<U256> {
    if value < 0 {
        return None;
    }
    Some(U256::from_u128(env, value as u128))
}

#[inline]
pub fn narrow(value: &U256) -> Option<i128> {
    match value.to_u128() {
        Some(v) if v <= i128::MAX as u128 => Some(v as i128),
        _ => None,
    }
}

pub fn mul_div_floor(env: &Env, a: i128, b: i128, denominator: i128) -> Option<i128> {
    if denominator <= 0 {
        return None;
    }
    let product = widen(env, a)?.mul(&widen(env, b)?);
    narrow(&product.div(&widen(env, denominator)?))
}

pub fn mul_div_ceil(env: &Env, a: i128, b: i128, denominator: i128) -> Option<i128> {
    if denominator <= 0 {
        return None;
    }
    let d = widen(env, denominator)?;
    let product = widen(env, a)?.mul(&widen(env, b)?);
    let rounded = product.add(&d).sub(&U256::from_u32(env, 1));
    narrow(&rounded.div(&d))
}

pub fn mul_div_trunc(env: &Env, a: i128, b: i128, denominator: i128) -> Option<i128> {
    if denominator <= 0 {
        return None;
    }
    let product = I256::from_i128(env, a).mul(&I256::from_i128(env, b));
    product.div(&I256::from_i128(env, denominator)).to_i128()
}

pub fn wide_mul_div_floor(env: &Env, n: &U256, b: i128, d: i128) -> Option<U256> {
    if d <= 0 {
        return None;
    }
    Some(n.mul(&widen(env, b)?).div(&widen(env, d)?))
}

pub fn mul_mul_div_ceil(env: &Env, a: i128, b: i128, c: i128, d: i128) -> Option<i128> {
    if d <= 0 {
        return None;
    }
    let divisor = widen(env, d)?;
    let product = widen(env, a)?.mul(&widen(env, b)?).mul(&widen(env, c)?);
    let rounded = product.add(&divisor).sub(&U256::from_u32(env, 1));
    narrow(&rounded.div(&divisor))
}

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
        assert_eq!(mul_div_floor(&e, 10, 3, 4), Some(7));
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
        assert_eq!(carried_div(&e, &n, 3, 1), Some((3, 2)));
        assert_eq!(carried_div(&e, &n, 3, 2), Some((4, 0)));
        assert_eq!(carried_div(&e, &n, 0, 0), None);
        assert_eq!(carried_div(&e, &n, 3, -1), None);
    }

    #[test]
    fn carried_div_takes_a_numerator_wider_than_128_bits() {
        let e = env();
        let big = U256::from_u128(&e, 10u128.pow(20)).mul(&U256::from_u128(&e, 10u128.pow(20)));
        let (q, r) = carried_div(&e, &big, 10i128.pow(18), 0).unwrap();
        assert_eq!(q, 10i128.pow(22));
        assert_eq!(r, 0);
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
