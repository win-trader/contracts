//! Tests for ProtocolLimits boundary validation.
//!
//! `update_protocol_limits` must reject:
//!   - min_collateral <= 0  (must be >= 1)
//!   - max_utilization_ratio <= 0 or > 10_000
//!
//! Invalid-input tests FAIL against the current implementation (no validation).
//! Valid-input tests PASS (regression anchors).

use soroban_sdk::Env;

use crate::{ConfigManagerError, ProtocolLimits};

use super::helpers::{deploy_initialized, valid_limits};

/// min_collateral = 0 must be rejected with InvalidLimits (5).
#[test]
fn test_update_protocol_limits_zero_min_collateral_errors() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let limits = ProtocolLimits {
        min_collateral: 0,
        cooldown_duration: 60,
        min_position_lifetime: 60,
        max_utilization_ratio: 8_500,
        funding_cut_bps: 500,
        adl_pnl_bps: 9_000,
        adl_utilization_bps: 9_500,
        liquidation_threshold_bps: 200,
    };

    let result = client.try_update_protocol_limits(&admin, &limits);
    assert!(result.is_err(), "min_collateral = 0 must return an error");
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(ConfigManagerError::InvalidMinCollateral as u32),
        "error code must be InvalidMinCollateral (30)"
    );
}

/// min_collateral = -1 must be rejected with InvalidLimits (5).
#[test]
fn test_update_protocol_limits_negative_min_collateral_errors() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let limits = ProtocolLimits {
        min_collateral: -1,
        cooldown_duration: 60,
        min_position_lifetime: 60,
        max_utilization_ratio: 8_500,
        funding_cut_bps: 500,
        adl_pnl_bps: 9_000,
        adl_utilization_bps: 9_500,
        liquidation_threshold_bps: 200,
    };

    let result = client.try_update_protocol_limits(&admin, &limits);
    assert!(result.is_err(), "min_collateral = -1 must return an error");
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(ConfigManagerError::InvalidMinCollateral as u32),
        "error code must be InvalidMinCollateral (30)"
    );
}

/// max_utilization_ratio = 0 must be rejected with InvalidLimits (5).
#[test]
fn test_update_protocol_limits_zero_max_utilization_errors() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let limits = ProtocolLimits {
        min_collateral: 100,
        cooldown_duration: 60,
        min_position_lifetime: 60,
        max_utilization_ratio: 0,
        funding_cut_bps: 500,
        adl_pnl_bps: 9_000,
        adl_utilization_bps: 9_500,
        liquidation_threshold_bps: 200,
    };

    let result = client.try_update_protocol_limits(&admin, &limits);
    assert!(result.is_err(), "max_utilization_ratio = 0 must return an error");
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(ConfigManagerError::InvalidMaxUtilization as u32),
        "error code must be InvalidMaxUtilization (31)"
    );
}

/// max_utilization_ratio = 10_001 (above 100%) must be rejected.
#[test]
fn test_update_protocol_limits_max_utilization_above_10000_errors() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let limits = ProtocolLimits {
        min_collateral: 100,
        cooldown_duration: 60,
        min_position_lifetime: 60,
        max_utilization_ratio: 10_001,
        funding_cut_bps: 500,
        adl_pnl_bps: 9_000,
        adl_utilization_bps: 9_500,
        liquidation_threshold_bps: 200,
    };

    let result = client.try_update_protocol_limits(&admin, &limits);
    assert!(result.is_err(), "max_utilization_ratio = 10_001 must return an error");
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(ConfigManagerError::InvalidMaxUtilization as u32),
        "error code must be InvalidMaxUtilization (31)"
    );
}

/// Valid limits must succeed and be persisted (regression anchor).
#[test]
fn test_update_protocol_limits_valid_values_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let limits = valid_limits();
    client.update_protocol_limits(&admin, &limits);

    let stored = client.get_protocol_limits();
    assert_eq!(stored.min_collateral, 100, "stored min_collateral must match input");
    assert_eq!(stored.max_utilization_ratio, 8_500, "stored max_utilization_ratio must match input");
    assert_eq!(stored.cooldown_duration, 60, "stored cooldown_duration must match input");
    assert_eq!(stored.min_position_lifetime, 60, "stored min_position_lifetime must match input");
}

