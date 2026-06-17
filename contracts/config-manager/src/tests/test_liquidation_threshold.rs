//! Tests for `liquidation_threshold_bps` on `ProtocolLimits`.
//!
//! This is the first atomic step toward making the liquidation health threshold
//! configurable on-chain. Scope of this file is strictly the ConfigManager
//! contract surface plus the shape of the shared `ProtocolLimits` struct:
//!
//!   - `initialize` writes a sensible default (`DEFAULT_LIQUIDATION_THRESHOLD_BPS = 200`).
//!   - `update_protocol_limits` accepts/round-trips the field.
//!   - `update_protocol_limits` rejects values above a sane 10% cap (1_000 bps).
//!   - `update_protocol_limits` emits the new field in the `LimitsUpdate` event.
//!
//! These tests are expected to FAIL at COMPILE time until code-writer adds the
//! `liquidation_threshold_bps: u32` field to `shared::ProtocolLimits`,
//! initialises it from `shared::constants::DEFAULT_LIQUIDATION_THRESHOLD_BPS = 200` in
//! `ConfigManagerContract::initialize`, validates it in `update_protocol_limits`
//! (must be <= 1_000), and includes it in the `LimitsUpdate` event payload.
//!
//! That compile-time failure is the correct TDD "red" signal for this loop.

use soroban_sdk::{
    symbol_short,
    testutils::Events as _,
    Env, Symbol, TryIntoVal,
};

use crate::ConfigManagerError;

use super::helpers::{deploy_initialized, valid_limits};

// ---------------------------------------------------------------------------
// 1. Initialize default
// ---------------------------------------------------------------------------

/// `initialize` must seed `liquidation_threshold_bps` to the protocol default
/// (200 bps = 2%) so that PositionManager can read a sane value before any
/// admin update arrives. Reading the default constant from the `shared` crate
/// is the load-bearing assertion — the value must come from
/// `shared::constants::DEFAULT_LIQUIDATION_THRESHOLD_BPS`, not be hard-coded inline in
/// `contract.rs`, so a single source of truth is preserved.
#[test]
fn test_initialize_seeds_default_liquidation_threshold_of_200_bps() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = deploy_initialized(&env);

    let stored = client.get_protocol_limits();
    assert_eq!(
        stored.liquidation_threshold_bps,
        shared::constants::DEFAULT_LIQUIDATION_THRESHOLD_BPS,
        "initialize must seed liquidation_threshold_bps from shared::constants::DEFAULT_LIQUIDATION_THRESHOLD_BPS"
    );
    assert_eq!(
        stored.liquidation_threshold_bps, 200,
        "DEFAULT_LIQUIDATION_THRESHOLD_BPS must be 200 (2%) per protocol design"
    );
}


// ---------------------------------------------------------------------------
// 2. Round-trip
// ---------------------------------------------------------------------------

/// Admin-driven `update_protocol_limits` with a non-default
/// `liquidation_threshold_bps` (350 bps = 3.5%) must persist exactly that value;
/// a subsequent `get_protocol_limits` read must return 350 — not the previous
/// default, not a clamped value.
#[test]
fn test_update_protocol_limits_round_trips_liquidation_threshold_350() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let mut limits = valid_limits();
    limits.liquidation_threshold_bps = 350;
    client.update_protocol_limits(&admin, &limits);

    let stored = client.get_protocol_limits();
    assert_eq!(
        stored.liquidation_threshold_bps, 350,
        "stored liquidation_threshold_bps must round-trip the input value (350) exactly"
    );
}

/// Round-trip preserves all OTHER fields when the new `liquidation_threshold_bps`
/// is updated — adversarial check that introducing the new field did not
/// accidentally drop any neighbouring field on save/load.
#[test]
fn test_update_protocol_limits_with_threshold_preserves_other_fields() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let mut limits = valid_limits();
    limits.liquidation_threshold_bps = 350;
    client.update_protocol_limits(&admin, &limits);

    let stored = client.get_protocol_limits();
    assert_eq!(stored.min_collateral, limits.min_collateral, "min_collateral preserved");
    assert_eq!(stored.cooldown_duration, limits.cooldown_duration, "cooldown_duration preserved");
    assert_eq!(stored.min_position_lifetime, limits.min_position_lifetime, "min_position_lifetime preserved");
    assert_eq!(stored.max_utilization_ratio, limits.max_utilization_ratio, "max_utilization_ratio preserved");
    assert_eq!(stored.funding_cut_bps, limits.funding_cut_bps, "funding_cut_bps preserved");
    assert_eq!(stored.adl_pnl_bps, limits.adl_pnl_bps, "adl_pnl_bps preserved");
    assert_eq!(stored.adl_utilization_bps, limits.adl_utilization_bps, "adl_utilization_bps preserved");
}

