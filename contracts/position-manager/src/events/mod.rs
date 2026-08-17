//! Position-lifecycle and governance events.
//!
//! One file per event: the `#[contractevent]` type and the `emit_*` function
//! that builds it from the aggregates a caller already holds. Nothing outside
//! this module assembles an event struct. The settlement events double as the
//! on-chain audit trail for the cash-transition table (doc §6), and the
//! offchain indexer keys on the topic literals. Wide settlement events use
//! `data_format = "map"` so fields are self-describing and can grow.

mod adl_reward_paid;
mod bad_debt;
mod close_reason;
mod execution_budget_funded;
mod execution_budget_withdrawn;
mod fee_source;
mod global_config_updated;
mod insolvency_reward_paid;
mod market_checkpoint;
mod market_config_updated;
mod market_status_changed;
mod order_executed;
mod pause_changed;
mod position_closed;
mod position_decreased;
mod position_increased;
mod position_opened;
mod protocol_claimed;
mod recapitalized;
mod revenue_split;
mod risk_state_changed;
mod tp_sl_updated;

pub use adl_reward_paid::*;
pub use bad_debt::*;
pub use close_reason::*;
pub use execution_budget_funded::*;
pub use execution_budget_withdrawn::*;
pub use fee_source::*;
pub use global_config_updated::*;
pub use insolvency_reward_paid::*;
pub use market_checkpoint::*;
pub use market_config_updated::*;
pub use market_status_changed::*;
pub use order_executed::*;
pub use pause_changed::*;
pub use position_closed::*;
pub use position_decreased::*;
pub use position_increased::*;
pub use position_opened::*;
pub use protocol_claimed::*;
pub use recapitalized::*;
pub use revenue_split::*;
pub use risk_state_changed::*;
pub use tp_sl_updated::*;
