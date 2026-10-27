use super::EventHeader;
use shared::Position;
use soroban_sdk::{contractevent, Address, Env};

#[contractevent(topics = ["tpsl"], data_format = "map")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TpSlUpdated {
    #[topic]
    pub position_id: u64,
    pub header: EventHeader,
    pub owner: Address,
    pub take_profit: i128,
    pub stop_loss: i128,
}

pub fn emit_tp_sl_updated(env: &Env, position: &Position) {
    TpSlUpdated {
        position_id: position.id,
        header: super::header(env, &position.market, &position.owner),
        owner: position.owner.clone(),
        take_profit: position.take_profit.price(),
        stop_loss: position.stop_loss.price(),
    }
    .publish(env);
}
