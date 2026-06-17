//! Multi-user integration tests.
//!
//! Simulates multiple LPs and traders interacting with the protocol
//! simultaneously, verifying that vault accounting, open interest tracking,
//! and share pricing all remain consistent.

use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env};
use test_suites::testutils::{Fixture, BTC_PRICE, TEST_TIMESTAMP, USDC_UNIT};

const PRECISION: i128 = 10_000_000;
const MIN_POSITION_LIFETIME: u64 = 60;

fn fund_trader(f: &Fixture, trader: &Address, amount: &i128) {
    f.usdc.mint(trader, amount);
}

// ---------------------------------------------------------------------------
// Multiple LPs deposit & withdraw proportionally
// ---------------------------------------------------------------------------

#[test]
fn test_multi_lp_proportional_shares() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let lp_a = Address::generate(&env);
    let lp_b = Address::generate(&env);
    let lp_c = Address::generate(&env);

    let dep_a = 100_000 * USDC_UNIT;
    let dep_b = 200_000 * USDC_UNIT;
    let dep_c = 300_000 * USDC_UNIT;

    f.usdc.mint(&lp_a, &dep_a);
    f.usdc.mint(&lp_b, &dep_b);
    f.usdc.mint(&lp_c, &dep_c);

    let shares_a = f.deposit(&dep_a, &lp_a, &lp_a, &lp_a);
    let shares_b = f.deposit(&dep_b, &lp_b, &lp_b, &lp_b);
    let shares_c = f.deposit(&dep_c, &lp_c, &lp_c, &lp_c);

    // B deposited 2x A, so should have ~2x shares
    // (not exact due to the initial 1M deposit in Fixture, but ratio should hold for these)
    let ratio_ba = (shares_b * 100) / shares_a;
    let ratio_ca = (shares_c * 100) / shares_a;
    assert!(
        (195..=205).contains(&ratio_ba),
        "B/A share ratio should be ~200%, got {}%",
        ratio_ba
    );
    assert!(
        (295..=305).contains(&ratio_ca),
        "C/A share ratio should be ~300%, got {}%",
        ratio_ca
    );

    // All can withdraw after cooldown
    f.advance_time(TEST_TIMESTAMP + 200);
    let max_a = f.vault.max_withdraw(&lp_a);
    let max_b = f.vault.max_withdraw(&lp_b);
    assert!(max_a > 0);
    assert!(max_b > 0);
}

// ---------------------------------------------------------------------------
// Opposing traders: long vs short on same market
// ---------------------------------------------------------------------------

#[test]
fn test_opposing_traders_long_vs_short() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let trader_long = Address::generate(&env);
    let trader_short = Address::generate(&env);

    let size = 20_000 * USDC_UNIT;
    let collateral = 2_000 * USDC_UNIT;

    fund_trader(&f, &trader_long, &(collateral * 2));
    fund_trader(&f, &trader_short, &(collateral * 2));

    // Both open positions
    f.increase_position(
        &trader_long,
        &symbol_short!("BTC"),
        &size,
        &collateral,
        &true,
        &0,
        &0, &0i128
    );

    f.increase_position(
        &trader_short,
        &symbol_short!("BTC"),
        &size,
        &collateral,
        &false,
        &0,
        &0, &0i128
    );

    // Market state: balanced OI
    let market = f.position_manager.get_market(&symbol_short!("BTC"));
    assert_eq!(market.long_open_interest, size);
    assert_eq!(market.short_open_interest, size);

    // Price goes up 5%: long profits, short loses
    f.advance_time(TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 10);
    let up_price: i128 = 52_500 * PRECISION;
    f.mock_oracle.set_price(&symbol_short!("BTC"), &up_price);

    let long_balance_before = f.usdc.balance(&trader_long);
    let short_balance_before = f.usdc.balance(&trader_short);

    f.decrease_position(&trader_long, &symbol_short!("BTC"), &size, &0_i128);
    f.decrease_position(&trader_short, &symbol_short!("BTC"), &size, &0_i128);

    let long_pnl = f.usdc.balance(&trader_long) - long_balance_before;
    let short_pnl = f.usdc.balance(&trader_short) - short_balance_before;

    assert!(long_pnl > collateral, "Long must profit");
    assert!(short_pnl < collateral, "Short must lose");

    // Market OI should be zero
    let market_after = f.position_manager.get_market(&symbol_short!("BTC"));
    assert_eq!(market_after.long_open_interest, 0);
    assert_eq!(market_after.short_open_interest, 0);
}

