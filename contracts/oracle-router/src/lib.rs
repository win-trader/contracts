#![no_std]

pub mod contract;
mod errors;
mod events;
mod logic;
mod storage;
mod types;

#[cfg(test)]
pub mod tests;

pub use contract::OracleRouterContract;
pub use errors::OracleRouterError;
pub use shared::{MigrationData, OracleRouterClient};
pub use types::OracleConfig;
