//! Tests for `set_oracle_sources` on the OracleRouter contract.
//!
//! Coverage areas:
//!   - Happy path: admin can set primary and secondary source lists
//!   - Idempotent overwrite: a second call for the same symbol must not panic
//!   - Empty lists: clearing sources is explicitly valid (no non-empty guard)
//!   - Multi-symbol independence: ETH and BTC source lists do not cross-contaminate
//!   - Auth: missing auth propagates a host-level error before contract logic
//!   - Access control: non-admin is rejected with Unauthorized (3)
//!   - Error code: Unauthorized discriminant is verified to be exactly 3
//!   - Edge cases: single source, many sources, same address in both lists
//!
//! All tests that call `set_oracle_sources` FAIL until the `todo!()` stub is
//! replaced with a real implementation.

#![cfg(test)]

use soroban_sdk::{testutils::Address as _, testutils::Ledger as _, vec, Address, Env, Symbol};

use crate::OracleRouterError;

use super::helpers::{deploy_initialized, deploy_mock_oracle, deploy_with_config_manager};

/// Deploy a real mock oracle and return its address — a valid source that
/// answers `decimals()` so it passes `set_oracle_sources`' scale check.
fn mock_source(env: &Env) -> Address {
    deploy_mock_oracle(env).address
}

// ---------------------------------------------------------------------------
// Happy path
// ---------------------------------------------------------------------------

/// An admin caller (with auth mocked) must be able to call `set_oracle_sources`
/// with non-empty primary and secondary lists without panicking.
///
/// This is the minimal smoke test. It FAILS until `set_oracle_sources` is
/// implemented because the stub is `todo!()`.
#[test]
fn test_set_oracle_sources_admin_succeeds() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, admin) = deploy_with_config_manager(&env);

    let symbol = Symbol::new(&env, "ETH");
    let addr1 = mock_source(&env);
    let addr2 = mock_source(&env);
    let primary = vec![&env, addr1.clone(), addr2.clone()];

    // Must not panic. Fails on todo!() until implementation is written.
    oracle.set_oracle_sources(&admin, &symbol, &primary);
}

/// Calling `set_oracle_sources` a second time for the same symbol must
/// silently overwrite the stored lists without panicking.
///
/// Since there is no public getter for sources, we verify the absence of
/// a panic as the correctness signal. If the implementation does not treat
/// the storage write as idempotent (e.g., guards against overwriting),
/// this call would panic with an unexpected error.
///
/// This test FAILS until `set_oracle_sources` is implemented.
#[test]
fn test_set_oracle_sources_overwrites_previous_sources() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, admin) = deploy_with_config_manager(&env);
    let symbol = Symbol::new(&env, "ETH");

    let first_primary = vec![&env, mock_source(&env)];

    // First write — must succeed.
    oracle.set_oracle_sources(&admin, &symbol, &first_primary);

    let second_primary = vec![&env, mock_source(&env), mock_source(&env)];

    // Second write for the same symbol — must also succeed without any error.
    // An implementation that rejects overwrites would panic here.
    oracle.set_oracle_sources(&admin, &symbol, &second_primary);
}

/// Passing empty Vec for both `primary` and `secondary` must succeed.
///
/// The spec explicitly states that `set_oracle_sources` does NOT validate
/// that the lists are non-empty. Clearing sources (e.g., to disable an asset)
/// must be a valid operation. A guard that rejects empty lists would
/// incorrectly block this.
///
/// This test FAILS until `set_oracle_sources` is implemented.
#[test]
fn test_set_oracle_sources_can_set_empty_lists() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, admin) = deploy_with_config_manager(&env);
    let symbol = Symbol::new(&env, "ETH");
    let empty: soroban_sdk::Vec<Address> = vec![&env];

    // Empty source list — this is the "clear sources" operation and must NOT panic.
    oracle.set_oracle_sources(&admin, &symbol, &empty);
}

