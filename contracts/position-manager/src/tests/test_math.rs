use soroban_sdk::Env;

use crate::math;
use shared::constants::{BPS, EXPOSURE_PRECISION, INDEX_PRECISION, PRECISION, SECONDS_PER_YEAR};

#[test]
fn exposure_pnl_matches_entry_price_formula() {
    let env = Env::default();
    let size = 10_000 * PRECISION;
    let entry = 50_000 * PRECISION;
    let mark = 60_000 * PRECISION;
    let base = math::calc_base_exposure(&env, size, entry);

    assert_eq!(
        math::calc_unrealized_pnl(&env, size, base, mark, true),
        2_000 * PRECISION
    );
    assert_eq!(
        math::calc_unrealized_pnl(&env, size, base, mark, false),
        -2_000 * PRECISION
    );
}

#[test]
fn aggregate_exposure_equals_sum_of_position_pnl_at_different_entries() {
    let env = Env::default();
    let size_a = 10_000 * PRECISION;
    let size_b = 10_000 * PRECISION;
    let entry_a = 50_000 * PRECISION;
    let entry_b = 60_000 * PRECISION;
    let mark = 60_000 * PRECISION;
    let base_a = math::calc_base_exposure(&env, size_a, entry_a);
    let base_b = math::calc_base_exposure(&env, size_b, entry_b);

    let individual = math::calc_unrealized_pnl(&env, size_a, base_a, mark, true)
        + math::calc_unrealized_pnl(&env, size_b, base_b, mark, true);
    let aggregate =
        math::calc_market_unrealized_pnl(&env, size_a + size_b, base_a + base_b, 0, 0, mark);

    assert!((individual - aggregate).abs() <= 1);
    assert!((aggregate - 2_000 * PRECISION).abs() <= 1);
}

#[test]
fn derived_entry_is_harmonic_not_arithmetic() {
    let env = Env::default();
    let size = 10_000 * PRECISION;
    let base = math::calc_base_exposure(&env, size, 50_000 * PRECISION)
        + math::calc_base_exposure(&env, size, 60_000 * PRECISION);
    let derived = math::derive_entry_price(&env, 2 * size, base);

    assert!((54_545 * PRECISION..=54_546 * PRECISION).contains(&derived));
    assert_ne!(derived, 55_000 * PRECISION);
}

#[test]
fn fee_debt_preserves_pre_increase_accrual() {
    let env = Env::default();
    let old_size = 10_000 * PRECISION;
    let added = 5_000 * PRECISION;
    let start = INDEX_PRECISION;
    let current = INDEX_PRECISION + INDEX_PRECISION / 10;
    let old_debt = math::calc_fee_debt(&env, old_size, start);
    let new_debt = old_debt + math::calc_fee_debt(&env, added, current);

    let before = math::calc_fee_from_debt(&env, old_size, current, old_debt);
    let after = math::calc_fee_from_debt(&env, old_size + added, current, new_debt);
    assert_eq!(before, after);
}

#[test]
fn balanced_market_has_zero_skew_rate() {
    let env = Env::default();
    assert_eq!(math::calc_skew_rate(&env, 100, 100, BPS, 5_000), 0);
}

#[test]
fn one_sided_full_utilization_reaches_max_skew_rate() {
    let env = Env::default();
    assert_eq!(math::calc_skew_rate(&env, 100, 0, BPS, 5_000), 5_000);
}

#[test]
fn skew_rate_is_quadratic_and_utilization_weighted() {
    let env = Env::default();
    // 75/25 => 50% concentration; at 80% utilization and a 50% max APR:
    // 5000 * 0.5^2 * 0.8 = 1000 bps.
    assert_eq!(math::calc_skew_rate(&env, 75, 25, 8_000, 5_000), 1_000);
}

#[test]
fn annual_index_delta_matches_rate() {
    let env = Env::default();
    let next = math::accumulate_fee_index(&env, INDEX_PRECISION, 5_000, SECONDS_PER_YEAR);
    assert_eq!(next, INDEX_PRECISION + INDEX_PRECISION / 2);
}

#[test]
fn exposure_precision_is_large_enough_for_small_positions() {
    let env = Env::default();
    let base = math::calc_base_exposure(&env, 1, 100_000 * PRECISION);
    assert!(base > 0);
    assert_eq!(EXPOSURE_PRECISION, 1_000_000_000_000_000_000);
}

#[test]
fn triggers_preserve_direction_semantics() {
    assert!(math::is_tp_triggered(110, 110, true));
    assert!(math::is_tp_triggered(90, 90, false));
    assert!(math::is_sl_triggered(90, 90, true));
    assert!(math::is_sl_triggered(110, 110, false));
}
