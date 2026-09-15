//! Position fee accounting — doc §11.
//!
//! Capitalization settles every accrued amount against stored collateral in
//! the §11.4 collection order (receiver-backed funding, then negative price
//! PnL, then LP-backed funding, then borrow) so a shortfall lands on the
//! least-protected claim, then resets the debt baselines.

use soroban_sdk::{panic_with_error, Address, Env};

use shared::constants::BPS;
use shared::{Market, MarketConfig, Position};

use crate::errors::PositionManagerError;
use crate::events::{self, FeeSource};
use crate::funding;
use crate::ledger::{self, Ledger};
use crate::referral;
use crate::{math, storage};

/// What one capitalization actually moved. Feeds the settlement events and
/// the close waterfall's bad-debt calculation.
#[derive(Clone, Copy, Debug, Default)]
pub struct CollectedFees {
    /// Funding credit moved from the guaranteed receiver claim into stored
    /// collateral (label move — total non-LP claims unchanged, §8.3).
    pub receiver_credit: i128,
    /// Receiver-backed payer funding collected from collateral.
    pub receiver_funding_paid: i128,
    /// LP-backed payer funding collected from collateral.
    pub lp_funding_paid: i128,
    /// Borrow fee collected from collateral.
    pub borrow_paid: i128,
    /// Negative price PnL collected from collateral.
    pub loss_collected: i128,
    /// Accrued obligations the position value could not cover.
    pub unpaid: i128,
}

/// §6.11 — distribute a collected **opening or closing** fee.
///
/// The LP share is computed off the full collected amount and receives no
/// stored credit: leaving it in the residual is what credits LPs. The
/// referral share is carved from the protocol slice, so the LP share is
/// never diluted, and the protocol takes the **exact remainder** — there is
/// no configured protocol percentage to disagree with it.
///
/// The fee must already have been removed from position collateral or action
/// escrow before this runs.
pub fn distribute_open_close_revenue(
    env: &Env,
    ledger: &mut Ledger,
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
    // §3.6 — a referred trader diverts a share of **both** the opening and
    // the closing fee. The superseded implementation accrued only on close.
    let referral = referral::accrue(env, ledger, owner, collected, position_id);
    let protocol = math::sub(env, math::sub(env, collected, lp), referral);
    ledger.credit(env, ledger::Bucket::ProtocolClaimable, protocol);
    events::emit_revenue_split(env, position_id, source, collected, lp, protocol, referral);
    protocol
}

/// §6.11 — distribute collected **borrow**. A separate split, with its own
/// LP share and no referral component. Funding never calls either
/// distribution function.
pub fn distribute_borrow_revenue(
    env: &Env,
    ledger: &mut Ledger,
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
        position_id,
        FeeSource::Borrow,
        collected,
        lp,
        protocol,
        0,
    );
}

/// §6.8 — the opening fee on added size.
///
/// Applied to an initial open's full size and to an increase's added size
/// only. A collateral-only addition never calls this.
pub fn calculate_opening_fee(env: &Env, added_size: i128, config: &MarketConfig) -> i128 {
    if added_size <= 0 {
        panic_with_error!(env, PositionManagerError::InvalidAmount);
    }
    math::mul_div_ceil(env, added_size, config.open_fee_bps as i128, BPS)
}

/// §6.9 — the closing fee, in its four parts.
#[derive(Clone, Copy, Debug, Default)]
pub struct ClosingFee {
    /// Reported for §12.6's fee event, which P9-07 requires to distinguish
    /// the charged target from what was actually collected.
    #[allow(dead_code)]
    pub size_component: i128,
    #[allow(dead_code)]
    pub pnl_component: i128,
    /// What the schedule asks for, capped at payable profit.
    #[allow(dead_code)]
    pub nominal: i128,
    /// What can actually be taken after every senior item. Only this is
    /// debited; `nominal - collectible` is waived immediately, is never
    /// stored, and is **not** bad debt.
    pub collectible: i128,
}

