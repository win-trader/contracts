use soroban_sdk::contracterror;

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum MarketGovernorError {
    Unauthorized = 500,

    NoPendingConfig = 510,

    ConfigTimelockNotElapsed = 520,
    ConfigProposalExpired = 521,

    InvalidConfig = 530,
    PriceUnavailable = 531,

    UpgradeNoPending = 540,
    UpgradeTimelockNotElapsed = 541,
    UpgradeHashMismatch = 542,
}