// ---------------------------------------------------------------------------
// 3. Floor — values below MIN_LIQUIDATION_THRESHOLD_BPS are rejected
// ---------------------------------------------------------------------------

/// `liquidation_threshold_bps` below the floor MUST be rejected. A zero (or
/// near-zero) threshold makes positions liquidatable only once health is
/// already negative, so every liquidation would start inside a bad-debt
/// window the bounty then deepens.
#[test]
fn test_update_protocol_limits_liquidation_threshold_below_floor_reverts() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    for below_floor in [0u32, shared::constants::MIN_LIQUIDATION_THRESHOLD_BPS - 1] {
        let mut limits = valid_limits();
        limits.liquidation_threshold_bps = below_floor;

        let result = client.try_update_protocol_limits(&admin, &limits);
        assert!(
            result.is_err(),
            "liquidation_threshold_bps = {below_floor} must be rejected (floor is {})",
            shared::constants::MIN_LIQUIDATION_THRESHOLD_BPS
        );
        assert_eq!(
            result.unwrap_err().unwrap(),
            soroban_sdk::Error::from_contract_error(
                ConfigManagerError::InvalidLiquidationThreshold as u32
            ),
            "error code must be InvalidLiquidationThreshold (35)"
        );
    }
}

/// The floor itself is inclusive: exactly MIN_LIQUIDATION_THRESHOLD_BPS
/// must succeed.
#[test]
fn test_update_protocol_limits_liquidation_threshold_at_floor_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let mut limits = valid_limits();
    limits.liquidation_threshold_bps = shared::constants::MIN_LIQUIDATION_THRESHOLD_BPS;
    client.update_protocol_limits(&admin, &limits);

    let stored = client.get_protocol_limits();
    assert_eq!(
        stored.liquidation_threshold_bps,
        shared::constants::MIN_LIQUIDATION_THRESHOLD_BPS,
        "threshold at the floor must round-trip"
    );
}

// ---------------------------------------------------------------------------
// 4. Boundary (inclusive) — 1000 bps = 10% maintenance margin
// ---------------------------------------------------------------------------

/// `liquidation_threshold_bps = 1_000` (10%) must succeed — this is the
/// inclusive upper cap. 10% maintenance margin is already at the aggressive
/// end of DeFi perp design; anything tighter would risk premature liquidation
/// on normal price wobble. Boundary is INCLUSIVE.
#[test]
fn test_update_protocol_limits_liquidation_threshold_at_1000_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let mut limits = valid_limits();
    limits.liquidation_threshold_bps = 1_000;

    let result = client.try_update_protocol_limits(&admin, &limits);
    assert!(
        result.is_ok(),
        "boundary: liquidation_threshold_bps = 1_000 (10% cap) must succeed (inclusive)"
    );

    let stored = client.get_protocol_limits();
    assert_eq!(
        stored.liquidation_threshold_bps, 1_000,
        "stored liquidation_threshold_bps must be 1_000 at the inclusive cap"
    );
}

// ---------------------------------------------------------------------------
// 5. Above cap — 1001 bps must error InvalidLimits
// ---------------------------------------------------------------------------

/// `liquidation_threshold_bps = 1_001` (just above the 10% cap) must be
/// rejected with `InvalidLimits = 5`. Off-by-one boundary check —
/// implementation must use strict `>` not `>=` against the cap.
#[test]
fn test_update_protocol_limits_liquidation_threshold_at_1001_errors() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let mut limits = valid_limits();
    limits.liquidation_threshold_bps = 1_001;

    let result = client.try_update_protocol_limits(&admin, &limits);
    assert!(
        result.is_err(),
        "liquidation_threshold_bps = 1_001 (just above 10% cap) must return an error"
    );
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(ConfigManagerError::InvalidLiquidationThreshold as u32),
        "error code must be InvalidLiquidationThreshold (35)"
    );
}

// ---------------------------------------------------------------------------
// 6. u32::MAX adversarial — must NOT overflow, must error InvalidLimits
// ---------------------------------------------------------------------------

