use shared::constants::{BPS, MAX_UPGRADE_TIMELOCK_SECS, MIN_UPGRADE_TIMELOCK};
use shared::PriceData;
use soroban_sdk::{Address, Env, Symbol, Vec};

use crate::storage::{self, FeedConfig};

pub const MAX_PUBLISHERS: u32 = 5;
/// Symbols per `set_prices`; the PositionManager caps active markets at 16.
pub const MAX_BATCH: u32 = 16;
pub const MAX_SOURCE_AGE_CEILING: u64 = 3_600;
pub const MAX_DEVIATION_BPS_CEILING: u32 = 2_000;
/// Prices above this are refused at publish, so the median sum and the
/// deviation numerator cannot overflow.
pub const MAX_PRICE: i128 = i128::MAX / BPS / 2;

pub fn timelock_is_valid(timelock_seconds: u64) -> bool {
    (MIN_UPGRADE_TIMELOCK..=MAX_UPGRADE_TIMELOCK_SECS).contains(&timelock_seconds)
}

pub fn sources_are_valid(publishers: &Vec<Address>, config: &FeedConfig) -> bool {
    let count = publishers.len();
    if count == 0 || count > MAX_PUBLISHERS {
        return false;
    }
    for i in 0..count {
        for j in (i + 1)..count {
            if publishers.get_unchecked(i) == publishers.get_unchecked(j) {
                return false;
            }
        }
    }
    config.min_sources >= 1
        && config.min_sources <= count
        && config.max_source_age_seconds >= 1
        && config.max_source_age_seconds <= MAX_SOURCE_AGE_CEILING
        && config.max_deviation_bps >= 1
        && config.max_deviation_bps <= MAX_DEVIATION_BPS_CEILING
}

/// Median of the fresh sources, stamped with the oldest of them, so a fill
/// that needs an observation after some time needs every counted source to
/// have published after it.
pub fn aggregate(env: &Env, symbol: &Symbol) -> Option<PriceData> {
    let config = storage::config(env);
    let now = env.ledger().timestamp();
    let mut prices: Vec<i128> = Vec::new(env);
    let mut oldest = u64::MAX;
    for publisher in storage::publishers(env).iter() {
        let Some(data) = storage::source_price(env, &publisher, symbol) else {
            continue;
        };
        if data.timestamp > now || now - data.timestamp > config.max_source_age_seconds {
            continue;
        }
        insert_sorted(&mut prices, data.price);
        oldest = oldest.min(data.timestamp);
    }
    if prices.is_empty() || prices.len() < config.min_sources {
        return None;
    }
    let median = median(&prices);
    let low = prices.first_unchecked();
    let high = prices.last_unchecked();
    let deviation = core::cmp::max(high - median, median - low) * BPS / median;
    if deviation > config.max_deviation_bps as i128 {
        return None;
    }
    Some(PriceData {
        price: median,
        timestamp: oldest,
    })
}

fn insert_sorted(prices: &mut Vec<i128>, price: i128) {
    let mut at = prices.len();
    while at > 0 && prices.get_unchecked(at - 1) > price {
        at -= 1;
    }
    prices.insert(at, price);
}

/// Even counts average the two middle prices, so two sources are not biased
/// toward the lower one.
fn median(sorted: &Vec<i128>) -> i128 {
    let n = sorted.len();
    if n % 2 == 1 {
        sorted.get_unchecked(n / 2)
    } else {
        (sorted.get_unchecked(n / 2 - 1) + sorted.get_unchecked(n / 2)) / 2
    }
}
