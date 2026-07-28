//! Funding mechanics — doc §8.
//!
//! Funding prices net directional imbalance. The dominant side pays a
//! quadratic-in-skew rate on its size open interest; light-side traders
//! receive the share matched by their counter-exposure and LPs receive the
//! rest. Receiver funding is guaranteed when it accrues (§5.4 of the theory
//! doc), which is why the payer flow is split into a receiver-backed index
//! and an LP-backed index with different collection accounting.

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

/// §8.1 — recompute the market's payer side, rate, and flow split from its
/// current base exposures, and roll the change into the global receiver-flow
/// sum (constant-complexity update, §8.3). Call only after the market has
/// been checkpointed to `now`.
pub fn recompute_market_flow(env: &Env, ledger: &mut Ledger, market: &mut Market) {
    let old_receiver = market.receiver_flow_per_second;
    let long_base = market.long.base_exposure;
    let short_base = market.short.base_exposure;
    if long_base == short_base || long_base + short_base == 0 {
        market.current_payer_side = PayerSide::None;
        market.current_payer_rate = 0;
        market.receiver_flow_per_second = 0;
        market.lp_flow_per_second = 0;
        if market.long.size_open_interest == 0 && market.short.size_open_interest == 0 {
            market.receiver_payer_remainder = 0;
            market.lp_payer_remainder = 0;
            market.receiver_index_remainder = 0;
            market.receiver_flow_remainder = 0;
        }
    } else {
        let skew = math::skew_bps(env, long_base, short_base);
        let rate = math::funding_rate(env, market.config.max_funding_rate_bps_day, skew);
        let long_pays = long_base > short_base;
        let (dominant_size, dominant_base, light_size, light_base) = if long_pays {
            (
                market.long.size_open_interest,
                long_base,
                market.short.size_open_interest,
                short_base,
            )
        } else {
            (
                market.short.size_open_interest,
                short_base,
                market.long.size_open_interest,
                long_base,
            )
        };
        let payer_flow = math::flow_per_second(env, dominant_size, rate);
        let receiver_flow = if light_size == 0 || light_base == 0 {
            0
        } else {
            let numerator = math::add(
                env,
                math::mul(env, payer_flow, light_base),
                market.receiver_flow_remainder,
            );
            market.receiver_flow_remainder = numerator % dominant_base;
            numerator / dominant_base
        };
        market.current_payer_side = if long_pays {
            PayerSide::Long
        } else {
            PayerSide::Short
        };
        market.current_payer_rate = rate;
        market.receiver_flow_per_second = receiver_flow;
        market.lp_flow_per_second = math::sub(env, payer_flow, receiver_flow);
    }
    ledger.receiver_flow_per_second = math::add(
        env,
        math::sub(env, ledger.receiver_flow_per_second, old_receiver),
        market.receiver_flow_per_second,
    );
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
