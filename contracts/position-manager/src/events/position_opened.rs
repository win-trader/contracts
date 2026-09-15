use shared::Position;
use soroban_sdk::{contractevent, Address, Env, Symbol};

#[contractevent(topics = ["posopen"], data_format = "map")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PositionOpened {
    #[topic]
    pub position_id: u64,
    pub owner: Address,
    pub market: Symbol,
    pub is_long: bool,
    pub size: i128,
    pub base_exposure: i128,
    pub stored_collateral: i128,
    pub price: i128,
    /// Zero means no trigger set.
    pub take_profit: i128,
    pub stop_loss: i128,
}

pub fn emit_opened(env: &Env, position: &Position, price: i128) {
    PositionOpened {
        position_id: position.id,
        owner: position.owner.clone(),
        market: position.market.clone(),
        is_long: position.is_long,
        size: position.size,
        base_exposure: position.base_exposure,
        stored_collateral: position.stored_collateral,
        price,
        take_profit: position.take_profit.price(),
        stop_loss: position.stop_loss.price(),
    }
    .publish(env);
}
