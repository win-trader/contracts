//! Settlement mechanics and boundary condition integration tests.
//!
//! Tests proportional fee deduction on partial closes, close-more-than-size
//! clamping, free liquidity formula correctness, index update no-op on
//! same block, min lifetime boundaries, exposure-derived display prices, and
//! fee-debt baselines.

use soroban_sdk::{symbol_short, Env};
use test_suites::testutils::{Fixture, TEST_TIMESTAMP, USDC_UNIT};

const PRECISION: i128 = 10_000_000;
const MIN_POSITION_LIFETIME: u64 = 60;

// ---------------------------------------------------------------------------
// Partial close fee proportionality
// ---------------------------------------------------------------------------

#[test]
fn test_partial_close_fee_proportional() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let size = 20_000 * USDC_UNIT;
    let collateral = 4_000 * USDC_UNIT;

    f.open_long(&f.trader, size, collateral);

    // Advance 24h and update indices to accrue borrow fees
    f.advance_time(TEST_TIMESTAMP + 86_400);
    f.set_btc_price(50_000);
    f.update_indices(&f.keeper, &symbol_short!("BTC"));

    let balance_before_first_close = f.usdc.balance(&f.trader);

    // Close 50%
    f.decrease_position(&f.trader, &symbol_short!("BTC"), &(size / 2), &0_i128);

    let returned_first = f.usdc.balance(&f.trader) - balance_before_first_close;

    let balance_before_second_close = f.usdc.balance(&f.trader);

    // Close remaining 50%
    f.decrease_position(&f.trader, &symbol_short!("BTC"), &(size / 2), &0_i128);

    let returned_second = f.usdc.balance(&f.trader) - balance_before_second_close;

    // Both halves should return approximately the same amount
    // (both close 10k at zero PnL, same borrow_fee_debt, same proportion of collateral)
    let diff = (returned_first - returned_second).abs();
    let tolerance = 100 * USDC_UNIT; // allow 100 USDC rounding tolerance
    assert!(
        diff < tolerance,
        "Two 50% closes must return similar amounts: first={}, second={}, diff={}",
        returned_first,
        returned_second,
        diff
    );

    // Both should be roughly 2k (half of 4k collateral minus proportional borrow fee)
    assert!(
        returned_first > 0 && returned_first < 2_000 * USDC_UNIT,
        "Each half must return less than half collateral due to fees: first={}",
        returned_first
    );
}

// ---------------------------------------------------------------------------
// Close with size_delta > position.size — reverts (does not clamp).
// ---------------------------------------------------------------------------

#[test]
fn test_close_more_than_size_reverts() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let size = 10_000 * USDC_UNIT;
    f.open_long(&f.trader, size, 1_000 * USDC_UNIT);

    f.advance_time(TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 10);
    f.set_btc_price(50_000);

    let result = f.position_manager.try_decrease_position(
        &f.trader,
        &symbol_short!("BTC"),
        &(size * 2),
        &0_i128,
    );
    assert!(
        result.is_err(),
        "Over-close must revert, not silently clamp"
    );

    // Position must still be open with full OI intact.
    let market = f.position_manager.get_market(&symbol_short!("BTC"));
    assert_eq!(
        market.long_open_interest, size,
        "OI must be unchanged after rejected over-close"
    );
}

// ---------------------------------------------------------------------------
// Free liquidity formula: all components significant
// ---------------------------------------------------------------------------

