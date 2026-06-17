//! Error path integration tests.
//!
//! Systematically triggers every reachable error variant through the
//! integration layer to ensure validation logic is correctly wired.

use soroban_sdk::{symbol_short, Env};
use test_suites::testutils::{Fixture, BTC_PRICE, TEST_TIMESTAMP, USDC_UNIT};

const PRECISION: i128 = 10_000_000;

// ---------------------------------------------------------------------------
// ZeroAmount errors
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn test_open_position_zero_size() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    f.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &0, // zero size
        &(1_000 * USDC_UNIT),
        &true,
        &0,
        &0, &0i128
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn test_open_position_zero_collateral() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    f.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &(10_000 * USDC_UNIT),
        &0, // zero collateral
        &true,
        &0,
        &0, &0i128
    );
}

// ---------------------------------------------------------------------------
// MarketNotConfigured
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "Error(Contract, #12)")]
fn test_open_on_unconfigured_market() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    // ETH is not configured — only BTC has max_leverage set
    f.mock_oracle.set_price(&symbol_short!("ETH"), &(3_000 * PRECISION));
    f.oracle_router.set_oracle_sources(
        &f.admin,
        &symbol_short!("ETH"),
        &soroban_sdk::vec![
            &env,
            f.mock_oracle.a.address.clone(),
            f.mock_oracle.b.address.clone()
        ],
    );

    f.increase_position(
        &f.trader,
        &symbol_short!("ETH"),
        &(10_000 * USDC_UNIT),
        &(1_000 * USDC_UNIT),
        &true,
        &0,
        &0, &0i128
    );
}

// ---------------------------------------------------------------------------
// ExcessiveLeverage
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "Error(Contract, #11)")]
fn test_excessive_leverage_rejected() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    // 100x max leverage is configured. Try 200x.
    f.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &(200_000 * USDC_UNIT), // 200k size
        &(1_000 * USDC_UNIT),   // 1k collateral = 200x leverage
        &true,
        &0,
        &0, &0i128
    );
}

// ---------------------------------------------------------------------------
// PositionNotOldEnough
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_close_before_min_lifetime() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    f.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &(10_000 * USDC_UNIT),
        &(1_000 * USDC_UNIT),
        &true,
        &0,
        &0, &0i128
    );

    // Try to close immediately — min_position_lifetime is 60s
    f.decrease_position(
        &f.trader,
        &symbol_short!("BTC"),
        &(10_000 * USDC_UNIT), &0_i128,
    );
}

// ---------------------------------------------------------------------------
// PositionNotFound
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn test_close_nonexistent_position() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    f.decrease_position(
        &f.trader,
        &symbol_short!("BTC"),
        &(10_000 * USDC_UNIT), &0_i128,
    );
}

// ---------------------------------------------------------------------------
// HealthFactorOk — liquidating a healthy position
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "Error(Contract, #9)")]
fn test_liquidate_healthy_position() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    f.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &(10_000 * USDC_UNIT),
        &(5_000 * USDC_UNIT), // 2x leverage — very healthy
        &true,
        &0,
        &0, &0i128
    );

    f.advance_time(TEST_TIMESTAMP + 75);
    f.mock_oracle.set_price(&symbol_short!("BTC"), &BTC_PRICE);

    f.liquidate(
        &f.keeper,
        &f.trader,
        &symbol_short!("BTC"),
    );
}

// ---------------------------------------------------------------------------
// AdlNotTriggered
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "Error(Contract, #10)")]
fn test_adl_when_conditions_not_met() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    f.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &(10_000 * USDC_UNIT),
        &(5_000 * USDC_UNIT),
        &true,
        &0,
        &0, &0i128
    );

    f.advance_time(TEST_TIMESTAMP + 75);
    f.mock_oracle.set_price(&symbol_short!("BTC"), &BTC_PRICE);

    // ADL conditions not met (low utilization, no extreme PnL)
    f.deleverage_position(
        &f.keeper,
        &f.trader,
        &symbol_short!("BTC"),
    );
}

// ---------------------------------------------------------------------------
// OrderNotTriggered — TP/SL not hit
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "Error(Contract, #13)")]
fn test_execute_order_no_trigger() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let tp_price: i128 = 60_000 * PRECISION; // TP at 60k
    let sl_price: i128 = 40_000 * PRECISION; // SL at 40k

    f.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &(10_000 * USDC_UNIT),
        &(1_000 * USDC_UNIT),
        &true,
        &tp_price,
        &sl_price, &0i128
    );

    f.advance_time(TEST_TIMESTAMP + 75);
    // Price is still at 50k — neither TP (60k) nor SL (40k) hit
    f.mock_oracle.set_price(&symbol_short!("BTC"), &BTC_PRICE);

    f.execute_order(
        &f.keeper,
        &f.trader,
        &symbol_short!("BTC"),
    );
}

// ---------------------------------------------------------------------------
// UtilizationCapBreached
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "Error(Contract, #4)")]
fn test_utilization_cap_breach() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    // Vault has ~1M USDC. 85% cap = 850k. Try to open 900k position.
    let trader = f.create_funded_trader(100_000 * USDC_UNIT);
    f.increase_position(
        &trader,
        &symbol_short!("BTC"),
        &(900_000 * USDC_UNIT),
        &(90_000 * USDC_UNIT),
        &true,
        &0,
        &0, &0i128
    );
}

// ---------------------------------------------------------------------------
// InvalidTpSl — negative TP rejected
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "Error(Contract, #14)")]
fn test_invalid_tp_negative() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    f.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &(10_000 * USDC_UNIT),
        &(1_000 * USDC_UNIT),
        &true,
        &(-1_i128),
        &0,
        &0i128,
    );
}

// ---------------------------------------------------------------------------
// InvalidTpSl — negative SL rejected
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "Error(Contract, #14)")]
fn test_invalid_sl_negative() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    f.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &(10_000 * USDC_UNIT),
        &(1_000 * USDC_UNIT),
        &false,
        &0,
        &(-1_i128),
        &0i128,
    );
}

// ---------------------------------------------------------------------------
// AlreadyInitialized
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn test_set_vault_twice_reverts() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    // Fixture::deploy already wired the Vault via the one-shot `set_vault`.
    // The trusted Vault link is immutable after deploy — re-wiring reverts
    // with AlreadyInitialized (#1).
    f.position_manager.set_vault(&f.admin, &f.vault_addr);
}
