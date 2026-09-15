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
    /// Reserved. Its raiser was deleted with the execution budget (P1-01);
    /// P9-01 renumbers the whole enum, so it is not reused before then.
    Reserved19 = 19,
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
    /// No entry order exists for the given id.
    OrderNotFound = 26,
    /// `execute_entry_order` called before the trigger price was crossed.
    OrderNotTriggered = 27,
    /// `register_referral_code` for a code that is already owned.
    ReferralCodeTaken = 28,
    /// A referral code failed the length/format bounds.
    ReferralCodeInvalid = 29,
    /// `set_referrer` for a code no one has registered.
    ReferralCodeNotFound = 30,
    /// A trader tried to set their own code as their referrer.
    SelfReferral = 31,
}