/// Boundary: max_utilization_ratio = 10_000 (exactly 100%) must succeed.
#[test]
fn test_update_protocol_limits_max_utilization_exactly_10000_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let limits = ProtocolLimits {
        min_collateral: 1,
        cooldown_duration: 0,
        min_position_lifetime: 0,
        max_utilization_ratio: 10_000,
        funding_cut_bps: 500,
        adl_pnl_bps: 9_000,
        adl_utilization_bps: 9_500,
        liquidation_threshold_bps: 200,
    };

    let result = client.try_update_protocol_limits(&admin, &limits);
    assert!(
        result.is_ok(),
        "Boundary: max_utilization_ratio = 10_000 must succeed (boundary is inclusive)"
    );
}

/// Boundary: min_collateral = 1 is the minimum legal positive value.
#[test]
fn test_update_protocol_limits_min_collateral_of_one_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let limits = ProtocolLimits {
        min_collateral: 1,
        cooldown_duration: 0,
        min_position_lifetime: 0,
        max_utilization_ratio: 8_500,
        funding_cut_bps: 500,
        adl_pnl_bps: 9_000,
        adl_utilization_bps: 9_500,
        liquidation_threshold_bps: 200,
    };

    let result = client.try_update_protocol_limits(&admin, &limits);
    assert!(
        result.is_ok(),
        "Boundary: min_collateral = 1 must succeed (minimum positive value)"
    );
}

/// Adversarial: i128::MIN min_collateral must be rejected — no implicit wrap-around.
#[test]
fn test_update_protocol_limits_i128_min_collateral_errors() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let limits = ProtocolLimits {
        min_collateral: i128::MIN,
        cooldown_duration: 60,
        min_position_lifetime: 60,
        max_utilization_ratio: 8_500,
        funding_cut_bps: 500,
        adl_pnl_bps: 9_000,
        adl_utilization_bps: 9_500,
        liquidation_threshold_bps: 200,
    };

    let result = client.try_update_protocol_limits(&admin, &limits);
    assert!(result.is_err(), "Adversarial: i128::MIN min_collateral must return an error");
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(ConfigManagerError::InvalidMinCollateral as u32),
        "Adversarial: error code must be InvalidMinCollateral (30)"
    );
}

/// funding_cut_bps = 10_000 (100%) must be rejected.
#[test]
fn test_update_protocol_limits_funding_cut_at_10000_errors() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let limits = ProtocolLimits {
        min_collateral: 100,
        cooldown_duration: 60,
        min_position_lifetime: 60,
        max_utilization_ratio: 8_500,
        funding_cut_bps: 10_000,
        adl_pnl_bps: 9_000,
        adl_utilization_bps: 9_500,
        liquidation_threshold_bps: 200,
    };

    let result = client.try_update_protocol_limits(&admin, &limits);
    assert!(result.is_err(), "funding_cut_bps = 10_000 must return an error");
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(ConfigManagerError::InvalidFundingCut as u32),
        "error code must be InvalidFundingCut (32)"
    );
}

/// funding_cut_bps = 0 is valid (no protocol cut).
#[test]
fn test_update_protocol_limits_funding_cut_zero_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let limits = ProtocolLimits {
        min_collateral: 100,
        cooldown_duration: 60,
        min_position_lifetime: 60,
        max_utilization_ratio: 8_500,
        funding_cut_bps: 0,
        adl_pnl_bps: 9_000,
        adl_utilization_bps: 9_500,
        liquidation_threshold_bps: 200,
    };

    let result = client.try_update_protocol_limits(&admin, &limits);
    assert!(result.is_ok(), "funding_cut_bps = 0 must succeed");
}

/// valid limits round-trip includes funding_cut_bps.
#[test]
fn test_update_protocol_limits_stores_funding_cut_bps() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let limits = valid_limits();
    client.update_protocol_limits(&admin, &limits);

    let stored = client.get_protocol_limits();
    assert_eq!(stored.funding_cut_bps, 500, "stored funding_cut_bps must match input");
}

/// Adversarial: negative max_utilization_ratio must be rejected.
#[test]
fn test_update_protocol_limits_negative_max_utilization_errors() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let limits = ProtocolLimits {
        min_collateral: 100,
        cooldown_duration: 60,
        min_position_lifetime: 60,
        max_utilization_ratio: -1,
        funding_cut_bps: 500,
        adl_pnl_bps: 9_000,
        adl_utilization_bps: 9_500,
        liquidation_threshold_bps: 200,
    };

    let result = client.try_update_protocol_limits(&admin, &limits);
    assert!(result.is_err(), "Adversarial: max_utilization_ratio = -1 must return an error");
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(ConfigManagerError::InvalidMaxUtilization as u32),
        "Adversarial: error code must be InvalidMaxUtilization (31)"
    );
}
