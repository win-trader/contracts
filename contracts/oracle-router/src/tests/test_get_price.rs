//! Tests for `get_price` on the OracleRouter contract.
//!
//! Coverage areas (cache hit path):
//!   - Cached price is returned without querying sources when within duration
//!   - Expired cache triggers a fresh fetch from sources
//!   - No cache entry at all triggers a fetch
//!
//! Coverage areas (cache miss / fetch path):
//!   - Single source: price is fetched and returned correctly
//!   - No sources configured → NoPriceSources (6)
//!   - All sources stale → StalePrice (4)
//!   - Stale sources are filtered when at least one fresh source exists
//!   - Median computation for three sources (odd count)
//!   - Lower-median selection for even source count
//!   - Deviation above threshold → PriceDeviationTooHigh (5)
//!   - Deviation within threshold → price returned
//!   - No OracleConfig set → NotInitialized (2)
//!
//! Broken oracle source isolation:
//!   - A source that panics must be skipped when another valid source exists
//!   - All sources panicking must return a clean contract error, not a host panic

#![cfg(test)]

use soroban_sdk::{testutils::Ledger as _, vec, Address, Env, Symbol};

use super::helpers::{deploy_mock_oracle, deploy_with_config_manager, deploy_with_price_feed};
use crate::OracleConfig;
use crate::OracleRouterError;

// ---------------------------------------------------------------------------
// Cache hit / miss invariants
// ---------------------------------------------------------------------------

/// Within `cache_duration` seconds of a prior fetch, `get_price` must return
/// the cached value without re-querying sources. We verify this by changing
/// the upstream source price between two calls (without advancing time) —
/// the second call must return the ORIGINAL price, proving the cache served
/// it.
#[test]
fn test_get_price_returns_cached_value_within_window() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, mock, _admin) = deploy_with_price_feed(&env);
    let eth = Symbol::new(&env, "ETH");

    let initial_price: i128 = 3_000_0000000;
    mock.set_price(&eth, &initial_price);
    assert_eq!(oracle.get_price(&eth), initial_price);

    // Change the source price; the cache must still serve the original.
    let updated_price: i128 = 9_999_0000000;
    mock.set_price(&eth, &updated_price);
    assert_eq!(
        oracle.get_price(&eth),
        initial_price,
        "within cache_duration the cached price must be returned, not the live source"
    );
}

/// A cached median must not remain usable after the source timestamps that
/// produced it have crossed `staleness_threshold`, even if the router's
/// `cache_duration` window has not elapsed yet.
#[test]
fn test_get_price_cached_value_expires_when_sources_become_stale() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, mock, _admin) = deploy_with_price_feed(&env);
    let eth = Symbol::new(&env, "ETH");

    env.ledger().set_timestamp(41);
    let price: i128 = 3_000_0000000;
    mock.set_price(&eth, &price);

    // At t=100 the source is 59s old, inside the 60s staleness threshold,
    // so the router may cache it.
    env.ledger().set_timestamp(100);
    assert_eq!(oracle.get_price(&eth), price);

    // At t=102 the router cache would still be inside its 10s duration from
    // t=100, but the underlying source update is now 61s old and stale.
    env.ledger().set_timestamp(102);
    let result = oracle.try_get_price(&eth);
    assert!(
        result.is_err(),
        "cache hit must be rejected once the source timestamp is stale"
    );
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(OracleRouterError::StalePrice as u32),
        "stale cached source data must force a refetch and return StalePrice"
    );
}

/// Updating OracleConfig must invalidate cached medians immediately. Otherwise
/// a stricter deviation threshold would not take effect until cache expiry.
#[test]
fn test_set_oracle_config_invalidates_cached_price_immediately() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, admin) = deploy_with_config_manager(&env);
    let eth = Symbol::new(&env, "ETH");

    let loose = OracleConfig {
        max_deviation_bps: 10_000,
        staleness_threshold: 60,
        cache_duration: 10,
        min_required_sources: 2,
    };
    oracle.set_oracle_config(&admin, &loose);

    let oracle_low = deploy_mock_oracle(&env);
    let oracle_high = deploy_mock_oracle(&env);
    oracle_low.set_price(&eth, &1_000_0000000i128);
    oracle_high.set_price(&eth, &2_000_0000000i128);
    oracle.set_oracle_sources(
        &admin,
        &eth,
        &vec![
            &env,
            oracle_low.address.clone(),
            oracle_high.address.clone(),
        ],
    );

    assert_eq!(oracle.get_price(&eth), 1_500_0000000i128);

    let strict = OracleConfig {
        max_deviation_bps: 100,
        staleness_threshold: 60,
        cache_duration: 10,
        min_required_sources: 2,
    };
    oracle.set_oracle_config(&admin, &strict);

    let result = oracle.try_get_price(&eth);
    assert!(
        result.is_err(),
        "config update must bypass the old cached median immediately"
    );
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(OracleRouterError::PriceDeviationTooHigh as u32),
        "strict deviation config must be applied without waiting for cache expiry"
    );
}

/// After the cache expires (current_time > fetched_at + cache_duration),
/// `get_price` must re-fetch from sources and surface the new price.
#[test]
fn test_get_price_refetches_after_time_advance() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, mock, _admin) = deploy_with_price_feed(&env);
    let eth = Symbol::new(&env, "ETH");

    let initial_price: i128 = 1_000_0000000;
    mock.set_price(&eth, &initial_price);
    oracle.get_price(&eth);

    env.ledger().with_mut(|li| {
        li.timestamp += 11;
    });

    let fresh_price: i128 = 2_000_0000000;
    mock.set_price(&eth, &fresh_price);

    let price = oracle.get_price(&eth);
    assert_eq!(
        price, fresh_price,
        "get_price must return the fresh price after the time advance; \
         every call refetches from sources"
    );
}

/// When no cache entry exists for a symbol (e.g., first ever call for that
/// symbol), `get_price` must fall through to the fetch path.
///
/// First `get_price` call for a symbol must consult the upstream source
/// and return its price (no caching layer between caller and source).
#[test]
fn test_get_price_first_call_fetches_from_source() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, mock, _admin) = deploy_with_price_feed(&env);
    let eth = Symbol::new(&env, "ETH");

    let expected: i128 = 1_500_0000000;
    mock.set_price(&eth, &expected);

    let price = oracle.get_price(&eth);
    assert_eq!(
        price, expected,
        "first get_price call for a symbol must fetch from sources and \
         return the mock oracle price"
    );
}

