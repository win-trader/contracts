use soroban_sdk::contracterror;

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum PositionManagerError {
    Unauthorized = 1,
    InvalidCaller = 2,

    PositionNotFound = 10,
    MarketNotConfigured = 11,
    ActionNotFound = 12,
    TriggerNotAttached = 14,
    NoPendingConfig = 15,
    UpgradeNoPending = 16,

    NotInitialized = 20,
    AlreadyInitialized = 21,
    Paused = 22,
    MarketDisabled = 23,
    RiskStateBlocked = 24,
    PositionHealthy = 25,
    TooEarly = 26,
    MutationPending = 27,
    MarketNotEmpty = 28,
    StateVersionMismatch = 29,
    ConfigTimelockNotElapsed = 30,
    UpgradeTimelockNotElapsed = 31,
    InsufficientCollateral = 32,
    MarketLimitExceeded = 33,
    ConfigProposalExpired = 34,

    InvalidAmount = 40,
    InvalidConfig = 41,
    InvalidOrder = 42,
    WrongActionKind = 46,
    UpgradeHashMismatch = 47,

    PriceUnavailable = 60,
    StalePrice = 61,

    InvariantViolation = 70,

    ArithmeticError = 80,
}
