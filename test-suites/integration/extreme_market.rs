//! Extreme market condition integration tests.
//!
//! Violent price movements, cascading liquidations, vault stress testing.
//! Ensures the protocol remains solvent under extreme conditions.

use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env};
use test_suites::testutils::{Fixture, TEST_TIMESTAMP, USDC_UNIT};

const MIN_POSITION_LIFETIME: u64 = 60;

// ---------------------------------------------------------------------------
// 50% crash — cascading liquidation of 5 traders
// ---------------------------------------------------------------------------

#[test]
fn test_50pct_crash_cascade_liquidation() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    // 5 traders all open 10x longs
    let mut traders = vec![];
    for _ in 0..5 {
        let trader = f.create_funded_trader(5_000 * USDC_UNIT);
        f.open_long(&trader, 20_000 * USDC_UNIT, 2_000 * USDC_UNIT);
        traders.push(trader);
    }

    let market = f.position_manager.get_market(&symbol_short!("BTC"));
    assert_eq!(market.long_open_interest, 100_000 * USDC_UNIT);

    // 50% crash
    f.advance_time(TEST_TIMESTAMP + 75);
    f.set_btc_price(25_000);

    // Liquidate all 5
    for trader in &traders {
        f.liquidate(&f.keeper, trader, &symbol_short!("BTC"));
    }

    let market_after = f.position_manager.get_market(&symbol_short!("BTC"));
    assert_eq!(market_after.long_open_interest, 0, "All longs liquidated");

    // Vault must remain solvent
    let total = f.vault.total_assets();
    assert!(total > 0, "Vault must remain solvent after cascade");
}

// ---------------------------------------------------------------------------
// Price doubles — all shorts wiped out
// ---------------------------------------------------------------------------

#[test]
fn test_price_doubles_short_wipeout() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let mut traders = vec![];
    for _ in 0..3 {
        let trader = f.create_funded_trader(5_000 * USDC_UNIT);
        f.open_short(&trader, 20_000 * USDC_UNIT, 2_000 * USDC_UNIT);
        traders.push(trader);
    }

    // Price doubles
    f.advance_time(TEST_TIMESTAMP + 75);
    f.set_btc_price(100_000);

    for trader in &traders {
        f.liquidate(&f.keeper, trader, &symbol_short!("BTC"));
    }

    let market = f.position_manager.get_market(&symbol_short!("BTC"));
    assert_eq!(market.short_open_interest, 0, "All shorts liquidated");
}

// ---------------------------------------------------------------------------
// Rapid pump then dump — verify final PnL correctness
// ---------------------------------------------------------------------------

#[test]
fn test_rapid_pump_dump_pnl_correct() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let size = 20_000 * USDC_UNIT;
    let collateral = 2_000 * USDC_UNIT;

    f.open_long(&f.trader, size, collateral);
    let balance_after_open = f.usdc.balance(&f.trader);

    // Price pumps 20%
    f.advance_time(TEST_TIMESTAMP + 30);
    f.set_btc_price(60_000);

    // Then dumps 30% from 60k -> 42k (net -16% from original 50k)
    f.advance_time(TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 10);
    f.set_btc_price(42_000);

    f.decrease_position(&f.trader, &symbol_short!("BTC"), &size, &0_i128);

    let balance_after_close = f.usdc.balance(&f.trader);
    let net_pnl = balance_after_close - balance_after_open;

    // Price went from 50k to 42k = -16%. On 20k size, PnL ≈ -3,200 USDC.
    // Trader gets back collateral minus loss.
    assert!(
        net_pnl < collateral,
        "Trader must have lost money: net_pnl={}, collateral={}",
        net_pnl,
        collateral
    );
}

// ---------------------------------------------------------------------------
// 99% crash — vault survives
// ---------------------------------------------------------------------------

#[test]
fn test_99pct_crash_vault_survives() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let trader = f.create_funded_trader(5_000 * USDC_UNIT);
    f.open_long(&trader, 20_000 * USDC_UNIT, 2_000 * USDC_UNIT);

    // Price crashes to $500 (99% down from $50k)
    f.advance_time(TEST_TIMESTAMP + 75);
    f.set_btc_price(500);

    f.liquidate(&f.keeper, &trader, &symbol_short!("BTC"));

    let total = f.vault.total_assets();
    assert!(total > 0, "Vault must survive even 99% crash");

    let market = f.position_manager.get_market(&symbol_short!("BTC"));
    assert_eq!(market.long_open_interest, 0);
}

