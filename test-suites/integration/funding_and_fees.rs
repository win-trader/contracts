//! Funding and fee mechanics integration tests.
//!
//! Tests borrow fee accumulation, funding rate behavior, fee distribution,
//! and protocol fee claiming.

use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env};
use test_suites::testutils::{Fixture, TEST_TIMESTAMP, USDC_UNIT};

const INDEX_PRECISION: i128 = 100_000_000_000_000; // 1e14 — must match position_manager::math

const MIN_POSITION_LIFETIME: u64 = 60;

// ---------------------------------------------------------------------------
// Borrow fee increases over time
// ---------------------------------------------------------------------------

#[test]
fn test_borrow_fee_increases_over_time() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let collateral = 5_000 * USDC_UNIT;
    let size = 20_000 * USDC_UNIT;

    f.open_long(&f.trader, size, collateral);
    let balance_after_open = f.usdc.balance(&f.trader);

    // Advance 24 hours, update indices
    f.advance_time(TEST_TIMESTAMP + 86_400);
    f.set_btc_price(50_000);
    f.update_indices(&f.keeper, &symbol_short!("BTC"));

    f.decrease_position(&f.trader, &symbol_short!("BTC"), &size, &0_i128);

    let returned = f.usdc.balance(&f.trader) - balance_after_open;

    // With 0 PnL, returned < collateral due to borrow fees
    assert!(
        returned < collateral,
        "Borrow fee must be deducted: returned={}, collateral={}",
        returned,
        collateral
    );

    // Fee should be small but non-zero for 1 day at reasonable utilization
    let fee_paid = collateral - returned;
    assert!(
        fee_paid > 0,
        "Borrow fee must be positive after 24h"
    );
}

// ---------------------------------------------------------------------------
// Higher utilization → higher borrow rate
// ---------------------------------------------------------------------------

#[test]
fn test_higher_utilization_higher_borrow_rate() {
    // Scenario A: low utilization (~5%)
    let env_a = Env::default();
    let f_a = Fixture::deploy(&env_a);

    let collateral = 5_000 * USDC_UNIT;
    let small_size = 50_000 * USDC_UNIT; // ~5% of 1M vault

    f_a.open_long(&f_a.trader, small_size, collateral);
    let bal_a_open = f_a.usdc.balance(&f_a.trader);

    f_a.advance_time(TEST_TIMESTAMP + 86_400);
    f_a.set_btc_price(50_000);
    f_a.position_manager
        .update_indices(&f_a.keeper, &symbol_short!("BTC"));
    f_a.position_manager
        .decrease_position(&f_a.trader, &symbol_short!("BTC"), &small_size, &0_i128);

    let fee_low = collateral - (f_a.usdc.balance(&f_a.trader) - bal_a_open);

    // Scenario B: high utilization (~80%)
    let env_b = Env::default();
    let f_b = Fixture::deploy(&env_b);

    let large_size = 800_000 * USDC_UNIT; // ~80% of 1M vault
    let large_collateral = 80_000 * USDC_UNIT;
    f_b.usdc.mint(&f_b.trader, &large_collateral);

    f_b.open_long(&f_b.trader, large_size, large_collateral);
    let bal_b_open = f_b.usdc.balance(&f_b.trader);

    f_b.advance_time(TEST_TIMESTAMP + 86_400);
    f_b.set_btc_price(50_000);
    f_b.position_manager
        .update_indices(&f_b.keeper, &symbol_short!("BTC"));
    f_b.position_manager
        .decrease_position(&f_b.trader, &symbol_short!("BTC"), &large_size, &0_i128);

    let returned_b = f_b.usdc.balance(&f_b.trader) - bal_b_open;
    let fee_high = large_collateral - returned_b;

    // Fee rate (normalized by size) should be higher for high utilization
    let rate_low = (fee_low * 10_000) / small_size;
    let rate_high = (fee_high * 10_000) / large_size;

    assert!(
        rate_high > rate_low,
        "Higher utilization must yield higher borrow rate: rate_low={}, rate_high={}",
        rate_low,
        rate_high
    );
}

// ---------------------------------------------------------------------------
// Funding rate: longs pay shorts when long OI > short OI
// ---------------------------------------------------------------------------

