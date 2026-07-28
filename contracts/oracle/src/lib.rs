#![no_std]

pub mod contract;
mod errors;
mod storage;

pub use contract::OracleContract;
pub use errors::OracleError;
pub use shared::{MigrationData, Oracle, OracleClient};
