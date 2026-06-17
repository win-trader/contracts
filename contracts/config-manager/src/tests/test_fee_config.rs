//! Tests for the FeeConfig surface — execution-bounty + open-fee parameters.
//!
//! FeeConfig is distinct from FeeSplits (revenue split between LP / dev /
//! stakers). It holds:
//!   - open_fee_bps: applied to notional on position open
//!   - liquidation_bounty_bps: applied to collateral, paid to the liquidator
//!   - tp_sl_execution_fee: flat USDC fee on TP/SL execution
//!
//! Each parameter has a hard ceiling enforced in validation. Three new
//! error codes back the rules: InvalidOpenFee (44), InvalidLiquidationBounty
//! (45), InvalidTpSlExecutionFee (46).

use shared::constants::{
    DEFAULT_LIQUIDATION_BOUNTY_BPS, DEFAULT_OPEN_FEE_BPS, DEFAULT_TP_SL_EXECUTION_FEE,
    MAX_LIQUIDATION_BOUNTY_BPS, MAX_OPEN_FEE_BPS, MAX_TP_SL_EXECUTION_FEE,
};
use shared::FeeConfig;
use soroban_sdk::{
    symbol_short, testutils::{Address as _, Events as _}, Address, Env, Symbol, TryIntoVal,
};

use crate::ConfigManagerError;

use super::helpers::{deploy_initialized, valid_fee_config};

// ---------------------------------------------------------------------------
// Happy-path: set_fee_config writes; get_fee_config reads.
// ---------------------------------------------------------------------------

/// Setting a valid FeeConfig must round-trip through storage exactly.
#[test]
fn test_set_fee_config_happy_path_round_trips() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let cfg = FeeConfig {
        open_fee_bps: 25,
        liquidation_bounty_bps: 200,
        tp_sl_execution_fee: 7_500_000,
    };
    client.set_fee_config(&admin, &cfg);

    let stored = client.get_fee_config();
    assert_eq!(stored.open_fee_bps, 25, "open_fee_bps must round-trip");
    assert_eq!(stored.liquidation_bounty_bps, 200, "liquidation_bounty_bps must round-trip");
    assert_eq!(stored.tp_sl_execution_fee, 7_500_000, "tp_sl_execution_fee must round-trip");
}

/// `get_fee_config` immediately after `initialize` returns the seeded defaults.
#[test]
fn test_get_fee_config_returns_defaults_after_initialize() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = deploy_initialized(&env);

    let stored = client.get_fee_config();
    assert_eq!(
        stored.open_fee_bps, DEFAULT_OPEN_FEE_BPS,
        "default open_fee_bps must equal DEFAULT_OPEN_FEE_BPS",
    );
    assert_eq!(
        stored.liquidation_bounty_bps, DEFAULT_LIQUIDATION_BOUNTY_BPS,
        "default liquidation_bounty_bps must equal DEFAULT_LIQUIDATION_BOUNTY_BPS",
    );
    assert_eq!(
        stored.tp_sl_execution_fee, DEFAULT_TP_SL_EXECUTION_FEE,
        "default tp_sl_execution_fee must equal DEFAULT_TP_SL_EXECUTION_FEE",
    );
}

/// Calling set_fee_config a second time must overwrite the previous value.
#[test]
fn test_set_fee_config_overwrites_previous_value() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let first = FeeConfig { open_fee_bps: 10, liquidation_bounty_bps: 50, tp_sl_execution_fee: 1_000_000 };
    client.set_fee_config(&admin, &first);

    let second = FeeConfig { open_fee_bps: 50, liquidation_bounty_bps: 500, tp_sl_execution_fee: 8_000_000 };
    client.set_fee_config(&admin, &second);

    let stored = client.get_fee_config();
    assert_eq!(stored.open_fee_bps, 50);
    assert_eq!(stored.liquidation_bounty_bps, 500);
    assert_eq!(stored.tp_sl_execution_fee, 8_000_000);
}

// ---------------------------------------------------------------------------
// Auth — only admin may invoke set_fee_config.
// ---------------------------------------------------------------------------

/// Non-admin caller must be rejected with Unauthorized (3).
#[test]
fn test_set_fee_config_non_admin_caller_errors_unauthorized() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = deploy_initialized(&env);
    let attacker = Address::generate(&env);

    let result = client.try_set_fee_config(&attacker, &valid_fee_config());
    assert!(result.is_err(), "non-admin must not be able to set_fee_config");
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(ConfigManagerError::Unauthorized as u32),
        "non-admin set_fee_config must return Unauthorized (3)",
    );
}

