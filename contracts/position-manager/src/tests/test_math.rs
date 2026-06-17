// Comprehensive tests for the position-manager math module.
// All functions under test are pure (no Env dependency).
// These tests are written BEFORE implementation and should ALL FAIL initially.

use crate::math;

// Default borrow/funding rate constants used in tests (previously hardcoded in math.rs).
const BASE_BORROW_RATE: i128 = 100;
const SLOPE1: i128 = 500;
const SLOPE2: i128 = 5_000;
const OPTIMAL_UTIL: i128 = 8_000;
const BASE_FUNDING_RATE: i128 = 100;

// ========================================================================
// 1. calc_unrealized_pnl
// ========================================================================

#[test]
fn test_unrealized_pnl_long_profit() {
    // BTC long: size=$100k, entry=50k, mark=55k => PnL = +$10k
    let pnl = math::calc_unrealized_pnl(
        100_000 * shared::constants::PRECISION,
        50_000 * shared::constants::PRECISION,
        55_000 * shared::constants::PRECISION,
        true,
    );
    assert_eq!(pnl, 10_000 * shared::constants::PRECISION);
}

#[test]
fn test_unrealized_pnl_long_loss() {
    // BTC long: size=$100k, entry=50k, mark=45k => PnL = -$10k
    let pnl = math::calc_unrealized_pnl(
        100_000 * shared::constants::PRECISION,
        50_000 * shared::constants::PRECISION,
        45_000 * shared::constants::PRECISION,
        true,
    );
    assert_eq!(pnl, -10_000 * shared::constants::PRECISION);
}

#[test]
fn test_unrealized_pnl_short_profit() {
    // BTC short: size=$100k, entry=50k, mark=45k => PnL = +$10k
    let pnl = math::calc_unrealized_pnl(
        100_000 * shared::constants::PRECISION,
        50_000 * shared::constants::PRECISION,
        45_000 * shared::constants::PRECISION,
        false,
    );
    assert_eq!(pnl, 10_000 * shared::constants::PRECISION);
}

#[test]
fn test_unrealized_pnl_short_loss() {
    // BTC short: size=$100k, entry=50k, mark=55k => PnL = -$10k
    let pnl = math::calc_unrealized_pnl(
        100_000 * shared::constants::PRECISION,
        50_000 * shared::constants::PRECISION,
        55_000 * shared::constants::PRECISION,
        false,
    );
    assert_eq!(pnl, -10_000 * shared::constants::PRECISION);
}

#[test]
fn test_unrealized_pnl_zero_price_move() {
    // No price movement => PnL = 0
    let pnl = math::calc_unrealized_pnl(
        100_000 * shared::constants::PRECISION,
        50_000 * shared::constants::PRECISION,
        50_000 * shared::constants::PRECISION,
        true,
    );
    assert_eq!(pnl, 0);
}

#[test]
fn test_unrealized_pnl_zero_size() {
    // Zero size => PnL = 0 regardless of price move
    let pnl = math::calc_unrealized_pnl(
        0,
        50_000 * shared::constants::PRECISION,
        55_000 * shared::constants::PRECISION,
        true,
    );
    assert_eq!(pnl, 0);
}

#[test]
fn test_unrealized_pnl_large_values_no_overflow() {
    // Whale position: size=$500M at BTC=$100k, mark=$100.5k (0.5% move)
    // PnL = 500_000_000e7 * (100_500e7 - 100_000e7) / 100_000e7 = 2_500_000e7
    let pnl = math::calc_unrealized_pnl(
        500_000_000 * shared::constants::PRECISION,
        100_000 * shared::constants::PRECISION,
        100_500 * shared::constants::PRECISION,
        true,
    );
    assert_eq!(pnl, 2_500_000 * shared::constants::PRECISION);
}

#[test]
fn test_unrealized_pnl_fractional_precision() {
    // Small position: size=$10, entry=$1.50, mark=$1.55
    // PnL = 10e7 * (1.55e7 - 1.50e7) / 1.50e7
    //      = 100_000_000 * 500_000 / 15_000_000 = 3_333_333 (truncated)
    let pnl = math::calc_unrealized_pnl(
        10 * shared::constants::PRECISION,  // $10
        15_000_000,            // $1.50
        15_500_000,            // $1.55
        true,
    );
    assert_eq!(pnl, 3_333_333); // ~$0.3333333 profit
}

