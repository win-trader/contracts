use soroban_sdk::{contractevent, Address, Symbol};

#[contractevent(topics = ["role"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoleChange {
    pub role: Symbol,
    pub account: Address,
    pub is_grant: bool,
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