// ---------------------------------------------------------------------------
// Max leverage at liquidation boundary — 1% move liquidates
// ---------------------------------------------------------------------------

#[test]
fn test_max_leverage_at_liquidation_boundary() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    // 100x leverage: 100k size on 1k collateral
    let trader = f.create_funded_trader(2_000 * USDC_UNIT);
    f.open_long(&trader, 100_000 * USDC_UNIT, 1_000 * USDC_UNIT);

    // 1.5% drop should push health negative (100x * 1.5% = 150% of collateral lost)
    f.advance_time(TEST_TIMESTAMP + 75);
    f.set_btc_price(49_250); // -1.5%

    f.liquidate(&f.keeper, &trader, &symbol_short!("BTC"));

    let market = f.position_manager.get_market(&symbol_short!("BTC"));
    assert_eq!(market.long_open_interest, 0, "100x position liquidated on small move");
}

// ---------------------------------------------------------------------------
// Flash crash → liquidations → price recovers → survivors close profitably
// ---------------------------------------------------------------------------

#[test]
fn test_flash_crash_recovery() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    // Risky trader (10x) and safe trader (2x)
    let risky = f.create_funded_trader(5_000 * USDC_UNIT);
    let safe = f.create_funded_trader(50_000 * USDC_UNIT);

    f.open_long(&risky, 20_000 * USDC_UNIT, 2_000 * USDC_UNIT);
    f.open_long(&safe, 20_000 * USDC_UNIT, 10_000 * USDC_UNIT);

    let safe_balance_after_open = f.usdc.balance(&safe);

    // Crash -15%
    f.advance_time(TEST_TIMESTAMP + 30);
    f.set_btc_price(42_500);

    // Liquidate risky
    f.liquidate(&f.keeper, &risky, &symbol_short!("BTC"));

    // Price recovers to 55k (+10% from original)
    f.advance_time(TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 10);
    f.set_btc_price(55_000);

    // Safe trader closes with profit
    f.decrease_position(&safe, &symbol_short!("BTC"), &(20_000 * USDC_UNIT), &0_i128);

    let safe_balance_after_close = f.usdc.balance(&safe);
    let safe_net = safe_balance_after_close - safe_balance_after_open;
    assert!(
        safe_net > 10_000 * USDC_UNIT,
        "Safe trader must profit after recovery"
    );
}

// ---------------------------------------------------------------------------
// Vault fully reserved blocks new positions
// ---------------------------------------------------------------------------

#[test]
fn test_vault_fully_reserved_blocks_new_positions() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    // Open up to 84% cap
    let trader_a = f.create_funded_trader(100_000 * USDC_UNIT);
    f.open_long(&trader_a, 840_000 * USDC_UNIT, 84_000 * USDC_UNIT);

    // Another large position should breach cap
    let trader_b = f.create_funded_trader(50_000 * USDC_UNIT);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        f.open_long(&trader_b, 50_000 * USDC_UNIT, 5_000 * USDC_UNIT);
    }));
    assert!(result.is_err(), "Positions beyond utilization cap must be rejected");
}

// ---------------------------------------------------------------------------
// LP withdraw after massive trader profit — share value drops
// ---------------------------------------------------------------------------

#[test]
fn test_lp_withdraw_after_massive_trader_profit() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let lp = Address::generate(&env);
    let deposit = 500_000 * USDC_UNIT;
    f.usdc.mint(&lp, &deposit);
    f.deposit(&deposit, &lp, &lp, &lp);

    // Trader opens big and wins big (+20%)
    let size = 100_000 * USDC_UNIT;
    let collateral = 10_000 * USDC_UNIT;
    f.open_long(&f.trader, size, collateral);

    f.advance_time(TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 10);
    f.set_btc_price(60_000); // +20%

    f.decrease_position(&f.trader, &symbol_short!("BTC"), &size, &0_i128);

    // LP withdraws — share value should have dropped
    f.advance_time(TEST_TIMESTAMP + 300);
    f.set_btc_price(60_000);
    let lp_max = f.vault.max_withdraw(&lp);
    assert!(
        lp_max < deposit,
        "LP must absorb trader profit: lp_max={}, deposit={}",
        lp_max,
        deposit
    );
}

// ---------------------------------------------------------------------------
// LP profit after massive trader loss
// ---------------------------------------------------------------------------

