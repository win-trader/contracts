use soroban_sdk::{contractevent, Address, Symbol};

#[contractevent(topics = ["role"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoleChange {
    pub role: Symbol,
    pub account: Address,
    pub is_grant: bool,
}

#[contractevent(topics = ["feecfg"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeeSplitsUpdate {
    pub lp_bps: u32,
    pub dev_bps: u32,
    pub staker_bps: u32,
}

#[contractevent(topics = ["feecnf"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeeConfigUpdate {
    pub open_fee_bps: u32,
    pub liquidation_bounty_bps: u32,
    pub tp_sl_execution_fee: i128,
}

#[contractevent(topics = ["limits"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LimitsUpdate {
    pub min_collateral: i128,
    pub cooldown_duration: u64,
    pub min_position_lifetime: u64,
    pub max_utilization_ratio: i128,
    pub funding_cut_bps: u32,
    pub adl_pnl_bps: u32,
    pub adl_utilization_bps: u32,
    pub liquidation_threshold_bps: u32,
}

#[contractevent(topics = ["rates"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BorrowRateUpdate {
    pub base_borrow_rate_bps: i128,
    pub slope1_bps: i128,
    pub slope2_bps: i128,
    pub optimal_utilization_bps: i128,
    pub base_funding_rate_bps: i128,
}

#[contractevent(topics = ["upgtl"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpgradeTimelockUpdate {
    pub timelock_seconds: u64,
}

#[contractevent(topics = ["adminprop"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminProposed {
    pub proposer: Address,
    pub new_admin: Address,
}

#[contractevent(topics = ["admincxl"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminProposalCancelled {
    pub canceller: Address,
}

// Upgrade events live in `interfaces::events` — the
// `TimelockedUpgradeable` trait's default methods emit them, so no
// per-contract definition is needed here.
