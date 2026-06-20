use shared::constants::{SHARED_BUMP, SHARED_THRESHOLD};
use soroban_sdk::{contracttype, panic_with_error, vec, Address, Env, Symbol, Vec};

use crate::errors::OracleRouterError;
use crate::types::OracleConfig;

/// Cached aggregated median price for a symbol. `fetched_at` bounds router
/// cache duration; `oldest_source_update` ensures the cached median is not
/// served after any source response used to compute it has gone stale.
#[contracttype]
#[derive(Clone)]
pub struct CachedPrice {
    pub price: i128,
    pub fetched_at: u64,
    pub oldest_source_update: u64,
}

#[contracttype]
pub enum StorageKey {
    /// Initialization flag.
    Initialized,
    /// Linked ConfigManager address.
    ConfigManager,
    /// Global oracle configuration.
    OracleConfig,
    /// Monotonic version bumped on every config update. Cache keys include
    /// this value so config changes invalidate old medians without scanning
    /// every symbol.
    ConfigVersion,
    /// Per-symbol flat source list (no primary/secondary tiering).
    Sources(Symbol),
    /// Legacy per-symbol cached aggregated price key. Kept so upgraded
    /// deployments do not try to decode old cache entries as V2 values.
    CachedPrice(Symbol),
    /// Per-symbol cached aggregated price for the active config version.
    CachedPriceV2(Symbol, u64),
    /// Current contract version — written by `_migrate` after a WASM upgrade.
    Version,
}

// ---------------------------------------------------------------------------
// Initialization helpers
// ---------------------------------------------------------------------------

fn is_initialized(env: &Env) -> bool {
    env.storage()
        .instance()
        .get::<_, bool>(&StorageKey::Initialized)
        .unwrap_or(false)
}

pub fn require_initialized(env: &Env) {
    if !is_initialized(env) {
        panic_with_error!(env, OracleRouterError::NotInitialized);
    }
}

pub fn set_initialized(env: &Env) {
    env.storage()
        .instance()
        .set(&StorageKey::Initialized, &true);
}

// ---------------------------------------------------------------------------
// ConfigManager helpers
// ---------------------------------------------------------------------------

pub fn load_config_manager(env: &Env) -> Address {
    env.storage()
        .instance()
        .get(&StorageKey::ConfigManager)
        .unwrap_or_else(|| panic_with_error!(env, OracleRouterError::Unauthorized))
}

pub fn set_config_manager(env: &Env, addr: &Address) {
    env.storage()
        .instance()
        .set(&StorageKey::ConfigManager, addr);
}

// ---------------------------------------------------------------------------
// OracleConfig helpers
// ---------------------------------------------------------------------------

pub fn load_oracle_config(env: &Env) -> OracleConfig {
    env.storage()
        .instance()
        .get(&StorageKey::OracleConfig)
        .unwrap_or_else(|| panic_with_error!(env, OracleRouterError::NotInitialized))
}

pub fn save_oracle_config(env: &Env, config: &OracleConfig) {
    env.storage()
        .instance()
        .set(&StorageKey::OracleConfig, config);
}

pub fn load_config_version(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get(&StorageKey::ConfigVersion)
        .unwrap_or(0)
}

pub fn bump_config_version(env: &Env) {
    let next = load_config_version(env)
        .checked_add(1)
        .unwrap_or_else(|| panic_with_error!(env, OracleRouterError::InvalidConfig));
    env.storage()
        .instance()
        .set(&StorageKey::ConfigVersion, &next);
}

// ---------------------------------------------------------------------------
// Oracle source helpers — single flat list per symbol.
// ---------------------------------------------------------------------------

pub fn load_sources(env: &Env, symbol: &Symbol) -> Vec<Address> {
    env.storage()
        .persistent()
        .get(&StorageKey::Sources(symbol.clone()))
        .unwrap_or_else(|| vec![env])
}

pub fn save_sources(env: &Env, symbol: &Symbol, sources: &Vec<Address>) {
    let key = StorageKey::Sources(symbol.clone());
    env.storage().persistent().set(&key, sources);
    env.storage()
        .persistent()
        .extend_ttl(&key, SHARED_THRESHOLD, SHARED_BUMP);
}

// ---------------------------------------------------------------------------
// Cached price helpers
// ---------------------------------------------------------------------------

pub fn load_cached_price(env: &Env, symbol: &Symbol) -> Option<CachedPrice> {
    let version = load_config_version(env);
    env.storage()
        .persistent()
        .get(&StorageKey::CachedPriceV2(symbol.clone(), version))
}

pub fn save_cached_price(env: &Env, symbol: &Symbol, entry: CachedPrice) {
    let version = load_config_version(env);
    let key = StorageKey::CachedPriceV2(symbol.clone(), version);
    env.storage().persistent().set(&key, &entry);
    env.storage()
        .persistent()
        .extend_ttl(&key, SHARED_THRESHOLD, SHARED_BUMP);
}

/// Extend the persistent TTL of a symbol's `Sources` and `CachedPrice` slots
/// when present. Called on every `get_price` read so a frequently-read symbol
/// — one that keeps hitting the cache and therefore never re-writes either
/// key — cannot have its pricing state silently expire (~46 days) and brick
/// the feed. No-op for keys that don't exist yet.
pub fn bump_symbol_ttl(env: &Env, symbol: &Symbol) {
    // The presence checks are required, not redundant: `extend_ttl` on a key
    // that has never been written aborts the host with `InvalidAction` rather
    // than no-opping, which would turn a missing-sources read into an opaque
    // panic instead of the typed `NoPriceSources` error. Matches PM's
    // `bump_market_unrealized_pnl_ttl`.
    let sources_key = StorageKey::Sources(symbol.clone());
    if env.storage().persistent().has(&sources_key) {
        env.storage()
            .persistent()
            .extend_ttl(&sources_key, SHARED_THRESHOLD, SHARED_BUMP);
    }
    let cache_key = StorageKey::CachedPriceV2(symbol.clone(), load_config_version(env));
    if env.storage().persistent().has(&cache_key) {
        env.storage()
            .persistent()
            .extend_ttl(&cache_key, SHARED_THRESHOLD, SHARED_BUMP);
    }
}

/// Drop any cached price for `symbol`. Called when the source set changes so
/// a feed disabled or rotated by an operator takes effect immediately rather
/// than serving the previous median for up to `cache_duration` seconds.
pub fn remove_cached_price(env: &Env, symbol: &Symbol) {
    let version = load_config_version(env);
    env.storage()
        .persistent()
        .remove(&StorageKey::CachedPrice(symbol.clone()));
    env.storage()
        .persistent()
        .remove(&StorageKey::CachedPriceV2(symbol.clone(), version));
}

// ---------------------------------------------------------------------------
// Version helper
// ---------------------------------------------------------------------------

pub fn save_version(env: &Env, version: u32) {
    env.storage()
        .instance()
        .set(&StorageKey::Version, &version);
}

// Pending upgrade storage now lives in `interfaces::upgrade` under a shared
// Symbol key — used by the `TimelockedUpgradeable` trait's default methods.