// ========================================================================
// 2. calc_borrow_fee
// ========================================================================

#[test]
fn test_borrow_fee_basic() {
    // Size=$100k, index went from 1e14 to 1.001e14 (0.1% accrued)
    // Fee = (1.001e14 - 1e14) * 100_000e7 / 1e14
    //     = 1e11 * 1e12 / 1e14 = 1e9 = 100e7 => $100
    let fee = math::calc_borrow_fee(
        100_000 * shared::constants::PRECISION,
        shared::constants::INDEX_PRECISION,
        shared::constants::INDEX_PRECISION + shared::constants::INDEX_PRECISION / 1000,
    );
    assert_eq!(fee, 100 * shared::constants::PRECISION);
}

#[test]
fn test_borrow_fee_zero_when_indices_equal() {
    let fee = math::calc_borrow_fee(
        100_000 * shared::constants::PRECISION,
        shared::constants::INDEX_PRECISION,
        shared::constants::INDEX_PRECISION,
    );
    assert_eq!(fee, 0);
}

#[test]
fn test_borrow_fee_zero_size() {
    let fee = math::calc_borrow_fee(
        0,
        shared::constants::INDEX_PRECISION,
        shared::constants::INDEX_PRECISION * 2,
    );
    assert_eq!(fee, 0);
}

// ========================================================================
// 3. calc_funding_fee
// ========================================================================

#[test]
fn test_funding_fee_long_pays_when_delta_positive() {
    // Positive delta means longs pay. Long fee = -(delta * size / INDEX_PRECISION)
    // delta = 0.001 * INDEX_PRECISION, size = $100k
    // fee = -(1e11 * 1e12 / 1e14) = -1e9 = -100e7 => trader pays $100
    let fee = math::calc_funding_fee(
        100_000 * shared::constants::PRECISION,
        shared::constants::INDEX_PRECISION,
        shared::constants::INDEX_PRECISION + shared::constants::INDEX_PRECISION / 1000,
        true, // long
    );
    assert_eq!(fee, -(100 * shared::constants::PRECISION));
}

#[test]
fn test_funding_fee_short_receives_when_delta_positive() {
    // Positive delta means shorts receive. Short fee = delta * size / INDEX_PRECISION
    let fee = math::calc_funding_fee(
        100_000 * shared::constants::PRECISION,
        shared::constants::INDEX_PRECISION,
        shared::constants::INDEX_PRECISION + shared::constants::INDEX_PRECISION / 1000,
        false, // short
    );
    assert_eq!(fee, 100 * shared::constants::PRECISION);
}

#[test]
fn test_funding_fee_long_receives_when_delta_negative() {
    // Negative delta => longs receive
    let fee = math::calc_funding_fee(
        100_000 * shared::constants::PRECISION,
        shared::constants::INDEX_PRECISION + shared::constants::INDEX_PRECISION / 1000,
        shared::constants::INDEX_PRECISION,
        true,
    );
    assert_eq!(fee, 100 * shared::constants::PRECISION);
}

#[test]
fn test_funding_fee_zero_delta() {
    let fee = math::calc_funding_fee(
        100_000 * shared::constants::PRECISION,
        shared::constants::INDEX_PRECISION,
        shared::constants::INDEX_PRECISION,
        true,
    );
    assert_eq!(fee, 0);
}

// ========================================================================
// 4. calc_health
// ========================================================================

#[test]
fn test_health_all_positive() {
    // collateral=$1000, pnl=+$200, borrow_fee=$50, funding_fee=+$30 (receiving)
    // health = 1000 + 200 - 50 + 30 = 1180
    let h = math::calc_health(
        1000 * shared::constants::PRECISION,
        200 * shared::constants::PRECISION,
        50 * shared::constants::PRECISION,
        30 * shared::constants::PRECISION,
    );
    assert_eq!(h, 1180 * shared::constants::PRECISION);
}

#[test]
fn test_health_negative_pnl_and_funding() {
    // collateral=$1000, pnl=-$800, borrow_fee=$100, funding_fee=-$50 (paying)
    // health = 1000 + (-800) - 100 + (-50) = 50
    let h = math::calc_health(
        1000 * shared::constants::PRECISION,
        -800 * shared::constants::PRECISION,
        100 * shared::constants::PRECISION,
        -50 * shared::constants::PRECISION,
    );
    assert_eq!(h, 50 * shared::constants::PRECISION);
}