// ---------------------------------------------------------------------------
// 2.5 — Source fetch path
// ---------------------------------------------------------------------------

/// With a single primary source configured, `get_price` must return exactly
/// the price reported by that source.  Validates the single-source path does
/// not mutate, average, or otherwise alter the price.
///
/// This test FAILS until `get_price` is implemented.
#[test]
fn test_get_price_fetches_from_single_source() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, mock, _admin) = deploy_with_price_feed(&env);
    let eth = Symbol::new(&env, "ETH");

    let expected: i128 = 2_500_0000000;
    mock.set_price(&eth, &expected);

    let price = oracle.get_price(&eth);
    assert_eq!(
        price, expected,
        "single source get_price must return the exact price from that source"
    );
}

/// Synonym for `test_get_price_fetches_from_single_source` with a different
/// price value, explicitly confirming the returned value is the source price.
///
/// This test FAILS until `get_price` is implemented.
#[test]
fn test_get_price_single_source_price_is_returned() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, mock, _admin) = deploy_with_price_feed(&env);
    let eth = Symbol::new(&env, "ETH");

    let expected: i128 = 42_0000000; // 42.0000000 (unusual value to catch aliasing)
    mock.set_price(&eth, &expected);

    assert_eq!(
        oracle.get_price(&eth),
        expected,
        "get_price must return the exact i128 price value that the source provides"
    );
}

/// If no primary sources are configured for a symbol, `get_price` must panic
/// with `OracleRouterError::NoPriceSources` (discriminant 6).
///
/// This guards against the fetch path silently returning 0 or panicking with
/// an unexpected host-level error when the source list is empty.
///
/// This test FAILS until `get_price` is implemented.
#[test]
fn test_get_price_no_sources_returns_no_price_sources_error() {
    let env = Env::default();
    env.mock_all_auths();

    // Deploy a fully initialized router with config, but register NO sources for BTC.
    let (oracle, _cm, admin) = deploy_with_config_manager(&env);
    let config = OracleConfig {
        max_deviation_bps: 200,
        staleness_threshold: 60,
        cache_duration: 10,
        min_required_sources: 2,
    };
    oracle.set_oracle_config(&admin, &config);

    // BTC has never had set_oracle_sources called — source list is empty.
    let btc = Symbol::new(&env, "BTC");
    let result = oracle.try_get_price(&btc);

    assert!(
        result.is_err(),
        "get_price with no sources configured must return an error, not 0 or default"
    );
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(OracleRouterError::NoPriceSources as u32),
        "get_price with no sources must return NoPriceSources (6)"
    );
}

/// Verify the NoPriceSources error discriminant is exactly 6.
/// This prevents accidental renumbering from breaking on-chain error matching.
#[test]
fn test_no_price_sources_error_code_is_6() {
    assert_eq!(
        OracleRouterError::NoPriceSources as u32,
        6,
        "OracleRouterError::NoPriceSources must always be discriminant 6"
    );
}

/// When all primary sources have a `last_update` that is older than
/// `staleness_threshold` seconds ago, `get_price` must panic with
/// `OracleRouterError::StalePrice` (discriminant 4).
///
/// Setup: staleness_threshold = 60 seconds. We advance the ledger by 61 seconds
/// AFTER setting the price, so that `last_update = 0` and
/// `current_time - last_update = 61 > 60`.
///
/// This test FAILS until `get_price` is implemented.
#[test]
fn test_get_price_all_sources_stale_returns_stale_price_error() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, mock, _admin) = deploy_with_price_feed(&env);
    let eth = Symbol::new(&env, "ETH");

    // Set price at t=0. last_update will be the current ledger timestamp.
    mock.set_price(&eth, &3_000_0000000i128);

    // Advance time by 61 seconds — past the 60-second staleness_threshold.
    env.ledger().with_mut(|li| {
        li.timestamp += 61;
    });

    // The single source is now stale — all sources stale → StalePrice.
    let result = oracle.try_get_price(&eth);

    assert!(
        result.is_err(),
        "get_price when all sources are stale must return an error"
    );
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(OracleRouterError::StalePrice as u32),
        "all-stale sources must return StalePrice (4)"
    );
}

/// Verify the StalePrice error discriminant is exactly 4.
#[test]
fn test_stale_price_error_code_is_4() {
    assert_eq!(
        OracleRouterError::StalePrice as u32,
        4,
        "OracleRouterError::StalePrice must always be discriminant 4"
    );
}

/// When one of two sources is stale but the other is fresh, the stale source
/// must be silently filtered out and the fresh source's price must be returned.
///
/// Setup: two mock oracles. Advance time 61 seconds, then set a fresh price
/// on the second oracle (which updates its last_update to the NEW timestamp).
/// The first oracle's price was set at t=0 and is now stale.
///
/// This test FAILS until `get_price` is implemented.
#[test]
fn test_get_price_stale_source_filtered_if_fresh_source_exists() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, admin) = deploy_with_config_manager(&env);
    let eth = Symbol::new(&env, "ETH");

    let config = OracleConfig {
        max_deviation_bps: 500, // 5% — generous threshold for this test
        staleness_threshold: 60,
        cache_duration: 10,
        min_required_sources: 2,
    };
    oracle.set_oracle_config(&admin, &config);

    // Deploy three independent mock oracles: one stale, two fresh (the quorum
    // floor requires two valid sources after the stale one is filtered).
    let stale_oracle = deploy_mock_oracle(&env);
    let fresh_a = deploy_mock_oracle(&env);
    let fresh_b = deploy_mock_oracle(&env);

    // Set stale_oracle price at t=0.
    stale_oracle.set_price(&eth, &2_000_0000000i128);

    let primary = vec![
        &env,
        stale_oracle.address.clone(),
        fresh_a.address.clone(),
        fresh_b.address.clone(),
    ];
    oracle.set_oracle_sources(&admin, &eth, &primary);

    // Advance time past staleness_threshold — stale_oracle is now stale.
    env.ledger().with_mut(|li| {
        li.timestamp += 61;
    });

    // Both fresh sources set their price AFTER the time advance — last_update
    // is now current. They agree, so the median is exactly the fresh price.
    let fresh_price: i128 = 2_000_0000000;
    fresh_a.set_price(&eth, &fresh_price);
    fresh_b.set_price(&eth, &fresh_price);

    // get_price must filter out stale_oracle and return the fresh median.
    let price = oracle.get_price(&eth);
    assert_eq!(
        price, fresh_price,
        "get_price must use the fresh sources' median when one source is stale; \
         the stale source must be silently discarded"
    );
}

