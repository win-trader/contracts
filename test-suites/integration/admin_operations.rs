//! Admin operations integration tests.
//!
//! Tests administrative actions while the protocol is live with active
//! positions and LP deposits: pause/unpause, role management, fee claims,
//! protocol limit updates.

use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env, Symbol};
use test_suites::testutils::{Fixture, BTC_PRICE, TEST_TIMESTAMP, USDC_UNIT};

const MIN_POSITION_LIFETIME: u64 = 60;

// ---------------------------------------------------------------------------
// Pause/unpause while positions are open
// ---------------------------------------------------------------------------

#[test]
fn test_pause_blocks_new_positions_but_allows_close() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    // Trader opens before pause
    f.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &(10_000 * USDC_UNIT),
        &(1_000 * USDC_UNIT),
        &true,
        &0,
        &0, &0i128
    );

    // Admin pauses
    f.pause_pm(&f.admin);

    // New position should fail
    let trader_b = Address::generate(&env);
    f.usdc.mint(&trader_b, &(5_000 * USDC_UNIT));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        f.increase_position(
            &trader_b,
            &symbol_short!("BTC"),
            &(10_000 * USDC_UNIT),
            &(1_000 * USDC_UNIT),
            &true,
            &0,
            &0, &0i128
    );
    }));
    assert!(result.is_err(), "New positions must be blocked when paused");

    // Existing position can still close
    f.advance_time(TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 10);
    f.mock_oracle.set_price(&symbol_short!("BTC"), &BTC_PRICE);
    f.decrease_position(&f.trader, &symbol_short!("BTC"), &(10_000 * USDC_UNIT), &0_i128);

    let market = f.position_manager.get_market(&symbol_short!("BTC"));
    assert_eq!(market.long_open_interest, 0);
}

#[test]
fn test_unpause_restores_normal_operations() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    f.pause_pm(&f.admin);
    f.unpause_pm(&f.admin);

    // Should work again
    f.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &(10_000 * USDC_UNIT),
        &(1_000 * USDC_UNIT),
        &true,
        &0,
        &0, &0i128
    );

    let pos = f
        .position_manager
        .get_position(&f.trader, &symbol_short!("BTC"));
    assert_eq!(pos.size, 10_000 * USDC_UNIT);
}

// ---------------------------------------------------------------------------
// Pause/unpause vault — blocks deposits but not withdrawals-in-progress
// ---------------------------------------------------------------------------

#[test]
fn test_vault_pause_blocks_deposits() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    f.pause_vault(&f.admin);

    let lp = Address::generate(&env);
    f.usdc.mint(&lp, &(100_000 * USDC_UNIT));

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        f.deposit(&(100_000 * USDC_UNIT), &lp, &lp, &lp);
    }));
    assert!(result.is_err(), "Deposits must be blocked when paused");

    // max_deposit returns 0 when paused
    let max_dep = f.vault.max_deposit(&lp);
    assert_eq!(max_dep, 0);
}

// ---------------------------------------------------------------------------
// Role management: grant/revoke keeper mid-flight
// ---------------------------------------------------------------------------

#[test]
fn test_revoke_keeper_blocks_index_updates() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let keeper_role = Symbol::new(&env, "KEEPER");

    // Revoke keeper role
    f.config_manager
        .revoke_role(&f.admin, &keeper_role, &f.keeper);

    // Keeper can no longer update indices
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        f.update_indices(&f.keeper, &symbol_short!("BTC"));
    }));
    assert!(
        result.is_err(),
        "Revoked keeper must not be able to update indices"
    );
}

