use soroban_sdk::{contractclient, contracttype, Address, Env, Symbol};

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PriceData {
    pub price: i128,
    pub timestamp: u64,
}

/// SEP-40 asset identifier. Markets are quoted as `Other(market symbol)`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Asset {
    Stellar(Address),
    Other(Symbol),
}

/// The SEP-40 subset of the external price feed the protocol reads.
#[contractclient(name = "PriceFeedClient")]
pub trait PriceFeed {
    /// The latest price for `asset`, stamped with when it was sampled.
    fn lastprice(env: Env, asset: Asset) -> Option<PriceData>;

    /// Decimals of the reported prices. Prices are rescaled to `PRICE_DECIMALS`.
    fn decimals(env: Env) -> u32;
}

/// Feeds reporting more decimals than this are refused, which keeps the
/// rescaling factor well inside i128.
pub const MAX_FEED_DECIMALS: u32 = 18;

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StampedPrice {
    pub price: i128,
    pub observed_at: u64,
}
