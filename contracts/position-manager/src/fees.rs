use soroban_sdk::{panic_with_error, Address, Env, Symbol};

use shared::constants::BPS;
use shared::{Market, MarketConfig, Position};

use crate::errors::PositionManagerError;
use crate::events::{self, FeeSource};
use crate::funding;
use crate::ledger::{self, Ledger};
use crate::referral;
use crate::{math, storage};

#[derive(Clone, Copy, Debug, Default)]
pub struct CollectedFees {
    pub receiver_credit: i128,
    pub receiver_funding_paid: i128,
    pub lp_funding_paid: i128,
    pub borrow_paid: i128,
    pub loss_collected: i128,
    pub unpaid: i128,
}

#[allow(clippy::too_many_arguments)]
pub fn distribute_open_close_revenue(
    env: &Env,
    ledger: &mut Ledger,
    market: &Symbol,
    actor: &Address,
    collected: i128,
    owner: &Address,
    source: FeeSource,
    position_id: u64,
) -> i128 {
    if collected <= 0 {
        return 0;
    }
    let config = storage::get_global_config(env);
    let lp = math::mul_div_floor(env, collected, config.fee_lp_revenue_share_bps as i128, BPS);
    let referral = referral::accrue(env, ledger, owner, collected, position_id);
    let protocol = math::sub(env, math::sub(env, collected, lp), referral);
    ledger.credit(env, ledger::Bucket::ProtocolClaimable, protocol);
    events::emit_revenue_split(
        env, market, actor, position_id, source, collected, lp, protocol, referral,
    );
    protocol
}

pub fn distribute_borrow_revenue(
    env: &Env,
    ledger: &mut Ledger,
    market: &Symbol,
    actor: &Address,
    collected: i128,
    position_id: u64,
) {
    if collected <= 0 {
        return;
    }
    let config = storage::get_global_config(env);
    let lp = math::mul_div_floor(
        env,
        collected,
        config.borrow_lp_revenue_share_bps as i128,
        BPS,
    );
    let protocol = math::sub(env, collected, lp);
    ledger.credit(env, ledger::Bucket::ProtocolClaimable, protocol);
    events::emit_revenue_split(
        env,
        market,
        actor,
        position_id,
        FeeSource::Borrow,
        collected,
        lp,
        protocol,
        0,
    );
}

pub fn calculate_opening_fee(env: &Env, added_size: i128, config: &MarketConfig) -> i128 {
    if added_size <= 0 {
        panic_with_error!(env, PositionManagerError::InvalidAmount);
    }
    math::mul_div_ceil(env, added_size, config.open_fee_bps as i128, BPS)
}

#[allow(clippy::too_many_arguments)]
pub fn calculate_closing_fee(
    env: &Env,
    size_removed: i128,
    payable_pnl: i128,
    pending: &funding::PendingFees,
    borrow_due: i128,
    keeper_reward: i128,
    config: &MarketConfig,
) -> i128 {
    if payable_pnl <= 0 {
        return 0;
    }
    let size_component = math::mul_div_ceil(env, size_removed, config.close_size_fee_bps as i128, BPS);
    let pnl_component = math::mul_div_ceil(env, payable_pnl, config.close_pnl_fee_bps as i128, BPS);
    let nominal = core::cmp::min(
        payable_pnl,
        core::cmp::max(size_component, pnl_component),
    );
    let mut after_senior = math::add(env, payable_pnl, pending.funding_received);
    after_senior = math::sub(env, after_senior, pending.funding_paid_to_receivers);
    after_senior = math::sub(env, after_senior, pending.funding_paid_to_lps);
    after_senior = math::sub(env, after_senior, borrow_due);
    after_senior = math::sub(env, after_senior, keeper_reward);
    core::cmp::min(nominal, core::cmp::max(after_senior, 0))
}

fn credit_received_funding(env: &Env, ledger: &mut Ledger, market: &mut Market, amount: i128) {
    if amount <= 0 {
        return;
    }
    if market.pending_receiver_funding < amount
        || ledger.pending_receiver_funding_total < amount
    {
        panic_with_error!(env, PositionManagerError::InvariantViolation);
    }
    market.pending_receiver_funding = math::sub(env, market.pending_receiver_funding, amount);
    ledger.release(env, ledger::Bucket::ReceiverFunding, amount);
}

