use soroban_sdk::contracterror;

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum VaultError {
    Unauthorized = 1,
    AlreadyInitialized = 2,
    NotInitialized = 3,
    InvalidAmount = 4,
    InvalidConfig = 5,
    Paused = 6,
    InsufficientCash = 7,
    InvalidCaller = 8,
    ArithmeticError = 9,
    /// `upgrade` called with no pending proposal.
    UpgradeNoPending = 10,
    /// `upgrade` called before the proposal's timelock eta.
    UpgradeTimelockNotElapsed = 11,
    /// `upgrade` called with a hash that differs from the proposal.
    UpgradeHashMismatch = 12,
}
