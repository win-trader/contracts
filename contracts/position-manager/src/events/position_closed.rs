use super::CloseReason;
use crate::settle::CloseSummary;
use soroban_sdk::{contractevent, Address, Env, Symbol};

/// A full close via any path — `reason` distinguishes trader close,
/// liquidation, ADL, and triggered orders.
#[contractevent(topics = ["posclose"], data_format = "map")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PositionClosed {
    #[topic]
    pub position_id: u64,
    pub owner: Address,
    pub market: Symbol,
    pub reason: CloseReason,
    pub size: i128,
    pub price: i128,
    pub raw_pnl: i128,
    pub payable_pnl: i128,
    /// Residual collateral paid to the owner after the waterfall.
    pub collateral_payout: i128,
    pub bad_debt: i128,
    pub liquidation_reward: i128,
    pub execution_budget_refunded: i128,
    /// §11.1 closing fee collected out of the realized winnings.
    pub closing_fee: i128,
    pub receiver_funding_paid: i128,
    pub lp_funding_paid: i128,
    pub borrow_paid: i128,
    pub funding_received: i128,
    /// Negative price PnL collected from collateral; with `bad_debt` this
    /// disambiguates the loss-vs-funding split of the waterfall.
    pub loss_collected: i128,
}

pub fn emit_closed(env: &Env, s: &CloseSummary, reason: CloseReason) {
    PositionClosed {
        position_id: s.position_id,
        owner: s.owner.clone(),
        market: s.market.clone(),
        reason,
        size: s.size_removed,
        price: s.price,
        raw_pnl: s.raw_pnl,
        payable_pnl: s.payable_pnl,
        collateral_payout: s.collateral_payout,
        bad_debt: s.bad_debt,
        liquidation_reward: s.liquidation_reward,
        execution_budget_refunded: s.execution_budget_refunded,
        closing_fee: s.closing_fee,
        receiver_funding_paid: s.fees.receiver_funding_paid,
        lp_funding_paid: s.fees.lp_funding_paid,
        borrow_paid: s.fees.borrow_paid,
        funding_received: s.fees.receiver_credit,
        loss_collected: s.fees.loss_collected,
    }
    .publish(env);
}
