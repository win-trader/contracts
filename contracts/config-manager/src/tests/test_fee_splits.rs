//! Tests for the FeeSplits revenue-split surface.
//!
//! `update_fee_splits` validates only one rule: lp_bps + dev_bps + staker_bps
//! must equal exactly 10_000. The sum is computed in u64 so adversarial u32
//! inputs cannot wrap. staker_bps = 0 is a valid configuration (stakers may
//! not be onboarded yet).

use soroban_sdk::{
    symbol_short, testutils::{Address as _, Events as _}, Address, Env, Symbol, TryIntoVal,
};

use crate::{ConfigManagerError, FeeSplits};

use super::helpers::{deploy_initialized, valid_splits};

// ---------------------------------------------------------------------------
// Happy-path tests — valid shapes round-trip through storage.
// ---------------------------------------------------------------------------

/// Canonical default split (9000/1000/0) is accepted and stored.
#[test]
fn test_update_fee_splits_default_shape_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let splits = FeeSplits { lp_bps: 9_000, dev_bps: 1_000, staker_bps: 0 };
    client.update_fee_splits(&admin, &splits);

    let stored = client.get_fee_splits();
    assert_eq!(stored.lp_bps, 9_000, "stored lp_bps must round-trip");
    assert_eq!(stored.dev_bps, 1_000, "stored dev_bps must round-trip");
    assert_eq!(stored.staker_bps, 0, "stored staker_bps = 0 must round-trip");
}

/// A three-way split where stakers are funded (5000/3000/2000) is accepted.
#[test]
fn test_update_fee_splits_three_way_shape_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let splits = FeeSplits { lp_bps: 5_000, dev_bps: 3_000, staker_bps: 2_000 };
    client.update_fee_splits(&admin, &splits);

    let stored = client.get_fee_splits();
    assert_eq!(stored.lp_bps, 5_000);
    assert_eq!(stored.dev_bps, 3_000);
    assert_eq!(stored.staker_bps, 2_000);
}

/// 100% to LP (10_000/0/0) is accepted — sum is the only invariant.
#[test]
fn test_update_fee_splits_all_to_lp_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let splits = FeeSplits { lp_bps: 10_000, dev_bps: 0, staker_bps: 0 };
    client.update_fee_splits(&admin, &splits);

    let stored = client.get_fee_splits();
    assert_eq!(stored.lp_bps, 10_000);
    assert_eq!(stored.dev_bps, 0);
    assert_eq!(stored.staker_bps, 0);
}

/// `valid_splits()` helper returns a split that round-trips through storage.
#[test]
fn test_update_fee_splits_valid_helper_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let splits = valid_splits();
    client.update_fee_splits(&admin, &splits);

    let stored = client.get_fee_splits();
    assert_eq!(stored.lp_bps, splits.lp_bps);
    assert_eq!(stored.dev_bps, splits.dev_bps);
    assert_eq!(stored.staker_bps, splits.staker_bps);
}

// ---------------------------------------------------------------------------
// Sum validation — the single invariant is `sum == 10_000`.
// ---------------------------------------------------------------------------

/// Sum < 10_000 must be rejected with InvalidFeeSplitSum (22).
#[test]
fn test_update_fee_splits_sum_below_target_errors() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let splits = FeeSplits { lp_bps: 5_000, dev_bps: 3_000, staker_bps: 1_999 };

    let result = client.try_update_fee_splits(&admin, &splits);
    assert!(result.is_err(), "sum != 10_000 must return an error");
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(ConfigManagerError::InvalidFeeSplitSum as u32),
        "error code must be InvalidFeeSplitSum (22)",
    );
}

/// Sum > 10_000 must be rejected with InvalidFeeSplitSum (22).
#[test]
fn test_update_fee_splits_sum_above_target_errors() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let splits = FeeSplits { lp_bps: 5_000, dev_bps: 3_000, staker_bps: 2_001 };

    let result = client.try_update_fee_splits(&admin, &splits);
    assert!(result.is_err(), "sum > 10_000 must return an error");
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(ConfigManagerError::InvalidFeeSplitSum as u32),
        "error code must be InvalidFeeSplitSum (22)",
    );
}

/// All-zero split has sum == 0 — must error with InvalidFeeSplitSum.
#[test]
fn test_update_fee_splits_all_zero_errors_sum() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let splits = FeeSplits { lp_bps: 0, dev_bps: 0, staker_bps: 0 };

    let result = client.try_update_fee_splits(&admin, &splits);
    assert!(result.is_err(), "all-zero FeeSplits must return an error");
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(ConfigManagerError::InvalidFeeSplitSum as u32),
        "all-zero must fire InvalidFeeSplitSum (22), NOT InvalidFeeSplitZero",
    );
}

