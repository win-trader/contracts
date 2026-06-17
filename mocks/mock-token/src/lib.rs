//! Mock SEP-41 fungible token for use in integration tests.
//!
//! Exposes `mint` and `burn` in addition to the standard token interface
//! so test fixtures can freely manage token supply.

#![no_std]

use soroban_sdk::{contract, contractimpl, Address, Env, MuxedAddress, String};
use stellar_tokens::fungible::{burnable::FungibleBurnable, Base, FungibleToken};

#[contract]
pub struct MockToken;

#[contractimpl]
impl MockToken {
    /// Deploy and configure the mock token.
    pub fn initialize(env: Env, _admin: Address, decimals: u32, name: String, symbol: String) {
        Base::set_metadata(&env, decimals, name, symbol);
    }

    /// Mint `amount` tokens to `to`. No access control — test-only.
    pub fn mint(env: Env, to: Address, amount: i128) {
        Base::mint(&env, &to, amount);
    }
}

/// SEP-41 token interface — auto-implemented by OZ Base.
#[contractimpl(contracttrait)]
impl FungibleToken for MockToken {
    type ContractType = Base;
}

/// Burn support — auto-implemented by OZ FungibleBurnable.
#[contractimpl(contracttrait)]
impl FungibleBurnable for MockToken {}