/// For three sources with prices [1000, 2000, 3000] (sorted), the median must
/// be 2000 (the middle element).
///
/// This test validates that:
///   1. All three sources are aggregated
///   2. The sort is correct (no partial-sort bug)
///   3. The middle element is selected for odd counts
///
/// This test FAILS until `get_price` is implemented.
#[test]
fn test_get_price_computes_median_of_three_sources() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, admin) = deploy_with_config_manager(&env);
    let eth = Symbol::new(&env, "ETH");

    // Use a generous deviation threshold so spread is not rejected.
    let config = OracleConfig {
        max_deviation_bps: 10_000, // 100% — won't reject any spread in this test
        staleness_threshold: 60,
        cache_duration: 10,
        min_required_sources: 2,
    };
    oracle.set_oracle_config(&admin, &config);

    let oracle_low = deploy_mock_oracle(&env);
    let oracle_mid = deploy_mock_oracle(&env);
    let oracle_high = deploy_mock_oracle(&env);

    let price_low: i128 = 1_000_0000000;
    let price_mid: i128 = 2_000_0000000;
    let price_high: i128 = 3_000_0000000;

    oracle_low.set_price(&eth, &price_low);
    oracle_mid.set_price(&eth, &price_mid);
    oracle_high.set_price(&eth, &price_high);

    // Register all three (in unsorted order to validate the sort).
    let primary = vec![
        &env,
        oracle_high.address.clone(),
        oracle_low.address.clone(),
        oracle_mid.address.clone(),
    ];
    oracle.set_oracle_sources(&admin, &eth, &primary);

    let price = oracle.get_price(&eth);
    assert_eq!(
        price, price_mid,
        "median of [1000, 2000, 3000] must be 2000; sort or median selection is \
         incorrect if a different value is returned"
    );
}

/// For four sources with prices [1000, 2000, 3000, 4000] (sorted), the median
/// must be the AVERAGE of the two middle elements: (2000 + 3000) / 2 = 2500.
/// Averaging avoids the systematic low bias a lower-median pick would impose
/// on even-count feeds.
#[test]
fn test_get_price_computes_averaged_median_for_even_count() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, admin) = deploy_with_config_manager(&env);
    let eth = Symbol::new(&env, "ETH");

    let config = OracleConfig {
        max_deviation_bps: 10_000, // 100% — won't reject any spread in this test
        staleness_threshold: 60,
        cache_duration: 10,
        min_required_sources: 2,
    };
    oracle.set_oracle_config(&admin, &config);

    let o1 = deploy_mock_oracle(&env);
    let o2 = deploy_mock_oracle(&env);
    let o3 = deploy_mock_oracle(&env);
    let o4 = deploy_mock_oracle(&env);

    o1.set_price(&eth, &4_000_0000000i128);
    o2.set_price(&eth, &1_000_0000000i128);
    o3.set_price(&eth, &3_000_0000000i128);
    o4.set_price(&eth, &2_000_0000000i128);

    let primary = vec![
        &env,
        o1.address.clone(),
        o2.address.clone(),
        o3.address.clone(),
        o4.address.clone(),
    ];
    oracle.set_oracle_sources(&admin, &eth, &primary);

    let price = oracle.get_price(&eth);
    assert_eq!(
        price, 2_500_0000000i128,
        "averaged median of [1000, 2000, 3000, 4000] must be (2000 + 3000) / 2 = 2500"
    );
}

/// When the spread between max and min prices exceeds `max_deviation_bps`, the
/// contract must panic with `OracleRouterError::PriceDeviationTooHigh` (5).
///
/// Calculation: deviation_bps = (max - min) * 10_000 / median
///   prices = [1000, 2000], median = 1000 (lower median for 2 sources)
///   deviation_bps = (2000 - 1000) * 10_000 / 1000 = 10_000 bps (100%)
///   With max_deviation_bps = 200 (2%), this must be rejected.
///
/// This test FAILS until `get_price` is implemented.
#[test]
fn test_get_price_high_deviation_returns_deviation_error() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, admin) = deploy_with_config_manager(&env);
    let eth = Symbol::new(&env, "ETH");

    let config = OracleConfig {
        max_deviation_bps: 200, // 2% maximum allowed spread
        staleness_threshold: 60,
        cache_duration: 10,
        min_required_sources: 2,
    };
    oracle.set_oracle_config(&admin, &config);

    let oracle_low = deploy_mock_oracle(&env);
    let oracle_high = deploy_mock_oracle(&env);

    // Price spread: 50% — far above the 2% threshold.
    oracle_low.set_price(&eth, &1_000_0000000i128);
    oracle_high.set_price(&eth, &1_500_0000000i128);

    let primary = vec![
        &env,
        oracle_low.address.clone(),
        oracle_high.address.clone(),
    ];
    oracle.set_oracle_sources(&admin, &eth, &primary);

    let result = oracle.try_get_price(&eth);

    assert!(
        result.is_err(),
        "get_price must return an error when price deviation exceeds max_deviation_bps"
    );
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(OracleRouterError::PriceDeviationTooHigh as u32),
        "excessive price spread must return PriceDeviationTooHigh (5)"
    );
}

/// Verify the PriceDeviationTooHigh error discriminant is exactly 5.
#[test]
fn test_price_deviation_error_code_is_5() {
    assert_eq!(
        OracleRouterError::PriceDeviationTooHigh as u32,
        5,
        "OracleRouterError::PriceDeviationTooHigh must always be discriminant 5"
    );
}

