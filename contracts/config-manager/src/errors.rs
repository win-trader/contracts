//! §12.5 — the configuration manager's error range, `300–399`, grouped by
//! cause. Disjoint from every other contract's.

use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ConfigManagerError {
    // -- Authorization. --
    Unauthorized = 300,
    /// The caller is not the address a pending admin handover names.
    NotPendingAdmin = 301,

    // -- Not found. --
    NoPendingAdmin = 310,
    NoPendingUpgrade = 311,

    // -- State. --
    NotInitialized = 320,
    AlreadyInitialized = 321,
    UpgradeTimelockNotElapsed = 322,
    AdminProposalExpired = 323,

    // -- Validation. --
    InvalidAdminProposal = 330,
    UpgradeTimelockTooShort = 331,
    UpgradeTimelockTooLong = 332,
    UpgradeHashMismatch = 333,
}
