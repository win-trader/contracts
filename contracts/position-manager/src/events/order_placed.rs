use shared::EntryOrder;
use soroban_sdk::{contractevent, Address, Env, Symbol};

/// A limit/stop entry order was placed (storage only — no funds moved).
#[contractevent(topics = ["ordplace"], data_format = "map")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrderPlaced {
    #[topic]
    pub order_id: u64,
    pub owner: Address,
    pub market: Symbol,
    pub is_long: bool,
    pub size: i128,
    pub collateral: i128,
    pub execution_budget: i128,
    pub take_profit: i128,
    pub stop_loss: i128,
    pub acceptable_price: i128,
    pub trigger_price: i128,
    pub trigger_above: bool,
    pub expires_at: u64,
}

pub fn emit_order_placed(env: &Env, order: &EntryOrder) {
    OrderPlaced {
        order_id: order.id,
        owner: order.owner.clone(),
        market: order.market.clone(),
        is_long: order.is_long,
        size: order.size,
        collateral: order.collateral,
        execution_budget: order.execution_budget,
        take_profit: order.take_profit,
        stop_loss: order.stop_loss,
        acceptable_price: order.acceptable_price,
        trigger_price: order.trigger_price,
        trigger_above: order.trigger_above,
        expires_at: order.expires_at,
    }
    .publish(env);
}
