//! §12.5 — the request router's error range, `400–499`, grouped by cause.
//!
//! **P9-01b decision.** §12.5's table names four owners and the protocol has
//! five contracts, so the router needed a range of its own. It gets
//! `400–499` rather than sharing the vault's `100–199`, because `resolve_next`
//! calls straight into the vault: a shared range would make a code coming
//! back from that call ambiguous between the two halves of the LP path,
//! which is precisely the collision disjoint ranges exist to prevent. Relying
//! on §12.5's wrapping rule instead would make correctness depend on every
//! call site remembering to wrap.
//!
//! `200–299` is left **vacant**. It belonged to the oracle router, which was
//! deleted rather than replaced, and recycling it would make an old code from
//! a decommissioned deployment look like a live one from this contract.

use soroban_sdk::contracterror;

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum RequestRouterError {
    // -- Authorization. --
    Unauthorized = 400,

    // -- Not found. --
    /// No request with this id, or the FIFO head is not `Pending`.
    InvalidRequest = 410,

    // -- State. --
    /// The vault is not accepting new LP requests: it is short of its claims
    /// or a market side is in `ADL`/`HardCap` (§7.17).
    LpActionBlocked = 420,

    // -- Validation. --
    /// A deposit must exceed `keeper_lp_resolve_reward`, and a withdrawal
    /// must be a positive number of shares (§7.17).
    InvalidAmount = 430,

    // -- Upgrade. --
    UpgradeNoPending = 440,
    UpgradeTimelockNotElapsed = 441,
    UpgradeHashMismatch = 442,
}
