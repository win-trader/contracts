#![no_std]

//! SEP-40 price feed aggregated from the protocol's own CEX publishers.
//!
//! Each publisher pushes `set_prices` from its exchange; `lastprice` returns the
//! median of the fresh sources, or `None` when too few are fresh or they
//! disagree by more than `max_deviation_bps`. The contract is not upgradeable:
//! replacing it goes through the MarketGovernor's price-feed timelock.

mod contract;
mod errors;
mod events;
mod logic;
mod storage;

pub use contract::{CexFeedContract, CexFeedContractClient};
pub use errors::CexFeedError;
pub use storage::{FeedConfig, PendingSources, SourcePrice};
