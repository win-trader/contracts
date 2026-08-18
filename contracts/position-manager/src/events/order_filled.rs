use soroban_sdk::{contractevent, Address, Env};

/// An entry order was filled: the collateral was pulled and a position
/// opened at `fill_price`. The order record is removed.
#[contractevent(topics = ["ordfill"], data_format = "map")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrderFilled {
    #[topic]
    pub order_id: u64,
    pub position_id: u64,
    pub executor: Address,
    pub fill_price: i128,
}

pub fn emit_order_filled(
    env: &Env,
    order_id: u64,
    position_id: u64,
    executor: &Address,
    fill_price: i128,
) {
    OrderFilled {
        order_id,
        position_id,
        executor: executor.clone(),
        fill_price,
    }
    .publish(env);
}
