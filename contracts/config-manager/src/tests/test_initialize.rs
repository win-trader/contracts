//! Tests for the Soroban constructor (`__constructor`).
//!
//! The constructor runs atomically during `env.register`, so there is no
//! separate `initialize` entrypoint and no uninitialized window. These tests
//! assert the post-construction STATE: the provided admin holds the ADMIN
//! role, the seeded defaults (FeeSplits / ProtocolLimits / FeeConfig /
//! CarryingFeeConfig) are readable via their getters, and the seed events are
//! emitted for off-chain indexers.

use soroban_sdk::{testutils::Address as _, Address, Env};

use super::helpers::{deploy, role_admin};

// ---------------------------------------------------------------------------
// Post-construction admin grant
// ---------------------------------------------------------------------------

/// Registering with an admin grants that admin the DEFAULT_ADMIN_ROLE.
#[test]
fn test_constructor_grants_admin_role() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let client = deploy(&env, &admin);

    assert!(
        client.has_role(&role_admin(&env), &admin),
        "admin must hold DEFAULT_ADMIN_ROLE immediately after construction"
    );
}

/// The admin address provided at construction is stored; a different address
/// must NOT hold DEFAULT_ADMIN_ROLE.
#[test]
fn test_constructor_stores_provided_admin_not_a_different_address() {
    let env = Env::default();
    let real_admin = Address::generate(&env);
    let impostor = Address::generate(&env);

    let client = deploy(&env, &real_admin);

    assert!(
        !client.has_role(&role_admin(&env), &impostor),
        "an address that was not the constructor admin must not hold DEFAULT_ADMIN_ROLE"
    );
}

/// The role-member persistent entry written by the constructor must be
/// readable immediately (TTL was extended at write time).
#[test]
fn test_constructor_role_member_entry_readable() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let client = deploy(&env, &admin);

    let result = client.try_has_role(&role_admin(&env), &admin);
    assert!(
        result.is_ok(),
        "has_role must not error — persistent entry must be live after construction"
    );
    assert!(
        result.unwrap().unwrap(),
        "ADMIN role entry must be true after construction with TTL correctly extended"
    );
}

// ---------------------------------------------------------------------------
// Construction emits seeded-default events so off-chain indexers populate
// `protocol_config` from ledger 0. Without these, the keeper's env-var
// fallback would mask a partially-empty config row.
// ---------------------------------------------------------------------------

#[test]
fn test_constructor_emits_seeded_default_events() {
    use soroban_sdk::{testutils::Events as _, Symbol, TryIntoVal, Val};

    let env = Env::default();
    let admin = Address::generate(&env);
    let client = deploy(&env, &admin);

    let cm_id = client.address.clone();
    let mut saw_feecfg = false;
    let mut saw_limits = false;
    let mut saw_rates = false;

    for (contract, topics, data) in env.events().all() {
        if contract != cm_id {
            continue;
        }
        if topics.len() == 0 {
            continue;
        }
        let topic0: Symbol = match topics.get(0).unwrap().try_into_val(&env) {
            Ok(s) => s,
            Err(_) => continue,
        };
        if topic0 == Symbol::new(&env, "feecfg") {
            let parsed: Result<(u32, u32, u32), _> = data.try_into_val(&env);
            let (lp, d, staker) = parsed.expect("feecfg event must unpack as (u32, u32, u32)");
            assert_eq!(
                lp,
                shared::constants::DEFAULT_LP_BPS,
                "first field of feecfg must be lp_bps (declaration order)",
            );
            assert_eq!(d, shared::constants::DEFAULT_DEV_BPS);
            assert_eq!(
                staker,
                shared::constants::DEFAULT_STAKER_BPS,
                "third field of feecfg must be staker_bps (new shape)",
            );
            saw_feecfg = true;
        } else if topic0 == Symbol::new(&env, "limits") {
            let parsed: Result<(i128, u64, u64, i128, u32, u32, u32), _> = data.try_into_val(&env);
            let tup = parsed
                .expect("limits event must unpack as 7-tuple including liquidation_threshold_bps");
            assert_eq!(tup.0, shared::constants::DEFAULT_MIN_COLLATERAL);
            assert_eq!(tup.1, shared::constants::DEFAULT_COOLDOWN_DURATION);
            assert_eq!(tup.6, shared::constants::DEFAULT_LIQUIDATION_THRESHOLD_BPS);
            saw_limits = true;
        } else if topic0 == Symbol::new(&env, "rates") {
            let parsed: Result<(i128, i128, i128, i128, i128), _> = data.try_into_val(&env);
            let tup = parsed.expect("rates event must unpack as 5-tuple");
            assert_eq!(tup.0, shared::constants::DEFAULT_BASE_BORROW_RATE_BPS);
            saw_rates = true;
        }
        let _: Val = topics.get(0).unwrap();
    }

    assert!(
        saw_feecfg,
        "constructor must emit a `feecfg` event with seeded defaults"
    );
    assert!(
        saw_limits,
        "constructor must emit a `limits` event with seeded defaults"
    );
    assert!(
        saw_rates,
        "constructor must emit a `rates` event with seeded defaults"
    );
}

