#![no_std]

mod action;
mod auth;
mod borrow;
mod contract;
mod errors;
mod events;
mod fees;
mod funding;
mod governance;
mod keeper;
mod ledger;
mod math;
mod position;
mod referral;
mod risk;
mod settle;
mod snapshot;
mod storage;
mod validation;
mod window;

pub use contract::PositionManagerContract;
// The generated client for the concrete contract, not the shared trait.
// `upgrade` and `migrate` (§12.4) are declared here rather than on
// `PositionManager`, so this is the only client that can reach them.
pub use contract::PositionManagerContractClient;
pub use errors::PositionManagerError;
