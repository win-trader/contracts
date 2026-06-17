//! Mock SEP-40 price oracle for use in integration tests.
//!
//! Allows tests to set arbitrary prices for any symbol so the OracleRouter
//! and PositionManager can be exercised under controlled price conditions.

#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, Env, Symbol};

#[contracttype]
pub enum StorageKey {
    Price(Symbol),
    LastUpdate(Symbol),
}

#[contract]
pub struct MockOracle;

#[contractimpl]
impl MockOracle {
    pub fn initialize(_env: Env) {}

    /// Manually set the price for `symbol` (scaled by 1e7). Test-only.
    pub fn set_price(env: Env, symbol: Symbol, price: i128) {
        env.storage()
            .instance()
            .set(&StorageKey::Price(symbol.clone()), &price);
        env.storage()
            .instance()
            .set(&StorageKey::LastUpdate(symbol), &env.ledger().timestamp());
    }

    /// Return the stored price for `symbol`. Implements SEP-40 price interface.
    pub fn get_price(env: Env, symbol: Symbol) -> i128 {
        env.storage()
            .instance()
            .get(&StorageKey::Price(symbol))
            .unwrap_or_else(|| panic!("no price set"))
    }

    /// Return the ledger timestamp when the price was last set.
    pub fn last_update(env: Env, symbol: Symbol) -> u64 {
        env.storage()
            .instance()
            .get(&StorageKey::LastUpdate(symbol))
            .unwrap_or(0u64)
    }

    /// Price scale this source reports — 7, matching the protocol-wide
    /// `shared::constants::PRICE_DECIMALS` so the router accepts it as a source.
    pub fn decimals(_env: Env) -> u32 {
        7
    }
}