/// Without any auths mocked, set_fee_config must panic on require_auth.
#[test]
#[should_panic]
fn test_set_fee_config_requires_admin_auth() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let client = super::helpers::deploy(&env, &admin);

    env.set_auths(&[]);

    client.set_fee_config(&admin, &valid_fee_config());
}

// ---------------------------------------------------------------------------
// open_fee_bps validation
// ---------------------------------------------------------------------------

/// open_fee_bps = MAX_OPEN_FEE_BPS is the inclusive ceiling — must succeed.
#[test]
fn test_set_fee_config_open_fee_at_ceiling_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let cfg = FeeConfig {
        open_fee_bps: MAX_OPEN_FEE_BPS,
        liquidation_bounty_bps: 100,
        tp_sl_execution_fee: 5_000_000,
    };
    client.set_fee_config(&admin, &cfg);

    assert_eq!(client.get_fee_config().open_fee_bps, MAX_OPEN_FEE_BPS);
}

/// open_fee_bps = 0 is allowed (admin may waive open fee).
#[test]
fn test_set_fee_config_open_fee_zero_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let cfg = FeeConfig {
        open_fee_bps: 0,
        liquidation_bounty_bps: 100,
        tp_sl_execution_fee: 5_000_000,
    };
    client.set_fee_config(&admin, &cfg);

    assert_eq!(client.get_fee_config().open_fee_bps, 0);
}

/// open_fee_bps > MAX_OPEN_FEE_BPS must error with InvalidOpenFee (44).
#[test]
fn test_set_fee_config_open_fee_above_ceiling_errors() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let cfg = FeeConfig {
        open_fee_bps: MAX_OPEN_FEE_BPS + 1,
        liquidation_bounty_bps: 100,
        tp_sl_execution_fee: 5_000_000,
    };

    let result = client.try_set_fee_config(&admin, &cfg);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(ConfigManagerError::InvalidOpenFee as u32),
        "open_fee_bps > MAX_OPEN_FEE_BPS must fire InvalidOpenFee (44)",
    );
}

/// Adversarial: open_fee_bps = u32::MAX must reject cleanly with InvalidOpenFee.
#[test]
fn test_set_fee_config_open_fee_u32_max_errors() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let cfg = FeeConfig {
        open_fee_bps: u32::MAX,
        liquidation_bounty_bps: 100,
        tp_sl_execution_fee: 5_000_000,
    };

    let result = client.try_set_fee_config(&admin, &cfg);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(ConfigManagerError::InvalidOpenFee as u32),
    );
}

// ---------------------------------------------------------------------------
// liquidation_bounty_bps validation
// ---------------------------------------------------------------------------

/// liquidation_bounty_bps = MAX_LIQUIDATION_BOUNTY_BPS is the inclusive ceiling.
#[test]
fn test_set_fee_config_liquidation_bounty_at_ceiling_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let cfg = FeeConfig {
        open_fee_bps: 10,
        liquidation_bounty_bps: MAX_LIQUIDATION_BOUNTY_BPS,
        tp_sl_execution_fee: 5_000_000,
    };
    client.set_fee_config(&admin, &cfg);

    assert_eq!(
        client.get_fee_config().liquidation_bounty_bps,
        MAX_LIQUIDATION_BOUNTY_BPS,
    );
}

/// liquidation_bounty_bps = 0 is allowed (free liquidation — admin's call).
#[test]
fn test_set_fee_config_liquidation_bounty_zero_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let cfg = FeeConfig {
        open_fee_bps: 10,
        liquidation_bounty_bps: 0,
        tp_sl_execution_fee: 5_000_000,
    };
    client.set_fee_config(&admin, &cfg);

    assert_eq!(client.get_fee_config().liquidation_bounty_bps, 0);
}

/// liquidation_bounty_bps > MAX must error with InvalidLiquidationBounty (45).
#[test]
fn test_set_fee_config_liquidation_bounty_above_ceiling_errors() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let cfg = FeeConfig {
        open_fee_bps: 10,
        liquidation_bounty_bps: MAX_LIQUIDATION_BOUNTY_BPS + 1,
        tp_sl_execution_fee: 5_000_000,
    };

    let result = client.try_set_fee_config(&admin, &cfg);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(
            ConfigManagerError::InvalidLiquidationBounty as u32,
        ),
        "liquidation_bounty_bps > ceiling must fire InvalidLiquidationBounty (45)",
    );
}

