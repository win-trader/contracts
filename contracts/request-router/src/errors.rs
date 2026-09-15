use soroban_sdk::contracterror;

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum RequestRouterError {
    InvalidAmount = 1,
    InvalidRequest = 2,
    TooEarly = 3,
    QueueBlocked = 4,
    LpActionBlocked = 5,
    /// Reserved. Its raiser went with the oracle router; P9-01b decides
    /// this contract's error range and renumbers the enum.
    Reserved6 = 6,
    Unauthorized = 7,
    /// `upgrade` called with no pending proposal.
    UpgradeNoPending = 8,
    /// `upgrade` called before the proposal's timelock eta.
    UpgradeTimelockNotElapsed = 9,
    /// `upgrade` called with a hash that differs from the proposal.
    UpgradeHashMismatch = 10,
}