/// When the spread between max and min prices is within `max_deviation_bps`,
/// `get_price` must succeed and return the median.
///
/// Calculation: prices = [99, 100], lower median = 99
///   deviation_bps = (100 - 99) * 10_000 / 99 ≈ 101 bps (1.01%)
///   With max_deviation_bps = 200 (2%), this must be accepted.
///
/// This test FAILS until `get_price` is implemented.
#[test]
fn test_get_price_deviation_within_threshold_succeeds() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, admin) = deploy_with_config_manager(&env);
    let eth = Symbol::new(&env, "ETH");

    let config = OracleConfig {
        max_deviation_bps: 200, // 2%
        staleness_threshold: 60,
        cache_duration: 10,
        min_required_sources: 2,
    };
    oracle.set_oracle_config(&admin, &config);

    let oracle_a = deploy_mock_oracle(&env);
    let oracle_b = deploy_mock_oracle(&env);

    // 1% spread — within 2% threshold.
    oracle_a.set_price(&eth, &100_0000000i128); // 100.0000000
    oracle_b.set_price(&eth, &101_0000000i128); // 101.0000000

    let primary = vec![&env, oracle_a.address.clone(), oracle_b.address.clone()];
    oracle.set_oracle_sources(&admin, &eth, &primary);

    let result = oracle.try_get_price(&eth);
    assert!(
        result.is_ok(),
        "get_price must succeed when deviation is within the allowed threshold; \
         got error: {:?}",
        result.err()
    );
}

/// Two successive `get_price` calls with the upstream price unchanged must
/// return the same value. The router has no cache — both calls refetch —
/// but the value should be stable as long as the source is.
#[test]
fn test_get_price_consecutive_calls_return_same_value() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, mock, _admin) = deploy_with_price_feed(&env);
    let eth = Symbol::new(&env, "ETH");

    let expected: i128 = 3_500_0000000;
    mock.set_price(&eth, &expected);

    let first_price = oracle.get_price(&eth);
    assert_eq!(
        first_price, expected,
        "first call must return the mock price"
    );

    let second_price = oracle.get_price(&eth);
    assert_eq!(
        second_price, first_price,
        "consecutive get_price calls with unchanged source must return the same value"
    );
}

/// Calling `get_price` on an initialized router where `set_oracle_config` has
/// NEVER been called must panic with `OracleRouterError::NotInitialized` (2).
///
/// The contract must check for OracleConfig presence before reading sources
/// or calling any oracle, since the staleness_threshold and cache_duration
/// are required for any validation step.
///
/// This test FAILS until `get_price` is implemented.
#[test]
fn test_get_price_no_oracle_config_returns_not_initialized() {
    let env = Env::default();
    env.mock_all_auths();

    // deploy_with_config_manager sets up the CM + router but does NOT call
    // set_oracle_config, so OracleConfig is absent from instance storage.
    let (oracle, _cm, _admin) = deploy_with_config_manager(&env);

    let eth = Symbol::new(&env, "ETH");
    let result = oracle.try_get_price(&eth);

    assert!(
        result.is_err(),
        "get_price with no OracleConfig set must return an error"
    );
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(OracleRouterError::NotInitialized as u32),
        "missing OracleConfig must return NotInitialized (2), not a different error or panic"
    );
}

// ---------------------------------------------------------------------------
// Adversarial: math safety and boundary conditions
// ---------------------------------------------------------------------------

/// Deviation check with prices at exact boundary: deviation_bps == max_deviation_bps
/// must be ACCEPTED (not-greater-than comparison: deviation > threshold → reject).
///
/// If the implementation uses `>=` instead of `>`, this test will catch it.
///
/// Calculation: prices = [49, 51], averaged median = 50
///   deviation_bps = (51 - 50) * 10_000 / 50 = 200 bps
///   With max_deviation_bps = 200: 200 > 200 is false → ACCEPT
#[test]
fn test_get_price_deviation_exactly_at_threshold_is_accepted() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, admin) = deploy_with_config_manager(&env);
    let eth = Symbol::new(&env, "ETH");

    let config = OracleConfig {
        max_deviation_bps: 200, // exactly 2%
        staleness_threshold: 60,
        cache_duration: 10,
        min_required_sources: 2,
    };
    oracle.set_oracle_config(&admin, &config);

    let oracle_a = deploy_mock_oracle(&env);
    let oracle_b = deploy_mock_oracle(&env);

    // averaged median = 50; deviation = (51 - 50) * 10_000 / 50 = 200 bps exactly
    oracle_a.set_price(&eth, &49_0000000i128);
    oracle_b.set_price(&eth, &51_0000000i128);

    let primary = vec![&env, oracle_a.address.clone(), oracle_b.address.clone()];
    oracle.set_oracle_sources(&admin, &eth, &primary);

    let result = oracle.try_get_price(&eth);
    assert!(
        result.is_ok(),
        "deviation exactly equal to max_deviation_bps must be accepted (not rejected); \
         the check must use > not >=; error: {:?}",
        result.err()
    );
}

/// Deviation check with prices above the threshold must be REJECTED.
///
/// Calculation: prices = [4900, 5102], averaged median = 5001
///   deviation_bps = (5102 - 5001) * 10_000 / 5001 = 201 bps (above 200)
#[test]
fn test_get_price_deviation_one_bps_above_threshold_is_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, admin) = deploy_with_config_manager(&env);
    let eth = Symbol::new(&env, "ETH");

    let config = OracleConfig {
        max_deviation_bps: 200,
        staleness_threshold: 60,
        cache_duration: 10,
        min_required_sources: 2,
    };
    oracle.set_oracle_config(&admin, &config);

    let oracle_a = deploy_mock_oracle(&env);
    let oracle_b = deploy_mock_oracle(&env);

    // averaged median = 5001; deviation = (5102 - 5001) * 10_000 / 5001 = 201 bps
    oracle_a.set_price(&eth, &4_900_0000000i128);
    oracle_b.set_price(&eth, &5_102_0000000i128);

    let primary = vec![&env, oracle_a.address.clone(), oracle_b.address.clone()];
    oracle.set_oracle_sources(&admin, &eth, &primary);

    let result = oracle.try_get_price(&eth);
    assert!(
        result.is_err(),
        "deviation 1 bps above the threshold must be rejected"
    );
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(OracleRouterError::PriceDeviationTooHigh as u32),
        "deviation 1 bps above threshold must return PriceDeviationTooHigh (5)"
    );
}

