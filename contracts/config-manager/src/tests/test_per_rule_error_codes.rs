//! Pin per-rule error codes that aren't exercised by other test files.
//!
//! Each per-rule error variant (codes 20–43 in `ConfigManagerError`) maps to
//! a specific validation failure. Most are exercised indirectly by
//! `test_protocol_limits.rs`, `test_fee_splits.rs`, `test_bounds_tightening.rs`,
//! and `test_liquidation_threshold.rs`, but a handful had no dedicated
//! coverage. This file fills those gaps so a future renumbering or
//! re-mapping of error codes is caught.

#![cfg(test)]

use soroban_sdk::Env;
use shared::constants::{
    BPS, DEFAULT_BASE_BORROW_RATE_BPS, DEFAULT_BASE_FUNDING_RATE_BPS,
    DEFAULT_OPTIMAL_UTILIZATION_BPS, DEFAULT_SLOPE1_BPS, MAX_LIQUIDATION_BOUNTY_BPS,
    MAX_OPEN_FEE_BPS, MAX_TP_SL_EXECUTION_FEE,
};

use crate::{BorrowRateConfig, ConfigManagerError};

use super::helpers::{deploy_initialized, valid_fee_config, valid_limits};

fn valid_rate_config() -> BorrowRateConfig {
    BorrowRateConfig {
        base_borrow_rate_bps: DEFAULT_BASE_BORROW_RATE_BPS,
        slope1_bps: DEFAULT_SLOPE1_BPS,
        slope2_bps: 5_000,
        optimal_utilization_bps: DEFAULT_OPTIMAL_UTILIZATION_BPS,
        base_funding_rate_bps: DEFAULT_BASE_FUNDING_RATE_BPS,
    }
}

// ---------------------------------------------------------------------------
// ProtocolLimits — InvalidAdlUtilization (34)
// ---------------------------------------------------------------------------

#[test]
fn test_update_protocol_limits_adl_utilization_zero_errors_invalid_adl_utilization() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let mut limits = valid_limits();
    limits.adl_utilization_bps = 0;

    let result = client.try_update_protocol_limits(&admin, &limits);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(ConfigManagerError::InvalidAdlUtilization as u32),
        "adl_utilization_bps = 0 must fire InvalidAdlUtilization (34)"
    );
}

#[test]
fn test_update_protocol_limits_adl_utilization_above_bps_errors_invalid_adl_utilization() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let mut limits = valid_limits();
    limits.adl_utilization_bps = (BPS as u32) + 1;

    let result = client.try_update_protocol_limits(&admin, &limits);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(ConfigManagerError::InvalidAdlUtilization as u32),
        "adl_utilization_bps > BPS must fire InvalidAdlUtilization (34)"
    );
}

// ---------------------------------------------------------------------------
// ProtocolLimits — InvalidMinPositionLifetime (37)
// ---------------------------------------------------------------------------

#[test]
fn test_update_protocol_limits_min_position_lifetime_above_one_day_errors() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let mut limits = valid_limits();
    limits.min_position_lifetime = 86_401; // one second past 1 day

    let result = client.try_update_protocol_limits(&admin, &limits);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(
            ConfigManagerError::InvalidMinPositionLifetime as u32
        ),
        "min_position_lifetime > 86_400 must fire InvalidMinPositionLifetime (37)"
    );
}

// ---------------------------------------------------------------------------
// BorrowRateConfig — InvalidBorrowRateNegative (40)
// ---------------------------------------------------------------------------

#[test]
fn test_update_borrow_rate_negative_base_rate_errors_invalid_borrow_rate_negative() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let mut cfg = valid_rate_config();
    cfg.base_borrow_rate_bps = -1;

    let result = client.try_update_borrow_rate_config(&admin, &cfg);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(
            ConfigManagerError::InvalidBorrowRateNegative as u32
        ),
        "negative base_borrow_rate_bps must fire InvalidBorrowRateNegative (40)"
    );
}

#[test]
fn test_update_borrow_rate_negative_funding_rate_errors_invalid_borrow_rate_negative() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let mut cfg = valid_rate_config();
    cfg.base_funding_rate_bps = -1;

    let result = client.try_update_borrow_rate_config(&admin, &cfg);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(
            ConfigManagerError::InvalidBorrowRateNegative as u32
        ),
        "negative base_funding_rate_bps must fire InvalidBorrowRateNegative (40)"
    );
}

// ---------------------------------------------------------------------------
// BorrowRateConfig — InvalidOptimalUtilization (41)
// ---------------------------------------------------------------------------

#[test]
fn test_update_borrow_rate_optimal_utilization_zero_errors_invalid_optimal_utilization() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let mut cfg = valid_rate_config();
    cfg.optimal_utilization_bps = 0;

    let result = client.try_update_borrow_rate_config(&admin, &cfg);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(
            ConfigManagerError::InvalidOptimalUtilization as u32
        ),
        "optimal_utilization_bps = 0 must fire InvalidOptimalUtilization (41)"
    );
}

