use shared::bump_instance_ttl;
use shared::constants::{
    BPS, MAX_DEVIATION_BPS_CEILING, MAX_ORACLE_SOURCES, MIN_REQUIRED_SOURCES_FLOOR, PRICE_DECIMALS,
    ROLE_ADMIN, ROLE_KEEPER, ROLE_PAUSER, ROLE_UPGRADER,
};
use shared::OracleClient;
use soroban_sdk::{panic_with_error, Address, Env, Symbol, Vec};

use crate::errors::OracleRouterError;
use crate::events;
use crate::storage;
use crate::types::OracleConfig;

/// Require `caller` to be authenticated and hold `role` in the linked
/// ConfigManager. Panics with `OracleRouterError::Unauthorized` (code 3) on
/// failure so the panic code identifies the source contract.
fn require_role_or_panic(env: &Env, caller: &Address, role: &str) {
    caller.require_auth();
    let cm = storage::load_config_manager(env);
    if !shared::has_role(env, &cm, role, caller) {
        panic_with_error!(env, OracleRouterError::Unauthorized);
    }
}

/// Require that `caller` holds the "ADMIN" role in the linked ConfigManager.
pub fn require_oracle_admin(env: &Env, caller: &Address) {
    require_role_or_panic(env, caller, ROLE_ADMIN);
}

pub fn require_keeper(env: &Env, caller: &Address) {
    require_role_or_panic(env, caller, ROLE_KEEPER);
}

/// Require that `caller` holds the "UPGRADER" role in the linked ConfigManager.
pub fn require_upgrader(env: &Env, caller: &Address) {
    require_role_or_panic(env, caller, ROLE_UPGRADER);
}

/// Require that `caller` holds the "PAUSER" role in the linked ConfigManager —
/// used by the `cancel_upgrade` veto path. Distinct getter so the caller's
/// intent ("PAUSER for upgrade veto", not generic pause) is clear.
pub fn require_pauser_for_upgrade(env: &Env, caller: &Address) {
    require_role_or_panic(env, caller, ROLE_PAUSER);
}

/// Bounds-validation surface for OracleConfig, mirroring the `Validate`
/// pattern used in `config-manager/src/validate.rs`. Implemented locally —
/// orphan rule prevents adding `impl` blocks on `shared::OracleConfig`
/// directly.
pub trait Validate {
    /// Panics with `OracleRouterError::InvalidConfig` on failure; returns
    /// normally otherwise.
    fn validate(&self, env: &Env);
}

impl Validate for OracleConfig {
    fn validate(&self, env: &Env) {
        if self.max_deviation_bps <= 0 || self.max_deviation_bps > MAX_DEVIATION_BPS_CEILING {
            panic_with_error!(env, OracleRouterError::InvalidConfig);
        }
        if self.staleness_threshold == 0 {
            panic_with_error!(env, OracleRouterError::InvalidConfig);
        }
        // Cache must not outlive the staleness window — otherwise a cached
        // price could be served after its underlying source feed has gone
        // stale.
        if self.cache_duration == 0 || self.cache_duration > self.staleness_threshold {
            panic_with_error!(env, OracleRouterError::InvalidConfig);
        }
        if self.min_required_sources < MIN_REQUIRED_SOURCES_FLOOR
            || self.min_required_sources > MAX_ORACLE_SOURCES
        {
            panic_with_error!(env, OracleRouterError::InvalidConfig);
        }
    }
}

/// Query every source, returning the prices that pass freshness, sign, and
/// future-timestamp checks plus the oldest source timestamp among those
/// accepted prices. Try-variants ensure a broken source is skipped rather
/// than aborting the whole call.
pub fn query_sources(
    env: &Env,
    sources: &Vec<Address>,
    symbol: &Symbol,
    config: &OracleConfig,
    current_time: u64,
) -> (Vec<i128>, Option<u64>) {
    let mut valid_prices: Vec<i128> = Vec::new(env);
    let mut oldest_source_update: Option<u64> = None;
    for source in sources.iter() {
        let client = OracleClient::new(env, &source);
        let price = match client.try_get_price(symbol) {
            Ok(Ok(p)) => p,
            _ => continue,
        };
        let last_update = match client.try_last_update(symbol) {
            Ok(Ok(ts)) => ts,
            _ => continue,
        };
        // Future-dated timestamps are rejected outright — prevents a source
        // from masquerading as perpetually fresh.
        if last_update > current_time {
            continue;
        }
        if current_time - last_update > config.staleness_threshold || price <= 0 {
            continue;
        }
        // Reject prices large enough to overflow the deviation math downstream
        // (`(max - median) * BPS`). A genuine price scaled by PRECISION is many
        // orders of magnitude below this bound, so a source returning one is
        // malfunctioning or hostile — drop it as an invalid response rather
        // than letting one source abort the whole fetch for every other.
        if price > i128::MAX / BPS {
            continue;
        }
        oldest_source_update = Some(match oldest_source_update {
            Some(oldest) => oldest.min(last_update),
            None => last_update,
        });
        valid_prices.push_back(price);
    }
    (valid_prices, oldest_source_update)
}

