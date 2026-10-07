use soroban_sdk::contracterror;

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum VaultError {
    Unauthorized = 100,
    InvalidCaller = 101,

    NotInitialized = 110,
    AlreadyInitialized = 111,
    Paused = 112,
    InsufficientCash = 113,

    InvalidAmount = 120,
    InvalidConfig = 121,

    UpgradeNoPending = 130,
    UpgradeTimelockNotElapsed = 131,
    UpgradeHashMismatch = 132,

    ArithmeticError = 180,
}
