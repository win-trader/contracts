use soroban_sdk::contracterror;

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum RequestRouterError {
    Unauthorized = 400,

    InvalidRequest = 410,

    LpActionBlocked = 420,

    InvalidAmount = 430,

    UpgradeNoPending = 440,
    UpgradeTimelockNotElapsed = 441,
    UpgradeHashMismatch = 442,
}
