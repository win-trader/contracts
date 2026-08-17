//! Funding mechanics — doc §8, whole life in one module.
//!
//! Funding prices net directional imbalance, blended with its history: the
//! side the §8.1 integral skew points at pays a quadratic rate on its size
//! open interest; the other side's traders receive the share matched by
//! their counter-exposure (capped at the whole flow — the payer can be the
//! lighter side) and LPs receive the rest. Receiver funding is guaranteed
//! when it accrues (§5.4 of the theory doc), which is why the payer flow is
//! split into a receiver-backed index and an LP-backed index with different
//! collection accounting.
//!
//! Reading top to bottom: `accrue` advances the indices and recognizes the
//! receiver liability per elapsed window; `cold_start` seeds the EMA on a
//! book's first open; `refresh_display`/`set_display` derive the displayed
//! payer side and rate; `pending_fees`/`reset_debts` are the per-position
//! index-clock boundary; `release_residue` sweeps the rounding residue when
//! the book empties. The market's funding fields (indices, remainders, EMA,
//! display, clock) are written only here.

use soroban_sdk::{panic_with_error, Env};

use shared::constants::{BPS, INDEX_PRECISION, SECONDS_PER_DAY};
use shared::{Market, PayerSide, Position};

use crate::errors::PositionManagerError;
use crate::ledger::{Bucket, Ledger};
use crate::{math, storage};

/// §10.2 — advance the market's three funding indices and the global
/// receiver liability over the elapsed window (closed-form §8.1 integral),
/// then advance the skew EMA itself. The window's payer is the side
/// `∫ I dt` points at — possibly the lighter one. A second call at the same
/// timestamp has no effect.
///
/// The borrow clock is piecewise-constant, but funding is not — the EMA
/// rate decays continuously — so the window resolves through the exact
/// closed-form integral: checkpoint frequency cannot change accrued value
/// beyond the decay table's quantization (§3), and every division carries
/// its remainder.
pub fn accrue(env: &Env, ledger: &mut Ledger, market: &mut Market, now: u64) {
    if now <= market.last_funding_checkpoint {
        return;
    }
    let elapsed = now - market.last_funding_checkpoint;
    let half_life = storage::get_global_config(env).funding_half_life_seconds;
    let window = math::funding_window(
        env,
        market.long.base_exposure,
        market.short.base_exposure,
        market.skew_ema,
        market.config.instant_weight_bps,
        half_life,
        market.config.max_funding_rate_bps_day,
        elapsed,
    );

    let long_pays = window.payer_sign > 0;
    let (payer_size, payer_base, receiver_size, receiver_base) = if long_pays {
        (
            market.long.size_open_interest,
            market.long.base_exposure,
            market.short.size_open_interest,
            market.short.base_exposure,
        )
    } else {
        (
            market.short.size_open_interest,
            market.short.base_exposure,
            market.long.size_open_interest,
            market.long.base_exposure,
        )
    };
    // With nobody on the payer side there is nothing to charge: the EMA
    // still advances below, so history keeps decaying while the book waits.
    if window.payer_sign != 0 && window.weight > 0 && payer_size > 0 {
        // Receivers absorb the share their counter-exposure matches, capped
        // at the whole flow — under the EMA the payer can be the *lighter*
        // side, and the cap is what keeps the LP slice non-negative (§8.1).
        let weight_receiver = if receiver_size == 0 || receiver_base == 0 {
            0
        } else if receiver_base >= payer_base {
            window.weight
        } else {
            math::mul_div_floor(env, window.weight, receiver_base, payer_base)
        };
        let weight_lp = math::sub(env, window.weight, weight_receiver);

        let denominator = BPS * SECONDS_PER_DAY as i128;
        let (receiver_delta, receiver_rem) = carried_div(
            env,
            weight_receiver,
            denominator,
            market.receiver_payer_remainder,
        );
        let (lp_delta, lp_rem) =
            carried_div(env, weight_lp, denominator, market.lp_payer_remainder);

        // The liability and the receiver credit both derive from the exact
        // amount the payer index will collect (§8.3), so a credit can never
        // outrun its backing accrual.
        let receiver_cash = math::mul(env, payer_size, receiver_delta);
        let (liability_delta, pending_rem) = carried_div(
            env,
            receiver_cash,
            INDEX_PRECISION,
            market.pending_remainder,
        );
        ledger.credit(env, Bucket::ReceiverFunding, liability_delta);
        market.pending_remainder = pending_rem;
        let (credit_delta, credit_rem) = if receiver_size > 0 {
            carried_div(
                env,
                receiver_cash,
                receiver_size,
                market.receiver_index_remainder,
            )
        } else {
            (0, market.receiver_index_remainder)
        };

        if long_pays {
            market.receiver_backed_index_long =
                math::add(env, market.receiver_backed_index_long, receiver_delta);
            market.lp_backed_index_long = math::add(env, market.lp_backed_index_long, lp_delta);
            market.receiver_index_short = math::add(env, market.receiver_index_short, credit_delta);
        } else {
            market.receiver_backed_index_short =
                math::add(env, market.receiver_backed_index_short, receiver_delta);
            market.lp_backed_index_short = math::add(env, market.lp_backed_index_short, lp_delta);
            market.receiver_index_long = math::add(env, market.receiver_index_long, credit_delta);
        }
        market.receiver_payer_remainder = receiver_rem;
        market.lp_payer_remainder = lp_rem;
        market.receiver_index_remainder = credit_rem;
    }
    market.skew_ema = window.ema_after;
    set_display(env, market, window.integral_now);
    market.last_funding_checkpoint = now;
}