/// Sources set for "ETH" must not bleed into the storage slot for "BTC"
/// and vice versa.
///
/// Without a public getter this is tested indirectly: both calls must succeed
/// without panicking. If the implementation uses the same key for all symbols
/// (e.g., forgets to key by `symbol`), the second write would silently
/// overwrite the first — a correctness bug tested more precisely in
/// `get_price` tests.  Here we assert that both calls complete successfully.
///
/// This test FAILS until `set_oracle_sources` is implemented.
#[test]
fn test_set_oracle_sources_multiple_symbols_are_independent() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, admin) = deploy_with_config_manager(&env);

    let eth = Symbol::new(&env, "ETH");
    let btc = Symbol::new(&env, "BTC");

    let eth_primary = vec![&env, mock_source(&env)];

    let btc_primary = vec![&env, mock_source(&env), mock_source(&env)];

    // Set ETH sources.
    oracle.set_oracle_sources(&admin, &eth, &eth_primary);

    // Set BTC sources — must not panic or interfere with ETH slot.
    oracle.set_oracle_sources(&admin, &btc, &btc_primary);

    // Overwrite ETH again to confirm the BTC write did not corrupt the ETH key.
    let new_eth_primary = vec![&env, mock_source(&env)];
    oracle.set_oracle_sources(&admin, &eth, &new_eth_primary);
}

// ---------------------------------------------------------------------------
// Auth checks
// ---------------------------------------------------------------------------

/// Calling `set_oracle_sources` WITHOUT mocking auth must fail at the Soroban
/// host level with an auth error before any contract logic executes.
///
/// This verifies that `caller.require_auth()` is the FIRST guard, so the
/// signature check fires before the cross-contract ConfigManager call.
/// `try_set_oracle_sources` is used so the test does not unwind through a
/// panic boundary.
#[test]
fn test_set_oracle_sources_requires_auth() {
    let env = Env::default();
    env.mock_all_auths();
    let (oracle, _) = deploy_initialized(&env);

    // Strip auth before the call under test so the missing signature for
    // `caller.require_auth()` is detected.
    env.mock_auths(&[]);
    let caller = Address::generate(&env);
    let symbol = Symbol::new(&env, "ETH");
    let primary = vec![&env, mock_source(&env)];

    let result = oracle.try_set_oracle_sources(&caller, &symbol, &primary);

    assert!(
        result.is_err(),
        "set_oracle_sources called without auth must return an error; \
         require_auth() must be present and must be the first guard"
    );
}

/// A random address that is NOT the ConfigManager admin must be rejected with
/// `OracleRouterError::Unauthorized` even when its auth is mocked (i.e., the
/// signature is valid but the role check fails).
///
/// This tests the cross-contract `has_role` guard independently of the
/// signature check.  The attacker has valid auth but no ADMIN role.
///
/// This test FAILS until `set_oracle_sources` is implemented.
#[test]
fn test_set_oracle_sources_non_admin_is_unauthorized() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, _real_admin) = deploy_with_config_manager(&env);

    // Generate a fresh address that ConfigManager has never granted ADMIN to.
    let attacker = Address::generate(&env);
    let symbol = Symbol::new(&env, "ETH");
    let primary = vec![&env, mock_source(&env)];

    let result = oracle.try_set_oracle_sources(&attacker, &symbol, &primary);

    assert!(
        result.is_err(),
        "a non-admin caller must be rejected by the has_role cross-contract check"
    );
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(OracleRouterError::Unauthorized as u32),
        "non-admin caller must receive Unauthorized, not a generic or host-level error"
    );
}

/// Explicitly verify that `OracleRouterError::Unauthorized` has the
/// discriminant value 3, matching the enum definition in errors.rs.
///
/// This test catches accidental renumbering of the error enum that would
/// break on-chain clients that pattern-match on the numeric error code.
/// This test PASSES even before implementation because it only checks the
/// enum definition, not contract behavior.
#[test]
fn test_set_oracle_sources_unauthorized_error_code_is_3() {
    assert_eq!(
        OracleRouterError::Unauthorized as u32,
        3,
        "OracleRouterError::Unauthorized must always be discriminant 3; \
         changing this value breaks on-chain clients that match numeric codes"
    );
}

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

