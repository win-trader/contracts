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

pub fn mul_div_trunc(env: &Env, a: i128, b: i128, denominator: i128) -> i128 {
    shared::math::mul_div_trunc(env, a, b, denominator).unwrap_or_else(|| fail(env))
}

pub fn carried_div(env: &Env, numerator: &U256, divisor: i128, remainder: i128) -> (i128, i128) {
    shared::math::carried_div(env, numerator, divisor, remainder).unwrap_or_else(|| fail(env))
}

pub fn widen(env: &Env, value: i128) -> U256 {
    shared::math::widen(env, value).unwrap_or_else(|| fail(env))
}

pub fn wide_mul_div_floor(env: &Env, n: &U256, b: i128, d: i128) -> U256 {
    shared::math::wide_mul_div_floor(env, n, b, d).unwrap_or_else(|| fail(env))
}

pub fn mul_mul_div_ceil(env: &Env, a: i128, b: i128, c: i128, d: i128) -> i128 {
    shared::math::mul_mul_div_ceil(env, a, b, c, d).unwrap_or_else(|| fail(env))
}

pub fn widen_mul(env: &Env, a: i128, b: i128) -> U256 {
    widen(env, a).mul(&widen(env, b))
}

pub fn base_added(env: &Env, size: i128, price: i128, is_long: bool) -> i128 {
    if is_long {
        mul_div_floor(env, size, PRICE_PRECISION, price)
    } else {
        mul_div_ceil(env, size, PRICE_PRECISION, price)
    }
}

pub fn risk_units_for(env: &Env, size: i128, factor_bps: u32) -> i128 {
    mul_div_floor(env, size, factor_bps as i128, BPS)
}

#[derive(Clone, Copy, Debug)]
pub struct AddedExposure {
    pub base_added: i128,
    pub risk_added: i128,
}

pub fn try_added_exposure(
    env: &Env,
    is_long: bool,
    current_size: i128,
    current_risk_units: i128,
    size_added: i128,
    price: i128,
    factor_bps: u32,
) -> Option<AddedExposure> {
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
        return None;
    }
    Some(AddedExposure {
        base_added,
        risk_added,
    })
}

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

pub fn integral_skew(env: &Env, skew: i128, ema: i128, instant_weight_bps: u32) -> i128 {
    let w = instant_weight_bps as i128;
    add(
        env,
        mul_div_trunc(env, skew, w, BPS),
        mul_div_trunc(env, ema, sub(env, BPS, w), BPS),
    )
}

pub fn rate_from_integral(env: &Env, max_rate: i128, integral: i128) -> i128 {
    let magnitude = integral.abs();
    let max_scaled = mul(env, max_rate, INDEX_PRECISION);
    let first = mul_div_floor(env, max_scaled, magnitude, INDEX_PRECISION);
    mul_div_floor(env, first, magnitude, INDEX_PRECISION)
}

pub fn utilization_bps(env: &Env, risk: i128, equity: i128) -> i128 {
    if risk == 0 {
        0
    } else if equity <= 0 {
        BPS
    } else {
        core::cmp::min(mul_div_floor(env, risk, BPS, equity), BPS)
    }
}

pub fn index_value_ceil(env: &Env, base: i128, index: i128) -> i128 {
    mul_div_ceil(env, base, index, INDEX_PRECISION)
}

pub fn index_value_floor(env: &Env, base: i128, index: i128) -> i128 {
    mul_div_floor(env, base, index, INDEX_PRECISION)
}

pub fn pnl(env: &Env, is_long: bool, size: i128, base: i128, price: i128) -> i128 {
    if is_long {
        sub(env, mul_div_floor(env, base, price, PRICE_PRECISION), size)
    } else {
        sub(env, size, mul_div_ceil(env, base, price, PRICE_PRECISION))
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
        let max_rate = 100;
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
        assert_eq!(utilization_bps(&e, 200, 100), BPS);
    }

    #[test]
    fn pnl_rounding_never_favors_the_trader() {
        let e = env();
        let size = 1;
        let base = 1;
        let price = UNIT + UNIT / 3;
        let long = pnl(&e, true, size, base, price);
        let short = pnl(&e, false, size, base, price);
        assert_eq!(long, 0);
        assert_eq!(short, -1);
    }

    #[test]
    fn index_value_rounding_directions() {
        let e = env();
        let index = INDEX_PRECISION / 3;
        assert_eq!(index_value_ceil(&e, 1, index), 1);
        assert_eq!(index_value_floor(&e, 1, index), 0);
    }
}
