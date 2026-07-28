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
}