#[test]
fn test_lp_profit_after_massive_trader_loss() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let lp = Address::generate(&env);
    let deposit = 500_000 * USDC_UNIT;
    f.usdc.mint(&lp, &deposit);
    f.deposit(&deposit, &lp, &lp, &lp);

    // Trader opens and loses (-8%)
    let size = 100_000 * USDC_UNIT;
    let collateral = 10_000 * USDC_UNIT;
    f.open_long(&f.trader, size, collateral);

    f.advance_time(TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 10);
    f.set_btc_price(46_000); // -8%

    f.decrease_position(&f.trader, &symbol_short!("BTC"), &size, &0_i128);

    // LP withdraws — should be more than deposited
    f.advance_time(TEST_TIMESTAMP + 300);
    f.set_btc_price(46_000);
    let lp_max = f.vault.max_withdraw(&lp);
    assert!(
        lp_max > deposit,
        "LP must profit from trader loss: lp_max={}, deposit={}",
        lp_max,
        deposit
    );
}

// ---------------------------------------------------------------------------
// Opposing positions — extreme move — one liquidated, one profits
// ---------------------------------------------------------------------------

#[test]
fn test_opposing_positions_extreme_move() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let long_trader = f.create_funded_trader(5_000 * USDC_UNIT);
    let short_trader = f.create_funded_trader(5_000 * USDC_UNIT);

    f.open_long(&long_trader, 20_000 * USDC_UNIT, 2_000 * USDC_UNIT);
    f.open_short(&short_trader, 20_000 * USDC_UNIT, 2_000 * USDC_UNIT);

    let short_bal_before = f.usdc.balance(&short_trader);

    // 40% crash — long gets liquidated, short profits
    f.advance_time(TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 10);
    f.set_btc_price(30_000);

    // Liquidate the long
    f.liquidate(&f.keeper, &long_trader, &symbol_short!("BTC"));

    // Short trader closes with massive profit
    f.decrease_position(&short_trader, &symbol_short!("BTC"), &(20_000 * USDC_UNIT), &0_i128);

    let short_bal_after = f.usdc.balance(&short_trader);
    let short_pnl = short_bal_after - short_bal_before;
    assert!(
        short_pnl > 2_000 * USDC_UNIT,
        "Short must profit hugely: pnl={}",
        short_pnl
    );

    let market = f.position_manager.get_market(&symbol_short!("BTC"));
    assert_eq!(market.long_open_interest, 0);
    assert_eq!(market.short_open_interest, 0);
}

// ---------------------------------------------------------------------------
// Price unchanged — close returns collateral (minus fees)
// ---------------------------------------------------------------------------

#[test]
fn test_price_unchanged_close_returns_collateral() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let collateral = 5_000 * USDC_UNIT;
    let size = 10_000 * USDC_UNIT;

    f.open_long(&f.trader, size, collateral);
    let balance_after_open = f.usdc.balance(&f.trader);

    f.advance_time(TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 10);
    f.set_btc_price(50_000); // same price

    f.decrease_position(&f.trader, &symbol_short!("BTC"), &size, &0_i128);

    let balance_after_close = f.usdc.balance(&f.trader);
    let returned = balance_after_close - balance_after_open;

    // Should get back ~collateral minus small borrow fee
    assert!(
        returned > 0 && returned <= collateral,
        "Should return collateral minus fees: returned={}",
        returned
    );
}

// ---------------------------------------------------------------------------
// Many small positions then crash — batch liquidation
// ---------------------------------------------------------------------------

#[test]
fn test_many_small_positions_then_crash() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let mut traders = vec![];
    for _ in 0..10 {
        let trader = f.create_funded_trader(3_000 * USDC_UNIT);
        f.open_long(&trader, 10_000 * USDC_UNIT, 1_000 * USDC_UNIT);
        traders.push(trader);
    }

    let market = f.position_manager.get_market(&symbol_short!("BTC"));
    assert_eq!(market.long_open_interest, 100_000 * USDC_UNIT);

    // 15% crash
    f.advance_time(TEST_TIMESTAMP + 75);
    f.set_btc_price(42_500);

    // Liquidate all
    for trader in &traders {
        f.liquidate(&f.keeper, trader, &symbol_short!("BTC"));
    }

    let market_after = f.position_manager.get_market(&symbol_short!("BTC"));
    assert_eq!(market_after.long_open_interest, 0);

    // Vault solvent
    assert!(f.vault.total_assets() > 0);
}
