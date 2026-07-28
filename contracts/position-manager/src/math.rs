use soroban_sdk::{panic_with_error, Env};

use shared::constants::{BPS, INDEX_PRECISION, PRECISION, SECONDS_PER_DAY};

use crate::errors::PositionManagerError;

fn fail(env: &Env) -> ! {
    panic_with_error!(env, PositionManagerError::ArithmeticError)
}

pub fn add(env: &Env, a: i128, b: i128) -> i128 {
    a.checked_add(b).unwrap_or_else(|| fail(env))
}

pub fn sub(env: &Env, a: i128, b: i128) -> i128 {
    a.checked_sub(b).unwrap_or_else(|| fail(env))
}

pub fn mul_div_floor(env: &Env, a: i128, b: i128, denominator: i128) -> i128 {
    if a < 0 || b < 0 || denominator <= 0 {
        fail(env);
    }
    a.checked_mul(b)
        .and_then(|v| v.checked_div(denominator))
        .unwrap_or_else(|| fail(env))
}

pub fn mul_div_ceil(env: &Env, a: i128, b: i128, denominator: i128) -> i128 {
    if a < 0 || b < 0 || denominator <= 0 {
        fail(env);
    }
    let product = a.checked_mul(b).unwrap_or_else(|| fail(env));
    if product == 0 {
        0
    } else {
        product
            .checked_add(denominator - 1)
            .and_then(|v| v.checked_div(denominator))
            .unwrap_or_else(|| fail(env))
    }
}

pub fn base_added(env: &Env, size: i128, price: i128) -> i128 {
    mul_div_floor(env, size, PRECISION, price)
}

pub fn risk_added(env: &Env, size: i128, factor_bps: u32) -> i128 {
    mul_div_floor(env, size, factor_bps as i128, BPS)
}

pub fn skew_bps(env: &Env, long_base: i128, short_base: i128) -> i128 {
    let total = add(env, long_base, short_base);
    if total == 0 {
        return 0;
    }
    mul_div_floor(env, (long_base - short_base).abs(), BPS, total)
}

pub fn funding_rate(env: &Env, max_rate: i128, skew: i128) -> i128 {
    let max_scaled = max_rate
        .checked_mul(INDEX_PRECISION)
        .unwrap_or_else(|| fail(env));
    let first = mul_div_floor(env, max_scaled, skew, BPS);
    mul_div_floor(env, first, skew, BPS)
}

/// Cash flow per second at `INDEX_PRECISION`.
pub fn flow_per_second(env: &Env, size: i128, rate_scaled_bps_day: i128) -> i128 {
    let daily_scaled = mul_div_floor(env, size, rate_scaled_bps_day, BPS);
    daily_scaled / SECONDS_PER_DAY as i128
}

pub fn borrow_rate(env: &Env, base: i128, variable: i128, utilization: i128) -> i128 {
    let base_scaled = base
        .checked_mul(INDEX_PRECISION)
        .unwrap_or_else(|| fail(env));
    let variable_scaled = variable
        .checked_mul(INDEX_PRECISION)
        .unwrap_or_else(|| fail(env));
    let first = mul_div_floor(env, variable_scaled, utilization, BPS);
    add(
        env,
        base_scaled,
        mul_div_floor(env, first, utilization, BPS),
    )
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
        sub(env, mul_div_floor(env, base, price, PRECISION), size)
    } else {
        sub(env, size, mul_div_ceil(env, base, price, PRECISION))
    }
}

pub fn opening_fee(env: &Env, size: i128, bps: u32) -> i128 {
    mul_div_ceil(env, size, bps as i128, BPS)
}

pub fn remaining(env: &Env, value: i128, old_size: i128, new_size: i128) -> i128 {
    if new_size == 0 {
        0
    } else {
        mul_div_floor(env, value, new_size, old_size)
    }
}