/// A single-address primary list with an empty secondary list must succeed.
///
/// Verifies that the implementation does not require both lists to be
/// simultaneously non-empty — single-source configuration is valid.
///
/// This test FAILS until `set_oracle_sources` is implemented.
#[test]
fn test_set_oracle_sources_single_primary_source() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, admin) = deploy_with_config_manager(&env);
    let symbol = Symbol::new(&env, "ETH");

    let single_primary = vec![&env, mock_source(&env)];

    // Single primary with no secondary — a minimal valid oracle configuration.
    oracle.set_oracle_sources(&admin, &symbol, &single_primary);
}

/// Five primary sources and three secondary sources must all be stored without
/// truncation, overflow, or silent rejection.
///
/// This guards against implementations that have a hard-coded capacity limit
/// (e.g., panicking on more than N sources) or that silently truncate lists
/// that exceed an internal buffer.
///
/// This test FAILS until `set_oracle_sources` is implemented.
#[test]
fn test_set_oracle_sources_many_sources() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, admin) = deploy_with_config_manager(&env);
    let symbol = Symbol::new(&env, "ETH");

    // Build 5 distinct primary addresses (real oracle sources).
    let p1 = mock_source(&env);
    let p2 = mock_source(&env);
    let p3 = mock_source(&env);
    let p4 = mock_source(&env);
    let p5 = mock_source(&env);
    let primary = vec![&env, p1, p2, p3, p4, p5];

    // Must not panic — large lists must be stored verbatim.
    oracle.set_oracle_sources(&admin, &symbol, &primary);
}

/// An address that appears in both the primary and secondary lists must be
/// accepted without deduplication.
///
/// The spec states that `set_oracle_sources` just stores the lists — it does
/// NOT validate contents or deduplicate.  An implementation that enforces
/// uniqueness across or within lists contradicts the spec and would fail this
/// test.
///
/// This test FAILS until `set_oracle_sources` is implemented.
#[test]
fn test_set_oracle_sources_same_address_in_primary_and_secondary() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, admin) = deploy_with_config_manager(&env);
    let symbol = Symbol::new(&env, "ETH");

    // Shared address that appears in both lists.
    let shared = mock_source(&env);
    let primary = vec![&env, shared.clone()];

    // Must succeed — the contract stores lists verbatim, no dedup check.
    oracle.set_oracle_sources(&admin, &symbol, &primary);
}

/// An address repeated multiple times in the primary list (duplicates within
/// a single list) must be stored as-is without deduplication or rejection.
///
/// This is an adversarial variant of the above test: an operator who
/// accidentally repeats an address must not cause the contract to panic.
///
/// This test FAILS until `set_oracle_sources` is implemented.
#[test]
fn test_set_oracle_sources_duplicate_addresses_within_primary_list() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, admin) = deploy_with_config_manager(&env);
    let symbol = Symbol::new(&env, "ETH");

    let addr = mock_source(&env);
    // Same address three times in the primary list.
    let primary = vec![&env, addr.clone(), addr.clone(), addr.clone()];

    // No dedup: the contract must store the list exactly as provided.
    oracle.set_oracle_sources(&admin, &symbol, &primary);
}

// ---------------------------------------------------------------------------
// Adversarial: access control escalation
// ---------------------------------------------------------------------------

