#![no_std]

//! Test-only SEP-40 price feed. Anyone can set prices; never deploy it outside
//! local and test networks. It defines its own `Asset`, as a third-party feed
//! would, so the protocol is exercised against the SEP-40 wire format.

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Symbol};

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PriceData {
    pub price: i128,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Asset {
    Stellar(Address),
    Other(Symbol),
}

#[contracttype]
pub enum StorageKey {
    Price(Symbol),
    Decimals,
}

#[contract]
pub struct MockOracle;

#[contractimpl]
impl MockOracle {
    pub fn initialize(_env: Env) {}

    pub fn set_price(env: Env, symbol: Symbol, price: i128) {
        let data = PriceData {
            price,
            timestamp: env.ledger().timestamp(),
        };
        env.storage()
            .instance()
            .set(&StorageKey::Price(symbol), &data);
    }

    pub fn set_price_at(env: Env, symbol: Symbol, price: i128, timestamp: u64) {
        let data = PriceData { price, timestamp };
        env.storage()
            .instance()
            .set(&StorageKey::Price(symbol), &data);
    }

    /// Report prices with `decimals` places (default 7).
    pub fn set_decimals(env: Env, decimals: u32) {
        env.storage().instance().set(&StorageKey::Decimals, &decimals);
    }

    pub fn lastprice(env: Env, asset: Asset) -> Option<PriceData> {
        match asset {
            Asset::Other(symbol) => env.storage().instance().get(&StorageKey::Price(symbol)),
            Asset::Stellar(_) => None,
        }
    }

    pub fn decimals(env: Env) -> u32 {
        env.storage().instance().get(&StorageKey::Decimals).unwrap_or(7)
    }
}
