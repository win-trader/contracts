//! §12.5 — the position manager's error range, `1–99`.
//!
//! Errors are part of the interface. A caller that cannot distinguish "your
//! price bound was missed" from "the feed had nothing to say" cannot report
//! anything useful, and a front end that guesses will guess wrong.
//!
//! Two rules shape this enum. Ranges are **disjoint across contracts**, so a
//! raw code is globally unambiguous without knowing which contract produced
//! it — every contract numbering from `1` is what previously gave four
//! contracts a code `9` each, with opposite remedies. And codes are grouped
//! by **cause**, so a caller can react to a class without enumerating its
//! members.
//!
//! What is deliberately *not* here: expected terminal failures. Slippage,
//! insufficient capacity, an exposure cap, and a blocked side all complete
//! successfully and record `FailureReason` in a result (§8.9, §12.5). They
//! are outcomes, not errors, and `SlippageExceeded` and `CapacityExceeded`
//! were removed from this enum when Phase 6 made them so.
//!
//! `Accounting` and `Arithmetic` must never be reachable through ordinary
//! use. If a well-formed call can trigger either, that is a defect, not a
//! user error.

use soroban_sdk::contracterror;

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum PositionManagerError {
    // -- Authorization: the caller is not who this operation requires. --
    /// The caller does not hold the required ConfigManager role.
    Unauthorized = 1,
    /// The caller is not the specific contract this entry point admits.
    InvalidCaller = 2,

    // -- Not found: the identifier does not exist, or was consumed. --
    PositionNotFound = 10,
    MarketNotConfigured = 11,
    /// No pending action for this id — **including** one whose id a terminal
    /// settlement already consumed (§8.13). That is what stops a second
    /// caller replaying a transfer or a keeper payment.
    ActionNotFound = 12,
    ReferralCodeNotFound = 13,
    /// The named trigger is not attached to the position.
    TriggerNotAttached = 14,
    /// §12.3 — `apply_configuration` with no proposal stored.
    NoPendingConfig = 15,
    /// `upgrade` called with no pending proposal.
    UpgradeNoPending = 16,

    // -- State: the vault or market forbids this operation right now. --
    NotInitialized = 20,
    AlreadyInitialized = 21,
    /// §12.2 operationally paused. Distinct from `RiskStateBlocked`: a pause
    /// is a vault-wide decision by an authority, a risk state is a
    /// consequence of the book, and the remedies differ.
    Paused = 22,
    MarketDisabled = 23,
    /// The market side will not take new exposure (§6.16.1).
    RiskStateBlocked = 24,
    /// Liquidation attempted on a position above its threshold.
    PositionHealthy = 25,
    /// A timing gate that is not a settlement gate — settlement gates
    /// return `NotReady` rather than reverting (§8.5).
    TooEarly = 26,
    /// §8.4 — the position already has a pending increase, decrease, or
    /// close.
    MutationPending = 27,
    /// §5.3 — `market_risk_factor_bps` changed while the market still has
    /// open interest or risk units on either side.
    MarketNotEmpty = 28,
    /// §12.4 — the stored ledger's `state_version` is not the one this build
    /// understands. Every operation rejects until a migration advances it.
    StateVersionMismatch = 29,
    /// §12.3 — `apply_configuration` before the proposal's `effective_at`.
    ConfigTimelockNotElapsed = 30,
    UpgradeTimelockNotElapsed = 31,
    /// The position or escrow cannot cover an amount this operation
    /// requires. At a *settlement* this is a `FailureReason` instead; here
    /// it is a creation-time rejection.
    InsufficientCollateral = 32,
    /// The active-market registry is full (§10.3.1).
    MarketLimitExceeded = 33,

    // -- Validation: the arguments are structurally invalid. --
    InvalidAmount = 40,
    InvalidConfig = 41,
    InvalidOrder = 42,
    /// `register_referral_code` for a code that is already owned.
    ReferralCodeTaken = 43,
    /// A referral code failed the length/format bounds.
    ReferralCodeInvalid = 44,
    /// A trader tried to set their own code as their referrer.
    SelfReferral = 45,
    /// §8.9 — a settlement call named an action of a different kind, or a
    /// reverse reference that does not agree with the action it names.
    WrongActionKind = 46,
    UpgradeHashMismatch = 47,

    // -- Oracle: no qualifying price. --
    /// The external feed returned no price for the symbol, or a
    /// non-positive one.
    PriceUnavailable = 60,
    /// The feed's latest observation is older than `max_price_age_seconds`.
    /// Distinct from `PriceUnavailable`: the feed answered, the answer is
    /// just too old to act on.
    StalePrice = 61,

    // -- Accounting: a §9 invariant would break. Never user-reachable. --
    /// E.g. a negative pending fee, which means a decreasing index or a
    /// corrupted debt baseline (§11.2).
    InvariantViolation = 70,

    // -- Arithmetic: overflow or a failed §2.1.1 narrowing check. --
    ArithmeticError = 80,
}