/// Staleness boundary: a source whose `last_update` is EXACTLY at the
/// staleness boundary (current_time - last_update == staleness_threshold)
/// must be treated as FRESH (not stale).
///
/// This verifies the staleness check uses `>` not `>=`.
///
/// This test FAILS until `get_price` is implemented.
#[test]
fn test_get_price_source_at_exact_staleness_boundary_is_fresh() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, mock, _admin) = deploy_with_price_feed(&env);
    // deploy_with_price_feed uses staleness_threshold = 60.
    let eth = Symbol::new(&env, "ETH");

    // Set price at t=0 (last_update will be 0).
    mock.set_price(&eth, &3_000_0000000i128);

    // Advance exactly to the boundary: current_time - last_update == 60.
    env.ledger().with_mut(|li| {
        li.timestamp = 60;
    });

    // At the boundary, the source must be considered FRESH, not stale.
    let result = oracle.try_get_price(&eth);
    assert!(
        result.is_ok(),
        "a source at the exact staleness boundary (age == threshold) must be treated \
         as fresh; got error: {:?}",
        result.err()
    );
}

/// Staleness boundary: one second past the threshold must be rejected.
///
/// This test FAILS until `get_price` is implemented.
#[test]
fn test_get_price_source_one_second_past_staleness_boundary_is_stale() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, mock, _admin) = deploy_with_price_feed(&env);
    let eth = Symbol::new(&env, "ETH");

    mock.set_price(&eth, &3_000_0000000i128);

    // Advance one second past the 60-second staleness threshold.
    env.ledger().with_mut(|li| {
        li.timestamp = 61;
    });

    let result = oracle.try_get_price(&eth);
    assert!(
        result.is_err(),
        "a source one second past the staleness threshold must be rejected"
    );
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(OracleRouterError::StalePrice as u32),
        "source one second past staleness boundary must return StalePrice (4)"
    );
}

/// A small time advance with the upstream price changed in between must
/// surface the new price — every `get_price` call refetches.
#[test]
fn test_get_price_picks_up_source_price_change_after_advance() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, mock, _admin) = deploy_with_price_feed(&env);
    let eth = Symbol::new(&env, "ETH");

    let initial_price: i128 = 5_000_0000000;
    mock.set_price(&eth, &initial_price);
    oracle.get_price(&eth);

    env.ledger().with_mut(|li| {
        li.timestamp += 11;
    });

    let fresh_price: i128 = 6_000_0000000;
    mock.set_price(&eth, &fresh_price);

    let price = oracle.get_price(&eth);
    assert_eq!(
        price, fresh_price,
        "every get_price call refetches; the second call must see the fresh price"
    );
}

/// Two independently configured symbols (ETH and BTC) must have completely
/// separate cache entries.  Priming the cache for ETH must not affect the
/// BTC cache or BTC price fetch behavior.
///
/// This test FAILS until `get_price` is implemented.
#[test]
fn test_get_price_cache_is_keyed_per_symbol() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, admin) = deploy_with_config_manager(&env);

    let config = OracleConfig {
        max_deviation_bps: 200,
        staleness_threshold: 60,
        cache_duration: 10,
        min_required_sources: 2,
    };
    oracle.set_oracle_config(&admin, &config);

    let eth = Symbol::new(&env, "ETH");
    let btc = Symbol::new(&env, "BTC");

    let eth_oracle_a = deploy_mock_oracle(&env);
    let eth_oracle_b = deploy_mock_oracle(&env);
    let btc_oracle_a = deploy_mock_oracle(&env);
    let btc_oracle_b = deploy_mock_oracle(&env);

    let eth_price: i128 = 3_000_0000000;
    let btc_price: i128 = 60_000_0000000;

    eth_oracle_a.set_price(&eth, &eth_price);
    eth_oracle_b.set_price(&eth, &eth_price);
    btc_oracle_a.set_price(&btc, &btc_price);
    btc_oracle_b.set_price(&btc, &btc_price);

    oracle.set_oracle_sources(
        &admin,
        &eth,
        &vec![
            &env,
            eth_oracle_a.address.clone(),
            eth_oracle_b.address.clone(),
        ],
    );
    oracle.set_oracle_sources(
        &admin,
        &btc,
        &vec![
            &env,
            btc_oracle_a.address.clone(),
            btc_oracle_b.address.clone(),
        ],
    );

    assert_eq!(
        oracle.get_price(&eth),
        eth_price,
        "get_price for ETH must return the ETH oracle price"
    );
    assert_eq!(
        oracle.get_price(&btc),
        btc_price,
        "get_price for BTC must return the BTC oracle price, not the ETH price"
    );

    // Update ETH price (both sources) and advance time to expire ETH cache.
    env.ledger().with_mut(|li| li.timestamp += 11);
    let new_eth_price: i128 = 4_000_0000000;
    eth_oracle_a.set_price(&eth, &new_eth_price);
    eth_oracle_b.set_price(&eth, &new_eth_price);

    // BTC price should still be cached (no time advancement for BTC cache start).
    // ETH must fetch new price.
    assert_eq!(
        oracle.get_price(&eth),
        new_eth_price,
        "after ETH cache expires, get_price must return the updated ETH price"
    );
}

/// Clearing a symbol's sources drops its cached price immediately, so a
/// disabled feed takes effect at once rather than serving the stale median
/// for up to `cache_duration` seconds. The error fires with NO time advance.
#[test]
fn test_get_price_sources_cleared_invalidates_cache_immediately() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, mock, admin) = deploy_with_price_feed(&env);
    let eth = Symbol::new(&env, "ETH");

    mock.set_price(&eth, &3_000_0000000i128);
    oracle.get_price(&eth); // prime the cache

    // Clear sources for ETH — this also drops the cached price.
    let empty: soroban_sdk::Vec<Address> = vec![&env];
    oracle.set_oracle_sources(&admin, &eth, &empty);

    // No time advance: the cache must already be gone, so the empty source
    // list surfaces NoPriceSources immediately.
    let result = oracle.try_get_price(&eth);
    assert!(
        result.is_err(),
        "get_price after clearing sources must return an error without waiting for cache expiry"
    );
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(OracleRouterError::NoPriceSources as u32),
        "cleared sources must return NoPriceSources (6) immediately"
    );
}

/// Calling `get_price` on a freshly constructed router that has no oracle
/// config set must surface a well-typed `NotInitialized` (2) contract error,
/// not a host-level error.
#[test]
fn test_get_price_with_no_config_returns_not_initialized() {
    let env = Env::default();
    env.mock_all_auths();

    // Construct the router with a config_manager arg but set no oracle config,
    // so the config slot is unset when get_price reads it.
    let config_manager = <soroban_sdk::Address as soroban_sdk::testutils::Address>::generate(&env);
    let oracle_id = env.register(crate::OracleRouterContract, (config_manager,));
    let oracle = crate::OracleRouterClient::new(&env, &oracle_id);

    let eth = Symbol::new(&env, "ETH");
    let result = oracle.try_get_price(&eth);

    assert!(
        result.is_err(),
        "get_price with no oracle config set must return an error"
    );
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(crate::OracleRouterError::NotInitialized as u32),
        "missing oracle config must surface NotInitialized (2)"
    );
}

