//! Property-based fuzz tests for vault operations.
//!
//! Uses proptest to generate random deposit/withdraw sequences and verify
//! vault invariants: share accounting, free liquidity, solvency.

use proptest::prelude::*;
use soroban_sdk::{testutils::Address as _, Address, Env};
use test_suites::testutils::{Fixture, TEST_TIMESTAMP, USDC_UNIT};

fn fund_lp(f: &Fixture, lp: &Address, amount: &i128) {
    f.usdc.mint(lp, amount);
}

// ---------------------------------------------------------------------------
// Invariant: deposit then full withdraw returns original amount (no trading)
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(30))]

    #[test]
    fn fuzz_deposit_withdraw_round_trip(
        deposit_mult in 1_i128..=500, // 1k–500k USDC
    ) {
        let env = Env::default();
        let f = Fixture::deploy(&env);

        let lp = Address::generate(&env);
        let deposit = deposit_mult * 1_000 * USDC_UNIT;
        fund_lp(&f, &lp, &deposit);

        let shares = f.deposit(&deposit, &lp, &lp, &lp);
        prop_assert!(shares > 0, "Must receive shares");

        // Wait for cooldown
        f.advance_time(TEST_TIMESTAMP + 200);

        let max_withdraw = f.vault.max_withdraw(&lp);

        // With no trading, withdraw amount should equal deposit
        // (small rounding error allowed due to share math)
        let diff = (max_withdraw - deposit).abs();
        prop_assert!(
            diff <= 1, // 1 micro-USDC rounding tolerance
            "Round-trip mismatch: deposited={}, max_withdraw={}, diff={}",
            deposit, max_withdraw, diff
        );
    }
}

// ---------------------------------------------------------------------------
// Invariant: total_assets >= sum of all deposits (no trading = no loss)
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20))]

    #[test]
    fn fuzz_multi_lp_total_assets_consistent(
        num_lps in 2_usize..=5,
        deposit_mult in 10_i128..=100,
    ) {
        let env = Env::default();
        let f = Fixture::deploy(&env);

        let per_deposit = deposit_mult * 1_000 * USDC_UNIT;
        let mut total_deposited: i128 = 0;

        // The fixture already deposited VAULT_DEPOSIT (1M)
        let initial_assets = f.vault.total_assets();

        for _ in 0..num_lps {
            let lp = Address::generate(&env);
            fund_lp(&f, &lp, &per_deposit);
            f.deposit(&per_deposit, &lp, &lp, &lp);
            total_deposited += per_deposit;
        }

        let final_assets = f.vault.total_assets();
        let expected = initial_assets + total_deposited;

        prop_assert_eq!(
            final_assets, expected,
            "total_assets must equal initial + all deposits"
        );
    }
}

// ---------------------------------------------------------------------------
// Invariant: shares are proportional to deposit amounts
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20))]

    #[test]
    fn fuzz_share_proportionality(
        base_mult in 10_i128..=100,
        ratio in 2_i128..=5,
    ) {
        let env = Env::default();
        let f = Fixture::deploy(&env);

        let lp_a = Address::generate(&env);
        let lp_b = Address::generate(&env);

        let dep_a = base_mult * 1_000 * USDC_UNIT;
        let dep_b = dep_a * ratio;

        fund_lp(&f, &lp_a, &dep_a);
        fund_lp(&f, &lp_b, &dep_b);

        let shares_a = f.deposit(&dep_a, &lp_a, &lp_a, &lp_a);
        let shares_b = f.deposit(&dep_b, &lp_b, &lp_b, &lp_b);

        // shares_b / shares_a should be ~ratio (within 1% tolerance)
        let actual_ratio_x100 = (shares_b * 100) / shares_a;
        let expected_ratio_x100 = ratio * 100;
        let tolerance = 2; // 2% tolerance for rounding

        prop_assert!(
            (actual_ratio_x100 - expected_ratio_x100).abs() <= tolerance,
            "Share ratio off: shares_a={}, shares_b={}, expected_ratio={}, actual_ratio_x100={}",
            shares_a, shares_b, ratio, actual_ratio_x100
        );
    }
}

// ---------------------------------------------------------------------------
// Invariant: free_liquidity is non-negative
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20))]

    #[test]
    fn fuzz_free_liquidity_non_negative(
        deposit_mult in 1_i128..=200,
    ) {
        let env = Env::default();
        let f = Fixture::deploy(&env);

        let lp = Address::generate(&env);
        let deposit = deposit_mult * 1_000 * USDC_UNIT;
        fund_lp(&f, &lp, &deposit);
        f.deposit(&deposit, &lp, &lp, &lp);

        let free = f.vault.free_liquidity();
        prop_assert!(free >= 0, "Free liquidity must never be negative: {}", free);
    }
}

// ---------------------------------------------------------------------------
// Invariant: convert_to_shares → convert_to_assets is idempotent
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(30))]

    #[test]
    fn fuzz_share_asset_conversion_roundtrip(
        asset_mult in 1_i128..=1000,
    ) {
        let env = Env::default();
        let f = Fixture::deploy(&env);

        let assets = asset_mult * USDC_UNIT;
        let shares = f.vault.convert_to_shares(&assets);
        let assets_back = f.vault.convert_to_assets(&shares);

        // May lose 1 unit to rounding
        let diff = (assets - assets_back).abs();
        prop_assert!(
            diff <= 1,
            "Conversion roundtrip error: {} -> {} shares -> {}, diff={}",
            assets, shares, assets_back, diff
        );
    }
}