/// Boundary: each component = 1 (sum = 3) — wrong sum must error.
#[test]
fn test_update_fee_splits_each_component_one_errors_sum() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let splits = FeeSplits { lp_bps: 1, dev_bps: 1, staker_bps: 1 };

    let result = client.try_update_fee_splits(&admin, &splits);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(ConfigManagerError::InvalidFeeSplitSum as u32),
    );
}

// ---------------------------------------------------------------------------
// u64-promotion overflow defence — adversarial u32 inputs must NOT trap.
// ---------------------------------------------------------------------------

/// Adversarial (u32::MAX, 1, 1) — u32 sum would wrap but the validator must
/// promote to u64 before summing, surfacing InvalidFeeSplitSum (22) cleanly.
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
    assert!(
        result.is_err(),
        "u32::MAX components must not panic the host — must surface InvalidFeeSplitSum",
    );
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(ConfigManagerError::InvalidFeeSplitSum as u32),
        "u64-promoted sum check must reject with InvalidFeeSplitSum (22)",
    );
}

/// Adversarial (u32::MAX, u32::MAX, u32::MAX) — sum vastly exceeds 10_000 and
/// must reject without trapping.
#[test]
fn test_update_fee_splits_three_u32_max_components_errors_sum() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let splits = FeeSplits {
        lp_bps: u32::MAX,
        dev_bps: u32::MAX,
        staker_bps: u32::MAX,
    };

    let result = client.try_update_fee_splits(&admin, &splits);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(ConfigManagerError::InvalidFeeSplitSum as u32),
    );
}

/// Each component just under u32::MAX with sum still exceeding 10_000 must
/// reject with InvalidFeeSplitSum (NOT panic / overflow).
#[test]
fn test_update_fee_splits_component_just_above_bps_errors_sum() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let splits = FeeSplits {
        lp_bps: 10_001,
        dev_bps: 0,
        staker_bps: 0,
    };

    let result = client.try_update_fee_splits(&admin, &splits);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(ConfigManagerError::InvalidFeeSplitSum as u32),
        "single component > 10_000 must reject via the sum rule, not a per-component check",
    );
}

// ---------------------------------------------------------------------------
// Auth — only admin can update.
// ---------------------------------------------------------------------------

/// Non-admin calling `update_fee_splits` must error with Unauthorized (3).
#[test]
fn test_update_fee_splits_non_admin_caller_errors_unauthorized() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = deploy_initialized(&env);
    let attacker = Address::generate(&env);

    let result = client.try_update_fee_splits(&attacker, &valid_splits());
    assert!(result.is_err(), "non-admin must not update fee splits");
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(ConfigManagerError::Unauthorized as u32),
        "error code must be Unauthorized (3)",
    );
}

/// Without any auth mocked, `update_fee_splits` must panic on require_auth.
#[test]
#[should_panic]
fn test_update_fee_splits_requires_admin_auth() {
    let env = Env::default();
    let (client, admin) = {
        env.mock_all_auths();
        let a = Address::generate(&env);
        let c = super::helpers::deploy(&env, &a);
        (c, a)
    };

    // Reset auth — no mocked auths means require_auth panics.
    env.set_auths(&[]);

    client.update_fee_splits(&admin, &valid_splits());
}

// ---------------------------------------------------------------------------
// Event emission — FeeSplitsUpdate carries staker_bps, not keeper_bps.
// ---------------------------------------------------------------------------

/// `update_fee_splits` must emit a `feecfg` event whose data unpacks as
/// (lp_bps, dev_bps, staker_bps) in declaration order.
#[test]
fn test_update_fee_splits_emits_event_with_staker_bps() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let splits = FeeSplits { lp_bps: 6_000, dev_bps: 3_000, staker_bps: 1_000 };
    client.update_fee_splits(&admin, &splits);

    let feecfg_topic: Symbol = symbol_short!("feecfg");
    let all = env.events().all();
    let mut found_payload: Option<(u32, u32, u32)> = None;
    // Walk events in reverse to find the most-recent feecfg event (the one
    // emitted by update_fee_splits, not the seeded-defaults one from initialize).
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
            if s == feecfg_topic {
                let parsed: Result<(u32, u32, u32), _> = data.try_into_val(&env);
                found_payload = Some(parsed.expect(
                    "feecfg payload must unpack as (lp_bps, dev_bps, staker_bps)",
                ));
                break;
            }
        }
    }

    let (lp, dev, staker) = found_payload
        .expect("update_fee_splits must emit a `feecfg` event on this contract");
    assert_eq!(lp, 6_000, "first field must be lp_bps (declaration order)");
    assert_eq!(dev, 3_000, "second field must be dev_bps");
    assert_eq!(
        staker, 1_000,
        "third field must be staker_bps — NOT keeper_bps",
    );
}