// ---------------------------------------------------------------------------
// Multiple traders open sequentially, keeper updates indices
// ---------------------------------------------------------------------------

#[test]
fn test_multi_trader_with_keeper_index_updates() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let trader_a = Address::generate(&env);
    let trader_b = Address::generate(&env);

    fund_trader(&f, &trader_a, &(10_000 * USDC_UNIT));
    fund_trader(&f, &trader_b, &(10_000 * USDC_UNIT));

    // Trader A opens
    f.increase_position(
        &trader_a,
        &symbol_short!("BTC"),
        &(20_000 * USDC_UNIT),
        &(2_000 * USDC_UNIT),
        &true,
        &0,
        &0, &0i128
    );

    // Advance 1 hour, keeper updates indices
    f.advance_time(TEST_TIMESTAMP + 3600);
    f.mock_oracle.set_price(&symbol_short!("BTC"), &BTC_PRICE);
    f.update_indices(&f.keeper, &symbol_short!("BTC"));

    let market_mid = f.position_manager.get_market(&symbol_short!("BTC"));
    assert!(
        market_mid.acc_borrow_index > 0,
        "Borrow index must have incremented"
    );

    // Trader B opens after index update (will snapshot the newer index)
    f.increase_position(
        &trader_b,
        &symbol_short!("BTC"),
        &(15_000 * USDC_UNIT),
        &(1_500 * USDC_UNIT),
        &true,
        &0,
        &0, &0i128
    );

    let pos_b = f
        .position_manager
        .get_position(&trader_b, &symbol_short!("BTC"));
    assert_eq!(
        pos_b.entry_borrow_index, market_mid.acc_borrow_index,
        "Trader B must snapshot the updated borrow index"
    );

    // Verify total OI
    let market_final = f.position_manager.get_market(&symbol_short!("BTC"));
    assert_eq!(
        market_final.long_open_interest,
        35_000 * USDC_UNIT,
        "Total long OI = 20k + 15k"
    );
}

// ---------------------------------------------------------------------------
// LP deposits during active trading
// ---------------------------------------------------------------------------

#[test]
fn test_lp_deposits_while_positions_open() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    // Trader opens first
    let size = 40_000 * USDC_UNIT;
    let collateral = 4_000 * USDC_UNIT;
    f.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &size,
        &collateral,
        &true,
        &0,
        &0, &0i128
    );

    // Now LP deposits (while liquidity is partially reserved)
    let lp = Address::generate(&env);
    let deposit = 200_000 * USDC_UNIT;
    f.usdc.mint(&lp, &deposit);
    let shares = f.deposit(&deposit, &lp, &lp, &lp);
    assert!(shares > 0, "LP must be able to deposit during active trading");

    // Free liquidity should reflect the reservation
    let free = f.vault.free_liquidity();
    assert!(
        free > 0,
        "Free liquidity must be positive with headroom"
    );
}

// ---------------------------------------------------------------------------
// Concurrent traders: one opens, one closes, vault stays consistent
// ---------------------------------------------------------------------------

#[test]
fn test_concurrent_open_and_close() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let trader_a = Address::generate(&env);
    let trader_b = Address::generate(&env);
    fund_trader(&f, &trader_a, &(10_000 * USDC_UNIT));
    fund_trader(&f, &trader_b, &(10_000 * USDC_UNIT));

    // A opens long
    f.increase_position(
        &trader_a,
        &symbol_short!("BTC"),
        &(20_000 * USDC_UNIT),
        &(2_000 * USDC_UNIT),
        &true,
        &0,
        &0, &0i128
    );

    f.advance_time(TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 10);
    f.mock_oracle.set_price(&symbol_short!("BTC"), &BTC_PRICE);

    // B opens long
    f.increase_position(
        &trader_b,
        &symbol_short!("BTC"),
        &(15_000 * USDC_UNIT),
        &(1_500 * USDC_UNIT),
        &true,
        &0,
        &0, &0i128
    );

    // A closes (A had enough time)
    f.decrease_position(&trader_a, &symbol_short!("BTC"), &(20_000 * USDC_UNIT), &0_i128);

    // Market should only have B's OI
    let market = f.position_manager.get_market(&symbol_short!("BTC"));
    assert_eq!(market.long_open_interest, 15_000 * USDC_UNIT);
}