#[test]
fn test_health_goes_negative_liquidatable() {
    // collateral=$100, pnl=-$80, borrow_fee=$30, funding_fee=-$10
    // health = 100 - 80 - 30 - 10 = -20 (underwater)
    let h = math::calc_health(
        100 * shared::constants::PRECISION,
        -80 * shared::constants::PRECISION,
        30 * shared::constants::PRECISION,
        -10 * shared::constants::PRECISION,
    );
    assert_eq!(h, -20 * shared::constants::PRECISION);
}

#[test]
fn test_health_zero_collateral() {
    let h = math::calc_health(0, 0, 0, 0);
    assert_eq!(h, 0);
}

// ========================================================================
// 5. calc_borrow_rate (kink model)
// ========================================================================

#[test]
fn test_borrow_rate_zero_utilization() {
    // U=0: rate = BASE = 100 BPS = 1%
    let rate = math::calc_borrow_rate(0, BASE_BORROW_RATE, SLOPE1, SLOPE2, OPTIMAL_UTIL);
    assert_eq!(rate, BASE_BORROW_RATE);
}

#[test]
fn test_borrow_rate_at_optimal() {
    // U=8000 (80%): rate = 100 + (8000 * 500 / 10000) = 100 + 400 = 500 BPS = 5%
    let rate = math::calc_borrow_rate(OPTIMAL_UTIL, BASE_BORROW_RATE, SLOPE1, SLOPE2, OPTIMAL_UTIL);
    assert_eq!(rate, 500);
}

#[test]
fn test_borrow_rate_below_optimal() {
    // U=4000 (40%): rate = 100 + (4000 * 500 / 10000) = 100 + 200 = 300 BPS
    let rate = math::calc_borrow_rate(4000, BASE_BORROW_RATE, SLOPE1, SLOPE2, OPTIMAL_UTIL);
    assert_eq!(rate, 300);
}

#[test]
fn test_borrow_rate_above_optimal() {
    // U=9000 (90%): rate = 100 + 400 + ((9000-8000)*5000/10000) = 500 + 500 = 1000 BPS = 10%
    let rate = math::calc_borrow_rate(9000, BASE_BORROW_RATE, SLOPE1, SLOPE2, OPTIMAL_UTIL);
    assert_eq!(rate, 1000);
}

#[test]
fn test_borrow_rate_full_utilization() {
    // U=10000 (100%): rate = 100 + 400 + ((10000-8000)*5000/10000) = 500 + 1000 = 1500 BPS = 15%
    let rate = math::calc_borrow_rate(shared::constants::BPS, BASE_BORROW_RATE, SLOPE1, SLOPE2, OPTIMAL_UTIL);
    assert_eq!(rate, 1500);
}

// ========================================================================
// 6. calc_funding_rate
// ========================================================================

#[test]
fn test_funding_rate_balanced() {
    // Equal OI => rate = 0
    let rate = math::calc_funding_rate(
        1_000_000 * shared::constants::PRECISION,
        1_000_000 * shared::constants::PRECISION,
        BASE_FUNDING_RATE,
    );
    assert_eq!(rate, 0);
}

#[test]
fn test_funding_rate_longs_dominant() {
    // long=150k, short=50k => rate = 100 * (150k-50k)/(150k+50k) = 100*100k/200k = 50
    let rate = math::calc_funding_rate(
        150_000 * shared::constants::PRECISION,
        50_000 * shared::constants::PRECISION,
        BASE_FUNDING_RATE,
    );
    assert_eq!(rate, 50); // 0.5% annualized, longs pay
}

#[test]
fn test_funding_rate_shorts_dominant() {
    // long=50k, short=150k => rate = 100 * (50k-150k)/(50k+150k) = -50
    let rate = math::calc_funding_rate(
        50_000 * shared::constants::PRECISION,
        150_000 * shared::constants::PRECISION,
        BASE_FUNDING_RATE,
    );
    assert_eq!(rate, -50); // shorts pay
}

#[test]
fn test_funding_rate_zero_oi() {
    // No open interest => rate = 0 (no division by zero)
    let rate = math::calc_funding_rate(0, 0, BASE_FUNDING_RATE);
    assert_eq!(rate, 0);
}

