#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, Env, Symbol};

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

    pub fn lastprice(env: Env, symbol: Symbol) -> Option<PriceData> {
        env.storage().instance().get(&StorageKey::Price(symbol))
    }

    pub fn decimals(_env: Env) -> u32 {
        7
    }
}
