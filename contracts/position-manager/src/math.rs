//! Financial calculations for additive exposure PnL and carrying fees.
//!
//! Authoritative formulas and rounding rules are documented in
//! `docs/math/pnl-and-skew-fees.md`.

use shared::constants::{BPS, EXPOSURE_PRECISION, INDEX_PRECISION, SECONDS_PER_YEAR};
use soroban_sdk::Env;
use stellar_contract_utils::math::{mul_div_i128, Rounding};

/// Convert quote notional into additive base exposure at the execution price.
pub fn calc_base_exposure(env: &Env, size: i128, price: i128) -> i128 {
    if size <= 0 {
        return 0;
    }
    mul_div_i128(env, size, EXPOSURE_PRECISION, price, Rounding::Floor)
}

/// Display-only average entry price. Settlement never feeds this rounded value
/// back into PnL calculations.
pub fn derive_entry_price(env: &Env, size: i128, base_exposure: i128) -> i128 {
    if size <= 0 || base_exposure <= 0 {
        return 0;
    }
    mul_div_i128(
        env,
        size,
        EXPOSURE_PRECISION,
        base_exposure,
        Rounding::Floor,
    )
}

pub fn calc_mark_value(env: &Env, base_exposure: i128, mark_price: i128) -> i128 {
    if base_exposure <= 0 {
        return 0;
    }
    mul_div_i128(
        env,
        base_exposure,
        mark_price,
        EXPOSURE_PRECISION,
        Rounding::Floor,
    )
}

pub fn calc_unrealized_pnl(
    env: &Env,
    size: i128,
    base_exposure: i128,
    mark_price: i128,
    is_long: bool,
) -> i128 {
    let mark_value = calc_mark_value(env, base_exposure, mark_price);
    if is_long {
        mark_value - size
    } else {
        size - mark_value
    }
}

/// Baseline debt for a newly-added position slice at `current_index`.
pub fn calc_fee_debt(env: &Env, size: i128, current_index: i128) -> i128 {
    mul_div_i128(env, size, current_index, INDEX_PRECISION, Rounding::Floor)
}

/// Fee accrued since the position slice's debt baseline was recorded.
pub fn calc_fee_from_debt(env: &Env, size: i128, current_index: i128, debt: i128) -> i128 {
    (calc_fee_debt(env, size, current_index) - debt).max(0)
}

pub fn calc_health(
    collateral: i128,
    unrealized_pnl: i128,
    borrow_fee: i128,
    skew_fee: i128,
) -> i128 {
    collateral + unrealized_pnl - borrow_fee - skew_fee
}

pub fn calc_borrow_rate(
    utilization_bps: i128,
    base_borrow_rate: i128,
    slope1: i128,
    slope2: i128,
    optimal_util: i128,
) -> i128 {
    if utilization_bps <= optimal_util {
        base_borrow_rate + (utilization_bps * slope1 / BPS)
    } else {
        base_borrow_rate
            + (optimal_util * slope1 / BPS)
            + ((utilization_bps - optimal_util) * slope2 / BPS)
    }
}

/// Annualized dominant-side skew surcharge. Concentration is quadratic and
/// utilization is linear. The caller decides which side is dominant.
pub fn calc_skew_rate(
    env: &Env,
    long_oi: i128,
    short_oi: i128,
    utilization_bps: i128,
    max_skew_rate_bps: i128,
) -> i128 {
    let total = long_oi + short_oi;
    if total <= 0 || long_oi == short_oi || utilization_bps <= 0 {
        return 0;
    }
    let skew = (long_oi - short_oi).abs();
    let concentration = mul_div_i128(env, skew, BPS, total, Rounding::Floor).min(BPS);
    let quadratic = mul_div_i128(env, concentration, concentration, BPS, Rounding::Floor);
    let concentrated_rate = mul_div_i128(env, max_skew_rate_bps, quadratic, BPS, Rounding::Floor);
    mul_div_i128(
        env,
        concentrated_rate,
        utilization_bps.min(BPS),
        BPS,
        Rounding::Floor,
    )
}

pub fn accumulate_fee_index(
    env: &Env,
    current_index: i128,
    rate_bps: i128,
    time_delta: u64,
) -> i128 {
    if rate_bps <= 0 || time_delta == 0 {
        return current_index;
    }
    let annual_delta = mul_div_i128(env, rate_bps, INDEX_PRECISION, BPS, Rounding::Floor);
    current_index
        + mul_div_i128(
            env,
            annual_delta,
            time_delta as i128,
            SECONDS_PER_YEAR as i128,
            Rounding::Floor,
        )
}

/// Returns true if the take-profit price is triggered.
pub fn is_tp_triggered(take_profit: i128, mark_price: i128, is_long: bool) -> bool {
    take_profit > 0
        && if is_long {
            mark_price >= take_profit
        } else {
            mark_price <= take_profit
        }
}

/// Returns true if the stop-loss price is triggered.
pub fn is_sl_triggered(stop_loss: i128, mark_price: i128, is_long: bool) -> bool {
    stop_loss > 0
        && if is_long {
            mark_price <= stop_loss
        } else {
            mark_price >= stop_loss
        }
}

pub fn calc_market_unrealized_pnl(
    env: &Env,
    long_oi: i128,
    long_base_exposure: i128,
    short_oi: i128,
    short_base_exposure: i128,
    mark_price: i128,
) -> i128 {
    let long_pnl = calc_mark_value(env, long_base_exposure, mark_price) - long_oi;
    let short_pnl = short_oi - calc_mark_value(env, short_base_exposure, mark_price);
    long_pnl + short_pnl
}

/// Utilization in bps, clamped to `[0, 2 * BPS]`.
pub fn calc_utilization_bps(reserved: i128, total_assets: i128) -> i128 {
    if total_assets <= 0 {
        return if reserved > 0 { 2 * BPS } else { 0 };
    }
    let util = reserved.saturating_mul(BPS) / total_assets;
    util.min(2 * BPS)
}

pub fn calc_open_fee(size: i128, open_fee_bps: u32) -> i128 {
    if size <= 0 {
        return 0;
    }
    size * (open_fee_bps as i128) / BPS
}

pub fn calc_liquidation_bounty(collateral: i128, bounty_bps: u32) -> i128 {
    if collateral <= 0 {
        return 0;
    }
    collateral * (bounty_bps as i128) / BPS
}