#[test]
fn test_funding_rate_one_sided_all_longs() {
    // All longs, zero shorts => rate = 100 * long / long = 100
    let rate = math::calc_funding_rate(100_000 * shared::constants::PRECISION, 0, BASE_FUNDING_RATE);
    assert_eq!(rate, BASE_FUNDING_RATE);
}

#[test]
fn test_funding_rate_one_sided_all_shorts() {
    // All shorts, zero longs => rate = 100 * (-short) / short = -100
    let rate = math::calc_funding_rate(0, 100_000 * shared::constants::PRECISION, BASE_FUNDING_RATE);
    assert_eq!(rate, -(BASE_FUNDING_RATE));
}

// ========================================================================
// 7. accumulate_borrow_index
// ========================================================================

#[test]
fn test_accumulate_borrow_index_one_hour() {
    // rate=500 BPS (5%), time=3600s (1 hour)
    let new_idx = math::accumulate_borrow_index(
        shared::constants::INDEX_PRECISION,
        500,
        3600,
    );
    let expected_delta: i128 =
        500 * shared::constants::INDEX_PRECISION * 3600 / (shared::constants::BPS * shared::constants::SECONDS_PER_YEAR as i128);
    assert_eq!(new_idx, shared::constants::INDEX_PRECISION + expected_delta);
}

#[test]
fn test_accumulate_borrow_index_zero_time() {
    let new_idx = math::accumulate_borrow_index(shared::constants::INDEX_PRECISION, 500, 0);
    assert_eq!(new_idx, shared::constants::INDEX_PRECISION);
}

#[test]
fn test_accumulate_borrow_index_zero_rate() {
    let new_idx = math::accumulate_borrow_index(shared::constants::INDEX_PRECISION, 0, 3600);
    assert_eq!(new_idx, shared::constants::INDEX_PRECISION);
}

#[test]
fn test_accumulate_borrow_index_full_year() {
    // rate=100 BPS (1%), time=1 year
    // delta = 100 * INDEX_PRECISION * SECONDS_PER_YEAR / (BPS * SECONDS_PER_YEAR)
    //       = 100 * INDEX_PRECISION / BPS = INDEX_PRECISION / 100
    let new_idx = math::accumulate_borrow_index(
        shared::constants::INDEX_PRECISION,
        100,
        shared::constants::SECONDS_PER_YEAR,
    );
    let expected_delta: i128 = shared::constants::INDEX_PRECISION / 100; // 1% of index
    assert_eq!(new_idx, shared::constants::INDEX_PRECISION + expected_delta);
}

// ========================================================================
// 8. accumulate_funding_index
// ========================================================================

#[test]
fn test_accumulate_funding_index_positive_rate() {
    let new_idx = math::accumulate_funding_index(shared::constants::INDEX_PRECISION, 50, 3600);
    let expected_delta: i128 =
        50 * shared::constants::INDEX_PRECISION * 3600 / (shared::constants::BPS * shared::constants::SECONDS_PER_YEAR as i128);
    assert_eq!(new_idx, shared::constants::INDEX_PRECISION + expected_delta);
}

#[test]
fn test_accumulate_funding_index_negative_rate() {
    // Negative rate => index decreases
    let new_idx = math::accumulate_funding_index(shared::constants::INDEX_PRECISION, -50, 3600);
    let expected_delta: i128 =
        -50 * shared::constants::INDEX_PRECISION * 3600 / (shared::constants::BPS * shared::constants::SECONDS_PER_YEAR as i128);
    assert_eq!(new_idx, shared::constants::INDEX_PRECISION + expected_delta);
    assert!(
        new_idx < shared::constants::INDEX_PRECISION,
        "Negative rate must decrease the index"
    );
}

// ========================================================================
// 9. update_global_avg_price
// ========================================================================

#[test]
fn test_avg_price_first_position() {
    // No existing size => avg = new_price
    let avg = math::update_global_avg_price(
        0,
        0,
        50_000 * shared::constants::PRECISION,
        10_000 * shared::constants::PRECISION,
    );
    assert_eq!(avg, 50_000 * shared::constants::PRECISION);
}