#[test]
fn test_free_liquidity_formula_all_components() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let initial_total = f.vault.total_assets();
    let initial_free = f.vault.free_liquidity();
    // Initially: reserved=0, unclaimed_fees=0, net_pnl=0 -> free == total
    assert_eq!(
        initial_free, initial_total,
        "Free must equal total with no activity"
    );

    // Open position. With open_fee_bps != 0, the trader pays `size * bps / BPS`
    // on top of collateral. That flow goes trader -> PM -> Vault, so the
    // vault's `total_assets` rises by the open fee at this point. To isolate
    // the free_liquidity formula from the open-fee contribution, drop the
    // open fee to 0 for this test.
    f.config_manager.set_fee_config(
        &f.admin,
        &shared::FeeConfig {
            open_fee_bps: 0,
            liquidation_bounty_bps: 100,
            tp_sl_execution_fee: 5_000_000,
        },
    );

    let trader_a = f.create_funded_trader(50_000 * USDC_UNIT);
    f.open_long(&trader_a, 200_000 * USDC_UNIT, 20_000 * USDC_UNIT);

    let free_after_reserve = f.vault.free_liquidity();
    assert!(
        free_after_reserve < initial_free,
        "Free must decrease after reservation"
    );
    let expected_reserved = 200_000 * USDC_UNIT;
    let expected_free = initial_total - expected_reserved;
    let diff = (free_after_reserve - expected_free).abs();
    assert!(
        diff < 100 * USDC_UNIT,
        "Free liquidity should be total - reserved when open_fee_bps=0: expected={}, got={}, diff={}",
        expected_free,
        free_after_reserve,
        diff
    );

    // Close at profit to generate close-time fees and positive net_pnl.
    f.advance_time(TEST_TIMESTAMP + 86_400);
    f.set_btc_price(55_000);
    f.update_indices(&f.keeper, &symbol_short!("BTC"));

    f.decrease_position(
        &trader_a,
        &symbol_short!("BTC"),
        &(200_000 * USDC_UNIT),
        &0_i128,
    );

    // Now: reserved=0 (position fully closed), unclaimed_fees > 0 (close fees).
    // `total_assets` reports the LP basis, which already excludes the fee
    // buffer, so with nothing reserved `free_liquidity == total_assets`.
    let free_after_close = f.vault.free_liquidity();
    let total_after_close = f.vault.total_assets();
    let unclaimed = f.vault.unclaimed_fees();
    let raw_custody = f.usdc.balance(&f.vault_addr);

    assert_eq!(
        free_after_close, total_after_close,
        "with reserved=0, free_liquidity must equal the LP-basis total_assets: free={}, total={}",
        free_after_close, total_after_close,
    );
    assert!(
        unclaimed > 0,
        "close must have accrued fees into the unclaimed buffer"
    );
    assert!(
        raw_custody > total_after_close,
        "raw custody {} must exceed LP-basis total_assets {} by the fee buffer",
        raw_custody,
        total_after_close,
    );
    assert!(free_after_close > 0, "Free liquidity must remain positive");
}

// ---------------------------------------------------------------------------
// Index update twice in same block is no-op
// ---------------------------------------------------------------------------

#[test]
fn test_index_update_same_block_noop() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    f.open_long(&f.trader, 20_000 * USDC_UNIT, 2_000 * USDC_UNIT);

    // Advance and update once
    f.advance_time(TEST_TIMESTAMP + 3600);
    f.set_btc_price(50_000);
    f.update_indices(&f.keeper, &symbol_short!("BTC"));

    let market_first = f.position_manager.get_market(&symbol_short!("BTC"));

    // Update again at the same timestamp — should be a no-op
    f.update_indices(&f.keeper, &symbol_short!("BTC"));

    let market_second = f.position_manager.get_market(&symbol_short!("BTC"));

    assert_eq!(
        market_first.acc_borrow_index, market_second.acc_borrow_index,
        "Second update at same timestamp must not change borrow index"
    );
    assert_eq!(
        market_first.acc_long_skew_index, market_second.acc_long_skew_index,
        "Second update at same timestamp must not change skew index"
    );
}

// ---------------------------------------------------------------------------
// Min position lifetime exact boundary
// ---------------------------------------------------------------------------

#[test]
fn test_min_lifetime_exact_boundary_passes() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    f.open_long(&f.trader, 10_000 * USDC_UNIT, 1_000 * USDC_UNIT);

    // Advance exactly to last_increased_time + min_lifetime
    // min_position_lifetime = 60, opened at TEST_TIMESTAMP
    // At TEST_TIMESTAMP + 60, now == last_increased_time + min_lifetime
    // The check is: now < last_increased_time + min_lifetime (strict <)
    // So now == boundary should PASS
    f.advance_time(TEST_TIMESTAMP + MIN_POSITION_LIFETIME);
    f.set_btc_price(50_000);

    f.decrease_position(
        &f.trader,
        &symbol_short!("BTC"),
        &(10_000 * USDC_UNIT),
        &0_i128,
    );

    let market = f.position_manager.get_market(&symbol_short!("BTC"));
    assert_eq!(
        market.long_open_interest, 0,
        "Close at exact min_lifetime boundary must succeed"
    );
}

