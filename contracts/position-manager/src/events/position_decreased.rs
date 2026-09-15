use crate::settle::SettleHeader;
use soroban_sdk::{contractevent, Address, Env, Symbol};

/// A partial close (§12.2). Fee fields are the amounts actually collected in
/// this settlement; `funding_received` is the credit capitalized from the
/// guaranteed receiver claim.
#[contractevent(topics = ["posdec"], data_format = "map")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PositionDecreased {
    #[topic]
    pub position_id: u64,
    pub owner: Address,
    pub market: Symbol,
    pub size_removed: i128,
    pub price: i128,
    pub raw_pnl: i128,
    pub payable_pnl: i128,
    /// §7.9 — realized profit stays in the position as stored collateral;
    /// there is no withdrawal leg. This is the resulting balance.
    pub stored_collateral: i128,
    /// §11.1 closing fee collected out of the realized winnings.
    pub closing_fee: i128,
    pub keeper_reward: i128,
    pub receiver_funding_paid: i128,
    pub lp_funding_paid: i128,
    pub borrow_paid: i128,
    pub funding_received: i128,
    pub loss_collected: i128,
}

pub fn emit_decreased(env: &Env, s: &SettleHeader) {
    PositionDecreased {
        position_id: s.position_id,
        owner: s.owner.clone(),
        market: s.market.clone(),
        size_removed: s.size_removed,
        price: s.price,
        raw_pnl: s.raw_pnl,
        payable_pnl: s.payable_pnl,
        stored_collateral: s.stored_collateral,
        closing_fee: s.closing_fee,
        keeper_reward: s.keeper_reward,
        receiver_funding_paid: s.fees.receiver_funding_paid,
        lp_funding_paid: s.fees.lp_funding_paid,
        borrow_paid: s.fees.borrow_paid,
        funding_received: s.fees.receiver_credit,
        loss_collected: s.fees.loss_collected,
    }
    .publish(env);
}