#[test]
fn test_avg_price_weighted() {
    // Existing: avg=$50k, size=$100k. New: price=$60k, size=$50k
    // new_avg = (50k*100k + 60k*50k) / (100k+50k) = 8e9/150k = ~53333.33
    let avg = math::update_global_avg_price(
        50_000 * shared::constants::PRECISION,
        100_000 * shared::constants::PRECISION,
        60_000 * shared::constants::PRECISION,
        50_000 * shared::constants::PRECISION,
    );
    let expected = (50_000 * shared::constants::PRECISION * 100_000 * shared::constants::PRECISION
        + 60_000 * shared::constants::PRECISION * 50_000 * shared::constants::PRECISION)
        / (150_000 * shared::constants::PRECISION);
    assert_eq!(avg, expected);
}

#[test]
fn test_avg_price_both_sizes_zero() {
    let avg = math::update_global_avg_price(
        50_000 * shared::constants::PRECISION,
        0,
        60_000 * shared::constants::PRECISION,
        0,
    );
    assert_eq!(avg, 0);
}

// ========================================================================
// 10. calc_utilization_bps
// ========================================================================

#[test]
fn test_utilization_basic() {
    // reserved=$800k, total=$1M => 8000 BPS = 80%
    let u = math::calc_utilization_bps(
        800_000 * shared::constants::PRECISION,
        1_000_000 * shared::constants::PRECISION,
    );
    assert_eq!(u, 8000);
}

#[test]
fn test_utilization_zero_assets() {
    // Reservations against a zero basis read as the clamp maximum, not 0 —
    // the rate curve and the open gate must fail toward "fully utilized".
    let u = math::calc_utilization_bps(100 * shared::constants::PRECISION, 0);
    assert_eq!(u, 2 * shared::constants::BPS);
}

#[test]
fn test_utilization_negative_assets() {
    let u = math::calc_utilization_bps(100 * shared::constants::PRECISION, -1);
    assert_eq!(u, 2 * shared::constants::BPS);
}

#[test]
fn test_utilization_zero_reserved_zero_assets() {
    // Nothing reserved against nothing: an idle market reads 0.
    let u = math::calc_utilization_bps(0, 0);
    assert_eq!(u, 0);
}

#[test]
fn test_utilization_clamped_at_two_hundred_percent() {
    // 300% raw utilization clamps to the 2 * BPS ceiling.
    let u = math::calc_utilization_bps(3_000, 1_000);
    assert_eq!(u, 2 * shared::constants::BPS);
}

#[test]
fn test_utilization_full() {
    // reserved == total => 10000 BPS = 100%
    let u = math::calc_utilization_bps(
        1_000_000 * shared::constants::PRECISION,
        1_000_000 * shared::constants::PRECISION,
    );
    assert_eq!(u, shared::constants::BPS);
}

#[test]
fn test_utilization_zero_reserved() {
    let u = math::calc_utilization_bps(0, 1_000_000 * shared::constants::PRECISION);
    assert_eq!(u, 0);
}

// ========================================================================
// Adversarial / edge-case scenarios
// ========================================================================

#[test]
fn test_pnl_short_price_doubles_max_loss() {
    // Short at $50k, price goes to $100k => PnL = size * (50k-100k)/50k = -size
    let pnl = math::calc_unrealized_pnl(
        100_000 * shared::constants::PRECISION,
        50_000 * shared::constants::PRECISION,
        100_000 * shared::constants::PRECISION,
        false,
    );
    assert_eq!(pnl, -100_000 * shared::constants::PRECISION);
}

#[test]
fn test_pnl_long_price_goes_to_near_zero() {
    // Long at $50k, price crashes to $1 => nearly total loss
    let pnl = math::calc_unrealized_pnl(
        100_000 * shared::constants::PRECISION,
        50_000 * shared::constants::PRECISION,
        1 * shared::constants::PRECISION,
        true,
    );
    let expected = 100_000 * shared::constants::PRECISION * (1 * shared::constants::PRECISION - 50_000 * shared::constants::PRECISION)
        / (50_000 * shared::constants::PRECISION);
    assert_eq!(pnl, expected);
}

#[test]
fn test_borrow_fee_large_index_gap() {
    // Position held for ages: index went from 1.0 to 2.0 (100% cumulative cost)
    let fee = math::calc_borrow_fee(
        100_000 * shared::constants::PRECISION,
        shared::constants::INDEX_PRECISION,
        2 * shared::constants::INDEX_PRECISION,
    );
    assert_eq!(fee, 100_000 * shared::constants::PRECISION);
}