/// One accumulator advance: `(numerator + remainder) / divisor`, returning
/// the delta and the carried remainder. `divisor` must be positive.
fn carried_div(env: &Env, numerator: i128, divisor: i128, remainder: i128) -> (i128, i128) {
    let total = math::add(env, numerator, remainder);
    (total / divisor, total % divisor)
}

/// §8.1 cold start — an empty book carries no history, and zero is not "no
/// information": it would grant a one-sided launch a decaying discount. The
/// EMA starts at the skew the first open creates.
pub fn cold_start(env: &Env, market: &mut Market) {
    market.skew_ema = math::skew_frac(env, market.long.base_exposure, market.short.base_exposure);
}

/// §8.3 — with no open positions anywhere, aggregate conservation makes
/// every market size zero: release the unassigned rounding residue to LP
/// residual cash without a market loop.
pub fn release_residue(env: &Env, ledger: &mut Ledger) {
    if ledger.open_position_count == 0 {
        let residue = ledger.pending_receiver_funding_total;
        ledger.release(env, Bucket::ReceiverFunding, residue);
    }
}

/// Map a signed integral skew onto the two display fields.
fn set_display(env: &Env, market: &mut Market, integral: i128) {
    market.current_payer_side = if integral > 0 {
        PayerSide::Long
    } else if integral < 0 {
        PayerSide::Short
    } else {
        PayerSide::None
    };
    market.current_payer_rate =
        math::rate_from_integral(env, market.config.max_funding_rate_bps_day, integral);
}

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
/// post-mutation book and EMA. A market that just emptied keeps no funding
/// memory: the EMA and the rounding remainders are wiped so the next open
/// cold-starts, and the displayed rate is zero rather than a stale blend.
/// Accrual happens in `accrue`; the two display fields exist for events and
/// off-chain consumers.
pub fn refresh_display(env: &Env, market: &mut Market) {
    if market.long.size_open_interest == 0 && market.short.size_open_interest == 0 {
        market.skew_ema = 0;
        market.receiver_payer_remainder = 0;
        market.lp_payer_remainder = 0;
        market.receiver_index_remainder = 0;
        market.pending_remainder = 0;
        market.current_payer_side = PayerSide::None;
        market.current_payer_rate = 0;
        return;
    }
    let skew = math::skew_frac(env, market.long.base_exposure, market.short.base_exposure);
    let integral =
        math::integral_skew(env, skew, market.skew_ema, market.config.instant_weight_bps);
    set_display(env, market, integral);
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
    // §11.2 — minimum borrow charge: every settlement pays at least the
    // configured index delta on its risk units (anti-churn floor; the
    // invariant check above runs on the raw accrual, not the floored
    // value). Baselines reset per touch, so the floor applies per
    // capitalization.
    let borrow_floor = math::index_value_ceil(
        env,
        position.risk_units,
        storage::get_global_config(env).min_borrow_index_delta,
    );
    PendingFees {
        funding_paid_to_receivers,
        funding_paid_to_lps,
        funding_received,
        borrow: core::cmp::max(borrow, borrow_floor),
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