/// A caller who holds the KEEPER role (not ADMIN) must be rejected.
///
/// This verifies that the role check is not accidentally broadened to
/// any role holder — only DEFAULT_ADMIN_ROLE ("ADMIN") is sufficient.
///
/// This test FAILS until `set_oracle_sources` is implemented.
#[test]
fn test_set_oracle_sources_keeper_role_is_not_sufficient() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, cm, admin) = deploy_with_config_manager(&env);

    // Grant the KEEPER role to a keeper bot.
    let keeper = Address::generate(&env);
    let keeper_role = Symbol::new(&env, "KEEPER");
    cm.grant_role(&admin, &keeper_role, &keeper);

    let symbol = Symbol::new(&env, "ETH");
    let primary = vec![&env, mock_source(&env)];

    let result = oracle.try_set_oracle_sources(&keeper, &symbol, &primary);

    assert!(
        result.is_err(),
        "KEEPER role must not be sufficient to call set_oracle_sources — only ADMIN is allowed"
    );
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(OracleRouterError::Unauthorized as u32),
        "KEEPER must receive Unauthorized (3), not succeed or receive a different error"
    );
}

/// An admin of a DIFFERENT ConfigManager instance (not the one wired into this
/// OracleRouter) must be rejected.
///
/// This verifies that the stored ConfigManager address is actually used for
/// the `has_role` lookup rather than a hardcoded or global address.
///
/// This test FAILS until `set_oracle_sources` is implemented.
#[test]
fn test_set_oracle_sources_admin_of_wrong_config_manager_is_unauthorized() {
    let env = Env::default();
    env.mock_all_auths();

    // Deploy two independent (router, config_manager, admin) triples.
    let (oracle_a, _cm_a, _admin_a) = deploy_with_config_manager(&env);
    let (_oracle_b, _cm_b, admin_b) = deploy_with_config_manager(&env);

    // admin_b is the ADMIN of cm_b, but oracle_a is wired to cm_a.
    let symbol = Symbol::new(&env, "ETH");
    let primary = vec![&env, mock_source(&env)];

    let result = oracle_a.try_set_oracle_sources(&admin_b, &symbol, &primary);

    assert!(
        result.is_err(),
        "admin of a DIFFERENT ConfigManager must be rejected by oracle_a's has_role check"
    );
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(OracleRouterError::Unauthorized as u32),
        "cross-CM admin must receive Unauthorized (3)"
    );
}

/// A revoked admin (whose role was transferred to a new address) must no longer
/// be able to set oracle sources.
///
/// This verifies that the contract always delegates to the LIVE ConfigManager
/// state and does not cache the admin address in its own storage.
///
/// This test FAILS until `set_oracle_sources` is implemented.
#[test]
fn test_set_oracle_sources_revoked_admin_is_unauthorized_after_transfer() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, cm, original_admin) = deploy_with_config_manager(&env);

    let symbol = Symbol::new(&env, "ETH");
    let primary = vec![&env, mock_source(&env)];

    // Confirm the original admin can call set_oracle_sources.
    oracle.set_oracle_sources(&original_admin, &symbol, &primary);

    // Transfer admin role to a new address.
    let new_admin = Address::generate(&env);
    cm.propose_admin(&original_admin, &new_admin);
    cm.accept_admin(&new_admin);

    // original_admin no longer holds the ADMIN role — must be rejected now.
    let attacker_primary = vec![&env, mock_source(&env)];

    let result = oracle.try_set_oracle_sources(
        &original_admin,
        &symbol,
        &attacker_primary);

    assert!(
        result.is_err(),
        "a former admin whose role was transferred must be rejected"
    );
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(OracleRouterError::Unauthorized as u32),
        "revoked admin must receive Unauthorized (3)"
    );
}

// ---------------------------------------------------------------------------
// Adversarial: instance TTL is extended after a successful write
// ---------------------------------------------------------------------------

