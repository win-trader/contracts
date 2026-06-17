//! Attack vector integration tests.
//!
//! Adversarial scenarios — simulates what a malicious actor would try
//! against the protocol. Verifies that all access controls and safety
//! checks hold under deliberate exploitation attempts.

use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env};
use test_suites::testutils::{Fixture, TEST_TIMESTAMP, USDC_UNIT};

// ---------------------------------------------------------------------------
// Sandwich attack: open → instant close blocked by min lifetime
// ---------------------------------------------------------------------------

#[test]
fn test_sandwich_open_close_blocked_by_min_lifetime() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    f.open_long(&f.trader, 10_000 * USDC_UNIT, 1_000 * USDC_UNIT);

    // Attempt instant close (same block / no time advance)
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        f.decrease_position(
            &f.trader,
            &symbol_short!("BTC"),
            &(10_000 * USDC_UNIT), &0_i128,
        );
    }));
    assert!(
        result.is_err(),
        "Instant close must be blocked by min_position_lifetime"
    );
}

// ---------------------------------------------------------------------------
// Non-keeper cannot update indices
// ---------------------------------------------------------------------------

#[test]
fn test_non_keeper_cannot_update_indices() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let random = Address::generate(&env);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        f.update_indices(&random, &symbol_short!("BTC"));
    }));
    assert!(result.is_err(), "Non-keeper must not update indices");
}

// ---------------------------------------------------------------------------
// Non-keeper cannot ADL
// ---------------------------------------------------------------------------

#[test]
fn test_non_keeper_cannot_adl() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    f.open_long(&f.trader, 10_000 * USDC_UNIT, 1_000 * USDC_UNIT);

    let random = Address::generate(&env);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        f.deleverage_position(&random, &f.trader, &symbol_short!("BTC"));
    }));
    assert!(result.is_err(), "Non-keeper must not ADL");
}

// ---------------------------------------------------------------------------
// Trader cannot self-liquidate when healthy
// ---------------------------------------------------------------------------

#[test]
fn test_trader_cannot_self_liquidate_healthy() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    f.open_long(&f.trader, 10_000 * USDC_UNIT, 5_000 * USDC_UNIT);

    f.advance_time(TEST_TIMESTAMP + 75);
    f.set_btc_price(50_000);

    // Even if trader had keeper role, healthy position can't be liquidated
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        f.liquidate(&f.keeper, &f.trader, &symbol_short!("BTC"));
    }));
    assert!(
        result.is_err(),
        "Healthy position must not be liquidatable"
    );
}

// ---------------------------------------------------------------------------
// Open position that drains all free liquidity — next one blocked
// ---------------------------------------------------------------------------

#[test]
fn test_open_position_drains_all_free_liquidity() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    // Vault has ~1M USDC. Open position at 84% utilization (just under 85% cap).
    let trader_a = f.create_funded_trader(100_000 * USDC_UNIT);
    f.open_long(&trader_a, 840_000 * USDC_UNIT, 84_000 * USDC_UNIT);

    // Now try to open one more small position — should breach 85% cap
    let trader_b = f.create_funded_trader(50_000 * USDC_UNIT);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        f.open_long(&trader_b, 20_000 * USDC_UNIT, 2_000 * USDC_UNIT);
    }));
    assert!(
        result.is_err(),
        "Position that would breach utilization cap must be rejected"
    );
}

// ---------------------------------------------------------------------------
// LP withdrawal limited when utilization is high
// ---------------------------------------------------------------------------

#[test]
fn test_vault_withdrawal_limited_during_high_utilization() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    // LP deposits, then large position reserves most liquidity
    let lp = Address::generate(&env);
    let deposit = 200_000 * USDC_UNIT;
    f.usdc.mint(&lp, &deposit);
    f.deposit(&deposit, &lp, &lp, &lp);

    // Now vault has ~1.2M total. Reserve 840k (just under 85% of ~1M base).
    let trader = f.create_funded_trader(100_000 * USDC_UNIT);
    f.open_long(&trader, 840_000 * USDC_UNIT, 84_000 * USDC_UNIT);

    f.advance_time(TEST_TIMESTAMP + 200);
    f.set_btc_price(50_000);

    // Free liquidity is constrained — LP's max_withdraw should be limited
    let free = f.vault.free_liquidity();
    let max_w = f.vault.max_withdraw(&lp);

    // max_withdraw should not exceed free liquidity
    assert!(
        max_w <= free,
        "LP max_withdraw must be bounded by free liquidity: max_w={}, free={}",
        max_w,
        free
    );
    // Free liquidity should be less than total vault assets (reservation constrains it)
    let total = f.vault.total_assets();
    assert!(
        free < total,
        "Free liquidity must be less than total assets when positions are reserved: free={}, total={}",
        free,
        total
    );
}

// ---------------------------------------------------------------------------
// Non-PM cannot call vault.pay_profit
// ---------------------------------------------------------------------------

#[test]
fn test_non_pm_cannot_call_vault_pay_profit() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let random = Address::generate(&env);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        f.vault
            .pay_profit(&random, &f.trader, &(1_000 * USDC_UNIT));
    }));
    assert!(result.is_err(), "Non-PM cannot call pay_profit");
}

// ---------------------------------------------------------------------------
// Non-PM cannot call vault.reserve_liquidity
// ---------------------------------------------------------------------------

#[test]
fn test_non_pm_cannot_reserve_liquidity() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let random = Address::generate(&env);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        f.vault
            .reserve_liquidity(&random, &(1_000 * USDC_UNIT));
    }));
    assert!(result.is_err(), "Non-PM cannot call reserve_liquidity");
}

// ---------------------------------------------------------------------------
// Non-admin cannot set max leverage
// ---------------------------------------------------------------------------

#[test]
fn test_non_admin_cannot_set_max_leverage() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let random = Address::generate(&env);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        f.set_max_leverage(&random, &symbol_short!("BTC"), &50_i128);
    }));
    assert!(result.is_err(), "Non-admin cannot set max leverage");
}

// ---------------------------------------------------------------------------
// Dust position with excessive leverage rejected
// ---------------------------------------------------------------------------

#[test]
fn test_grief_dust_position_excessive_leverage() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    // Dust collateral with high leverage (>100x) gets rejected by leverage check
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        f.increase_position(
            &f.trader,
            &symbol_short!("BTC"),
            &(1_000 * USDC_UNIT), // 1k USDC size
            &(1),                  // 0.000001 USDC collateral → 1B x leverage
            &true,
            &0,
            &0, &0i128
    );
    }));
    assert!(result.is_err(), "Dust collateral with excessive leverage must be rejected");
}
