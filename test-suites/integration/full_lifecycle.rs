//! Full lifecycle integration test: deposit → trade → PnL → withdraw.
//!
//! Simulates a complete user journey through the protocol:
//! 1. LP deposits USDC into the vault and receives shares.
//! 2. Trader opens a leveraged long position.
//! 3. Oracle price moves up — trader closes with profit.
//! 4. LP withdraws — share value reflects the loss the vault absorbed.

use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env};
use test_suites::testutils::{Fixture, BTC_PRICE, TEST_TIMESTAMP, USDC_UNIT};

const PRECISION: i128 = 10_000_000;
const MIN_POSITION_LIFETIME: u64 = 60;

// ---------------------------------------------------------------------------
// Happy path: LP → Trader profit → LP withdraw
// ---------------------------------------------------------------------------

#[test]
fn test_full_lifecycle_trader_profits_lp_absorbs_loss() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    // --- Snapshot initial state ---
    let lp = Address::generate(&env);
    let deposit = 500_000 * USDC_UNIT;
    f.usdc.mint(&lp, &deposit);

    // LP deposits
    let shares = f.deposit(&deposit, &lp, &lp, &lp);
    assert!(shares > 0, "LP must receive shares");

    // --- Trader opens 10x long ---
    let size = 50_000 * USDC_UNIT;
    let collateral = 5_000 * USDC_UNIT;

    f.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &size,
        &collateral,
        &true,
        &0,
        &0, &0i128
    );

    let trader_balance_after_open = f.usdc.balance(&f.trader);

    // --- Price pumps 10% ---
    f.advance_time(TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 10);
    let pump_price: i128 = 55_000 * PRECISION;
    f.mock_oracle.set_price(&symbol_short!("BTC"), &pump_price);

    // Trader closes
    f.decrease_position(&f.trader, &symbol_short!("BTC"), &size, &0_i128);

    let trader_balance_after_close = f.usdc.balance(&f.trader);
    let trader_pnl = trader_balance_after_close - trader_balance_after_open;
    assert!(trader_pnl > collateral, "Trader must receive profit + collateral");

    // --- LP withdraws after cooldown ---
    f.advance_time(TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 200);
    f.mock_oracle.set_price(&symbol_short!("BTC"), &pump_price);

    let lp_assets_out = f.vault.max_withdraw(&lp);
    // LP should get less than deposited because the vault paid out trader profit
    assert!(
        lp_assets_out < deposit,
        "LP must absorb trader profit: got {} vs deposited {}",
        lp_assets_out,
        deposit
    );
}

// ---------------------------------------------------------------------------
// Trader loses → LP profits
// ---------------------------------------------------------------------------

#[test]
fn test_full_lifecycle_trader_loses_lp_profits() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let lp = Address::generate(&env);
    let deposit = 500_000 * USDC_UNIT;
    f.usdc.mint(&lp, &deposit);
    f.deposit(&deposit, &lp, &lp, &lp);

    // Trader opens long
    let size = 50_000 * USDC_UNIT;
    let collateral = 5_000 * USDC_UNIT;
    f.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &size,
        &collateral,
        &true,
        &0,
        &0, &0i128
    );

    // Price drops 5%
    f.advance_time(TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 10);
    let drop_price: i128 = 47_500 * PRECISION;
    f.mock_oracle.set_price(&symbol_short!("BTC"), &drop_price);

    f.decrease_position(&f.trader, &symbol_short!("BTC"), &size, &0_i128);

    // LP withdraws — should get more than deposited (trader loss stays in vault)
    f.advance_time(TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 200);
    f.mock_oracle.set_price(&symbol_short!("BTC"), &drop_price);

    let lp_max = f.vault.max_withdraw(&lp);
    assert!(
        lp_max > deposit,
        "LP must profit from trader loss: got {} vs deposited {}",
        lp_max,
        deposit
    );
}

// ---------------------------------------------------------------------------
// Full cycle: open → partial close → full close
// ---------------------------------------------------------------------------

#[test]
fn test_full_lifecycle_partial_then_full_close() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let size = 20_000 * USDC_UNIT;
    let collateral = 2_000 * USDC_UNIT;

    f.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &size,
        &collateral,
        &true,
        &0,
        &0, &0i128
    );

    f.advance_time(TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 10);
    f.mock_oracle.set_price(&symbol_short!("BTC"), &BTC_PRICE);

    // Partial close — half the position
    let half = size / 2;
    f.decrease_position(&f.trader, &symbol_short!("BTC"), &half, &0_i128);

    let pos = f
        .position_manager
        .get_position(&f.trader, &symbol_short!("BTC"));
    assert_eq!(pos.size, half, "Position should be halved");

    let market = f.position_manager.get_market(&symbol_short!("BTC"));
    assert_eq!(market.long_open_interest, half);

    // Full close the remainder
    f.decrease_position(&f.trader, &symbol_short!("BTC"), &half, &0_i128);

    let market_after = f.position_manager.get_market(&symbol_short!("BTC"));
    assert_eq!(market_after.long_open_interest, 0);
}

// ---------------------------------------------------------------------------
// Short position lifecycle
// ---------------------------------------------------------------------------

#[test]
fn test_full_lifecycle_short_position_profit() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let size = 30_000 * USDC_UNIT;
    let collateral = 3_000 * USDC_UNIT;

    f.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &size,
        &collateral,
        &false, // short
        &0,
        &0, &0i128
    );

    let balance_after_open = f.usdc.balance(&f.trader);

    // Price drops 8% — good for shorts
    f.advance_time(TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 10);
    let drop_price: i128 = 46_000 * PRECISION;
    f.mock_oracle.set_price(&symbol_short!("BTC"), &drop_price);

    f.decrease_position(&f.trader, &symbol_short!("BTC"), &size, &0_i128);

    let balance_after_close = f.usdc.balance(&f.trader);
    let received = balance_after_close - balance_after_open;
    assert!(
        received > collateral,
        "Short trader must profit when price drops"
    );
}
