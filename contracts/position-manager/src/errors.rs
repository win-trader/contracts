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
    /// Reserved. Its raiser went with the oracle router; prices now come
    /// from an external feed and fail as `PriceUnavailable` or `StalePrice`.
    Reserved16 = 16,
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
    /// Reserved. Entry orders became `PendingAction`s in Phase 6; absence
    /// is `ActionNotFound`.
    Reserved26 = 26,
    /// Reserved. An untriggered observation is now the non-terminal
    /// `Pending` outcome (§7.4), not an error; P9-01 renumbers the enum, so
    /// the code is not reused before then.
    Reserved27 = 27,
    /// `register_referral_code` for a code that is already owned.
    ReferralCodeTaken = 28,
    /// A referral code failed the length/format bounds.
    ReferralCodeInvalid = 29,
    /// `set_referrer` for a code no one has registered.
    ReferralCodeNotFound = 30,
    /// A trader tried to set their own code as their referrer.
    SelfReferral = 31,
    /// §12.4 — the stored ledger's `state_version` is not the one this
    /// build understands. Every operation rejects until a migration has
    /// advanced it.
    StateVersionMismatch = 32,
    /// No pending action exists for the given id — including one whose id
    /// was consumed by a terminal settlement (§8.13).
    ActionNotFound = 33,
    /// §12.3 — `apply_configuration` with no proposal stored.
    NoPendingConfig = 34,
    /// §12.3 — `apply_configuration` before the proposal's `effective_at`.
    ConfigTimelockNotElapsed = 35,
    /// §5.3 — `market_risk_factor_bps` changed while the market still has
    /// open interest or risk units on either side.
    MarketNotEmpty = 36,
    /// The external price feed returned no price for the symbol, or a
    /// non-positive one.
    PriceUnavailable = 37,
    /// The feed's latest observation is older than `max_price_age_seconds`.
    /// Distinct from `PriceUnavailable`: the feed answered, the answer is
    /// just too old to act on.
    StalePrice = 38,
    /// §8.9 — a settlement call named an action of a different kind. An
    /// unexpected condition: it reverts and leaves the action untouched.
    WrongActionKind = 39,
    /// §8.4 — the position already has a pending increase, decrease, or
    /// close. At most one ordinary mutation may be committed against a
    /// given pre-action state.
    MutationPending = 40,
    /// The named trigger is not attached to the position.
    TriggerNotAttached = 41,
}
