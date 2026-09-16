#![no_std]

mod contract;
mod errors;
mod events;
mod storage;

pub use contract::VaultContract;
pub use contract::VaultContractClient;
pub use errors::VaultError;
