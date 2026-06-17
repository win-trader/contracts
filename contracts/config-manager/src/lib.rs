#![no_std]

pub mod contract;
pub mod errors;
mod events;
pub mod logic;
pub mod storage;
pub mod types;
pub mod validate;

#[cfg(test)]
mod tests;

pub use contract::ConfigManagerContract;
pub use errors::ConfigManagerError;
pub use interfaces::{ConfigManager, ConfigManagerClient, MigrationData};
pub use types::{BorrowRateConfig, FeeSplits, ProtocolLimits};
