use crate::fees::CollectedFees;
use shared::Position;
use soroban_sdk::{contractevent, Address, Env, Symbol};

#[contractevent(topics = ["posinc"], data_format = "map")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PositionIncreased {
    #[topic]
    pub position_id: u64,
    pub owner: Address,
    pub market: Symbol,
    pub size_added: i128,
    pub base_added: i128,
    pub collateral_added: i128,
    pub price: i128,
    /// Stored collateral after capitalization.
    pub stored_collateral: i128,
    /// Accrued amounts the increase capitalized before adding new size —
    /// the same decomposition the decrease/close events carry.
    pub receiver_funding_paid: i128,
    pub lp_funding_paid: i128,
    pub borrow_paid: i128,
    pub funding_received: i128,
}

pub fn emit_increased(
    env: &Env,
    position: &Position,
    size_added: i128,
    base_added: i128,
    collateral_added: i128,
    price: i128,
    collected: &CollectedFees,
) {
    PositionIncreased {
        position_id: position.id,
        owner: position.owner.clone(),
        market: position.market.clone(),
        size_added,
        base_added,
        collateral_added,
        price,
        stored_collateral: position.stored_collateral,
        receiver_funding_paid: collected.receiver_funding_paid,
        lp_funding_paid: collected.lp_funding_paid,
        borrow_paid: collected.borrow_paid,
        funding_received: collected.receiver_credit,
    }
    .publish(env);
}
