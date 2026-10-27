use shared::PriceData;
use soroban_sdk::{contracttype, Address, Env, Symbol, Vec};

/// Ledgers (two hours) a published price stays in temporary storage. Above the
/// `max_source_age_seconds` ceiling, so expiry never decides freshness.
pub const PRICE_TTL_LEDGERS: u32 = 1_440;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeedConfig {
    /// A source older than this is ignored.
    pub max_source_age_seconds: u64,
    /// Largest one-sided distance of any fresh source from the median.
    pub max_deviation_bps: u32,
    /// Fewer fresh sources than this and `lastprice` returns `None`.
    pub min_sources: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourcePrice {
    pub symbol: Symbol,
    pub price: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingSources {
    pub publishers: Vec<Address>,
    pub config: FeedConfig,
    pub effective_at: u64,
}

#[contracttype]
#[derive(Clone)]
pub enum Key {
    ConfigManager,
    TimelockSeconds,
    Publishers,
    Config,
    PendingSources,
    Price(Address, Symbol),
}

pub fn init(
    env: &Env,
    config_manager: &Address,
    timelock_seconds: u64,
    publishers: &Vec<Address>,
    config: &FeedConfig,
) {
    let instance = env.storage().instance();
    instance.set(&Key::ConfigManager, config_manager);
    instance.set(&Key::TimelockSeconds, &timelock_seconds);
    save_sources(env, publishers, config);
}

pub fn config_manager(env: &Env) -> Address {
    env.storage().instance().get(&Key::ConfigManager).unwrap()
}

pub fn timelock_seconds(env: &Env) -> u64 {
    env.storage().instance().get(&Key::TimelockSeconds).unwrap()
}

pub fn publishers(env: &Env) -> Vec<Address> {
    env.storage().instance().get(&Key::Publishers).unwrap()
}

pub fn config(env: &Env) -> FeedConfig {
    env.storage().instance().get(&Key::Config).unwrap()
}

pub fn save_publishers(env: &Env, publishers: &Vec<Address>) {
    env.storage().instance().set(&Key::Publishers, publishers);
}

pub fn save_sources(env: &Env, publishers: &Vec<Address>, config: &FeedConfig) {
    save_publishers(env, publishers);
    env.storage().instance().set(&Key::Config, config);
}

pub fn pending_sources(env: &Env) -> Option<PendingSources> {
    env.storage().instance().get(&Key::PendingSources)
}

pub fn save_pending_sources(env: &Env, pending: &PendingSources) {
    env.storage().instance().set(&Key::PendingSources, pending);
}

pub fn clear_pending_sources(env: &Env) {
    env.storage().instance().remove(&Key::PendingSources);
}

pub fn source_price(env: &Env, publisher: &Address, symbol: &Symbol) -> Option<PriceData> {
    env.storage()
        .temporary()
        .get(&Key::Price(publisher.clone(), symbol.clone()))
}

pub fn save_source_price(env: &Env, publisher: &Address, symbol: &Symbol, data: &PriceData) {
    let key = Key::Price(publisher.clone(), symbol.clone());
    let temporary = env.storage().temporary();
    temporary.set(&key, data);
    temporary.extend_ttl(&key, PRICE_TTL_LEDGERS, PRICE_TTL_LEDGERS);
}