// ---------------------------------------------------------------------------
// Min position lifetime one second before boundary fails
// ---------------------------------------------------------------------------

#[test]
fn test_min_lifetime_one_second_before_fails() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    f.open_long(&f.trader, 10_000 * USDC_UNIT, 1_000 * USDC_UNIT);

    // One second before boundary
    f.advance_time(TEST_TIMESTAMP + MIN_POSITION_LIFETIME - 1);
    f.set_btc_price(50_000);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        f.decrease_position(
            &f.trader,
            &symbol_short!("BTC"),
            &(10_000 * USDC_UNIT),
            &0_i128,
        );
    }));
    assert!(
        result.is_err(),
        "Close one second before min_lifetime must fail"
    );
}

// ---------------------------------------------------------------------------
// Multiple increases preserve additive exposure and derive a display price.
// ---------------------------------------------------------------------------

#[test]
fn test_multiple_increases_derive_entry_price_from_exposure() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    // First increase at 50k
    f.open_long(&f.trader, 10_000 * USDC_UNIT, 1_000 * USDC_UNIT);

    let pos1 = f
        .position_manager
        .get_position(&f.trader, &symbol_short!("BTC"));
    assert_eq!(pos1.entry_price, 50_000 * PRECISION, "First entry at 50k");

    // Second increase at 60k
    f.advance_time(TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 10);
    f.set_btc_price(60_000);

    f.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &(10_000 * USDC_UNIT),
        &(1_000 * USDC_UNIT),
        &true,
        &0,
        &0,
        &0i128,
    );

    let pos2 = f
        .position_manager
        .get_position(&f.trader, &symbol_short!("BTC"));
    let exposure_precision = 1_000_000_000_000_000_000_i128;
    let exposure_1 = 10_000 * USDC_UNIT * exposure_precision / (50_000 * PRECISION);
    let exposure_2 = 10_000 * USDC_UNIT * exposure_precision / (60_000 * PRECISION);
    let expected_avg = 20_000 * USDC_UNIT * exposure_precision / (exposure_1 + exposure_2);
    assert_eq!(
        pos2.entry_price, expected_avg,
        "display entry must be derived from additive exposure"
    );
    assert_eq!(pos2.size, 20_000 * USDC_UNIT);
    assert_eq!(pos2.collateral, 2_000 * USDC_UNIT);

    // Third increase at 40k (larger size to shift average down)
    f.advance_time(TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 200);
    f.set_btc_price(40_000);

    f.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &(20_000 * USDC_UNIT),
        &(2_000 * USDC_UNIT),
        &true,
        &0,
        &0,
        &0i128,
    );

    let pos3 = f
        .position_manager
        .get_position(&f.trader, &symbol_short!("BTC"));
    let exposure_3 = 20_000 * USDC_UNIT * exposure_precision / (40_000 * PRECISION);
    let expected_avg3 =
        40_000 * USDC_UNIT * exposure_precision / (exposure_1 + exposure_2 + exposure_3);
    assert_eq!(
        pos3.entry_price, expected_avg3,
        "display entry must remain exposure-derived after another increase"
    );
    assert_eq!(pos3.size, 40_000 * USDC_UNIT);
}

// ---------------------------------------------------------------------------
// The minority side receives no skew credit and accrues only borrow.
// ---------------------------------------------------------------------------

