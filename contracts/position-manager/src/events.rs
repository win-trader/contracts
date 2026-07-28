use soroban_sdk::{contractevent, Address, Symbol};

#[contractevent(topics = ["posopen"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PositionOpened {
    pub position_id: u64,
    pub owner: Address,
    pub market: Symbol,
}

#[contractevent(topics = ["baddebt"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BadDebt {
    pub position_id: u64,
    pub amount: i128,
}
