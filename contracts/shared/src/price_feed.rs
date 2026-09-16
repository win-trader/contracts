use soroban_sdk::{contractclient, contracttype, Env, Symbol};

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PriceData {
    pub price: i128,
    pub timestamp: u64,
}

/// The SEP-40 subset of the external price feed the protocol reads.
#[contractclient(name = "PriceFeedClient")]
pub trait PriceFeed {
    /// The latest price for `symbol`, stamped with when it was sampled.
    fn lastprice(env: Env, symbol: Symbol) -> Option<PriceData>;

    /// Decimals of the reported prices; must equal `PRICE_DECIMALS`.
    fn decimals(env: Env) -> u32;
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StampedPrice {
    pub price: i128,
    pub observed_at: u64,
}