/// Full price fetch: cache hit short-circuit, otherwise query every source,
/// require ≥ `min_required_sources` valid responses, compute and validate
/// the median, write cache, emit, return.
pub fn fetch_and_validate_price(env: &Env, symbol: Symbol) -> i128 {
    let config = storage::load_oracle_config(env);
    let current_time = env.ledger().timestamp();

    // Keep this symbol's persistent pricing state (sources + cached median)
    // alive on every read. A hot symbol that always hits the cache below would
    // otherwise never re-write these keys, letting them expire (~46 days) and
    // halt pricing. No-op until the keys exist.
    storage::bump_symbol_ttl(env, &symbol);

    // Cache hit — return immediately only while both the router cache window
    // and the underlying source freshness window remain valid.
    if let Some(entry) = storage::load_cached_price(env, &symbol) {
        let cache_live = current_time <= entry.fetched_at.saturating_add(config.cache_duration);
        let sources_live = current_time
            <= entry
                .oldest_source_update
                .saturating_add(config.staleness_threshold);
        if cache_live && sources_live {
            return entry.price;
        }
    }

    let sources = storage::load_sources(env, &symbol);
    if sources.is_empty() {
        panic_with_error!(env, OracleRouterError::NoPriceSources);
    }

    let (valid_prices, oldest_source_update) =
        query_sources(env, &sources, &symbol, &config, current_time);

    // No valid responses at all → StalePrice (every source was stale, broken,
    // future-dated, or returned a non-positive price). This is distinct from
    // "some valid responses but below quorum", which uses InsufficientSources.
    if valid_prices.is_empty() {
        panic_with_error!(env, OracleRouterError::StalePrice);
    }
    if (valid_prices.len() as u32) < config.min_required_sources {
        panic_with_error!(env, OracleRouterError::InsufficientSources);
    }

    let mut sorted = valid_prices;
    insertion_sort(&mut sorted);

    let n = sorted.len();
    let median = median(env, &sorted);
    let dev = deviation_bps(
        env,
        median,
        sorted.get(0).unwrap(),
        sorted.get(n - 1).unwrap(),
    );
    if dev > config.max_deviation_bps {
        panic_with_error!(env, OracleRouterError::PriceDeviationTooHigh);
    }

    storage::save_cached_price(
        env,
        &symbol,
        storage::CachedPrice {
            price: median,
            fetched_at: current_time,
            oldest_source_update: oldest_source_update.unwrap_or(current_time),
        },
    );
    events::PriceFetch {
        symbol: symbol.clone(),
        price: median,
        timestamp: current_time,
    }
    .publish(env);
    bump_instance_ttl(env);

    median
}

/// Cross-call every source's `decimals()` and reject the set unless all
/// report `PRICE_DECIMALS`. A source publishing at the wrong scale (e.g. 1e6
/// or 1e8) would silently skew the median while still passing the deviation
/// gate, so the mismatch is caught at configuration time. A source that
/// cannot answer `decimals()` is treated as invalid.
pub fn validate_source_decimals(env: &Env, sources: &Vec<Address>) {
    for source in sources.iter() {
        let client = OracleClient::new(env, &source);
        match client.try_decimals() {
            Ok(Ok(d)) if d == PRICE_DECIMALS => {}
            _ => panic_with_error!(env, OracleRouterError::InvalidSourceDecimals),
        }
    }
}

/// Deduplicate an address list, preserving first-occurrence order. O(n²) —
/// fine because source lists are bounded at MAX_ORACLE_SOURCES.
pub fn dedup_sources(env: &Env, sources: &Vec<Address>) -> Vec<Address> {
    let mut result: Vec<Address> = Vec::new(env);
    'outer: for addr in sources.iter() {
        for existing in result.iter() {
            if addr == existing {
                continue 'outer;
            }
        }
        result.push_back(addr);
    }
    result
}

/// In-place insertion sort (ascending). O(n²) — fine for source lists bounded
/// at MAX_ORACLE_SOURCES.
pub(crate) fn insertion_sort(prices: &mut Vec<i128>) {
    let n = prices.len();
    for i in 1..n {
        let key = prices.get(i).unwrap();
        let mut j = i;
        while j > 0 {
            let prev = prices.get(j - 1).unwrap();
            if prev <= key {
                break;
            }
            prices.set(j, prev);
            j -= 1;
        }
        prices.set(j, key);
    }
}

/// Median of a sorted, non-empty price list. Odd length → the middle element;
/// even length → the average of the two middle elements (so a 2-source feed
/// is not systematically biased toward the lower price). The average uses
/// checked arithmetic — overflow raises `MedianOverflow` rather than trapping.
pub(crate) fn median(env: &Env, sorted: &Vec<i128>) -> i128 {
    let n = sorted.len();
    if n % 2 == 1 {
        sorted.get(n / 2).unwrap()
    } else {
        let hi = sorted.get(n / 2).unwrap();
        let lo = sorted.get(n / 2 - 1).unwrap();
        match lo.checked_add(hi) {
            Some(sum) => sum / 2,
            None => panic_with_error!(env, OracleRouterError::MedianOverflow),
        }
    }
}

/// Max one-sided deviation in basis points:
/// `max(max − median, median − min) × BPS / median`.
/// All arithmetic is checked — overflow on adversarial prices raises
/// `DeviationOverflow` instead of trapping the host.
pub(crate) fn deviation_bps(env: &Env, median: i128, min: i128, max: i128) -> i128 {
    let upper_num = match max.checked_sub(median).and_then(|v| v.checked_mul(BPS)) {
        Some(v) => v,
        None => panic_with_error!(env, OracleRouterError::DeviationOverflow),
    };
    let upper = match upper_num.checked_div(median) {
        Some(v) => v,
        None => panic_with_error!(env, OracleRouterError::DeviationOverflow),
    };
    let lower_num = match median.checked_sub(min).and_then(|v| v.checked_mul(BPS)) {
        Some(v) => v,
        None => panic_with_error!(env, OracleRouterError::DeviationOverflow),
    };
    let lower = match lower_num.checked_div(median) {
        Some(v) => v,
        None => panic_with_error!(env, OracleRouterError::DeviationOverflow),
    };
    if upper > lower {
        upper
    } else {
        lower
    }
}
