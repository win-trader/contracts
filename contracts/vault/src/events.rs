use soroban_sdk::{contractevent, Address};

use shared::LpConfig;

#[contractevent(topics = ["lpdep"], data_format = "map")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DepositSettled {
    pub owner: Address,
    pub assets: i128,
    pub shares: i128,
    pub share_supply: i128,
    pub vault_nav: i128,
}

#[contractevent(topics = ["lpwd"], data_format = "map")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WithdrawalSettled {
    pub owner: Address,
    pub shares: i128,
    pub assets: i128,
    pub share_supply: i128,
    pub vault_nav: i128,
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