#[test]
fn test_minority_side_receives_no_skew_credit() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    // Create heavy long imbalance
    let long_trader = f.create_funded_trader(50_000 * USDC_UNIT);
    let short_trader = f.create_funded_trader(10_000 * USDC_UNIT);

    f.open_long(&long_trader, 300_000 * USDC_UNIT, 30_000 * USDC_UNIT);
    f.open_short(&short_trader, 50_000 * USDC_UNIT, 5_000 * USDC_UNIT);

    // Advance 7 days to accrue carrying fees.
    f.advance_time(TEST_TIMESTAMP + 7 * 86_400);
    f.set_btc_price(50_000);
    f.update_indices(&f.keeper, &symbol_short!("BTC"));

    let market = f.position_manager.get_market(&symbol_short!("BTC"));
    assert_eq!(market.acc_short_skew_index, 100_000_000_000_000_i128);

    // Close short flat. It pays borrow and receives no counterparty transfer.
    let balance_before = f.usdc.balance(&short_trader);
    f.decrease_position(
        &short_trader,
        &symbol_short!("BTC"),
        &(50_000 * USDC_UNIT),
        &0_i128,
    );

    let returned = f.usdc.balance(&short_trader) - balance_before;

    assert!(
        returned < 5_000 * USDC_UNIT,
        "minority side must not receive a skew credit: returned={}",
        returned
    );
}

// ---------------------------------------------------------------------------
// Skew fees stay with LPs rather than entering protocol unclaimed fees.
// ---------------------------------------------------------------------------

#[test]
fn test_skew_fee_is_not_part_of_protocol_fee_split() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    // Create a long-dominant market.
    let long_trader = f.create_funded_trader(50_000 * USDC_UNIT);
    let short_trader = f.create_funded_trader(10_000 * USDC_UNIT);

    f.open_long(&long_trader, 300_000 * USDC_UNIT, 30_000 * USDC_UNIT);
    f.open_short(&short_trader, 50_000 * USDC_UNIT, 5_000 * USDC_UNIT);

    // Advance 7 days for significant carrying-fee accrual.
    f.advance_time(TEST_TIMESTAMP + 7 * 86_400);
    f.set_btc_price(50_000);
    f.update_indices(&f.keeper, &symbol_short!("BTC"));

    let pos = f
        .position_manager
        .get_position(&long_trader, &symbol_short!("BTC"));
    let market = f.position_manager.get_market(&symbol_short!("BTC"));
    let borrow_fee =
        pos.size * market.acc_borrow_index / 100_000_000_000_000_i128 - pos.borrow_fee_debt;
    let unclaimed_before = f.vault.unclaimed_fees();

    f.decrease_position(
        &long_trader,
        &symbol_short!("BTC"),
        &(300_000 * USDC_UNIT),
        &0_i128,
    );

    let unclaimed_delta = f.vault.unclaimed_fees() - unclaimed_before;
    assert_eq!(
        unclaimed_delta,
        borrow_fee / 10,
        "only the 10% protocol slice of borrow fees may become unclaimed"
    );
}

// ---------------------------------------------------------------------------
// Global avg price updates correctly with mixed operations
// ---------------------------------------------------------------------------

#[test]
fn test_global_avg_price_tracking() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    // Trader A opens long at 50k
    let trader_a = f.create_funded_trader(10_000 * USDC_UNIT);
    f.open_long(&trader_a, 20_000 * USDC_UNIT, 2_000 * USDC_UNIT);

    let market1 = f.position_manager.get_market(&symbol_short!("BTC"));
    assert_eq!(
        market1.global_long_avg_price,
        50_000 * PRECISION,
        "First long avg price should be 50k"
    );

    // Trader B opens long at 60k
    f.advance_time(TEST_TIMESTAMP + 30);
    f.set_btc_price(60_000);

    let trader_b = f.create_funded_trader(10_000 * USDC_UNIT);
    f.open_long(&trader_b, 20_000 * USDC_UNIT, 2_000 * USDC_UNIT);

    let market2 = f.position_manager.get_market(&symbol_short!("BTC"));
    // Equal quote notionals produce the exposure-derived harmonic mean.
    let expected_long_display = 545_454_545_454_i128;
    assert_eq!(
        market2.global_long_avg_price, expected_long_display,
        "global display price must derive from additive exposure"
    );

    // Short positions tracked separately
    let trader_c = f.create_funded_trader(10_000 * USDC_UNIT);
    f.open_short(&trader_c, 10_000 * USDC_UNIT, 1_000 * USDC_UNIT);

    let market3 = f.position_manager.get_market(&symbol_short!("BTC"));
    assert_eq!(
        market3.global_short_avg_price,
        60_000 * PRECISION,
        "Short avg price should be 60k (opened at 60k)"
    );
    assert_eq!(
        market3.global_long_avg_price, expected_long_display,
        "Long avg price unchanged by short open"
    );
}