// ---------------------------------------------------------------------------
// FeeConfig seeded defaults must be readable via get_fee_config immediately
// after construction so PositionManager never reads an empty / panic state.
// ---------------------------------------------------------------------------

#[test]
fn test_constructor_seeds_fee_config_defaults_readable_via_getter() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let client = deploy(&env, &admin);

    let cfg = client.get_fee_config();
    assert_eq!(
        cfg.open_fee_bps,
        shared::constants::DEFAULT_OPEN_FEE_BPS,
        "constructor must seed open_fee_bps with DEFAULT_OPEN_FEE_BPS",
    );
    assert_eq!(
        cfg.liquidation_bounty_bps,
        shared::constants::DEFAULT_LIQUIDATION_BOUNTY_BPS,
        "constructor must seed liquidation_bounty_bps with DEFAULT_LIQUIDATION_BOUNTY_BPS",
    );
    assert_eq!(
        cfg.tp_sl_execution_fee,
        shared::constants::DEFAULT_TP_SL_EXECUTION_FEE,
        "constructor must seed tp_sl_execution_fee with DEFAULT_TP_SL_EXECUTION_FEE",
    );
}

#[test]
fn test_constructor_seeds_fee_splits_defaults_with_new_shape() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let client = deploy(&env, &admin);

    let splits = client.get_fee_splits();
    assert_eq!(
        splits.lp_bps,
        shared::constants::DEFAULT_LP_BPS,
        "constructor must seed lp_bps with DEFAULT_LP_BPS",
    );
    assert_eq!(
        splits.dev_bps,
        shared::constants::DEFAULT_DEV_BPS,
        "constructor must seed dev_bps with DEFAULT_DEV_BPS",
    );
    assert_eq!(
        splits.staker_bps,
        shared::constants::DEFAULT_STAKER_BPS,
        "constructor must seed staker_bps with DEFAULT_STAKER_BPS",
    );
}

#[test]
fn test_constructor_seeds_protocol_limits_readable_via_getter() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let client = deploy(&env, &admin);

    let limits = client.get_protocol_limits();
    assert_eq!(
        limits.min_collateral,
        shared::constants::DEFAULT_MIN_COLLATERAL,
        "constructor must seed min_collateral with DEFAULT_MIN_COLLATERAL",
    );
    assert_eq!(
        limits.liquidation_threshold_bps,
        shared::constants::DEFAULT_LIQUIDATION_THRESHOLD_BPS,
        "constructor must seed liquidation_threshold_bps with DEFAULT_LIQUIDATION_THRESHOLD_BPS",
    );
}

#[test]
fn test_constructor_seeds_carrying_fee_config_readable_via_getter() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let client = deploy(&env, &admin);

    let rates = client.get_carrying_fee_config();
    assert_eq!(
        rates.base_borrow_rate_bps,
        shared::constants::DEFAULT_BASE_BORROW_RATE_BPS,
        "constructor must seed base_borrow_rate_bps with DEFAULT_BASE_BORROW_RATE_BPS",
    );
}