#[test]
fn test_funding_rate_longs_pay_shorts() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let long_trader = f.create_funded_trader(10_000 * USDC_UNIT);
    let short_trader = f.create_funded_trader(10_000 * USDC_UNIT);

    // More longs than shorts
    f.open_long(&long_trader, 30_000 * USDC_UNIT, 3_000 * USDC_UNIT);
    f.open_short(&short_trader, 10_000 * USDC_UNIT, 1_000 * USDC_UNIT);

    // Advance and update indices
    f.advance_time(TEST_TIMESTAMP + 86_400);
    f.set_btc_price(50_000);
    f.update_indices(&f.keeper, &symbol_short!("BTC"));

    let market = f.position_manager.get_market(&symbol_short!("BTC"));
    // Funding index should be above baseline (longs pay)
    assert!(
        market.acc_funding_index > INDEX_PRECISION,
        "Funding index must be above INDEX_PRECISION when long OI > short OI"
    );

    // Close both at same price — long should pay more fees, short receives funding benefit
    let long_bal_before = f.usdc.balance(&long_trader);
    let short_bal_before = f.usdc.balance(&short_trader);

    f.decrease_position(&long_trader, &symbol_short!("BTC"), &(30_000 * USDC_UNIT), &0_i128);
    f.decrease_position(&short_trader, &symbol_short!("BTC"), &(10_000 * USDC_UNIT), &0_i128);

    let long_returned = f.usdc.balance(&long_trader) - long_bal_before;
    let short_returned = f.usdc.balance(&short_trader) - short_bal_before;

    // Long should get less than collateral (pays borrow + funding)
    assert!(
        long_returned < 3_000 * USDC_UNIT,
        "Long must pay fees: returned={}",
        long_returned
    );

    // Compare effective fee as fraction of collateral.
    // Long fee / long_collateral should be greater than short_fee / short_collateral
    // Using cross-multiplication to avoid truncation:
    // (3000 - long_returned) / 3000 > (1000 - short_returned) / 1000
    // ⟹ (3000 - long_returned) * 1000 > (1000 - short_returned) * 3000
    let long_fee = 3_000 * USDC_UNIT - long_returned;
    let short_fee = 1_000 * USDC_UNIT - short_returned;

    assert!(
        long_fee * (1_000 * USDC_UNIT) > short_fee * (3_000 * USDC_UNIT),
        "Long effective fee rate must exceed short: long_fee={}, short_fee={}",
        long_fee,
        short_fee
    );
}

// ---------------------------------------------------------------------------
// Funding rate: shorts pay longs when short OI > long OI
// ---------------------------------------------------------------------------

#[test]
fn test_funding_rate_shorts_pay_longs() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let long_trader = f.create_funded_trader(10_000 * USDC_UNIT);
    let short_trader = f.create_funded_trader(10_000 * USDC_UNIT);

    // More shorts than longs
    f.open_long(&long_trader, 10_000 * USDC_UNIT, 1_000 * USDC_UNIT);
    f.open_short(&short_trader, 30_000 * USDC_UNIT, 3_000 * USDC_UNIT);

    f.advance_time(TEST_TIMESTAMP + 86_400);
    f.set_btc_price(50_000);
    f.update_indices(&f.keeper, &symbol_short!("BTC"));

    let market = f.position_manager.get_market(&symbol_short!("BTC"));
    // Funding index should be below baseline (shorts pay)
    assert!(
        market.acc_funding_index < INDEX_PRECISION,
        "Funding index must be below INDEX_PRECISION when short OI > long OI"
    );
}

// ---------------------------------------------------------------------------
// Balanced OI → zero funding
// ---------------------------------------------------------------------------

#[test]
fn test_balanced_oi_zero_funding() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let long_trader = f.create_funded_trader(5_000 * USDC_UNIT);
    let short_trader = f.create_funded_trader(5_000 * USDC_UNIT);

    // Equal OI
    f.open_long(&long_trader, 20_000 * USDC_UNIT, 2_000 * USDC_UNIT);
    f.open_short(&short_trader, 20_000 * USDC_UNIT, 2_000 * USDC_UNIT);

    f.advance_time(TEST_TIMESTAMP + 86_400);
    f.set_btc_price(50_000);
    f.update_indices(&f.keeper, &symbol_short!("BTC"));

    let market = f.position_manager.get_market(&symbol_short!("BTC"));
    // Funding rate should be zero when OI is balanced — index stays at baseline
    assert_eq!(
        market.acc_funding_index, INDEX_PRECISION,
        "Balanced OI must produce zero funding delta"
    );
}

