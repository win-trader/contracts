use soroban_sdk::contracterror;

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum PositionManagerError {
    Unauthorized = 1,
    NotInitialized = 2,
    AlreadyInitialized = 3,
    InvalidAmount = 4,
    InvalidConfig = 5,
    PositionNotFound = 6,
    MarketNotConfigured = 7,
    MarketDisabled = 8,
    SlippageExceeded = 9,
    CapacityExceeded = 10,
    MarketLimitExceeded = 11,
    InsufficientCollateral = 12,
    PositionHealthy = 13,
    RiskStateBlocked = 14,
    ArithmeticError = 15,
    InvalidOracleRound = 16,
    TooEarly = 17,
    InvalidOrder = 18,
    InsufficientExecutionBudget = 19,
    InvalidCaller = 20,
    /// The contract is operationally paused (distinct from a risk state).
    Paused = 21,
    /// An accounting invariant broke — e.g. a negative pending fee, which
    /// means a decreasing index or corrupted debt baseline (§11.2).
    InvariantViolation = 22,
    /// `upgrade` called with no pending proposal.
    UpgradeNoPending = 23,
    /// `upgrade` called before the proposal's timelock eta.
    UpgradeTimelockNotElapsed = 24,
    /// `upgrade` called with a hash that differs from the proposal.
    UpgradeHashMismatch = 25,
}