// ---------------------------------------------------------------------------
// Zero and negative price filtering
// ---------------------------------------------------------------------------
//
// A SEP-40 source returning price <= 0 must be treated as invalid and silently
// filtered out — exactly the same treatment as a stale source. If ALL sources
// return price <= 0 (and are therefore filtered), the valid_prices collection is
// empty and the contract panics with StalePrice (4), the same error used when
// all sources are temporally stale.

/// A primary source that returns a price of exactly zero must be silently
/// filtered out.  If at least one other source returns a valid positive price,
/// `get_price` must succeed and return the valid source's price.
///
/// Adversarial scenario: a misconfigured or manipulated SEP-40 oracle reports
/// price = 0.  Without filtering, the zero price would pull the median down or
/// cause a division-by-zero in the deviation calculation.
///
/// This test FAILS until the implementation filters `price <= 0`.
#[test]
fn test_get_price_zero_price_from_source_is_filtered_out() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, admin) = deploy_with_config_manager(&env);
    let eth = Symbol::new(&env, "ETH");

    // Use a generous deviation threshold so the single valid price is not rejected.
    let config = OracleConfig {
        max_deviation_bps: 500, // 5%
        staleness_threshold: 60,
        cache_duration: 10,
        min_required_sources: 2,
    };
    oracle.set_oracle_config(&admin, &config);

    let zero_oracle = deploy_mock_oracle(&env);
    let valid_a = deploy_mock_oracle(&env);
    let valid_b = deploy_mock_oracle(&env);

    // Zero price — must be filtered, not included in valid_prices.
    zero_oracle.set_price(&eth, &0i128);
    // Two valid positive prices remain after filtering (configured quorum = 2).
    let valid_price: i128 = 2_000_0000000; // 2 000.0000000
    valid_a.set_price(&eth, &valid_price);
    valid_b.set_price(&eth, &valid_price);

    let primary = vec![
        &env,
        zero_oracle.address.clone(),
        valid_a.address.clone(),
        valid_b.address.clone(),
    ];
    oracle.set_oracle_sources(&admin, &eth, &primary);

    // Must return the valid median without panicking with division-by-zero
    // or returning 0 as the median.
    let price = oracle.get_price(&eth);
    assert_eq!(
        price, valid_price,
        "zero-price source must be filtered out; the remaining valid sources' median \
         ({valid_price}) must be returned, not the zero price"
    );
}

/// A primary source that returns a strictly negative price must be silently
/// filtered out.  If at least one other source returns a valid positive price,
/// `get_price` must succeed and return that valid price.
///
/// Adversarial scenario: a compromised oracle reports price = -1 to manipulate
/// liquidation conditions.  Without filtering, a negative price would corrupt
/// the median and deviation calculations.
///
/// This test FAILS until the implementation filters `price <= 0`.
#[test]
fn test_get_price_negative_price_from_source_is_filtered_out() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, admin) = deploy_with_config_manager(&env);
    let eth = Symbol::new(&env, "ETH");

    let config = OracleConfig {
        max_deviation_bps: 500,
        staleness_threshold: 60,
        cache_duration: 10,
        min_required_sources: 2,
    };
    oracle.set_oracle_config(&admin, &config);

    let negative_oracle = deploy_mock_oracle(&env);
    let valid_a = deploy_mock_oracle(&env);
    let valid_b = deploy_mock_oracle(&env);

    // Negative price — must be filtered.
    negative_oracle.set_price(&eth, &-1_0000000i128); // -1.0000000
    let valid_price: i128 = 1_500_0000000; // 1 500.0000000
    valid_a.set_price(&eth, &valid_price);
    valid_b.set_price(&eth, &valid_price);

    let primary = vec![
        &env,
        negative_oracle.address.clone(),
        valid_a.address.clone(),
        valid_b.address.clone(),
    ];
    oracle.set_oracle_sources(&admin, &eth, &primary);

    let price = oracle.get_price(&eth);
    assert_eq!(
        price, valid_price,
        "negative-price source must be filtered out; valid sources' median ({valid_price}) \
         must be returned"
    );
}

/// When ALL configured primary sources return a price of zero, every source is
/// filtered as invalid.  The resulting valid_prices collection is empty, and
/// `get_price` must panic with `OracleRouterError::StalePrice` (4) — the same
/// error used when all sources are temporally stale.
///
/// Rationale: "no valid prices" and "all prices stale" are operationally
/// equivalent from the consumer's perspective and must share the same error code.
///
/// This test FAILS until the implementation filters `price <= 0`.
#[test]
fn test_get_price_all_sources_return_zero_panics_with_stale_price() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, admin) = deploy_with_config_manager(&env);
    let eth = Symbol::new(&env, "ETH");

    let config = OracleConfig {
        max_deviation_bps: 500,
        staleness_threshold: 60,
        cache_duration: 10,
        min_required_sources: 2,
    };
    oracle.set_oracle_config(&admin, &config);

    let zero_oracle_a = deploy_mock_oracle(&env);
    let zero_oracle_b = deploy_mock_oracle(&env);

    // Both sources return zero — both must be filtered, leaving valid_prices empty.
    zero_oracle_a.set_price(&eth, &0i128);
    zero_oracle_b.set_price(&eth, &0i128);

    let primary = vec![
        &env,
        zero_oracle_a.address.clone(),
        zero_oracle_b.address.clone(),
    ];
    oracle.set_oracle_sources(&admin, &eth, &primary);

    let result = oracle.try_get_price(&eth);

    assert!(
        result.is_err(),
        "get_price when all sources return zero must return an error (no valid prices \
         remain after filtering)"
    );
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(OracleRouterError::StalePrice as u32),
        "all-zero sources must return StalePrice (4), same as all-stale — \
         do not introduce a new error code"
    );
}