/// `liquidation_threshold_bps = u32::MAX` (~4.29 billion bps) must be rejected
/// with `InvalidLimits = 5` — adversarial check that the cap comparison is
/// done as a plain `>` against 1_000 (no integer-cast trickery, no wraparound,
/// no panic on overflow inside validation).
#[test]
fn test_update_protocol_limits_liquidation_threshold_u32_max_errors() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let mut limits = valid_limits();
    limits.liquidation_threshold_bps = u32::MAX;

    let result = client.try_update_protocol_limits(&admin, &limits);
    assert!(
        result.is_err(),
        "adversarial: liquidation_threshold_bps = u32::MAX must return an error, not overflow or wrap"
    );
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(ConfigManagerError::InvalidLiquidationThreshold as u32),
        "error code must be InvalidLiquidationThreshold (35)"
    );
}

// ---------------------------------------------------------------------------
// 7. Event emission — LimitsUpdate must include liquidation_threshold_bps
// ---------------------------------------------------------------------------

/// `update_protocol_limits` must emit a `LimitsUpdate` event whose `vec`-format
/// data payload includes the new `liquidation_threshold_bps` value. Indexers
/// downstream consume this event to populate the `protocol_limits` table — if
/// the new field is silently dropped from the event, the indexer goes stale.
///
/// Assertion strategy:
///   1. Drain `env.events().all()` AFTER `update_protocol_limits` to filter to
///      events from THIS contract.
///   2. Find the event with topic `("limits",)` (set by the
///      `#[contractevent(topics = ["limits"])]` macro on `LimitsUpdate`).
///   3. Unpack the data `Val` (which is a Vec underneath) into an 8-arity
///      tuple and assert each element including the trailing
///      `liquidation_threshold_bps`.
///
/// If code-writer forgets to add `liquidation_threshold_bps` to the
/// `LimitsUpdate` struct, the runtime data Vec only has 7 elements, the
/// 8-tuple unpack returns Err, and this test fails — the desired red signal.
#[test]
fn test_update_protocol_limits_emits_event_including_liquidation_threshold() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = deploy_initialized(&env);

    let mut limits = valid_limits();
    limits.liquidation_threshold_bps = 350;
    client.update_protocol_limits(&admin, &limits);

    // Filter to events emitted by THIS contract with the "limits" topic.
    let limits_topic: Symbol = symbol_short!("limits");
    let all = env.events().all();
    let mut matched_data: Option<soroban_sdk::Val> = None;
    for entry in all.iter() {
        let (contract, topics, data) = entry;
        if contract != client.address {
            continue;
        }
        // Topic 0 must be the symbol `"limits"` (set by #[contractevent(topics = ["limits"])]).
        if topics.len() == 0 {
            continue;
        }
        let first_topic_val = topics.get(0).unwrap();
        let first_topic: Result<Symbol, _> = first_topic_val.try_into_val(&env);
        if let Ok(s) = first_topic {
            if s == limits_topic {
                matched_data = Some(data);
                break;
            }
        }
    }

    let data = matched_data.expect(
        "update_protocol_limits must emit a `limits`-topic event on this contract",
    );

    // The `LimitsUpdate` event uses `data_format = "vec"`, so the underlying
    // payload is a Vec<Val> whose elements are the struct fields in
    // declaration order. Soroban's tuple TryFromVal<Env, Val> impl unpacks a
    // VecObject into a fixed-arity tuple.
    //
    // The struct has SEVEN existing fields plus the new
    // `liquidation_threshold_bps`, so the tuple here is 8-arity. If
    // code-writer forgets to add the field to the `LimitsUpdate` struct in
    // events.rs, the runtime Vec will only contain 7 elements and this 8-tuple
    // unpack will return an error — exactly the desired red signal.
    let parsed: Result<(i128, u64, u64, i128, u32, u32, u32, u32), _> =
        data.try_into_val(&env);
    let tuple = parsed.expect(
        "LimitsUpdate event data must unpack into the 8-field tuple including liquidation_threshold_bps; if this fails, the event struct is missing the new trailing field"
    );

    assert_eq!(tuple.0, limits.min_collateral, "event[0] = min_collateral");
    assert_eq!(tuple.1, limits.cooldown_duration, "event[1] = cooldown_duration");
    assert_eq!(tuple.2, limits.min_position_lifetime, "event[2] = min_position_lifetime");
    assert_eq!(tuple.3, limits.max_utilization_ratio, "event[3] = max_utilization_ratio");
    assert_eq!(tuple.4, limits.funding_cut_bps, "event[4] = funding_cut_bps");
    assert_eq!(tuple.5, limits.adl_pnl_bps, "event[5] = adl_pnl_bps");
    assert_eq!(tuple.6, limits.adl_utilization_bps, "event[6] = adl_utilization_bps");
    assert_eq!(
        tuple.7, limits.liquidation_threshold_bps,
        "event[7] = liquidation_threshold_bps — load-bearing assertion: this is the new field"
    );
}
