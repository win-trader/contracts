use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ConfigManagerError {
    Unauthorized = 300,
    NotPendingAdmin = 301,

    NoPendingAdmin = 310,
    NoPendingUpgrade = 311,

    NotInitialized = 320,
    AlreadyInitialized = 321,
    UpgradeTimelockNotElapsed = 322,
    AdminProposalExpired = 323,

    InvalidAdminProposal = 330,
    UpgradeTimelockTooShort = 331,
    UpgradeTimelockTooLong = 332,
    UpgradeHashMismatch = 333,
}
