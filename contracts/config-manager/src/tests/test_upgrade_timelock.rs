//! Tests for `upgrade_timelock_seconds` — the configurable timelock that
//! gates `commit_upgrade` on every UPGRADER-controlled contract.
//!
//! The value is stored in ConfigManager so a single admin action retunes the
//! delay protocol-wide. The setter validates a compile-time floor in
//! `shared::constants::MIN_UPGRADE_TIMELOCK` so a compromised admin cannot
//! shorten the timelock to zero and immediately commit a malicious upgrade.

use soroban_sdk::{
    testutils::{Address as _, Events as _},
    Address, Env, Symbol, TryIntoVal,
};

use crate::ConfigManagerError;
use shared::constants::{DEFAULT_UPGRADE_TIMELOCK, MIN_UPGRADE_TIMELOCK};

use super::helpers::deploy_initialized;

/// initialize() seeds the default value.
#[test]
fn test_upgrade_timelock_defaults_after_initialize() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = deploy_initialized(&env);

    assert_eq!(client.get_upgrade_timelock(), DEFAULT_UPGRADE_TIMELOCK);
}

/// Admin can set the timelock to any value at or above the floor.
#[test]
fn test_set_upgrade_timelock_at_floor_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    client.set_upgrade_timelock(&admin, &MIN_UPGRADE_TIMELOCK);
    assert_eq!(client.get_upgrade_timelock(), MIN_UPGRADE_TIMELOCK);
}

/// 48h timelock (well above floor) round-trips.
#[test]
fn test_set_upgrade_timelock_above_floor_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let two_days: u64 = 2 * 86_400;
    client.set_upgrade_timelock(&admin, &two_days);
    assert_eq!(client.get_upgrade_timelock(), two_days);
}

/// Below-floor values are rejected with the typed error — a compromised admin
/// cannot neutralise the timelock by setting it to 0.
#[test]
fn test_set_upgrade_timelock_below_floor_reverts() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let too_short = MIN_UPGRADE_TIMELOCK - 1;
    let result = client.try_set_upgrade_timelock(&admin, &too_short);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(
            ConfigManagerError::UpgradeTimelockTooShort as u32
        ),
    );
}

/// Zero is rejected — degenerate case of the below-floor check.
#[test]
fn test_set_upgrade_timelock_zero_reverts() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let result = client.try_set_upgrade_timelock(&admin, &0u64);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(
            ConfigManagerError::UpgradeTimelockTooShort as u32
        ),
    );
}

/// A non-admin caller cannot set the timelock — auth check rejects them.
#[test]
#[should_panic]
fn test_set_upgrade_timelock_non_admin_reverts() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = deploy_initialized(&env);
    let attacker = Address::generate(&env);

    client.set_upgrade_timelock(&attacker, &MIN_UPGRADE_TIMELOCK);
}

/// Set emits an UpgradeTimelockUpdate event with the new value.
#[test]
fn test_set_upgrade_timelock_emits_event() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let new_value: u64 = 3 * 86_400;
    client.set_upgrade_timelock(&admin, &new_value);

    let cm_id = client.address.clone();
    let mut saw = false;
    for (contract, topics, data) in env.events().all() {
        if contract != cm_id || topics.len() == 0 {
            continue;
        }
        let topic0: Symbol = match topics.get(0).unwrap().try_into_val(&env) {
            Ok(s) => s,
            Err(_) => continue,
        };
        if topic0 == Symbol::new(&env, "upgtl") {
            let parsed: (u64,) = data
                .try_into_val(&env)
                .expect("upgtl event data unpacks as (u64,)");
            assert_eq!(parsed.0, new_value);
            saw = true;
        }
    }
    assert!(saw, "expected UpgradeTimelockUpdate event with new value");
}
