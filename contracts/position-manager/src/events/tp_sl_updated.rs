use shared::Position;
use soroban_sdk::{contractevent, Address, Env, Symbol};

/// Take-profit / stop-loss triggers changed on an open position. Zero means
/// no trigger set.
#[contractevent(topics = ["tpsl"], data_format = "map")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TpSlUpdated {
    #[topic]
    pub position_id: u64,
    pub owner: Address,
    pub market: Symbol,
    pub take_profit: i128,
    pub stop_loss: i128,
}

pub fn emit_tp_sl_updated(env: &Env, position: &Position) {
    TpSlUpdated {
        position_id: position.id,
        owner: position.owner.clone(),
        market: position.market.clone(),
        take_profit: position.take_profit,
        stop_loss: position.stop_loss,
    }
    .publish(env);
}
