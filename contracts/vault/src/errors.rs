//! §12.5 — the vault's error range, `100–199`, grouped by cause.
//!
//! Disjoint from every other contract's: a raw code is globally unambiguous
//! without knowing which contract produced it. Code `9` used to mean
//! `SlippageExceeded` in the position manager and `ArithmeticError` here,
//! and a caller receiving it could not tell a rejected price bound from a
//! corrupted calculation — two conditions with opposite remedies.

use soroban_sdk::contracterror;

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum VaultError {
    // -- Authorization. --
    /// The caller does not hold the required ConfigManager role.
    Unauthorized = 100,
    /// The caller is not the position manager or request router this entry
    /// point admits.
    InvalidCaller = 101,

    // -- State. --
    NotInitialized = 110,
    AlreadyInitialized = 111,
    Paused = 112,
    /// A transfer would exceed the vault's physical cash. An accounting
    /// guard, not a business outcome: §7.17's gates decide what may be paid
    /// out, and reaching this means one of them was skipped.
    InsufficientCash = 113,

    // -- Validation. --
    InvalidAmount = 120,
    /// Includes §12.1's token-decimals requirement, checked when the asset
    /// is wired rather than at the first deposit.
    InvalidConfig = 121,

    // -- Upgrade. --
    UpgradeNoPending = 130,
    UpgradeTimelockNotElapsed = 131,
    UpgradeHashMismatch = 132,

    // -- Arithmetic. Never reachable through ordinary use. --
    ArithmeticError = 180,
}
