use crate::settle::SettleHeader;
use super::EventHeader;
use soroban_sdk::{contractevent, Address, Env};

#[contractevent(topics = ["posdec"], data_format = "map")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PositionDecreased {
    #[topic]
    pub position_id: u64,
    pub header: EventHeader,
    pub owner: Address,
    pub size_removed: i128,
    pub price: i128,
    pub raw_pnl: i128,
    pub payable_pnl: i128,
    pub stored_collateral: i128,
    pub closing_fee: i128,
    pub keeper_reward: i128,
    pub receiver_funding_paid: i128,
    pub lp_funding_paid: i128,
    pub borrow_paid: i128,
    pub funding_received: i128,
    pub loss_collected: i128,
}

pub fn emit_decreased(env: &Env, actor: &Address, s: &SettleHeader) {
    PositionDecreased {
        position_id: s.position_id,
        header: super::header(env, &s.market, actor),
        owner: s.owner.clone(),
        size_removed: s.size_removed,
        price: s.price,
        raw_pnl: s.raw_pnl,
        payable_pnl: s.signed_payable_pnl(),
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