#[test]
fn test_health_exactly_zero_liquidation_boundary() {
    // collateral=$100, pnl=-$80, borrow=$20, funding=0 => health=0
    let h = math::calc_health(
        100 * shared::constants::PRECISION,
        -80 * shared::constants::PRECISION,
        20 * shared::constants::PRECISION,
        0,
    );
    assert_eq!(h, 0);
}

#[test]
fn test_borrow_rate_above_10000_bps() {
    // Adversarial: utilization beyond 100% (12000 BPS passed in)
    // rate = 100 + 400 + (12000-8000)*5000/10000 = 500 + 2000 = 2500
    let rate = math::calc_borrow_rate(12_000, BASE_BORROW_RATE, SLOPE1, SLOPE2, OPTIMAL_UTIL);
    assert_eq!(rate, 2500);
}

#[test]
fn test_funding_rate_extreme_imbalance() {
    // Massive long imbalance — should not overflow
    let big = i128::MAX / (2 * shared::constants::PRECISION);
    let rate = math::calc_funding_rate(big * shared::constants::PRECISION, 1 * shared::constants::PRECISION, BASE_FUNDING_RATE);
    // (big - 1) / (big + 1) ~ 1 for big >> 1, so rate ~ BASE_FUNDING_RATE
    assert!(
        rate >= BASE_FUNDING_RATE - 1 && rate <= BASE_FUNDING_RATE,
        "Extreme imbalance rate should be near BASE_FUNDING_RATE, got {}",
        rate
    );
}

#[test]
fn test_accumulate_borrow_index_large_time_delta() {
    // 10 years at 15% rate (1500 BPS) — must not overflow
    let new_idx = math::accumulate_borrow_index(
        shared::constants::INDEX_PRECISION,
        1500,
        shared::constants::SECONDS_PER_YEAR * 10,
    );
    let expected_delta: i128 = 1500 * shared::constants::INDEX_PRECISION
        * (shared::constants::SECONDS_PER_YEAR * 10) as i128
        / (shared::constants::BPS * shared::constants::SECONDS_PER_YEAR as i128);
    assert_eq!(new_idx, shared::constants::INDEX_PRECISION + expected_delta);
}

#[test]
fn test_avg_price_add_zero_size_returns_current() {
    // Adding 0 size should not change the average
    let avg = math::update_global_avg_price(
        50_000 * shared::constants::PRECISION,
        100_000 * shared::constants::PRECISION,
        999_999 * shared::constants::PRECISION, // wild price, but size=0 so irrelevant
        0,
    );
    assert_eq!(avg, 50_000 * shared::constants::PRECISION);
}

// ========================================================================
// 11. calc_open_fee
// fee = size * open_fee_bps / BPS, defensively zero for non-positive size.
// ========================================================================

#[test]
fn test_open_fee_zero_size_returns_zero() {
    let fee = math::calc_open_fee(0, shared::constants::DEFAULT_OPEN_FEE_BPS);
    assert_eq!(fee, 0);
}

#[test]
fn test_open_fee_negative_size_returns_zero() {
    // Defensive: callers should never pass negative size, but the function
    // must never emit a negative fee or panic. A negative fee would be
    // catastrophic — it would credit the trader instead of charging them.
    let fee = math::calc_open_fee(-1_000_000_000, shared::constants::DEFAULT_OPEN_FEE_BPS);
    assert_eq!(fee, 0);
}

#[test]
fn test_open_fee_default_rate_on_1000_usdc_notional() {
    // size = $1000 notional at 7-decimal precision, open_fee_bps = 10 (default 0.1%)
    // fee = 10_000_000_000 * 10 / 10_000 = 10_000_000 ($1)
    let size = 1_000 * shared::constants::PRECISION;
    let fee = math::calc_open_fee(size, shared::constants::DEFAULT_OPEN_FEE_BPS);
    assert_eq!(fee, 1 * shared::constants::PRECISION);
}

#[test]
fn test_open_fee_max_rate_on_1_usdc_notional() {
    // size = $1, open_fee_bps = MAX_OPEN_FEE_BPS (100 bps = 1%)
    // fee = 10_000_000 * 100 / 10_000 = 100_000 ($0.01)
    let size = 1 * shared::constants::PRECISION;
    let fee = math::calc_open_fee(size, shared::constants::MAX_OPEN_FEE_BPS);
    assert_eq!(fee, 100_000);
}

