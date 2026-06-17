//! Tightened bounds on protocol-config setters.
//!
//! These tests cover:
//!   * `update_fee_splits` u32 overflow — adversarial (u32::MAX, 1, 1) must
//!     return `InvalidFeeSplits` rather than trapping the host on overflow.
//!   * `update_protocol_limits` `funding_cut_bps` ceiling — must reject
//!     values above `MAX_FUNDING_CUT_BPS` (= 30%, stops admin from sending
//!     the entire funding stream to the protocol).
//!   * `update_protocol_limits` `adl_pnl_bps` floor — must reject values
//!     below `MIN_ADL_PNL_BPS` (= 50%, stops admin from configuring
//!     continuous ADL).
//!   * `update_borrow_rate_config` `slope2_bps` ceiling — must reject values
//!     above `MAX_SLOPE2_BPS` to prevent PM borrow-fee math overflow.
//!   * `update_protocol_limits` `cooldown_duration` ceiling — must reject
//!     values exceeding the TTL of the `LockupExpiresAt` slot.

use soroban_sdk::Env;

use crate::{BorrowRateConfig, ConfigManagerError, FeeSplits};
use shared::constants::{
    BPS, DEFAULT_BASE_BORROW_RATE_BPS, DEFAULT_BASE_FUNDING_RATE_BPS, DEFAULT_OPTIMAL_UTILIZATION_BPS,
    DEFAULT_SLOPE1_BPS, MAX_COOLDOWN_DURATION, MAX_FUNDING_CUT_BPS, MAX_SLOPE2_BPS, MIN_ADL_PNL_BPS,
};

use super::helpers::{deploy_initialized, valid_limits};

// ---------------------------------------------------------------------------
// update_fee_splits — u32 overflow defense via u64-promoted sum check
// ---------------------------------------------------------------------------

/// Adversarial (u32::MAX, 1, 1) must not overflow. The validator promotes
/// the sum to u64 before comparing to 10_000 — the wrong sum surfaces as
/// InvalidFeeSplitSum, NOT a host trap.
#[test]
fn test_update_fee_splits_u32_max_does_not_overflow() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let splits = FeeSplits {
        lp_bps: u32::MAX,
        dev_bps: 1,
        staker_bps: 1,
    };

    let result = client.try_update_fee_splits(&admin, &splits);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(ConfigManagerError::InvalidFeeSplitSum as u32),
        "u32-max component must reject via the u64-sum rule, not a per-component check",
    );
}

/// A component alone exceeding BPS but combined with zero pads still fails
/// the sum check (validator no longer enforces a separate per-component
/// over-bps check — the sum invariant suffices).
#[test]
fn test_update_fee_splits_component_above_bps_rejected_via_sum() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let splits = FeeSplits {
        lp_bps: (BPS as u32) + 1,
        dev_bps: 0,
        staker_bps: 0,
    };

    let result = client.try_update_fee_splits(&admin, &splits);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(ConfigManagerError::InvalidFeeSplitSum as u32),
    );
}

// ---------------------------------------------------------------------------
// update_protocol_limits — funding_cut_bps ceiling
// ---------------------------------------------------------------------------

/// funding_cut_bps = MAX_FUNDING_CUT_BPS (boundary, inclusive) is accepted.
#[test]
fn test_update_protocol_limits_funding_cut_at_ceiling_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let mut limits = valid_limits();
    limits.funding_cut_bps = MAX_FUNDING_CUT_BPS;

    client.update_protocol_limits(&admin, &limits);
    assert_eq!(client.get_protocol_limits().funding_cut_bps, MAX_FUNDING_CUT_BPS);
}

/// funding_cut_bps > MAX_FUNDING_CUT_BPS rejected with InvalidLimits.
#[test]
fn test_update_protocol_limits_funding_cut_above_ceiling_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let mut limits = valid_limits();
    limits.funding_cut_bps = MAX_FUNDING_CUT_BPS + 1;

    let result = client.try_update_protocol_limits(&admin, &limits);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(ConfigManagerError::InvalidFundingCut as u32),
    );
}

// ---------------------------------------------------------------------------
// update_protocol_limits — adl_pnl_bps floor
// ---------------------------------------------------------------------------

/// adl_pnl_bps = MIN_ADL_PNL_BPS (boundary, inclusive) is accepted.
#[test]
fn test_update_protocol_limits_adl_pnl_at_floor_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let mut limits = valid_limits();
    limits.adl_pnl_bps = MIN_ADL_PNL_BPS;

    client.update_protocol_limits(&admin, &limits);
    assert_eq!(client.get_protocol_limits().adl_pnl_bps, MIN_ADL_PNL_BPS);
}

/// adl_pnl_bps < MIN_ADL_PNL_BPS rejected with InvalidLimits.
#[test]
fn test_update_protocol_limits_adl_pnl_below_floor_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let mut limits = valid_limits();
    limits.adl_pnl_bps = MIN_ADL_PNL_BPS - 1;

    let result = client.try_update_protocol_limits(&admin, &limits);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(ConfigManagerError::InvalidAdlPnl as u32),
    );
}

// ---------------------------------------------------------------------------
// update_protocol_limits — cooldown_duration ceiling
// ---------------------------------------------------------------------------

/// cooldown_duration = MAX_COOLDOWN_DURATION (boundary, inclusive) succeeds.
#[test]
fn test_update_protocol_limits_cooldown_at_ceiling_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let mut limits = valid_limits();
    limits.cooldown_duration = MAX_COOLDOWN_DURATION;

    client.update_protocol_limits(&admin, &limits);
    assert_eq!(client.get_protocol_limits().cooldown_duration, MAX_COOLDOWN_DURATION);
}

/// cooldown_duration > MAX_COOLDOWN_DURATION rejected — the lockup slot
/// must not outlive its TTL bump window.
#[test]
fn test_update_protocol_limits_cooldown_above_ceiling_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let mut limits = valid_limits();
    limits.cooldown_duration = MAX_COOLDOWN_DURATION + 1;

    let result = client.try_update_protocol_limits(&admin, &limits);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(ConfigManagerError::InvalidCooldownDuration as u32),
    );
}

// ---------------------------------------------------------------------------
// update_borrow_rate_config — slope2_bps ceiling
// ---------------------------------------------------------------------------

fn valid_rate_config() -> BorrowRateConfig {
    BorrowRateConfig {
        base_borrow_rate_bps: DEFAULT_BASE_BORROW_RATE_BPS,
        slope1_bps: DEFAULT_SLOPE1_BPS,
        slope2_bps: 5_000,
        optimal_utilization_bps: DEFAULT_OPTIMAL_UTILIZATION_BPS,
        base_funding_rate_bps: DEFAULT_BASE_FUNDING_RATE_BPS,
    }
}

/// slope2_bps = MAX_SLOPE2_BPS (boundary, inclusive) succeeds.
#[test]
fn test_update_borrow_rate_slope2_at_ceiling_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let mut cfg = valid_rate_config();
    cfg.slope2_bps = MAX_SLOPE2_BPS;

    client.update_borrow_rate_config(&admin, &cfg);
    assert_eq!(client.get_borrow_rate_config().slope2_bps, MAX_SLOPE2_BPS);
}

/// slope2_bps > MAX_SLOPE2_BPS rejected with InvalidLimits.
#[test]
fn test_update_borrow_rate_slope2_above_ceiling_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let mut cfg = valid_rate_config();
    cfg.slope2_bps = MAX_SLOPE2_BPS + 1;

    let result = client.try_update_borrow_rate_config(&admin, &cfg);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(ConfigManagerError::InvalidSlopeTooSteep as u32),
    );
}
