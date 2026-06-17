//! Property-based fuzz tests for position management.
//!
//! Uses proptest to generate random but bounded inputs (sizes, collateral,
//! price movements, directions) and asserts protocol invariants hold.

use proptest::prelude::*;
use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env};
use test_suites::testutils::{Fixture, BTC_PRICE, TEST_TIMESTAMP, USDC_UNIT};

const MIN_POSITION_LIFETIME: u64 = 60;

fn fund_trader(f: &Fixture, trader: &Address, amount: &i128) {
    f.usdc.mint(trader, amount);
}

// ---------------------------------------------------------------------------
// Invariant: OI tracking is always consistent after open + close
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn fuzz_open_close_oi_consistent(
        size_mult in 1_i128..=40,        // 1k–40k USDC size
        collateral_bps in 500_u32..=5000, // 5%–50% collateral ratio
        is_long in proptest::bool::ANY,
        price_change_pct in -10_i64..=10, // -10% to +10% price move
    ) {
        let env = Env::default();
        let f = Fixture::deploy(&env);

        let size = size_mult * 1_000 * USDC_UNIT;
        let collateral = (size * collateral_bps as i128) / 10_000;
        let collateral = core::cmp::max(collateral, 2 * USDC_UNIT); // ensure >= min_collateral

        let trader = Address::generate(&env);
        fund_trader(&f, &trader, &(collateral + 1_000 * USDC_UNIT));

        // Open position
        f.increase_position(
            &trader,
            &symbol_short!("BTC"),
            &size,
            &collateral,
            &is_long,
            &0,
            &0, &0i128
    );

        let market_after_open = f.position_manager.get_market(&symbol_short!("BTC"));
        if is_long {
            prop_assert_eq!(market_after_open.long_open_interest, size);
        } else {
            prop_assert_eq!(market_after_open.short_open_interest, size);
        }

        // Move price (ensure it stays positive and doesn't cause liquidation)
        let clamped_pct = price_change_pct.clamp(-3, 3); // keep small to avoid liquidation
        let new_price = BTC_PRICE + (BTC_PRICE * clamped_pct as i128 / 100);
        f.advance_time(TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 10);
        f.mock_oracle.set_price(&symbol_short!("BTC"), &new_price);

        // Close position
        f.decrease_position(&trader, &symbol_short!("BTC"), &size, &0_i128);

        // INVARIANT: OI must be zero after full close
        let market_after_close = f.position_manager.get_market(&symbol_short!("BTC"));
        prop_assert_eq!(market_after_close.long_open_interest, 0);
        prop_assert_eq!(market_after_close.short_open_interest, 0);
    }
}

// ---------------------------------------------------------------------------
// Invariant: position entry price always equals oracle price at open time
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(30))]

    #[test]
    fn fuzz_entry_price_equals_oracle(
        size_mult in 1_i128..=20,
        is_long in proptest::bool::ANY,
    ) {
        let env = Env::default();
        let f = Fixture::deploy(&env);

        let size = size_mult * 1_000 * USDC_UNIT;
        let collateral = size / 10; // 10x leverage
        let collateral = core::cmp::max(collateral, 2 * USDC_UNIT);

        let trader = Address::generate(&env);
        fund_trader(&f, &trader, &(collateral + 1_000 * USDC_UNIT));

        f.increase_position(
            &trader,
            &symbol_short!("BTC"),
            &size,
            &collateral,
            &is_long,
            &0,
            &0, &0i128
    );

        let pos = f.position_manager.get_position(&trader, &symbol_short!("BTC"));
        prop_assert_eq!(pos.entry_price, BTC_PRICE, "Entry price must match oracle");
        prop_assert_eq!(pos.is_long, is_long);
        prop_assert_eq!(pos.size, size);
        prop_assert_eq!(pos.collateral, collateral);
    }
}

// ---------------------------------------------------------------------------
// Invariant: trader balance change matches position direction
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(30))]

    #[test]
    fn fuzz_pnl_direction_matches_price_move(
        size_mult in 2_i128..=10,
        is_long in proptest::bool::ANY,
        // Use only price moves of at least 2% to ensure non-trivial PnL
        price_dir in proptest::bool::ANY,
    ) {
        let env = Env::default();
        let f = Fixture::deploy(&env);

        let size = size_mult * 1_000 * USDC_UNIT;
        let collateral = size / 5; // 5x leverage — safe from liquidation at ±3%

        let trader = Address::generate(&env);
        fund_trader(&f, &trader, &(collateral + 1_000 * USDC_UNIT));

        f.increase_position(
            &trader,
            &symbol_short!("BTC"),
            &size,
            &collateral,
            &is_long,
            &0,
            &0, &0i128
    );

        let balance_after_open = f.usdc.balance(&trader);

        // Move price 3% up or down
        let price_delta = BTC_PRICE * 3 / 100;
        let new_price = if price_dir {
            BTC_PRICE + price_delta
        } else {
            BTC_PRICE - price_delta
        };

        f.advance_time(TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 10);
        f.mock_oracle.set_price(&symbol_short!("BTC"), &new_price);

        f.decrease_position(&trader, &symbol_short!("BTC"), &size, &0_i128);

        let balance_after_close = f.usdc.balance(&trader);
        let received = balance_after_close - balance_after_open;

        // Long+up or short+down = profit (received > collateral)
        // Long+down or short+up = loss (received < collateral)
        let expect_profit = is_long == price_dir;

        if expect_profit {
            prop_assert!(
                received > collateral,
                "Expected profit: received={}, collateral={}",
                received, collateral
            );
        } else {
            prop_assert!(
                received < collateral,
                "Expected loss: received={}, collateral={}",
                received, collateral
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Invariant: multiple random opens then full closes leave OI at zero
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20))]

    #[test]
    fn fuzz_multi_open_close_clears_oi(
        num_traders in 2_usize..=5,
    ) {
        let env = Env::default();
        let f = Fixture::deploy(&env);

        let size = 5_000 * USDC_UNIT;
        let collateral = 2_500 * USDC_UNIT; // 2x leverage — very safe

        let mut traders = Vec::new();
        for _ in 0..num_traders {
            let t = Address::generate(&env);
            fund_trader(&f, &t, &(collateral + 1_000 * USDC_UNIT));
            traders.push(t);
        }

        // All open longs
        for t in &traders {
            f.increase_position(
                t,
                &symbol_short!("BTC"),
                &size,
                &collateral,
                &true,
                &0,
                &0, &0i128
    );
        }

        let market_open = f.position_manager.get_market(&symbol_short!("BTC"));
        prop_assert_eq!(
            market_open.long_open_interest,
            size * num_traders as i128
        );

        f.advance_time(TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 10);
        f.mock_oracle.set_price(&symbol_short!("BTC"), &BTC_PRICE);

        // All close
        for t in &traders {
            f.decrease_position(t, &symbol_short!("BTC"), &size, &0_i128);
        }

        let market_closed = f.position_manager.get_market(&symbol_short!("BTC"));
        prop_assert_eq!(market_closed.long_open_interest, 0);
        prop_assert_eq!(market_closed.short_open_interest, 0);
    }
}
