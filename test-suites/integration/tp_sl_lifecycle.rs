//! TP/SL lifecycle integration tests.
//!
//! Comprehensive coverage of take-profit and stop-loss mechanics for both
//! long and short positions, including payout verification, the standalone
//! set_tp_sl function, and boundary validation.

use soroban_sdk::{symbol_short, Env};
use test_suites::testutils::{Fixture, TEST_TIMESTAMP, USDC_UNIT};

const PRECISION: i128 = 10_000_000;

// ---------------------------------------------------------------------------
// Short TP triggered — price drops below take-profit
// ---------------------------------------------------------------------------

#[test]
fn test_short_tp_triggered() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let trader = f.create_funded_trader(5_000 * USDC_UNIT);

    // Short at 50k, TP at 45k (profit when price drops)
    let tp_price: i128 = 45_000 * PRECISION;
    f.increase_position(
        &trader,
        &symbol_short!("BTC"),
        &(20_000 * USDC_UNIT),
        &(2_000 * USDC_UNIT),
        &false, // short
        &tp_price,
        &0, &0i128
    );

    let balance_after_open = f.usdc.balance(&trader);

    // Price drops to 44k — below TP (45k), should trigger
    f.advance_time(TEST_TIMESTAMP + 75);
    f.set_btc_price(44_000);

    f.execute_order(&f.keeper, &trader, &symbol_short!("BTC"));

    // Position should be fully closed
    let market = f.position_manager.get_market(&symbol_short!("BTC"));
    assert_eq!(market.short_open_interest, 0, "Short OI must be zero after TP");

    // Trader should receive profit (price dropped ~12%)
    let balance_after = f.usdc.balance(&trader);
    let returned = balance_after - balance_after_open;
    assert!(
        returned > 2_000 * USDC_UNIT,
        "Short trader must profit on TP trigger: returned={}",
        returned
    );
}

// ---------------------------------------------------------------------------
// Short SL triggered — price rises above stop-loss
// ---------------------------------------------------------------------------

#[test]
fn test_short_sl_triggered() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let trader = f.create_funded_trader(5_000 * USDC_UNIT);

    // Short at 50k, SL at 53k (loss when price rises)
    let sl_price: i128 = 53_000 * PRECISION;
    f.increase_position(
        &trader,
        &symbol_short!("BTC"),
        &(20_000 * USDC_UNIT),
        &(2_000 * USDC_UNIT),
        &false,
        &0,
        &sl_price, &0i128
    );

    let balance_after_open = f.usdc.balance(&trader);

    // Price rises to 54k — above SL (53k), should trigger
    f.advance_time(TEST_TIMESTAMP + 75);
    f.set_btc_price(54_000);

    f.execute_order(&f.keeper, &trader, &symbol_short!("BTC"));

    let market = f.position_manager.get_market(&symbol_short!("BTC"));
    assert_eq!(market.short_open_interest, 0, "Short OI must be zero after SL");

    // Trader should get back less than collateral (loss)
    let balance_after = f.usdc.balance(&trader);
    let returned = balance_after - balance_after_open;
    assert!(
        returned < 2_000 * USDC_UNIT,
        "Short trader must lose on SL trigger: returned={}",
        returned
    );
}

// ---------------------------------------------------------------------------
// Long TP payout verification — exact profit amount
// ---------------------------------------------------------------------------

#[test]
fn test_long_tp_payout_correct() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let size = 20_000 * USDC_UNIT;
    let collateral = 2_000 * USDC_UNIT;
    let tp_price: i128 = 55_000 * PRECISION; // TP at 55k

    f.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &size,
        &collateral,
        &true,
        &tp_price,
        &0, &0i128
    );

    let balance_after_open = f.usdc.balance(&f.trader);

    // Price hits TP exactly
    f.advance_time(TEST_TIMESTAMP + 75);
    f.set_btc_price(55_000);

    f.execute_order(&f.keeper, &f.trader, &symbol_short!("BTC"));

    let balance_after = f.usdc.balance(&f.trader);
    let returned = balance_after - balance_after_open;

    // Expected PnL: 20k * (55k - 50k) / 50k = 20k * 10% = 2k
    // Trader gets ~collateral + PnL - small borrow fee
    assert!(
        returned > collateral,
        "Must return more than collateral on TP profit: returned={}",
        returned
    );
    // PnL should be roughly 2k USDC (minus small borrow fee for 75s)
    let profit = returned - collateral;
    assert!(
        profit > 1_900 * USDC_UNIT && profit < 2_100 * USDC_UNIT,
        "Profit should be ~2k USDC: profit={}",
        profit
    );
}

