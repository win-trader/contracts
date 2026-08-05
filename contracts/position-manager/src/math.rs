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

use shared::constants::{BPS, INDEX_PRECISION, PRICE_PRECISION, SECONDS_PER_DAY};

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

/// §8.1 — normalized base-exposure skew in bps: 0 balanced, `BPS` one-sided.
pub fn skew_bps(env: &Env, long_base: i128, short_base: i128) -> i128 {
    let total = add(env, long_base, short_base);
    if total == 0 {
        return 0;
    }
    mul_div_floor(env, (long_base - short_base).abs(), BPS, total)
}

/// §8.1 — quadratic payer rate: `max_rate * skew² / BPS²`, scaled by
/// `INDEX_PRECISION` (bps/day input, `INDEX_PRECISION`-scaled bps/day output).
pub fn funding_rate(env: &Env, max_rate: i128, skew: i128) -> i128 {
    let max_scaled = mul(env, max_rate, INDEX_PRECISION);
    let first = mul_div_floor(env, max_scaled, skew, BPS);
    mul_div_floor(env, first, skew, BPS)
}

/// Cash flow per second at `INDEX_PRECISION`, from an `INDEX_PRECISION`-scaled
/// bps/day rate.
pub fn flow_per_second(env: &Env, size: i128, rate_scaled_bps_day: i128) -> i128 {
    let daily_scaled = mul_div_floor(env, size, rate_scaled_bps_day, BPS);
    daily_scaled / SECONDS_PER_DAY as i128
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
        assert_eq!(skew_bps(&e, 0, 0), 0);
        assert_eq!(skew_bps(&e, 500, 500), 0);
        assert_eq!(skew_bps(&e, 1_000, 0), BPS);
        assert_eq!(skew_bps(&e, 0, 1_000), BPS);
        // 75/25 split → |50| * BPS / 100 = 5_000
        assert_eq!(skew_bps(&e, 75, 25), 5_000);
    }

    #[test]
    fn funding_rate_is_quadratic_in_skew() {
        let e = env();
        let max_rate = 100; // bps/day
        assert_eq!(funding_rate(&e, max_rate, 0), 0);
        let full = funding_rate(&e, max_rate, BPS);
        assert_eq!(full, max_rate * INDEX_PRECISION);
        // Half skew → quarter rate.
        let half = funding_rate(&e, max_rate, BPS / 2);
        assert_eq!(half, max_rate * INDEX_PRECISION / 4);
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
