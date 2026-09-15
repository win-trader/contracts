//! The external price feed this protocol reads from.
//!
//! **The protocol does not publish prices and owns no oracle.** It consumes
//! a third-party feed, and this is the whole of the interface it depends on:
//! one call that returns a price and the moment it was observed.
//!
//! That is deliberately smaller than the surface a self-operated oracle
//! needs. Source aggregation — median, quorum, deviation rejection — belongs
//! to the provider. What stays here is what only this protocol can decide:
//! how old a price may be before an action refuses it (§12.7.4's staleness
//! bound, now `max_price_age_seconds`), and the per-action price bounds a
//! trader sets for themselves.
//!
//! The shape below is the SEP-40 subset the protocol actually uses. A
//! provider whose entry point differs — a different asset identifier, a
//! different struct — is bridged by a thin adapter contract that implements
//! this trait and forwards; the adapter is deployment wiring, not protocol.

use soroban_sdk::{contractclient, contracttype, Env, Symbol};

/// A price and the moment it was observed, as the feed reports them.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PriceData {
    /// Scaled by `crate::constants::PRICE_PRECISION`.
    pub price: i128,
    /// When the feed observed it. This is the cursor §7.0's
    /// fresh-observation test compares against, so it must be the
    /// observation time, not the time the value was served.
    pub timestamp: u64,
}

/// The one call the protocol makes into the feed.
#[contractclient(name = "PriceFeedClient")]
pub trait PriceFeed {
    /// The latest price for `symbol`, or `None` if the feed has none.
    ///
    /// Must not be answered from a retained value. A retained stamp is
    /// backdated by however long it was retained, which would let a fill
    /// satisfy the fresh-observation test against an observation that
    /// predates the commitment it is settling — the test passing with no new
    /// information having arrived. On the forced paths a retained price is a
    /// price the caller chose: a keeper able to pin a momentary adverse
    /// print could liquidate a currently-healthy position.
    fn lastprice(env: Env, symbol: Symbol) -> Option<PriceData>;

    /// Decimal scale of the prices this feed reports. Must equal
    /// `crate::constants::PRICE_DECIMALS`; checked when the feed is wired.
    fn decimals(env: Env) -> u32;
}

/// An accepted price together with its observation cursor — what every
/// protocol action reads instead of a bare `i128`.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StampedPrice {
    pub price: i128,
    /// The feed's observation time, carried so a commitment can require a
    /// strictly newer observation to settle against (§7.0).
    pub observed_at: u64,
}
