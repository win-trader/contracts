use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ConfigManagerError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    UpgradeTimelockTooShort = 6,
    InvalidAdminProposal = 7,
    NotPendingAdmin = 8,
    NoPendingAdmin = 9,
    NoPendingUpgrade = 10,
    UpgradeTimelockNotElapsed = 11,
    UpgradeHashMismatch = 12,
    AdminProposalExpired = 13,
    UpgradeTimelockTooLong = 14,
}