/// Adversarial: liquidation_bounty_bps = u32::MAX must reject cleanly.
#[test]
fn test_set_fee_config_liquidation_bounty_u32_max_errors() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let cfg = FeeConfig {
        open_fee_bps: 10,
        liquidation_bounty_bps: u32::MAX,
        tp_sl_execution_fee: 5_000_000,
    };

    let result = client.try_set_fee_config(&admin, &cfg);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(
            ConfigManagerError::InvalidLiquidationBounty as u32,
        ),
    );
}

// ---------------------------------------------------------------------------
// tp_sl_execution_fee validation
// ---------------------------------------------------------------------------

/// tp_sl_execution_fee = MAX is the inclusive ceiling.
#[test]
fn test_set_fee_config_tp_sl_fee_at_ceiling_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let cfg = FeeConfig {
        open_fee_bps: 10,
        liquidation_bounty_bps: 100,
        tp_sl_execution_fee: MAX_TP_SL_EXECUTION_FEE,
    };
    client.set_fee_config(&admin, &cfg);

    assert_eq!(client.get_fee_config().tp_sl_execution_fee, MAX_TP_SL_EXECUTION_FEE);
}

/// tp_sl_execution_fee = 0 is allowed (free TP/SL — admin's call).
#[test]
fn test_set_fee_config_tp_sl_fee_zero_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let cfg = FeeConfig {
        open_fee_bps: 10,
        liquidation_bounty_bps: 100,
        tp_sl_execution_fee: 0,
    };
    client.set_fee_config(&admin, &cfg);

    assert_eq!(client.get_fee_config().tp_sl_execution_fee, 0);
}

/// tp_sl_execution_fee = -1 must error with InvalidTpSlExecutionFee (46).
#[test]
fn test_set_fee_config_tp_sl_fee_negative_one_errors() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let cfg = FeeConfig {
        open_fee_bps: 10,
        liquidation_bounty_bps: 100,
        tp_sl_execution_fee: -1,
    };

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

/// tp_sl_execution_fee = i128::MIN (extreme negative) must error.
#[test]
fn test_set_fee_config_tp_sl_fee_i128_min_errors() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let cfg = FeeConfig {
        open_fee_bps: 10,
        liquidation_bounty_bps: 100,
        tp_sl_execution_fee: i128::MIN,
    };

    let result = client.try_set_fee_config(&admin, &cfg);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(
            ConfigManagerError::InvalidTpSlExecutionFee as u32,
        ),
    );
}

/// tp_sl_execution_fee > MAX must error with InvalidTpSlExecutionFee (46).
#[test]
fn test_set_fee_config_tp_sl_fee_above_ceiling_errors() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let cfg = FeeConfig {
        open_fee_bps: 10,
        liquidation_bounty_bps: 100,
        tp_sl_execution_fee: MAX_TP_SL_EXECUTION_FEE + 1,
    };

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

/// tp_sl_execution_fee = i128::MAX (extreme positive) must error.
#[test]
fn test_set_fee_config_tp_sl_fee_i128_max_errors() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let cfg = FeeConfig {
        open_fee_bps: 10,
        liquidation_bounty_bps: 100,
        tp_sl_execution_fee: i128::MAX,
    };

    let result = client.try_set_fee_config(&admin, &cfg);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(
            ConfigManagerError::InvalidTpSlExecutionFee as u32,
        ),
    );
}

// ---------------------------------------------------------------------------
// Validation ordering — first-violation reporting.
// ---------------------------------------------------------------------------

/// When two fields are simultaneously invalid the error code identifies
/// whichever check fires first — this test pins the contract behaviour by
/// asserting the multiple-violation panic is at least one of the per-rule
/// codes, never a generic Unauthorized / unrelated error.
#[test]
fn test_set_fee_config_multiple_invalid_fields_error_is_per_rule() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let cfg = FeeConfig {
        open_fee_bps: MAX_OPEN_FEE_BPS + 1,
        liquidation_bounty_bps: MAX_LIQUIDATION_BOUNTY_BPS + 1,
        tp_sl_execution_fee: -1,
    };

    let result = client.try_set_fee_config(&admin, &cfg);
    let err = result.unwrap_err().unwrap();
    let open = soroban_sdk::Error::from_contract_error(ConfigManagerError::InvalidOpenFee as u32);
    let bounty =
        soroban_sdk::Error::from_contract_error(ConfigManagerError::InvalidLiquidationBounty as u32);
    let tpsl = soroban_sdk::Error::from_contract_error(
        ConfigManagerError::InvalidTpSlExecutionFee as u32,
    );
    assert!(
        err == open || err == bounty || err == tpsl,
        "multi-violation FeeConfig must surface one of the per-rule error codes (44/45/46)",
    );
}

