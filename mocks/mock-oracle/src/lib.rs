//! Test stand-in for the third-party price feed.
//!
//! The protocol owns no oracle. This mock exists only so integration tests
//! can drive prices deterministically; in every real deployment the address
//! wired into the position manager belongs to an external provider (or to a
//! thin adapter that forwards to one).

#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, Env, Symbol};

/// Mirrors `shared::PriceData`. Declared locally so the mock has no
/// dependency on the protocol crates.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PriceData {
    pub price: i128,
    pub timestamp: u64,
}

#[contracttype]
pub enum StorageKey {
    Price(Symbol),
}

#[contract]
pub struct MockOracle;

#[contractimpl]
impl MockOracle {
    pub fn initialize(_env: Env) {}

    /// Set the price for `symbol` (scaled by 1e7), stamped with the current
    /// ledger timestamp. Test-only.
    pub fn set_price(env: Env, symbol: Symbol, price: i128) {
        let data = PriceData {
            price,
            timestamp: env.ledger().timestamp(),
        };
        env.storage()
            .instance()
            .set(&StorageKey::Price(symbol), &data);
    }

    /// Set a price with an explicit observation timestamp, so tests can
    /// exercise the staleness bound and the fresh-observation cursor.
    pub fn set_price_at(env: Env, symbol: Symbol, price: i128, timestamp: u64) {
        let data = PriceData { price, timestamp };
        env.storage()
            .instance()
            .set(&StorageKey::Price(symbol), &data);
    }

    /// `shared::PriceFeed::lastprice`.
    pub fn lastprice(env: Env, symbol: Symbol) -> Option<PriceData> {
        env.storage().instance().get(&StorageKey::Price(symbol))
    }

    /// `shared::PriceFeed::decimals` — 7, matching `PRICE_DECIMALS`.
    pub fn decimals(_env: Env) -> u32 {
        7
    }
}