/// §6.9 — `max(size component, PnL component)`, capped at payable profit,
/// then capped again at the profit left after every senior item.
///
/// The two components have no ordering relationship by design: the charged
/// target is their maximum and the profit caps provide the economic bound.
/// A loser pays nothing, so there is no shortfall path.
#[allow(clippy::too_many_arguments)]
pub fn calculate_closing_fee(
    env: &Env,
    size_removed: i128,
    payable_pnl: i128,
    pending: &funding::PendingFees,
    borrow_due: i128,
    keeper_reward: i128,
    config: &MarketConfig,
) -> ClosingFee {
    if payable_pnl <= 0 {
        return ClosingFee::default();
    }
    let size_component = math::mul_div_ceil(env, size_removed, config.close_size_fee_bps as i128, BPS);
    let pnl_component = math::mul_div_ceil(env, payable_pnl, config.close_pnl_fee_bps as i128, BPS);
    let nominal = core::cmp::min(
        payable_pnl,
        core::cmp::max(size_component, pnl_component),
    );
    // The senior items: funding received is a credit, the two funding
    // obligations and borrow are debits, and the keeper reward outranks the
    // fee. A settlement that cannot pay its own fee waives it rather than
    // eating into the trader's original collateral.
    let mut after_senior = math::add(env, payable_pnl, pending.funding_received);
    after_senior = math::sub(env, after_senior, pending.funding_paid_to_receivers);
    after_senior = math::sub(env, after_senior, pending.funding_paid_to_lps);
    after_senior = math::sub(env, after_senior, borrow_due);
    after_senior = math::sub(env, after_senior, keeper_reward);
    let profit_after_senior_items = core::cmp::max(after_senior, 0);
    ClosingFee {
        size_component,
        pnl_component,
        nominal,
        collectible: core::cmp::min(nominal, profit_after_senior_items),
    }
}

/// §6.4 `credit_received_funding` — move a receiver's accrued funding from
/// the guaranteed claim into their position: an ownership relabel, not a
/// cash movement.
///
/// **Both** the market's share and the global total are decremented, and
/// sufficiency is required on both. The superseded implementation moved only
/// the global total and capped the credit at it; per §5.12 the global total
/// is the sum of the per-market amounts, so leaving the market's share
/// untouched would let it drift above the global figure it is supposed to
/// compose.
///
/// These are `require`s, not caps, because §9.4 guarantees the backing
/// exists — the distribution remainder is reset whenever the divisor changes
/// (§4.5.1), which is precisely what keeps a credit from outrunning the
/// accrual that justifies it. Reaching this check means that reset did not
/// run, and a clamp here would hide it.
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

/// §11.4 — capitalize all accrued amounts plus `negative_pnl` against the
/// position's stored collateral, in the specified collection order, then
/// reset the debt baselines. Collateral from the current action and realized
/// positive PnL must already be in stored collateral when this runs.
/// §6.10 `capitalize_for_surviving_mutation` — credit received funding,
/// then require stored collateral to cover **every** obligation in full
/// before collecting any of it.
///
/// The check is up front rather than per-leg: collecting greedily and
/// discovering the shortfall on the last leg would leave the position
/// stripped of collateral by an action that must not complete at all. A
/// survivor that cannot pay its window is not a survivor.
pub fn capitalize_for_surviving_mutation(
    env: &Env,
    ledger: &mut Ledger,
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
    distribute_borrow_revenue(env, ledger, borrow_collected, position.id);
    funding::reset_debts(env, position, market);
    CollectedFees {
        receiver_credit: pending.funding_received,
        receiver_funding_paid: receiver_collected,
        lp_funding_paid: lp_collected,
        borrow_paid: borrow_collected,
        loss_collected: 0,
        unpaid: 0,
    }
}

/// §6.10 terminal capitalization: the same collection order, but it may
/// collect less than is owed and reports the remainder as bad debt.
pub fn capitalize(
    env: &Env,
    ledger: &mut Ledger,
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

    distribute_borrow_revenue(env, ledger, borrow_collected, position.id);
    funding::reset_debts(env, position, market);

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