/// When ALL configured primary sources return negative prices, every source is
/// filtered as invalid.  The resulting valid_prices collection is empty, and
/// `get_price` must panic with `OracleRouterError::StalePrice` (4).
///
/// This test FAILS until the implementation filters `price <= 0`.
#[test]
fn test_get_price_all_sources_return_negative_panics_with_stale_price() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, admin) = deploy_with_config_manager(&env);
    let eth = Symbol::new(&env, "ETH");

    let config = OracleConfig {
        max_deviation_bps: 500,
        staleness_threshold: 60,
        cache_duration: 10,
        min_required_sources: 2,
    };
    oracle.set_oracle_config(&admin, &config);

    let neg_oracle_a = deploy_mock_oracle(&env);
    let neg_oracle_b = deploy_mock_oracle(&env);
    let neg_oracle_c = deploy_mock_oracle(&env);

    // Use varied negative values to ensure the filter does not special-case -1.
    neg_oracle_a.set_price(&eth, &-1i128);
    neg_oracle_b.set_price(&eth, &-1_000_0000000i128); // large magnitude
    neg_oracle_c.set_price(&eth, &i128::MIN); // minimum i128 — extreme adversarial value

    let primary = vec![
        &env,
        neg_oracle_a.address.clone(),
        neg_oracle_b.address.clone(),
        neg_oracle_c.address.clone(),
    ];
    oracle.set_oracle_sources(&admin, &eth, &primary);

    let result = oracle.try_get_price(&eth);

    assert!(
        result.is_err(),
        "get_price when all sources return negative prices must return an error"
    );
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(OracleRouterError::StalePrice as u32),
        "all-negative sources must return StalePrice (4)"
    );
}

/// When some sources return zero or negative prices and at least one source
/// returns a valid positive price, `get_price` must use ONLY the valid sources
/// for median and deviation calculation.
///
/// This test validates that the filter correctly partitions the source list:
/// invalid prices are discarded entirely rather than treated as 0 in the sort.
///
/// Setup: three sources — [0, -500, 2000].  After filtering, valid_prices = [2000].
/// The median of a single-element list is that element: 2000.
///
/// This test FAILS until the implementation filters `price <= 0`.
#[test]
fn test_get_price_mix_of_zero_and_valid_prices_uses_valid_only() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, admin) = deploy_with_config_manager(&env);
    let eth = Symbol::new(&env, "ETH");

    let config = OracleConfig {
        max_deviation_bps: 500,
        staleness_threshold: 60,
        cache_duration: 10,
        min_required_sources: 2,
    };
    oracle.set_oracle_config(&admin, &config);

    let zero_oracle = deploy_mock_oracle(&env);
    let neg_oracle = deploy_mock_oracle(&env);
    let valid_a = deploy_mock_oracle(&env);
    let valid_b = deploy_mock_oracle(&env);

    zero_oracle.set_price(&eth, &0i128);
    neg_oracle.set_price(&eth, &-500_0000000i128);
    let valid_price: i128 = 2_000_0000000;
    valid_a.set_price(&eth, &valid_price);
    valid_b.set_price(&eth, &valid_price);

    // Register in order: zero, negative, valid, valid — verifies filtering is
    // not order-dependent and two valid sources survive (configured quorum = 2).
    let primary = vec![
        &env,
        zero_oracle.address.clone(),
        neg_oracle.address.clone(),
        valid_a.address.clone(),
        valid_b.address.clone(),
    ];
    oracle.set_oracle_sources(&admin, &eth, &primary);

    // Expected: median([2000, 2000]) = 2000 (only the valid sources survive).
    let price = oracle.get_price(&eth);
    assert_eq!(
        price, valid_price,
        "with sources [0, -500, 2000, 2000], only the 2000s survive filtering; \
         their median = 2000; zero and negative sources must be completely discarded"
    );
}

/// When a source returns price = 0, the deviation calculation must NOT be
/// reached with 0 as a participant.  Specifically, if the implementation
/// included 0 in valid_prices, the deviation check would compute:
///   upper_dev = (2000 - 0) * 10_000 / 0  → division by zero
///
/// This test guarantees that the `price <= 0` filter prevents the contract
/// from ever performing arithmetic on a zero price, protecting against a
/// host-level panic in the deviation step.
///
/// The test passes as soon as zero prices are filtered; it should NOT panic
/// with any host error — it must return the valid source's price cleanly.
///
/// This test FAILS until the implementation filters `price <= 0`.
#[test]
fn test_get_price_zero_price_does_not_cause_division_by_zero() {
    let env = Env::default();
    env.mock_all_auths();

    let (oracle, _cm, admin) = deploy_with_config_manager(&env);
    let eth = Symbol::new(&env, "ETH");

    // A tight deviation threshold to confirm the deviation step is reached
    // safely (with only valid prices).  If a zero price reached the deviation
    // step, the division by zero would occur before any threshold check.
    let config = OracleConfig {
        max_deviation_bps: 100, // 1%
        staleness_threshold: 60,
        cache_duration: 10,
        min_required_sources: 2,
    };
    oracle.set_oracle_config(&admin, &config);

    // Single source returning zero — the ONLY source. After filtering it out,
    // valid_prices is empty → StalePrice, NOT a host arithmetic panic.
    let zero_oracle = deploy_mock_oracle(&env);
    zero_oracle.set_price(&eth, &0i128);

    let primary = vec![&env, zero_oracle.address.clone()];
    oracle.set_oracle_sources(&admin, &eth, &primary);

    // The contract MUST return a contract-level error (StalePrice), not a
    // host-level arithmetic trap.  A host trap would cause the Err to be an
    // InvokeError::Abort rather than a contract error, failing the downcast.
    let result = oracle.try_get_price(&eth);

    assert!(
        result.is_err(),
        "zero price from the only source must result in an error, not a successful return"
    );
    // Confirm this is a clean contract error, not a host arithmetic panic.
    // unwrap_err() gives InvokeError; unwrap() on that extracts the soroban_sdk::Error.
    // If the host panicked with a divide-by-zero, the inner unwrap() would fail.
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(OracleRouterError::StalePrice as u32),
        "a sole zero-price source must return StalePrice (4), not crash with a \
         host arithmetic trap — confirms division-by-zero is impossible"
    );
}

// ---------------------------------------------------------------------------
// Broken oracle source isolation
//
// `get_price` uses `client.try_get_price(&symbol)` and
// `client.try_last_update(&symbol)` so a panicking source is skipped rather
// than aborting the entire transaction.
// ---------------------------------------------------------------------------