// ---------------------------------------------------------------------------
// Long SL payout verification — loss amount
// ---------------------------------------------------------------------------

#[test]
fn test_long_sl_payout_correct() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let size = 20_000 * USDC_UNIT;
    let collateral = 5_000 * USDC_UNIT;
    let sl_price: i128 = 45_000 * PRECISION; // SL at 45k

    f.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &size,
        &collateral,
        &true,
        &0,
        &sl_price, &0i128
    );

    let balance_after_open = f.usdc.balance(&f.trader);

    // Price drops to SL
    f.advance_time(TEST_TIMESTAMP + 75);
    f.set_btc_price(45_000);

    f.execute_order(&f.keeper, &f.trader, &symbol_short!("BTC"));

    let balance_after = f.usdc.balance(&f.trader);
    let returned = balance_after - balance_after_open;

    // Expected PnL: 20k * (45k - 50k) / 50k = 20k * -10% = -2k
    // Trader gets ~5k - 2k - borrow fee = ~3k
    assert!(
        returned < collateral,
        "Must return less than collateral on SL loss: returned={}",
        returned
    );
    assert!(
        returned > 2_500 * USDC_UNIT && returned < 3_100 * USDC_UNIT,
        "Returned should be ~3k USDC (collateral - loss): returned={}",
        returned
    );
}

// ---------------------------------------------------------------------------
// set_tp_sl post-open — verify it takes effect
// ---------------------------------------------------------------------------

#[test]
fn test_set_tp_sl_after_open_takes_effect() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    // Open position without TP/SL
    f.open_long(&f.trader, 20_000 * USDC_UNIT, 2_000 * USDC_UNIT);

    // Verify position has no TP/SL
    let pos = f
        .position_manager
        .get_position(&f.trader, &symbol_short!("BTC"));
    assert_eq!(pos.take_profit, 0);
    assert_eq!(pos.stop_loss, 0);

    // Set TP/SL after open
    let tp = 55_000 * PRECISION;
    let sl = 45_000 * PRECISION;
    f.set_tp_sl(&f.trader, &symbol_short!("BTC"), &tp, &sl);

    // Verify stored
    let pos2 = f
        .position_manager
        .get_position(&f.trader, &symbol_short!("BTC"));
    assert_eq!(pos2.take_profit, tp, "TP must be set");
    assert_eq!(pos2.stop_loss, sl, "SL must be set");

    // Now move price to trigger TP — execute_order should work
    f.advance_time(TEST_TIMESTAMP + 75);
    f.set_btc_price(56_000); // above TP

    f.execute_order(&f.keeper, &f.trader, &symbol_short!("BTC"));

    let market = f.position_manager.get_market(&symbol_short!("BTC"));
    assert_eq!(market.long_open_interest, 0, "TP set post-open must trigger");
}

// ---------------------------------------------------------------------------
// set_tp_sl clears existing TP/SL (reset to 0)
// ---------------------------------------------------------------------------

#[test]
fn test_set_tp_sl_reset_to_zero() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let tp = 55_000 * PRECISION;
    let sl = 45_000 * PRECISION;

    f.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &(20_000 * USDC_UNIT),
        &(2_000 * USDC_UNIT),
        &true,
        &tp,
        &sl, &0i128
    );

    // Reset to 0
    f.set_tp_sl(&f.trader, &symbol_short!("BTC"), &0, &0);

    let pos = f
        .position_manager
        .get_position(&f.trader, &symbol_short!("BTC"));
    assert_eq!(pos.take_profit, 0, "TP must be cleared");
    assert_eq!(pos.stop_loss, 0, "SL must be cleared");

    // Now even if price hits old TP, execute_order should fail (no TP/SL set)
    f.advance_time(TEST_TIMESTAMP + 75);
    f.set_btc_price(56_000);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        f.execute_order(&f.keeper, &f.trader, &symbol_short!("BTC"));
    }));
    assert!(
        result.is_err(),
        "Order must not trigger after TP/SL cleared"
    );
}

// ---------------------------------------------------------------------------
// TP == entry_price now accepted for long (validator drops entry-pin rule)
// ---------------------------------------------------------------------------

