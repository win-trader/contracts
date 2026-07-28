//! LP share lifecycle and governance events.

use soroban_sdk::{contractevent, Address};

use shared::LpConfig;

/// A queued deposit settled: `assets` entered the vault, `shares` minted to
/// `owner` (§13.5).
#[contractevent(topics = ["lpdep"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DepositSettled {
    pub owner: Address,
    pub assets: i128,
    pub shares: i128,
}

/// A queued withdrawal settled: `shares` burned, `assets` paid to `owner`
/// (§13.6).
#[contractevent(topics = ["lpwd"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WithdrawalSettled {
    pub owner: Address,
    pub shares: i128,
    pub assets: i128,
}

#[contractevent(topics = ["cfglp"], data_format = "map")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LpConfigUpdated {
    pub config: LpConfig,
}

#[contractevent(topics = ["pause"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PauseChanged {
    pub paused: bool,
}
