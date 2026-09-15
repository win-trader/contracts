//! Position-lifecycle and governance events.
//!
//! One file per event: the `#[contractevent]` type and the `emit_*` function
//! that builds it from the aggregates a caller already holds. Nothing outside
//! this module assembles an event struct. The settlement events double as the
//! on-chain audit trail for the cash-transition table (doc §6), and the
//! offchain indexer keys on the topic literals. Wide settlement events use
//! `data_format = "map"` so fields are self-describing and can grow.

mod action_cancelled;
mod action_committed;
mod action_expired;
mod action_failed;
mod action_settled;
mod action_superseded;
mod bad_debt;
mod borrow_checkpoint;
mod collateral_added;
mod close_reason;
mod config_proposed;
mod fee_source;
mod funding_checkpoint;
mod global_config_updated;
mod header;
mod market_checkpoint;
mod market_config_updated;
mod market_status_changed;
mod pause_changed;
mod price_feed_changed;
mod position_closed;
mod position_decreased;
mod position_increased;
mod position_opened;
mod protocol_claimed;
mod recapitalized;
mod referral_accrued;
mod referral_claimed;
mod referral_code_registered;
mod referrer_set;
mod revenue_split;
mod risk_state_changed;
mod tp_sl_updated;

pub use action_cancelled::*;
pub use action_committed::*;
pub use action_expired::*;
pub use action_failed::*;
pub use action_settled::*;
pub use action_superseded::*;
pub use bad_debt::*;
pub use borrow_checkpoint::*;
pub use collateral_added::*;
pub use close_reason::*;
pub use config_proposed::*;
pub use fee_source::*;
pub use funding_checkpoint::*;
pub use global_config_updated::*;
pub use header::*;
pub use market_checkpoint::*;
pub use market_config_updated::*;
pub use market_status_changed::*;
pub use pause_changed::*;
pub use price_feed_changed::*;
pub use position_closed::*;
pub use position_decreased::*;
pub use position_increased::*;
pub use position_opened::*;
pub use protocol_claimed::*;
pub use recapitalized::*;
pub use referral_accrued::*;
pub use referral_claimed::*;
pub use referral_code_registered::*;
pub use referrer_set::*;
pub use revenue_split::*;
pub use risk_state_changed::*;
pub use tp_sl_updated::*;