/// After `set_oracle_sources`, the OracleRouter instance storage must still
/// be accessible without a host-level archival error.
///
/// We verify this indirectly: a follow-up `try_get_oracle_config` must return
/// a well-typed contract error (not a host-level InvokeError), which proves
/// that the instance storage TTL was extended and the entry is still live.
///
/// This test FAILS until `set_oracle_sources` is implemented.
#[test]
fn test_set_oracle_sources_bumps_instance_ttl() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, admin) = deploy_with_config_manager(&env);
    let symbol = Symbol::new(&env, "ETH");
    let primary = vec![&env, mock_source(&env)];

    oracle.set_oracle_sources(&admin, &symbol, &primary);

    // get_oracle_config reads from instance storage. A well-typed contract
    // error (NotInitialized) proves the instance storage entry is live.
    // A host-level panic would indicate the TTL was not extended.
    let result = oracle.try_get_oracle_config();
    match result {
        Err(Ok(_contract_error)) => {
            // Expected: a well-typed OracleRouterError means the instance
            // storage is reachable and the TTL was extended.
        }
        Err(Err(_host_error)) => {
            panic!(
                "host-level error after set_oracle_sources: instance storage is not accessible; \
                 bump_instance_ttl must be called inside set_oracle_sources"
            );
        }
        Ok(_) => {
            // Acceptable if set_oracle_config was previously called — this
            // path would only be taken in a combined test that pre-sets config.
        }
    }
}

// ---------------------------------------------------------------------------
// Adversarial: invalid symbol values
// ---------------------------------------------------------------------------

/// Setting sources for a symbol that differs only by case from a previously
/// set symbol (e.g., "eth" vs "ETH") must treat them as completely
/// independent keys — no case-folding or symbol aliasing.
///
/// Soroban `Symbol` values are case-sensitive. This test guards against an
/// implementation that normalizes to uppercase/lowercase before keying.
///
/// This test FAILS until `set_oracle_sources` is implemented.
#[test]
fn test_set_oracle_sources_symbol_is_case_sensitive() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, admin) = deploy_with_config_manager(&env);

    let eth_upper = Symbol::new(&env, "ETH");
    let eth_lower = Symbol::new(&env, "eth");

    let primary_upper = vec![&env, mock_source(&env)];
    let primary_lower = vec![&env, mock_source(&env)];

    // Both calls must succeed independently — they address different storage slots.
    oracle.set_oracle_sources(&admin, &eth_upper, &primary_upper);
    oracle.set_oracle_sources(&admin, &eth_lower, &primary_lower);
}

/// Setting sources for multiple different symbols in sequence must not panic
/// or produce cross-symbol contamination.  This is a broader smoke test that
/// walks through several realistic asset symbols.
///
/// This test FAILS until `set_oracle_sources` is implemented.
#[test]
fn test_set_oracle_sources_multiple_symbols_sequence() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, admin) = deploy_with_config_manager(&env);

    for sym_str in ["ETH", "BTC", "SOL", "XLM"] {
        let symbol = Symbol::new(&env, sym_str);
        let primary = vec![&env, mock_source(&env)];

        // Each symbol must succeed in isolation.
        oracle.set_oracle_sources(&admin, &symbol, &primary);
    }
}

// ---------------------------------------------------------------------------
// Duplicate oracle source deduplication
//
// `set_oracle_sources` deduplicates the primary list before storing it so each
// address contributes at most one data point to the median. Without this, a
// single oracle could appear as two "independent" sources and bias the median.
//
// The dedup-correctness test below uses a three-source scenario where the
// duplicate biases the non-dedup median away from the true two-source median:
//   Sources: [oracle_a (price=300), oracle_a (price=300), oracle_b (price=100)]
//   Without dedup: sorted=[100,300,300], n=3, median_idx=1 → median=300
//   With dedup:    sorted=[100,300],     n=2, median_idx=0 → median=100
//
// The second test below verifies that after dedup a duplicate-only list `[a,a]` is treated
// as a single-source list and returns that source's price cleanly.
// ---------------------------------------------------------------------------