#[test]
fn test_tp_equal_entry_accepted_long() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let tp_at_entry: i128 = 50_000 * PRECISION;
    f.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &(10_000 * USDC_UNIT),
        &(1_000 * USDC_UNIT),
        &true,
        &tp_at_entry,
        &0, &0i128
    );
}

// ---------------------------------------------------------------------------
// TP == entry_price now accepted for short
// ---------------------------------------------------------------------------

#[test]
fn test_tp_equal_entry_accepted_short() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let tp_at_entry: i128 = 50_000 * PRECISION;
    f.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &(10_000 * USDC_UNIT),
        &(1_000 * USDC_UNIT),
        &false,
        &tp_at_entry,
        &0, &0i128
    );
}

// ---------------------------------------------------------------------------
// SL == entry_price now accepted for long
// ---------------------------------------------------------------------------

#[test]
fn test_sl_equal_entry_accepted_long() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let sl_at_entry: i128 = 50_000 * PRECISION;
    f.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &(10_000 * USDC_UNIT),
        &(1_000 * USDC_UNIT),
        &true,
        &0,
        &sl_at_entry, &0i128
    );
}

// ---------------------------------------------------------------------------
// SL == entry_price now accepted for short
// ---------------------------------------------------------------------------

#[test]
fn test_sl_equal_entry_accepted_short() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let sl_at_entry: i128 = 50_000 * PRECISION;
    f.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &(10_000 * USDC_UNIT),
        &(1_000 * USDC_UNIT),
        &false,
        &0,
        &sl_at_entry, &0i128
    );
}

// ---------------------------------------------------------------------------
// Short TP payout verification — exact profit
// ---------------------------------------------------------------------------

#[test]
fn test_short_tp_payout_correct() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let trader = f.create_funded_trader(10_000 * USDC_UNIT);
    let size = 20_000 * USDC_UNIT;
    let collateral = 4_000 * USDC_UNIT;
    let tp_price: i128 = 45_000 * PRECISION;

    f.increase_position(
        &trader,
        &symbol_short!("BTC"),
        &size,
        &collateral,
        &false,
        &tp_price,
        &0, &0i128
    );

    let balance_after_open = f.usdc.balance(&trader);

    // Price drops to 44k (below TP of 45k)
    f.advance_time(TEST_TIMESTAMP + 75);
    f.set_btc_price(44_000);

    f.execute_order(&f.keeper, &trader, &symbol_short!("BTC"));

    let balance_after = f.usdc.balance(&trader);
    let returned = balance_after - balance_after_open;

    // PnL = 20k * (50k - 44k) / 50k = 20k * 12% = 2.4k
    // Returned ≈ 4k + 2.4k - tiny borrow = ~6.4k
    assert!(
        returned > collateral,
        "Short must profit: returned={}, collateral={}",
        returned,
        collateral
    );
    let profit = returned - collateral;
    assert!(
        profit > 2_200 * USDC_UNIT && profit < 2_600 * USDC_UNIT,
        "Short profit should be ~2.4k USDC: profit={}",
        profit
    );
}

// ---------------------------------------------------------------------------
// set_tp_sl on short position — validate direction constraints
// ---------------------------------------------------------------------------

#[test]
fn test_set_tp_sl_short_position() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let trader = f.create_funded_trader(5_000 * USDC_UNIT);

    // Open short without TP/SL
    f.open_short(&trader, 20_000 * USDC_UNIT, 2_000 * USDC_UNIT);

    // Valid short TP/SL: TP below entry (profit target), SL above entry (stop loss)
    let tp = 45_000 * PRECISION;
    let sl = 55_000 * PRECISION;
    f.set_tp_sl(&trader, &symbol_short!("BTC"), &tp, &sl);

    let pos = f
        .position_manager
        .get_position(&trader, &symbol_short!("BTC"));
    assert_eq!(pos.take_profit, tp);
    assert_eq!(pos.stop_loss, sl);

    // TP above entry on a short is now accepted (validator no longer pins
    // TP/SL to entry-relative direction — trailing/profit-locking workflows
    // depend on this). Verify it persists.
    let aggressive_tp = 55_000 * PRECISION;
    f.set_tp_sl(&trader, &symbol_short!("BTC"), &aggressive_tp, &0);
    let pos2 = f
        .position_manager
        .get_position(&trader, &symbol_short!("BTC"));
    assert_eq!(pos2.take_profit, aggressive_tp);
}
