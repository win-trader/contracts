//! Position fee accounting — doc §11.
//!
//! Capitalization settles every accrued amount against stored collateral in
//! the §11.4 collection order (receiver-backed funding, then negative price
//! PnL, then LP-backed funding, then borrow) so a shortfall lands on the
//! least-protected claim, then resets the debt baselines.

use soroban_sdk::Env;

use shared::constants::BPS;
use shared::{Market, Position};

use crate::events::{self, FeeSource};
use crate::funding;
use crate::ledger::{self, Ledger};
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

/// §11.4 — split a collected opening or borrow fee between protocol
/// claimable revenue and (implicitly) residual LP cash.
///
/// `referral` is a carve-out already credited to a referrer by the caller
/// (only nonzero on a referred closing fee, §11.1); it comes purely out of
/// the protocol slice, so the LP share — computed off the full `collected`
/// — is never diluted. `lp + protocol + referral == collected`, and
/// `protocol ≥ 0` because the validated share sum bounds
/// `lp + referral ≤ collected`.
pub fn split_revenue(
    env: &Env,
    ledger: &mut Ledger,
    collected: i128,
    referral: i128,
    source: FeeSource,
    position_id: u64,
) {
    if collected == 0 {
        return;
    }
    let config = storage::get_global_config(env);
    let lp = math::mul_div_floor(env, collected, config.lp_revenue_share_bps as i128, BPS);
    let protocol = math::sub(env, math::sub(env, collected, lp), referral);
    ledger.credit(env, ledger::Bucket::ProtocolClaimable, protocol);
    events::emit_revenue_split(env, position_id, source, collected, lp, protocol, referral);
}

/// §11.4 — capitalize all accrued amounts plus `negative_pnl` against the
/// position's stored collateral, in the specified collection order, then
/// reset the debt baselines. Collateral from the current action and realized
/// positive PnL must already be in stored collateral when this runs.
pub fn capitalize(
    env: &Env,
    ledger: &mut Ledger,
    position: &mut Position,
    market: &mut Market,
    negative_pnl: i128,
) -> CollectedFees {
    let pending = funding::pending_fees(env, ledger, position, market);
    let receiver_credit = core::cmp::min(
        pending.funding_received,
        ledger.pending_receiver_funding_total,
    );

    let (receiver_collected, loss_collected, lp_collected, borrow_collected) = {
        let is_long = position.is_long;
        let side = market.side_mut(is_long);
        if receiver_credit > 0 {
            let released = ledger.release(env, ledger::Bucket::ReceiverFunding, receiver_credit);
            ledger::add_stored_collateral(env, ledger, position, side, released);
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

    split_revenue(
        env,
        ledger,
        borrow_collected,
        0,
        FeeSource::Borrow,
        position.id,
    );
    funding::reset_debts(env, ledger, position, market);

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