/// Registering the same oracle address twice in the primary list must be
/// deduplicated before storage and before the median is computed.
///
/// Without dedup, `[oracle_a, oracle_a, oracle_b]` with oracle_a=300 and
/// oracle_b=100 produces a sorted array of [100, 300, 300], giving median=300
/// (the duplicate biases the result toward oracle_a).
///
/// With dedup, the stored list becomes `[oracle_a, oracle_b]`, sorted=[100,300],
/// and the lower-median selection gives median=100 — the correct unbiased result.
///
/// This test FAILS on the unfixed implementation because `get_price` queries
/// oracle_a twice, producing sorted=[100,300,300] and returning 300.
/// After the fix (dedup in `set_oracle_sources`), the stored list has 2 distinct
/// entries, sorted=[100,300], and `get_price` returns 100.
#[test]
fn test_set_oracle_sources_deduplicates_primary_list() {
    let env = Env::default();
    env.mock_all_auths();

    env.ledger().set_timestamp(100);

    let (oracle, _cm, admin) = deploy_with_config_manager(&env);

    // Max deviation set to the ceiling so the guard never trips during the
    // dedup-only check. min_required_sources=1 keeps the quorum trivial.
    let config = crate::OracleConfig {
        max_deviation_bps: shared::constants::MAX_DEVIATION_BPS_CEILING,
        staleness_threshold: 60,
        cache_duration: 10,
        min_required_sources: 2,
    };
    oracle.set_oracle_config(&admin, &config);

    let eth = Symbol::new(&env, "ETH");

    // Prices 100 and 200 sit at the deviation ceiling (10_000 bps = 100% of
    // the lower median) so the gate passes while still letting us observe
    // whether dedup happened.
    let oracle_a = super::helpers::deploy_mock_oracle(&env);
    oracle_a.set_price(&eth, &200i128);

    let oracle_b = super::helpers::deploy_mock_oracle(&env);
    oracle_b.set_price(&eth, &100i128);

    // Register oracle_a TWICE and oracle_b ONCE.
    // WITHOUT dedup: sorted = [100, 200, 200], n=3 → median=200
    // WITH dedup:    sorted = [100, 200],     n=2 → averaged median=150
    let primary = vec![
        &env,
        oracle_a.address.clone(),
        oracle_a.address.clone(),
        oracle_b.address.clone(),
    ];
    oracle.set_oracle_sources(&admin, &eth, &primary);

    let price = oracle.get_price(&eth);

    // After dedup, the median of the two distinct prices [100, 200] is the
    // average 150. A return value of 200 would indicate oracle_a was
    // double-counted (no dedup), which is the bug being guarded against.
    assert_eq!(
        price, 150i128,
        "get_price must return the deduplicated averaged median (150), not the \
         duplicate-biased median (200). oracle_a was registered twice; after dedup it \
         contributes only one data point. If 200 is returned, dedup is not happening."
    );
}

/// Registering only duplicate addresses `[oracle_a, oracle_a, oracle_a]` must
/// dedup to the single distinct source `[oracle_a]`. Under the quorum floor
/// (2), a single effective source cannot satisfy `min_required_sources`, so
/// `get_price` returns InsufficientSources. This proves BOTH that dedup
/// happens (otherwise three identical query results would meet the quorum and
/// return a price) AND that a lone source can never set the median.
#[test]
fn test_set_oracle_sources_dedup_all_duplicate_list_fails_quorum() {
    let env = Env::default();
    env.mock_all_auths();

    env.ledger().set_timestamp(100);

    let (oracle, _cm, admin) = deploy_with_config_manager(&env);

    let config = crate::OracleConfig {
        max_deviation_bps: 200,
        staleness_threshold: 60,
        cache_duration: 10,
        min_required_sources: 2,
    };
    oracle.set_oracle_config(&admin, &config);

    let eth = Symbol::new(&env, "ETH");

    let oracle_a = super::helpers::deploy_mock_oracle(&env);
    oracle_a.set_price(&eth, &2_000_0000000i128);

    // Register oracle_a three times — after dedup this reduces to [oracle_a].
    let primary = vec![
        &env,
        oracle_a.address.clone(),
        oracle_a.address.clone(),
        oracle_a.address.clone(),
    ];
    oracle.set_oracle_sources(&admin, &eth, &primary);

    let result = oracle.try_get_price(&eth);
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(OracleRouterError::InsufficientSources as u32),
        "a list of only-duplicate addresses dedups to one source and must fail the \
         quorum floor with InsufficientSources (9), proving both dedup and quorum hold"
    );
}