#[test]
fn test_open_fee_zero_bps_returns_zero() {
    // Free opens (bps=0) must produce a zero fee.
    let size = 1_000_000 * shared::constants::PRECISION;
    let fee = math::calc_open_fee(size, 0);
    assert_eq!(fee, 0);
}

#[test]
fn test_open_fee_boundary_max_bps_exact_math() {
    // size=10_000 (raw), bps=100 => fee = 10_000 * 100 / 10_000 = 100
    let fee = math::calc_open_fee(10_000, shared::constants::MAX_OPEN_FEE_BPS);
    assert_eq!(fee, 100);
}

#[test]
fn test_open_fee_large_size_near_overflow_boundary() {
    // Adversarial: pick size = i128::MAX / BPS and bps=1.
    // Numerator = size * 1 = i128::MAX / BPS — does not overflow.
    // Result   = (i128::MAX / BPS) / BPS.
    let size = i128::MAX / shared::constants::BPS;
    let fee = math::calc_open_fee(size, 1);
    let expected = size * 1 / shared::constants::BPS;
    assert_eq!(fee, expected);
}

// ========================================================================
// 12. calc_liquidation_bounty
// bounty = collateral * bounty_bps / BPS, defensively zero for non-positive collateral.
// ========================================================================

#[test]
fn test_liquidation_bounty_zero_collateral_returns_zero() {
    let bounty = math::calc_liquidation_bounty(0, shared::constants::DEFAULT_LIQUIDATION_BOUNTY_BPS);
    assert_eq!(bounty, 0);
}

#[test]
fn test_liquidation_bounty_negative_collateral_returns_zero() {
    // Defensive: a liquidator must never receive a negative bounty (which
    // would invert the cashflow direction). Negative collateral is impossible
    // in normal flow, but the math must not panic or produce signed garbage.
    let bounty = math::calc_liquidation_bounty(
        -1_000_000_000,
        shared::constants::DEFAULT_LIQUIDATION_BOUNTY_BPS,
    );
    assert_eq!(bounty, 0);
}

#[test]
fn test_liquidation_bounty_default_rate_on_1000_usdc_collateral() {
    // collateral = $1000, bounty_bps = 100 (default 1%)
    // bounty = 10_000_000_000 * 100 / 10_000 = 100_000_000 ($10)
    let collateral = 1_000 * shared::constants::PRECISION;
    let bounty = math::calc_liquidation_bounty(
        collateral,
        shared::constants::DEFAULT_LIQUIDATION_BOUNTY_BPS,
    );
    assert_eq!(bounty, 10 * shared::constants::PRECISION);
}

#[test]
fn test_liquidation_bounty_zero_bps_returns_zero() {
    // No-bounty configuration must produce zero.
    let collateral = 1_000_000 * shared::constants::PRECISION;
    let bounty = math::calc_liquidation_bounty(collateral, 0);
    assert_eq!(bounty, 0);
}

#[test]
fn test_liquidation_bounty_max_bps_is_one_tenth_of_collateral() {
    // collateral arbitrary, bounty_bps = MAX_LIQUIDATION_BOUNTY_BPS (1000 = 10%)
    // bounty = collateral * 1000 / 10_000 = collateral / 10
    let collateral = 2_500 * shared::constants::PRECISION;
    let bounty = math::calc_liquidation_bounty(
        collateral,
        shared::constants::MAX_LIQUIDATION_BOUNTY_BPS,
    );
    assert_eq!(bounty, collateral / 10);
}

#[test]
fn test_liquidation_bounty_boundary_exact_math() {
    // collateral=10_000 (raw), bps=MAX (1000) => bounty = 10_000 * 1000 / 10_000 = 1000
    let bounty = math::calc_liquidation_bounty(10_000, shared::constants::MAX_LIQUIDATION_BOUNTY_BPS);
    assert_eq!(bounty, 1_000);
}

#[test]
fn test_liquidation_bounty_large_collateral_near_overflow_boundary() {
    // Adversarial: collateral = i128::MAX / BPS, bps = 1 => no overflow.
    let collateral = i128::MAX / shared::constants::BPS;
    let bounty = math::calc_liquidation_bounty(collateral, 1);
    let expected = collateral * 1 / shared::constants::BPS;
    assert_eq!(bounty, expected);
}
