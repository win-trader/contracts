use super::EventHeader;
use shared::Position;
use soroban_sdk::{contractevent, Address, Env};

#[contractevent(topics = ["colladd"], data_format = "map")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CollateralAdded {
    #[topic]
    pub position_id: u64,
    pub header: EventHeader,
    pub owner: Address,
    pub amount: i128,
    pub stored_collateral: i128,
}

pub fn emit_collateral_added(env: &Env, position: &Position, amount: i128) {
    CollateralAdded {
        position_id: position.id,
        header: super::header(env, &position.market, &position.owner),
        owner: position.owner.clone(),
        amount,
        stored_collateral: position.stored_collateral,
    }
    .publish(env);
}
