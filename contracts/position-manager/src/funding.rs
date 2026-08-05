//! Funding mechanics — doc §8.
//!
//! Funding prices net directional imbalance, blended with its history: the
//! side the §8.1 integral skew points at pays a quadratic rate on its size
//! open interest; the other side's traders receive the share matched by
//! their counter-exposure (capped at the whole flow — the payer can be the
//! lighter side) and LPs receive the rest. Receiver funding is guaranteed
//! when it accrues (§5.4 of the theory doc), which is why the payer flow is
//! split into a receiver-backed index and an LP-backed index with different
//! collection accounting. The accrual itself lives in
//! `checkpoint::checkpoint_market`.

use soroban_sdk::{panic_with_error, Env};

use shared::{Market, PayerSide, Position};

use crate::errors::PositionManagerError;
use crate::ledger::Ledger;
use crate::math;

/// §11.2 — the pending amounts a position has accrued since its debt
/// baselines were last reset. All four are non-negative by construction; a
/// negative value means a decreasing index or corrupted baseline.
#[derive(Clone, Copy, Debug)]
pub struct PendingFees {
    /// Owed to receiver-backed funding (rounds up, §16).
    pub funding_paid_to_receivers: i128,
    /// Owed to LP-backed funding (rounds up, §16).
    pub funding_paid_to_lps: i128,
    /// Funding credit receivable (rounds down, §16).
    pub funding_received: i128,
    /// Owed borrow fee on risk units (rounds up, §16).
    pub borrow: i128,
}

/// §8.1 — refresh the market's displayed payer side and rate from the
/// post-mutation book and EMA, and drop the rounding remainders once the
/// book is completely empty. Accrual happens in `checkpoint_market`; these
/// two fields exist for events and off-chain consumers.
pub fn refresh_display(env: &Env, market: &mut Market) {
    let skew = math::skew_frac(env, market.long.base_exposure, market.short.base_exposure);
    let integral = math::integral_skew(env, skew, market.skew_ema, market.config.instant_weight_bps);
    market.current_payer_side = if integral > 0 {
        PayerSide::Long
    } else if integral < 0 {
        PayerSide::Short
    } else {
        PayerSide::None
    };
    market.current_payer_rate =
        math::rate_from_integral(env, market.config.max_funding_rate_bps_day, integral);
    if market.long.size_open_interest == 0 && market.short.size_open_interest == 0 {
        market.receiver_payer_remainder = 0;
        market.lp_payer_remainder = 0;
        market.receiver_index_remainder = 0;
        market.pending_remainder = 0;
    }
}

/// §11.2 — pending amounts for a position against the current indices.
/// Panics with `InvariantViolation` if any pending amount is negative — that
/// identifies an invalid baseline or a decreasing index, not bad arithmetic.
pub fn pending_fees(
    env: &Env,
    ledger: &Ledger,
    position: &Position,
    market: &Market,
) -> PendingFees {
    let indices = market.funding_indices(position.is_long);
    let funding_paid_to_receivers = math::sub(
        env,
        math::index_value_ceil(env, position.size, indices.receiver_backed_payer),
        position.funding_paid_to_receivers_debt,
    );
    let funding_paid_to_lps = math::sub(
        env,
        math::index_value_ceil(env, position.size, indices.lp_backed_payer),
        position.funding_paid_to_lps_debt,
    );
    let funding_received = math::sub(
        env,
        math::index_value_floor(env, position.size, indices.receiver),
        position.funding_received_debt,
    );
    let borrow = math::sub(
        env,
        math::index_value_ceil(env, position.risk_units, ledger.borrow_index),
        position.borrow_debt,
    );
    if funding_paid_to_receivers < 0
        || funding_paid_to_lps < 0
        || funding_received < 0
        || borrow < 0
    {
        panic_with_error!(env, PositionManagerError::InvariantViolation);
    }
    PendingFees {
        funding_paid_to_receivers,
        funding_paid_to_lps,
        funding_received,
        borrow,
    }
}

/// §11.4 step 7 — reset every debt baseline to the current index values so
/// the position's next accrual starts now (§18.4: new size starts at the
/// current baseline).
pub fn reset_debts(env: &Env, ledger: &Ledger, position: &mut Position, market: &Market) {
    let indices = market.funding_indices(position.is_long);
    position.funding_paid_to_receivers_debt =
        math::index_value_ceil(env, position.size, indices.receiver_backed_payer);
    position.funding_paid_to_lps_debt =
        math::index_value_ceil(env, position.size, indices.lp_backed_payer);
    position.funding_received_debt = math::index_value_floor(env, position.size, indices.receiver);
    position.borrow_debt = math::index_value_ceil(env, position.risk_units, ledger.borrow_index);
}