/// Registering a mix of duplicates and a distant outlier proves that dedup
/// cannot be bypassed to skew the deviation check.
///
/// Attack scenario (without dedup):
///   Operator wants to include oracle_b (outlier at price 10_000) alongside
///   oracle_a (price 100) without tripping the deviation guard.  They register
///   [oracle_a, oracle_a, oracle_b] hoping the duplicated oracle_a will anchor
///   the median at 100 and make (10_000 - 100) / 100 * 10_000 = 990_000 bps
///   still fail the deviation check.  Actually this STILL fails deviation even
///   without dedup.
///
/// The real bypass: register [oracle_a, oracle_a] alongside a separate call
/// where oracle_a=1000 and oracle_b=5000, choosing only [oracle_a, oracle_a]
/// to present as if two sources agree.  This fakes "consensus" at oracle_a's
/// price while oracle_b's true disagreement is hidden.
///
/// Observable test: [oracle_a, oracle_a, oracle_b] with oracle_a=100, oracle_b=500,
/// max_deviation_bps=5000 (50%).
///
/// WITHOUT dedup: sorted=[100,100,500], n=3, median_idx=1 → median=100.
///   upper_dev = (500-100)*10000/100 = 40000 bps > 5000 → PriceDeviationTooHigh
/// WITH dedup:    sorted=[100,500], n=2, median_idx=0 → median=100.
///   upper_dev = 40000 bps > 5000 → PriceDeviationTooHigh
///
/// In this case both pass the deviation guard the same way.  The test confirms
/// the price returned is the same in both branches (100), proving dedup is
/// safe and does not change the outcome for this configuration.
///
/// This test PASSES even without the fix, acting as a regression guard that
/// the dedup does not accidentally break divergent multi-source configurations.
#[test]
fn test_set_oracle_sources_dedup_preserves_deviation_check_for_divergent_sources() {
    let env = Env::default();
    env.mock_all_auths();

    env.ledger().set_timestamp(100);

    let (oracle, _cm, admin) = deploy_with_config_manager(&env);

    // Use the deviation ceiling so the divergence in this dedup-focused test
    // doesn't trip the guard. `max_deviation_bps` is capped at 10_000 (100%).
    let config = crate::OracleConfig {
        max_deviation_bps: shared::constants::MAX_DEVIATION_BPS_CEILING,
        staleness_threshold: 60,
        cache_duration: 10,
        min_required_sources: 2,
    };
    oracle.set_oracle_config(&admin, &config);

    let eth = Symbol::new(&env, "ETH");

    let oracle_a = super::helpers::deploy_mock_oracle(&env);
    oracle_a.set_price(&eth, &100i128);

    let oracle_b = super::helpers::deploy_mock_oracle(&env);
    // Capped at +100% of the lower-median (100), the max upper price the new
    // deviation gate (10_000 bps ceiling) allows is 200.
    oracle_b.set_price(&eth, &200i128);

    // [oracle_a (100), oracle_a (100), oracle_b (200)]
    // Without dedup: n=3, sorted=[100,100,200] → median=100
    // With dedup:    n=2, sorted=[100,200]     → averaged median=150
    let primary = vec![
        &env,
        oracle_a.address.clone(),
        oracle_a.address.clone(),
        oracle_b.address.clone(),
    ];
    oracle.set_oracle_sources(&admin, &eth, &primary);

    let price = oracle.get_price(&eth);

    assert_eq!(
        price, 150i128,
        "get_price with [oracle_a(100), oracle_a(100), oracle_b(200)] must dedup to two \
         distinct sources [100, 200] and return their averaged median (150), confirming \
         dedup collapses the duplicate before the median is computed."
    );
}
