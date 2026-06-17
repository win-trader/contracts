// Pure financial calculation functions — no Env dependency.
// All numerical constants live in `shared::constants` (single source of truth).

use shared::constants::{BPS, INDEX_PRECISION, SECONDS_PER_YEAR};


pub fn calc_unrealized_pnl(
    size: i128,
    entry_price: i128,
    mark_price: i128,
    is_long: bool,
) -> i128 {
    if entry_price == 0 || size == 0 {
        return 0;
    }
    let price_diff = if is_long {
        mark_price - entry_price
    } else {
        entry_price - mark_price
    };
    size * price_diff / entry_price
}

pub fn calc_borrow_fee(
    size: i128,
    entry_borrow_index: i128,
    current_borrow_index: i128,
) -> i128 {
    (current_borrow_index - entry_borrow_index) * size / INDEX_PRECISION
}

pub fn calc_funding_fee(
    size: i128,
    entry_funding_index: i128,
    current_funding_index: i128,
    is_long: bool,
) -> i128 {
    let delta = current_funding_index - entry_funding_index;
    if is_long {
        -(delta * size / INDEX_PRECISION)
    } else {
        delta * size / INDEX_PRECISION
    }
}

pub fn calc_health(
    collateral: i128,
    unrealized_pnl: i128,
    borrow_fee: i128,
    funding_fee: i128,
) -> i128 {
    collateral + unrealized_pnl - borrow_fee + funding_fee
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

pub fn calc_funding_rate(long_oi: i128, short_oi: i128, base_funding_rate: i128) -> i128 {
    let total = long_oi + short_oi;
    if total == 0 {
        return 0;
    }
    let imbalance = long_oi - short_oi;
    match imbalance.checked_mul(base_funding_rate) {
        Some(product) => product / total,
        None => {
            // Progressive halving preserves the imbalance/total ratio
            // better than dividing by a shared scale factor.
            let mut im = imbalance;
            let mut tot = total;
            loop {
                im /= 2;
                tot /= 2;
                if tot == 0 {
                    return 0;
                }
                if let Some(product) = im.checked_mul(base_funding_rate) {
                    return product / tot;
                }
            }
        }
    }
}

pub fn accumulate_borrow_index(
    current_index: i128,
    rate_bps: i128,
    time_delta: u64,
) -> i128 {
    current_index
        + (rate_bps * INDEX_PRECISION * time_delta as i128)
            / (BPS * SECONDS_PER_YEAR as i128)
}

pub fn accumulate_funding_index(
    current_index: i128,
    rate_bps: i128,
    time_delta: u64,
) -> i128 {
    current_index
        + (rate_bps * INDEX_PRECISION * time_delta as i128)
            / (BPS * SECONDS_PER_YEAR as i128)
}

/// Funding-pool credit for one index accrual interval: the payer side's
/// aggregate accrual. A positive index delta means longs pay (long-heavy
/// market), a negative delta means shorts pay. Floor division matches the
/// per-position `calc_funding_fee` rounding, so the pool never under-funds
/// the receivers' floored claims.
pub fn calc_funding_pool_accrual(index_delta: i128, long_oi: i128, short_oi: i128) -> i128 {
    if index_delta == 0 {
        return 0;
    }
    let payer_oi = if index_delta > 0 { long_oi } else { short_oi };
    let abs_delta = if index_delta < 0 { -index_delta } else { index_delta };
    abs_delta * payer_oi / INDEX_PRECISION
}

pub fn update_global_avg_price(
    current_avg: i128,
    current_size: i128,
    new_price: i128,
    new_size: i128,
) -> i128 {
    let total_size = current_size + new_size;
    if total_size == 0 {
        return 0;
    }
    if current_size == 0 {
        return new_price;
    }
    (current_avg * current_size + new_price * new_size) / total_size
}

/// Returns true if the take-profit price is triggered.
/// For longs: mark_price >= take_profit. For shorts: mark_price <= take_profit.
pub fn is_tp_triggered(take_profit: i128, mark_price: i128, is_long: bool) -> bool {
    if take_profit <= 0 {
        return false;
    }
    if is_long {
        mark_price >= take_profit
    } else {
        mark_price <= take_profit
    }
}

/// Returns true if the stop-loss price is triggered.
/// For longs: mark_price <= stop_loss. For shorts: mark_price >= stop_loss.
pub fn is_sl_triggered(stop_loss: i128, mark_price: i128, is_long: bool) -> bool {
    if stop_loss <= 0 {
        return false;
    }
    if is_long {
        mark_price <= stop_loss
    } else {
        mark_price >= stop_loss
    }
}

/// Recalculate the global average price after removing a portion of OI.
/// Returns 0 if the remaining OI is zero or the result would be negative.
pub fn remove_from_global_avg_price(
    current_avg: i128,
    current_oi: i128,
    removed_entry_price: i128,
    removed_size: i128,
) -> i128 {
    let remaining_oi = current_oi - removed_size;
    if remaining_oi <= 0 {
        return 0;
    }
    let numerator = current_avg * current_oi - removed_entry_price * removed_size;
    if numerator <= 0 {
        return 0;
    }
    numerator / remaining_oi
}

/// Calculate the unrealized PnL for an entire market side.
/// Long PnL = long_oi * (mark - long_avg) / long_avg
/// Short PnL = short_oi * (short_avg - mark) / short_avg
/// Returns the net (long_pnl + short_pnl).
pub fn calc_market_unrealized_pnl(
    long_oi: i128,
    long_avg: i128,
    short_oi: i128,
    short_avg: i128,
    mark_price: i128,
) -> i128 {
    let long_pnl = if long_oi > 0 && long_avg > 0 {
        long_oi * (mark_price - long_avg) / long_avg
    } else {
        0
    };
    let short_pnl = if short_oi > 0 && short_avg > 0 {
        short_oi * (short_avg - mark_price) / short_avg
    } else {
        0
    };
    long_pnl + short_pnl
}

/// Utilization in bps, clamped to `[0, 2 * BPS]`. A zero/negative basis with
/// outstanding reservations reads as the clamp maximum, NOT zero: the vault
/// is at its most stressed exactly when the LP basis is exhausted, so the
/// borrow-rate curve and the open gate must both fail toward "fully
/// utilized" rather than "empty".
pub fn calc_utilization_bps(reserved: i128, total_assets: i128) -> i128 {
    if total_assets <= 0 {
        return if reserved > 0 { 2 * BPS } else { 0 };
    }
    let util = reserved * BPS / total_assets;
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