#[test]
fn test_grant_new_keeper_can_liquidate() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let new_keeper = Address::generate(&env);
    let keeper_role = Symbol::new(&env, "KEEPER");
    f.config_manager
        .grant_role(&f.admin, &keeper_role, &new_keeper);

    // Trader opens risky position
    let trader = Address::generate(&env);
    f.usdc.mint(&trader, &(5_000 * USDC_UNIT));
    f.increase_position(
        &trader,
        &symbol_short!("BTC"),
        &(20_000 * USDC_UNIT),
        &(2_000 * USDC_UNIT),
        &true,
        &0,
        &0, &0i128
    );

    // Crash
    f.advance_time(TEST_TIMESTAMP + 75);
    let crash_price: i128 = 44_000 * 10_000_000;
    f.mock_oracle.set_price(&symbol_short!("BTC"), &crash_price);

    // New keeper can liquidate
    f.liquidate(&new_keeper, &trader, &symbol_short!("BTC"));

    let market = f.position_manager.get_market(&symbol_short!("BTC"));
    assert_eq!(market.long_open_interest, 0);
}

// ---------------------------------------------------------------------------
// Admin transfers ownership
// ---------------------------------------------------------------------------

#[test]
fn test_admin_transfer_and_new_admin_operates() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let new_admin = Address::generate(&env);
    f.config_manager.propose_admin(&f.admin, &new_admin);
    f.config_manager.accept_admin(&new_admin);

    // Old admin can't update limits anymore
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        f.config_manager.update_protocol_limits(
            &f.admin,
            &config_manager::ProtocolLimits {
                min_collateral: 500_000,
                cooldown_duration: 30,
                min_position_lifetime: 30,
                max_utilization_ratio: 9_000,
                funding_cut_bps: 500,
                adl_pnl_bps: 9_000,
                adl_utilization_bps: 9_500,
                liquidation_threshold_bps: 200,
            },
        );
    }));
    assert!(
        result.is_err(),
        "Old admin must lose privileges after transfer"
    );

    // New admin can update limits
    f.config_manager.update_protocol_limits(
        &new_admin,
        &config_manager::ProtocolLimits {
            min_collateral: 500_000,
            cooldown_duration: 30,
            min_position_lifetime: 30,
            max_utilization_ratio: 9_000,
            funding_cut_bps: 500,
            adl_pnl_bps: 9_000,
            adl_utilization_bps: 9_500,
            liquidation_threshold_bps: 200,
        },
    );
    f.config_manager.update_borrow_rate_config(&new_admin, &config_manager::BorrowRateConfig {
        base_borrow_rate_bps: 100,
        slope1_bps: 500,
        slope2_bps: 5_000,
        optimal_utilization_bps: 8_000,
        base_funding_rate_bps: 100,
    });

    let limits = f.config_manager.get_protocol_limits();
    assert_eq!(limits.max_utilization_ratio, 9_000);
}

// ---------------------------------------------------------------------------
// Non-admin cannot pause
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn test_random_user_cannot_pause() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let random = Address::generate(&env);
    f.pause_pm(&random);
}

// ---------------------------------------------------------------------------
// Protocol limit update affects new positions
// ---------------------------------------------------------------------------

#[test]
fn test_updated_utilization_cap_allows_larger_positions() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    // Default cap is 85%. Raise to 95%.
    f.config_manager.update_protocol_limits(
        &f.admin,
        &config_manager::ProtocolLimits {
            min_collateral: 1_000_000,
            cooldown_duration: 60,
            min_position_lifetime: 60,
            max_utilization_ratio: 9_500,
            funding_cut_bps: 500,
            adl_pnl_bps: 9_000,
            adl_utilization_bps: 9_500,
            liquidation_threshold_bps: 200,
        },
    );
    f.config_manager.update_borrow_rate_config(&f.admin, &config_manager::BorrowRateConfig {
        base_borrow_rate_bps: 100,
        slope1_bps: 500,
        slope2_bps: 5_000,
        optimal_utilization_bps: 8_000,
        base_funding_rate_bps: 100,
    });

    // Now a 90% utilization position should work (was blocked at 85%)
    let size = 900_000 * USDC_UNIT;
    let collateral = 90_000 * USDC_UNIT;
    f.usdc.mint(&f.trader, &collateral);

    f.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &size,
        &collateral,
        &true,
        &0,
        &0, &0i128
    );

    let market = f.position_manager.get_market(&symbol_short!("BTC"));
    assert_eq!(market.long_open_interest, size);
}
