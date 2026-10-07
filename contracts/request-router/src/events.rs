use soroban_sdk::{contractevent, Address};

use shared::{LpRequestKind, LpRequestStatus};

#[contractevent(topics = ["lpreq"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LpRequestCreated {
    pub request_id: u64,
    pub owner: Address,
    pub kind: LpRequestKind,
    pub amount: i128,
    pub reward: i128,
    pub execute_after: u64,
}

#[contractevent(topics = ["lpres"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LpRequestResolved {
    pub request_id: u64,
    pub owner: Address,
    pub kind: LpRequestKind,
    pub status: LpRequestStatus,
    pub settled_amount: i128,
    pub reward: i128,
}

#[contractevent(topics = ["lpdefer"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LpPayoutDeferred {
    pub owner: Address,
    pub amount: i128,
}

#[contractevent(topics = ["lpskip"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LpRequestSkipped {
    pub request_id: u64,
    pub caller: Address,
}

#[contractevent(topics = ["lpclaim"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LpPayoutClaimed {
    pub owner: Address,
    pub amount: i128,
}
