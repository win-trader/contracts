use soroban_sdk::contracterror;

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum RequestRouterError {
    InvalidAmount = 1,
    InvalidRequest = 2,
    /// Reserved. §7.17 makes "not yet resolvable" a returned
    /// `SettlementStatus::NotReady` rather than an error: an LP request has
    /// no `Expired` outcome, so a premature call must leave the head pending
    /// rather than revert. P9-01b renumbers this enum.
    Reserved3 = 3,
    /// Reserved. P9-01b renumbers this enum.
    Reserved4 = 4,
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
