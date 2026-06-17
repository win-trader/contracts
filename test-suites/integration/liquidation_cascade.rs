//! Liquidation cascade integration tests.
//!
//! Simulates market crash scenarios where multiple traders get liquidated,
//! verifying that the vault stays solvent and healthy positions remain safe.

use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env};
use test_suites::testutils::{Fixture, TEST_TIMESTAMP, USDC_UNIT};

const PRECISION: i128 = 10_000_000;
const MIN_POSITION_LIFETIME: u64 = 60;

fn fund_trader(f: &Fixture, trader: &Address, amount: &i128) {
    f.usdc.mint(trader, amount);
}

// ---------------------------------------------------------------------------
// Crash scenario: 3 traders, 2 get liquidated, 1 survives
// ---------------------------------------------------------------------------

#[test]
fn test_crash_liquidates_undercollateralized_spares_healthy() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let risky_a = Address::generate(&env);
    let risky_b = Address::generate(&env);
    let safe_c = Address::generate(&env);

    // Risky traders: 10x leverage (thin margin)
    fund_trader(&f, &risky_a, &(5_000 * USDC_UNIT));
    fund_trader(&f, &risky_b, &(5_000 * USDC_UNIT));
    // Safe trader: 2x leverage (thick margin)
    fund_trader(&f, &safe_c, &(50_000 * USDC_UNIT));

    f.increase_position(
        &risky_a,
        &symbol_short!("BTC"),
        &(20_000 * USDC_UNIT),
        &(2_000 * USDC_UNIT),
        &true,
        &0,
        &0, &0i128
    );

    f.increase_position(
        &risky_b,
        &symbol_short!("BTC"),
        &(30_000 * USDC_UNIT),
        &(3_000 * USDC_UNIT),
        &true,
        &0,
        &0, &0i128
    );

    f.increase_position(
        &safe_c,
        &symbol_short!("BTC"),
        &(40_000 * USDC_UNIT),
        &(20_000 * USDC_UNIT),
        &true,
        &0,
        &0, &0i128
    );

    // 12% crash — enough to wipe risky traders, safe_c survives
    f.advance_time(TEST_TIMESTAMP + 75);
    let crash_price: i128 = 44_000 * PRECISION;
    f.mock_oracle.set_price(&symbol_short!("BTC"), &crash_price);

    // Liquidate risky_a
    f.liquidate(&f.keeper, &risky_a, &symbol_short!("BTC"));

    // Liquidate risky_b
    f.liquidate(&f.keeper, &risky_b, &symbol_short!("BTC"));

    // safe_c should NOT be liquidatable — verify position still exists
    let pos_c = f
        .position_manager
        .get_position(&safe_c, &symbol_short!("BTC"));
    assert_eq!(pos_c.size, 40_000 * USDC_UNIT, "Safe position must survive");

    // Market OI: only safe_c remains
    let market = f.position_manager.get_market(&symbol_short!("BTC"));
    assert_eq!(market.long_open_interest, 40_000 * USDC_UNIT);

    // Vault must still be functional — safe_c can close
    f.advance_time(TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 100);
    f.mock_oracle.set_price(&symbol_short!("BTC"), &crash_price);
    f.decrease_position(&safe_c, &symbol_short!("BTC"), &(40_000 * USDC_UNIT), &0_i128);

    let market_empty = f.position_manager.get_market(&symbol_short!("BTC"));
    assert_eq!(market_empty.long_open_interest, 0);
}

// ---------------------------------------------------------------------------
// Liquidation during pause: must still work
// ---------------------------------------------------------------------------

#[test]
fn test_liquidation_works_when_paused() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let trader = Address::generate(&env);
    fund_trader(&f, &trader, &(5_000 * USDC_UNIT));

    f.increase_position(
        &trader,
        &symbol_short!("BTC"),
        &(20_000 * USDC_UNIT),
        &(2_000 * USDC_UNIT),
        &true,
        &0,
        &0, &0i128
    );

    // Crash price
    f.advance_time(TEST_TIMESTAMP + 75);
    let crash_price: i128 = 44_000 * PRECISION;
    f.mock_oracle.set_price(&symbol_short!("BTC"), &crash_price);

    // Pause the protocol
    f.pause_pm(&f.admin);

    // Liquidation must still succeed — cannot block solvency protection
    f.liquidate(&f.keeper, &trader, &symbol_short!("BTC"));

    let market = f.position_manager.get_market(&symbol_short!("BTC"));
    assert_eq!(market.long_open_interest, 0);
}

// ---------------------------------------------------------------------------
// Post-liquidation: vault allows LP withdrawals
// ---------------------------------------------------------------------------

#[test]
fn test_vault_solvent_after_liquidations() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let lp = Address::generate(&env);
    let deposit = 500_000 * USDC_UNIT;
    f.usdc.mint(&lp, &deposit);
    f.deposit(&deposit, &lp, &lp, &lp);

    let trader = Address::generate(&env);
    fund_trader(&f, &trader, &(5_000 * USDC_UNIT));

    f.increase_position(
        &trader,
        &symbol_short!("BTC"),
        &(30_000 * USDC_UNIT),
        &(3_000 * USDC_UNIT),
        &true,
        &0,
        &0, &0i128
    );

    // Crash and liquidate
    f.advance_time(TEST_TIMESTAMP + 75);
    let crash_price: i128 = 44_000 * PRECISION;
    f.mock_oracle.set_price(&symbol_short!("BTC"), &crash_price);

    f.liquidate(&f.keeper, &trader, &symbol_short!("BTC"));

    // LP can still withdraw after cooldown
    f.advance_time(TEST_TIMESTAMP + 300);
    let lp_max = f.vault.max_withdraw(&lp);
    assert!(
        lp_max > 0,
        "LP must be able to withdraw after liquidation event"
    );

    // Vault total assets should be positive
    let total = f.vault.total_assets();
    assert!(total > 0, "Vault must remain solvent");
}

// ---------------------------------------------------------------------------
// Short liquidation when price spikes
// ---------------------------------------------------------------------------

#[test]
fn test_short_liquidation_on_price_spike() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let trader = Address::generate(&env);
    fund_trader(&f, &trader, &(5_000 * USDC_UNIT));

    // 10x short
    f.increase_position(
        &trader,
        &symbol_short!("BTC"),
        &(20_000 * USDC_UNIT),
        &(2_000 * USDC_UNIT),
        &false, // short
        &0,
        &0, &0i128
    );

    // Price spikes 12% — devastating for shorts
    f.advance_time(TEST_TIMESTAMP + 75);
    let spike_price: i128 = 56_000 * PRECISION;
    f.mock_oracle.set_price(&symbol_short!("BTC"), &spike_price);

    f.liquidate(&f.keeper, &trader, &symbol_short!("BTC"));

    let market = f.position_manager.get_market(&symbol_short!("BTC"));
    assert_eq!(market.short_open_interest, 0);
}