// ---------------------------------------------------------------------------
// Fee-debt accounting preserves old accrual on position increase.
// ---------------------------------------------------------------------------

#[test]
fn test_fee_debt_adds_new_slice_baseline_on_increase() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    f.open_long(&f.trader, 10_000 * USDC_UNIT, 1_000 * USDC_UNIT);

    let pos1 = f
        .position_manager
        .get_position(&f.trader, &symbol_short!("BTC"));
    let initial_borrow_debt = pos1.borrow_fee_debt;

    // Advance 24h and update indices
    f.advance_time(TEST_TIMESTAMP + 86_400);
    f.set_btc_price(50_000);
    f.update_indices(&f.keeper, &symbol_short!("BTC"));

    let market = f.position_manager.get_market(&symbol_short!("BTC"));
    let updated_borrow_idx = market.acc_borrow_index;
    assert!(
        updated_borrow_idx > 100_000_000_000_000_i128,
        "Borrow index must have increased"
    );

    // Increase position — add the new slice's debt at the current index.
    f.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &(10_000 * USDC_UNIT),
        &(1_000 * USDC_UNIT),
        &true,
        &0,
        &0,
        &0i128,
    );

    let pos2 = f
        .position_manager
        .get_position(&f.trader, &symbol_short!("BTC"));

    let size_delta = 10_000 * USDC_UNIT;
    let expected_debt =
        initial_borrow_debt + size_delta * updated_borrow_idx / 100_000_000_000_000_i128;
    assert_eq!(pos2.borrow_fee_debt, expected_debt);
}

// ---------------------------------------------------------------------------
// Profit settlement: vault pays trader from its assets
// ---------------------------------------------------------------------------

#[test]
fn test_profit_settlement_vault_pays_trader() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let size = 50_000 * USDC_UNIT;
    let collateral = 5_000 * USDC_UNIT;
    f.open_long(&f.trader, size, collateral);

    let trader_bal_after_open = f.usdc.balance(&f.trader);
    let vault_total_before = f.vault.total_assets();

    // +20% profit
    f.advance_time(TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 10);
    f.set_btc_price(60_000);

    f.decrease_position(&f.trader, &symbol_short!("BTC"), &size, &0_i128);

    let trader_returned = f.usdc.balance(&f.trader) - trader_bal_after_open;
    let vault_total_after = f.vault.total_assets();

    // Trader should get collateral + profit (~10k PnL on 50k at +20%)
    assert!(
        trader_returned > collateral,
        "Trader must receive profit: returned={}",
        trader_returned
    );

    // Vault total should decrease (it paid out profit)
    assert!(
        vault_total_after < vault_total_before,
        "Vault total must decrease after paying profit: before={}, after={}",
        vault_total_before,
        vault_total_after
    );
}

// ---------------------------------------------------------------------------
// Loss settlement: trader margin absorbed by vault
// ---------------------------------------------------------------------------

#[test]
fn test_loss_settlement_vault_absorbs_margin() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let size = 50_000 * USDC_UNIT;
    let collateral = 5_000 * USDC_UNIT;
    f.open_long(&f.trader, size, collateral);

    let vault_total_before = f.vault.total_assets();

    // -5% loss → PnL = -2.5k
    f.advance_time(TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 10);
    f.set_btc_price(47_500);

    f.decrease_position(&f.trader, &symbol_short!("BTC"), &size, &0_i128);

    let vault_total_after = f.vault.total_assets();

    // Vault total should increase (absorbed trader's loss + collateral - payout)
    assert!(
        vault_total_after > vault_total_before,
        "Vault must grow from trader loss: before={}, after={}",
        vault_total_before,
        vault_total_after
    );
}