/// A source that panics on `get_price` (no price has been set in MockOracle)
/// must be silently skipped when at least one other source returns a valid,
/// fresh price.
///
/// Setup:
///   - oracle_a: MockOracle with NO price set → panics on `get_price`
///   - oracle_b: MockOracle with a valid price (1_000_0000000) at timestamp 100
///   - Both registered as primary sources for "ETH"
///
/// Expected after fix: `get_price` returns oracle_b's price (1_000_0000000).
///
/// Currently FAILS: the bare `client.get_price` call against oracle_a causes
/// a host-level panic that aborts the entire transaction before oracle_b is
/// ever queried.
#[test]
fn test_get_price_broken_source_is_skipped_if_other_sources_valid() {
    let env = Env::default();
    env.mock_all_auths();

    // Set ledger timestamp to 100 so freshness checks are deterministic.
    env.ledger().set_timestamp(100);

    let (oracle, _cm, admin) = deploy_with_config_manager(&env);

    // staleness_threshold = 60: a source updated at t=100 has age 0 → fresh.
    // cache_duration = 10: cache is inactive because we do not warm it.
    // max_deviation_bps = 10_000: very permissive — single source, no spread.
    let config = OracleConfig {
        max_deviation_bps: 10_000,
        staleness_threshold: 60,
        cache_duration: 10,
        min_required_sources: 2,
    };
    oracle.set_oracle_config(&admin, &config);

    let eth = Symbol::new(&env, "ETH");

    // oracle_a: intentionally left with no price set.
    // MockOracle::get_price panics with "no price set" when called without a
    // prior set_price call.  This is the "broken source" that the fix must
    // catch and skip.
    let oracle_a = deploy_mock_oracle(&env);

    // oracle_b, oracle_c: valid prices at current timestamp → fresh. Two valid
    // sources are needed to meet the configured quorum after the broken one is skipped.
    let oracle_b = deploy_mock_oracle(&env);
    let oracle_c = deploy_mock_oracle(&env);
    let valid_price: i128 = 1_000_0000000; // 1000.0000000 (7-decimal scaled)
    oracle_b.set_price(&eth, &valid_price);
    oracle_c.set_price(&eth, &valid_price);

    // Register all three: oracle_a first (broken), then two valid sources.
    // The implementation must iterate all, skip oracle_a's panic, and use the rest.
    let primary = vec![
        &env,
        oracle_a.address.clone(),
        oracle_b.address.clone(),
        oracle_c.address.clone(),
    ];
    oracle.set_oracle_sources(&admin, &eth, &primary);

    // oracle_a's bare panic must be caught via try-variant calls and skipped;
    // the two valid sources provide the median.
    let price = oracle.get_price(&eth);

    assert_eq!(
        price, valid_price,
        "get_price must return the valid sources' median ({}) even though oracle_a panics; \
         broken sources must be caught via try-variant calls and skipped, not propagated",
        valid_price
    );
}

/// When every registered oracle source panics (i.e., no oracle has a price
/// set), `get_price` must return a clean contract-level error — either
/// `StalePrice` (4) or `PriceFetchFailed` (7) — rather than propagating a
/// host-level `InvokeError::Abort`.
///
/// Setup:
///   - oracle_a: MockOracle with NO price set → panics on `get_price`
///   - oracle_a is the only registered primary source for "ETH"
///
/// Expected after fix: `try_get_price` returns
///   `Err(Ok(soroban_sdk::Error::from_contract_error(4)))` — a typed
///   contract error.
///
/// Currently FAILS: the bare `client.get_price` call causes a host-level
/// abort that the Soroban test harness surfaces as `Err(Err(InvokeError))`.
/// The inner `unwrap()` in the assertion below then panics at the test level,
/// making the test itself crash rather than asserting a clean contract error.
#[test]
fn test_get_price_all_sources_broken_returns_clean_error() {
    let env = Env::default();
    env.mock_all_auths();

    env.ledger().set_timestamp(100);

    let (oracle, _cm, admin) = deploy_with_config_manager(&env);

    let config = OracleConfig {
        max_deviation_bps: 10_000,
        staleness_threshold: 60,
        cache_duration: 10,
        min_required_sources: 2,
    };
    oracle.set_oracle_config(&admin, &config);

    let eth = Symbol::new(&env, "ETH");

    // The sole registered oracle has NO price set — it panics on get_price.
    let broken_oracle = deploy_mock_oracle(&env);
    // Intentionally do NOT call broken_oracle.set_price(...).

    let primary = vec![&env, broken_oracle.address.clone()];
    oracle.set_oracle_sources(&admin, &eth, &primary);

    // Use try_get_price so the test can inspect the error type without the
    // test runner itself unwinding from a panic.
    let result = oracle.try_get_price(&eth);

    assert!(
        result.is_err(),
        "get_price with every source broken must return an error, not a price"
    );

    // The critical assertion: the error must be a clean, typed contract error
    // (Err(Ok(soroban_sdk::Error))), NOT a host-level abort (Err(Err(...))).
    //
    // On the unfixed implementation, `result.unwrap_err().unwrap()` panics at
    // the test level because `unwrap_err()` gives `Err(InvokeError::Abort)`
    // and the subsequent `unwrap()` on the InvokeError fails.
    //
    // After the fix, this downcast succeeds and we can inspect the error code.
    let contract_error = result.unwrap_err().expect(
        "get_price with all sources broken must produce a clean contract error \
             (Err(Ok(...))), not a host-level InvokeError::Abort — the fix must use \
             try-variant cross-contract calls to catch panicking sources",
    );

    // Accept either StalePrice (all try-call results skipped → no valid prices)
    // or PriceFetchFailed (explicit error for failed cross-contract calls).
    // Both are clean contract errors that the caller can handle gracefully.
    let is_acceptable_error = contract_error
        == soroban_sdk::Error::from_contract_error(OracleRouterError::StalePrice as u32)
        || contract_error
            == soroban_sdk::Error::from_contract_error(OracleRouterError::PriceFetchFailed as u32);

    assert!(
        is_acceptable_error,
        "expected StalePrice (4) or PriceFetchFailed (7) when all sources panic, \
         but got: {:?}. The error must be a typed contract error the caller can \
         pattern-match on, not an opaque host abort.",
        contract_error
    );
}