#[test]
fn test_update_borrow_rate_optimal_utilization_above_bps_errors_invalid_optimal_utilization() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let mut cfg = valid_rate_config();
    cfg.optimal_utilization_bps = BPS + 1;

    let result = client.try_update_borrow_rate_config(&admin, &cfg);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(
            ConfigManagerError::InvalidOptimalUtilization as u32
        ),
        "optimal_utilization_bps > BPS must fire InvalidOptimalUtilization (41)"
    );
}

// ---------------------------------------------------------------------------
// BorrowRateConfig — InvalidSlopeOrdering (42)
// ---------------------------------------------------------------------------

#[test]
fn test_update_borrow_rate_slope2_below_slope1_errors_invalid_slope_ordering() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let mut cfg = valid_rate_config();
    cfg.slope1_bps = 1_000;
    cfg.slope2_bps = 999;

    let result = client.try_update_borrow_rate_config(&admin, &cfg);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(ConfigManagerError::InvalidSlopeOrdering as u32),
        "slope2 < slope1 must fire InvalidSlopeOrdering (42)"
    );
}

// ---------------------------------------------------------------------------
// FeeConfig — InvalidOpenFee (44)
// ---------------------------------------------------------------------------

#[test]
fn test_set_fee_config_open_fee_above_ceiling_errors_invalid_open_fee() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let mut cfg = valid_fee_config();
    cfg.open_fee_bps = MAX_OPEN_FEE_BPS + 1;

    let result = client.try_set_fee_config(&admin, &cfg);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(ConfigManagerError::InvalidOpenFee as u32),
        "open_fee_bps > MAX_OPEN_FEE_BPS must fire InvalidOpenFee (44)",
    );
}

// ---------------------------------------------------------------------------
// FeeConfig — InvalidLiquidationBounty (45)
// ---------------------------------------------------------------------------

#[test]
fn test_set_fee_config_liquidation_bounty_above_ceiling_errors_invalid_liquidation_bounty() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let mut cfg = valid_fee_config();
    cfg.liquidation_bounty_bps = MAX_LIQUIDATION_BOUNTY_BPS + 1;

    let result = client.try_set_fee_config(&admin, &cfg);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(
            ConfigManagerError::InvalidLiquidationBounty as u32,
        ),
        "liquidation_bounty_bps > MAX must fire InvalidLiquidationBounty (45)",
    );
}

// ---------------------------------------------------------------------------
// FeeConfig — InvalidTpSlExecutionFee (46)
// ---------------------------------------------------------------------------

#[test]
fn test_set_fee_config_tp_sl_fee_negative_errors_invalid_tp_sl_execution_fee() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let mut cfg = valid_fee_config();
    cfg.tp_sl_execution_fee = -1;

    let result = client.try_set_fee_config(&admin, &cfg);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(
            ConfigManagerError::InvalidTpSlExecutionFee as u32,
        ),
        "negative tp_sl_execution_fee must fire InvalidTpSlExecutionFee (46)",
    );
}

#[test]
fn test_set_fee_config_tp_sl_fee_above_ceiling_errors_invalid_tp_sl_execution_fee() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let mut cfg = valid_fee_config();
    cfg.tp_sl_execution_fee = MAX_TP_SL_EXECUTION_FEE + 1;

    let result = client.try_set_fee_config(&admin, &cfg);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(
            ConfigManagerError::InvalidTpSlExecutionFee as u32,
        ),
        "tp_sl_execution_fee > MAX_TP_SL_EXECUTION_FEE must fire InvalidTpSlExecutionFee (46)",
    );
}

// ---------------------------------------------------------------------------
// Compile-time pins — discriminants
// ---------------------------------------------------------------------------

/// Compile-time pin: per-rule error code discriminants must not silently
/// shift. If any of these asserts fail to compile, the enum changed.
#[test]
fn test_per_rule_error_code_discriminants_are_stable() {
    const _: () = assert!(ConfigManagerError::InvalidAdlUtilization as u32 == 34);
    const _: () = assert!(ConfigManagerError::InvalidMinPositionLifetime as u32 == 37);
    const _: () = assert!(ConfigManagerError::InvalidBorrowRateNegative as u32 == 40);
    const _: () = assert!(ConfigManagerError::InvalidOptimalUtilization as u32 == 41);
    const _: () = assert!(ConfigManagerError::InvalidSlopeOrdering as u32 == 42);
    const _: () = assert!(ConfigManagerError::InvalidOpenFee as u32 == 44);
    const _: () = assert!(ConfigManagerError::InvalidLiquidationBounty as u32 == 45);
    const _: () = assert!(ConfigManagerError::InvalidTpSlExecutionFee as u32 == 46);
}
