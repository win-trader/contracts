#![no_std]

pub mod contract;
mod errors;
mod events;
mod logic;
mod storage;

#[cfg(test)]
pub mod tests;

pub use contract::{VaultContract, VaultContractClient};
pub use errors::VaultError;
pub use interfaces::{MigrationData, VaultClient};