// ---------------------------------------------------------------------------
// State-isolation — a rejected set_fee_config must NOT mutate storage.
// ---------------------------------------------------------------------------

/// A failed set_fee_config must leave the previously stored FeeConfig intact.
#[test]
fn test_set_fee_config_rejection_leaves_prior_state_intact() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    // Write a known-good config first.
    let good = FeeConfig {
        open_fee_bps: 25,
        liquidation_bounty_bps: 200,
        tp_sl_execution_fee: 5_000_000,
    };
    client.set_fee_config(&admin, &good);

    // Try to write a bad config.
    let bad = FeeConfig {
        open_fee_bps: MAX_OPEN_FEE_BPS + 1,
        liquidation_bounty_bps: 200,
        tp_sl_execution_fee: 5_000_000,
    };
    let _ = client.try_set_fee_config(&admin, &bad);

    // The good config must still be readable.
    let stored = client.get_fee_config();
    assert_eq!(stored.open_fee_bps, 25, "open_fee_bps must remain from prior good write");
    assert_eq!(stored.liquidation_bounty_bps, 200);
    assert_eq!(stored.tp_sl_execution_fee, 5_000_000);
}

// ---------------------------------------------------------------------------
// Event emission — FeeConfigUpdate carries the three FeeConfig fields.
// ---------------------------------------------------------------------------

/// `set_fee_config` must emit a `feecnf` event whose data unpacks as
/// (open_fee_bps, liquidation_bounty_bps, tp_sl_execution_fee).
#[test]
fn test_set_fee_config_emits_event_with_new_values() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let cfg = FeeConfig {
        open_fee_bps: 42,
        liquidation_bounty_bps: 333,
        tp_sl_execution_fee: 9_999_999,
    };
    client.set_fee_config(&admin, &cfg);

    let feecnf_topic: Symbol = symbol_short!("feecnf");
    let all = env.events().all();
    let mut found_payload: Option<(u32, u32, i128)> = None;
    for entry in all.iter().rev() {
        let (contract, topics, data) = entry;
        if contract != client.address {
            continue;
        }
        if topics.len() == 0 {
            continue;
        }
        let first_topic_val = topics.get(0).unwrap();
        let first_topic: Result<Symbol, _> = first_topic_val.try_into_val(&env);
        if let Ok(s) = first_topic {
            if s == feecnf_topic {
                let parsed: Result<(u32, u32, i128), _> = data.try_into_val(&env);
                found_payload = Some(parsed.expect(
                    "feecnf payload must unpack as (open_fee_bps, liquidation_bounty_bps, tp_sl_execution_fee)",
                ));
                break;
            }
        }
    }

    let (open, bounty, tpsl) = found_payload
        .expect("set_fee_config must emit a `feecnf` event on this contract");
    assert_eq!(open, 42);
    assert_eq!(bounty, 333);
    assert_eq!(tpsl, 9_999_999);
}

/// Initialize must also emit a `feecnf` event carrying the seeded defaults,
/// so indexers populate `protocol_config` from ledger 0.
#[test]
fn test_initialize_emits_feecnf_event_with_defaults() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let client = super::helpers::deploy(&env, &admin);

    let feecnf_topic: Symbol = symbol_short!("feecnf");
    let all = env.events().all();
    let mut found_payload: Option<(u32, u32, i128)> = None;
    for entry in all.iter() {
        let (contract, topics, data) = entry;
        if contract != client.address {
            continue;
        }
        if topics.len() == 0 {
            continue;
        }
        let first_topic_val = topics.get(0).unwrap();
        let first_topic: Result<Symbol, _> = first_topic_val.try_into_val(&env);
        if let Ok(s) = first_topic {
            if s == feecnf_topic {
                let parsed: Result<(u32, u32, i128), _> = data.try_into_val(&env);
                found_payload = Some(
                    parsed.expect("feecnf payload must unpack from initialize"),
                );
                break;
            }
        }
    }

    let (open, bounty, tpsl) = found_payload.expect(
        "initialize must emit a `feecnf` event with seeded FeeConfig defaults",
    );
    assert_eq!(open, DEFAULT_OPEN_FEE_BPS);
    assert_eq!(bounty, DEFAULT_LIQUIDATION_BOUNTY_BPS);
    assert_eq!(tpsl, DEFAULT_TP_SL_EXECUTION_FEE);
}