// ---------------------------------------------------------------------------
// Fee claiming by admin
// ---------------------------------------------------------------------------

#[test]
fn test_fee_claiming_by_admin() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    // Open and close a position to generate borrow fees
    let size = 50_000 * USDC_UNIT;
    let collateral = 5_000 * USDC_UNIT;
    f.open_long(&f.trader, size, collateral);

    f.advance_time(TEST_TIMESTAMP + 86_400);
    f.set_btc_price(50_000);
    f.update_indices(&f.keeper, &symbol_short!("BTC"));

    f.decrease_position(&f.trader, &symbol_short!("BTC"), &size, &0_i128);

    // Vault should have unclaimed fees
    let admin_balance_before = f.usdc.balance(&f.admin);
    f.claim_fees(&f.admin, &f.admin);

    let admin_balance_after = f.usdc.balance(&f.admin);
    let claimed = admin_balance_after - admin_balance_before;
    assert!(claimed > 0, "Admin must receive claimed fees: claimed={}", claimed);
}

// ---------------------------------------------------------------------------
// Fee claiming by non-admin rejected
// ---------------------------------------------------------------------------

#[test]
fn test_fee_claiming_non_admin_rejected() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let random = Address::generate(&env);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        f.claim_fees(&random, &random);
    }));
    assert!(result.is_err(), "Non-admin cannot claim fees");
}

// ---------------------------------------------------------------------------
// Borrow fee above optimal utilization (SLOPE2)
// ---------------------------------------------------------------------------

#[test]
fn test_borrow_fee_above_optimal_utilization() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    // Open at 82% utilization (above 80% optimal → SLOPE2 kicks in)
    let large_collateral = 85_000 * USDC_UNIT;
    f.usdc.mint(&f.trader, &large_collateral);

    let size = 820_000 * USDC_UNIT;
    f.open_long(&f.trader, size, large_collateral);

    let balance_after_open = f.usdc.balance(&f.trader);

    f.advance_time(TEST_TIMESTAMP + 86_400);
    f.set_btc_price(50_000);
    f.update_indices(&f.keeper, &symbol_short!("BTC"));

    f.decrease_position(&f.trader, &symbol_short!("BTC"), &size, &0_i128);

    let returned = f.usdc.balance(&f.trader) - balance_after_open;
    let fee = large_collateral - returned;

    // At 82% util and SLOPE2, fee rate should be noticeably higher
    // than the base rate (1% annualized = ~0.0027%/day)
    let daily_rate_bps = (fee * 10_000) / size;
    assert!(
        daily_rate_bps > 0,
        "SLOPE2 must produce measurable borrow fee: daily_rate_bps={}",
        daily_rate_bps
    );
}

// ---------------------------------------------------------------------------
// Multiple index updates compound correctly
// ---------------------------------------------------------------------------

#[test]
fn test_multiple_index_updates_compound() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    f.open_long(&f.trader, 20_000 * USDC_UNIT, 2_000 * USDC_UNIT);

    // 3 updates, each 8 hours apart
    let mut prev_borrow_index = INDEX_PRECISION;
    for i in 1..=3 {
        f.advance_time(TEST_TIMESTAMP + 28_800 * i);
        f.set_btc_price(50_000);
        f.update_indices(&f.keeper, &symbol_short!("BTC"));

        let market = f.position_manager.get_market(&symbol_short!("BTC"));
        assert!(
            market.acc_borrow_index > prev_borrow_index,
            "Borrow index must increase after update {}: prev={}, now={}",
            i,
            prev_borrow_index,
            market.acc_borrow_index
        );
        prev_borrow_index = market.acc_borrow_index;
    }

    // Now close — total fee should reflect 24h of compounded updates
    f.advance_time(TEST_TIMESTAMP + 86_400 + MIN_POSITION_LIFETIME);
    f.set_btc_price(50_000);

    let balance_before = f.usdc.balance(&f.trader);
    f.decrease_position(&f.trader, &symbol_short!("BTC"), &(20_000 * USDC_UNIT), &0_i128);

    let returned = f.usdc.balance(&f.trader) - balance_before;
    assert!(
        returned < 2_000 * USDC_UNIT,
        "Total borrow fee across 3 updates must be deducted"
    );
}
