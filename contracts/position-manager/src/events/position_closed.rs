use super::CloseReason;
use crate::settle::{ClosedTail, SettleHeader};
use super::EventHeader;
use soroban_sdk::{contractevent, Address, Env};

#[contractevent(topics = ["posclose"], data_format = "map")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PositionClosed {
    #[topic]
    pub position_id: u64,
    pub header: EventHeader,
    pub owner: Address,
    pub reason: CloseReason,
    pub size: i128,
    pub price: i128,
    pub raw_pnl: i128,
    pub payable_pnl: i128,
    pub collateral_payout: i128,
    pub bad_debt: i128,
    pub unpaid_profit: i128,
    pub closing_fee: i128,
    pub keeper_reward: i128,
    pub keeper_from_lp_backstop: i128,
    pub keeper_unpaid: i128,
    pub effective_collateral: i128,
    pub liquidation_threshold: i128,
    pub payout_factor: i128,
    pub receiver_funding_paid: i128,
    pub lp_funding_paid: i128,
    pub borrow_paid: i128,
    pub funding_received: i128,
    pub loss_collected: i128,
}

pub fn emit_closed(
    env: &Env,
    actor: &Address,
    s: &SettleHeader,
    tail: &ClosedTail,
    reason: CloseReason,
) {
    PositionClosed {
        position_id: s.position_id,
        header: super::header(env, &s.market, actor),
        owner: s.owner.clone(),
        reason,
        size: s.size_removed,
        price: s.price,
        raw_pnl: s.raw_pnl,
        payable_pnl: s.signed_payable_pnl(),
        collateral_payout: tail.collateral_payout,
        bad_debt: tail.bad_debt,
        unpaid_profit: s.unpaid_profit,
        closing_fee: s.closing_fee,
        keeper_reward: s.keeper_reward,
        keeper_from_lp_backstop: s.keeper_from_lp_backstop,
        keeper_unpaid: s.keeper_unpaid,
        effective_collateral: s.effective_collateral,
        liquidation_threshold: s.liquidation_threshold,
        payout_factor: s.payout_factor,
        receiver_funding_paid: s.fees.receiver_funding_paid,
        lp_funding_paid: s.fees.lp_funding_paid,
        borrow_paid: s.fees.borrow_paid,
        funding_received: s.fees.receiver_credit,
        loss_collected: s.fees.loss_collected,
    }
    .publish(env);
}