pub fn capitalize_for_surviving_mutation(
    env: &Env,
    ledger: &mut Ledger,
    market_id: &Symbol,
    actor: &Address,
    position: &mut Position,
    market: &mut Market,
) -> CollectedFees {
    let pending = funding::pending_fees(env, ledger, position, market);
    credit_received_funding(env, ledger, market, pending.funding_received);
    {
        let is_long = position.is_long;
        let side = market.side_mut(is_long);
        if pending.funding_received > 0 {
            ledger::add_stored_collateral(env, ledger, position, side, pending.funding_received);
        }
    }
    let owed = math::add(
        env,
        math::add(env, pending.funding_paid_to_receivers, pending.funding_paid_to_lps),
        pending.borrow,
    );
    if position.stored_collateral < owed {
        panic_with_error!(env, PositionManagerError::InsufficientCollateral);
    }
    let (receiver_collected, lp_collected, borrow_collected) = {
        let is_long = position.is_long;
        let side = market.side_mut(is_long);
        (
            ledger::collect_stored_collateral(
                env,
                ledger,
                position,
                side,
                pending.funding_paid_to_receivers,
            ),
            ledger::collect_stored_collateral(env, ledger, position, side, pending.funding_paid_to_lps),
            ledger::collect_stored_collateral(env, ledger, position, side, pending.borrow),
        )
    };
    distribute_borrow_revenue(env, ledger, market_id, actor, borrow_collected, position.id);
    funding::snapshot_funding_indices(position, market);
    CollectedFees {
        receiver_credit: pending.funding_received,
        receiver_funding_paid: receiver_collected,
        lp_funding_paid: lp_collected,
        borrow_paid: borrow_collected,
        loss_collected: 0,
        unpaid: 0,
    }
}

pub fn capitalize(
    env: &Env,
    ledger: &mut Ledger,
    market_id: &Symbol,
    actor: &Address,
    position: &mut Position,
    market: &mut Market,
    negative_pnl: i128,
) -> CollectedFees {
    let pending = funding::pending_fees(env, ledger, position, market);
    let receiver_credit = pending.funding_received;

    let (receiver_collected, loss_collected, lp_collected, borrow_collected) = {
        let is_long = position.is_long;
        credit_received_funding(env, ledger, market, receiver_credit);
        let side = market.side_mut(is_long);
        if receiver_credit > 0 {
            ledger::add_stored_collateral(env, ledger, position, side, receiver_credit);
        }
        let receiver_collected = ledger::collect_stored_collateral(
            env,
            ledger,
            position,
            side,
            pending.funding_paid_to_receivers,
        );
        let loss_collected =
            ledger::collect_stored_collateral(env, ledger, position, side, negative_pnl);
        let lp_collected = ledger::collect_stored_collateral(
            env,
            ledger,
            position,
            side,
            pending.funding_paid_to_lps,
        );
        let borrow_collected =
            ledger::collect_stored_collateral(env, ledger, position, side, pending.borrow);
        (
            receiver_collected,
            loss_collected,
            lp_collected,
            borrow_collected,
        )
    };

    distribute_borrow_revenue(env, ledger, market_id, actor, borrow_collected, position.id);
    funding::snapshot_funding_indices(position, market);

    let guaranteed_and_loss = math::add(
        env,
        math::sub(env, pending.funding_paid_to_receivers, receiver_collected),
        math::sub(env, negative_pnl, loss_collected),
    );
    let unpaid = math::add(
        env,
        math::add(
            env,
            guaranteed_and_loss,
            math::sub(env, pending.funding_paid_to_lps, lp_collected),
        ),
        math::sub(env, pending.borrow, borrow_collected),
    );
    CollectedFees {
        receiver_credit,
        receiver_funding_paid: receiver_collected,
        lp_funding_paid: lp_collected,
        borrow_paid: borrow_collected,
        loss_collected,
        unpaid,
    }
}
