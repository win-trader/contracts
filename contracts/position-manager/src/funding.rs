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

use soroban_sdk::{panic_with_error, Env, U256};

use shared::constants::{BPS, INDEX_PRECISION, SECONDS_PER_DAY};
use shared::{Market, PayerSide, Position, RemainderGroup};

use crate::errors::PositionManagerError;
use crate::ledger::{Bucket, Ledger};
use crate::window::{self, Segment};
use crate::{math, storage};

/// §6.2 — advance the market's funding indices and the receiver liability
/// over the elapsed window, then advance the EMA itself.
///
/// The window is resolved by the pure §6.2.1 integrator, which **splits it
/// at a funding sign change**. That split is the correctness point: the
/// previous implementation returned one weight and picked the payer from
/// `sign(∫ I dt)`, which assigns funding generated on one side of a
/// crossing to the other.
///
/// A second call at the same timestamp has no effect.
pub fn accrue(env: &Env, ledger: &mut Ledger, market: &mut Market, now: u64) {
    if now <= market.last_funding_checkpoint {
        return;
    }
    let elapsed = now - market.last_funding_checkpoint;
    let half_life = storage::get_global_config(env).funding_half_life_seconds;
    let window = window::integrate_funding_window_by_sign(
        env,
        market.long.base_exposure,
        market.short.base_exposure,
        market.skew_ema,
        market.config.instant_weight_bps,
        half_life,
        market.config.max_funding_rate_bps_day,
        elapsed,
    );

    // Chronological order. It does not change the arithmetic, but it does
    // change which carried remainder each division sees.
    for segment in window.segments() {
        accrue_segment(env, ledger, market, segment);
    }

    // Advanced in **every** branch, including when no payer side has
    // exposure: history keeps decaying while the book waits.
    market.skew_ema = window.ema_after;
    market.current_payer_side = window.payer_side_at_end;
    market.current_payer_rate = window.displayed_rate_at_end;
    market.last_funding_checkpoint = now;
}

/// §6.2 — one constant-payer segment: advance both payer indices, create the
/// guaranteed receiver liability, and distribute the receiver credit.
fn accrue_segment(env: &Env, ledger: &mut Ledger, market: &mut Market, segment: &Segment) {
    let long_pays = segment.long_pays();
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
    let zero = U256::from_u32(env, 0);
    // With nobody on the payer side there is nothing to charge, and a zero
    // weight moves no index; skip the index and liability changes. The EMA
    // and timestamp still advance, in the caller.
    if payer_size == 0 || segment.funding_weight == zero {
        return;
    }

    // Every carry below belongs to *this* payer direction's stream (§4.5).
    let carries = market.payer_remainders(long_pays).clone();

    // Receivers absorb the share their counter-exposure matches, capped at
    // the whole flow — under the EMA the payer can be the *lighter* side,
    // and the cap is what keeps the LP slice non-negative (§3.4).
    let receiver_weight = if receiver_size == 0 || receiver_base == 0 {
        zero
    } else if receiver_base >= payer_base {
        segment.funding_weight.clone()
    } else {
        math::wide_mul_div_floor(env, &segment.funding_weight, receiver_base, payer_base)
    };
    let lp_weight = segment.funding_weight.sub(&receiver_weight);

    // §6.2 — the weight carries an extra INDEX_PRECISION factor from the
    // window integral, so the divisor carries it too. One division, not two.
    let denominator = math::mul(env, INDEX_PRECISION, BPS * SECONDS_PER_DAY as i128);
    let (receiver_payer_delta, receiver_payer_rem) = math::carried_div(
        env,
        &receiver_weight,
        denominator,
        carries.receiver_payer_remainder,
    );
    let (lp_payer_delta, lp_payer_rem) =
        math::carried_div(env, &lp_weight, denominator, carries.lp_payer_remainder);

    // The guaranteed liability and the receiver credit index derive from the
    // **same** exact backing, so a credit can never outrun the accrual that
    // justifies it (§9.4).
    let receiver_backing_scaled = math::widen_mul(env, payer_size, receiver_payer_delta);
    let (liability_delta, liability_rem) = math::carried_div(
        env,
        &receiver_backing_scaled,
        INDEX_PRECISION,
        carries.receiver_liability_remainder,
    );
    ledger.credit(env, Bucket::ReceiverFunding, liability_delta);
    // §5.12 — the global total is the sum of these per-market amounts.
    market.pending_receiver_funding =
        math::add(env, market.pending_receiver_funding, liability_delta);

    let (credit_delta, distribution_rem) = if receiver_size > 0 {
        math::carried_div(
            env,
            &receiver_backing_scaled,
            receiver_size,
            carries.distribution_remainder,
        )
    } else {
        (0, carries.distribution_remainder)
    };

    if long_pays {
        market.receiver_backed_index_long = math::add(
            env,
            market.receiver_backed_index_long,
            receiver_payer_delta,
        );
        market.lp_backed_index_long = math::add(env, market.lp_backed_index_long, lp_payer_delta);
        market.receiver_index_short = math::add(env, market.receiver_index_short, credit_delta);
    } else {
        market.receiver_backed_index_short = math::add(
            env,
            market.receiver_backed_index_short,
            receiver_payer_delta,
        );
        market.lp_backed_index_short = math::add(env, market.lp_backed_index_short, lp_payer_delta);
        market.receiver_index_long = math::add(env, market.receiver_index_long, credit_delta);
    }

    let carries = market.payer_remainders_mut(long_pays);
    carries.receiver_payer_remainder = receiver_payer_rem;
    carries.lp_payer_remainder = lp_payer_rem;
    carries.receiver_liability_remainder = liability_rem;
    carries.distribution_remainder = distribution_rem;
}

/// §4.5.1 / §6.13 — zero the **opposite** payer stream's
/// receiver-distribution carry when a side's `size_open_interest` changes.
///
/// That carry divides by the receiving side's size. A remainder produced
/// modulo a large receiver size is a small fraction of a large base; carried
/// into a division by a smaller receiver size it becomes a much larger
/// fraction of a smaller base, and receivers can be credited more than the
/// receiver-backed accrual that justifies it. Without this reset §9.4's
/// sufficiency check in `credit_received_funding` becomes reachable and a
/// receiver position cannot be settled. It must not be replaced by a clamp.
///
/// The long-payer stream distributes to short receivers, so a change on the
/// short side clears the **long**-payer carry. Runs after the checkpoint and
/// before the exposure mutation (§4.9 step 7), so every carry lives inside a
/// window of constant receiver size and the reset itself loses no accrual.
/// The discarded fraction is under one cash unit and is never re-credited,
/// so the reset can only under-distribute.
pub fn reset_receiver_distribution_remainder(market: &mut Market, changed_side_is_long: bool) {
    market
        .payer_remainders_mut(!changed_side_is_long)
        .distribution_remainder = 0;
}

/// §8.1 cold start — an empty book carries no history, and zero is not "no
/// information": it would grant a one-sided launch a decaying discount. The
/// EMA starts at the skew the first open creates.
pub fn cold_start(env: &Env, market: &mut Market) {
    market.skew_ema = math::skew_frac(env, market.long.base_exposure, market.short.base_exposure);
}

/// §4.13 — when the **final position across the whole vault** is removed,
/// the global receiver liability must already be zero, because each market
/// released its own share as it emptied. This is the assertion that says so.
///
/// It is a release rather than a bare check only because a rounding residue
/// attributable to no market at all would otherwise strand cash no one can
/// claim; in a correct run it releases nothing.
pub fn verify_no_final_receiver_residue(env: &Env, ledger: &mut Ledger) {
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

/// §4.13 / §6.17 — refresh the market's displayed payer side and rate from
/// the post-mutation book and EMA, and run the empty-market reset when the
/// book has just emptied. Accrual happens in `accrue`; the two display
/// fields exist for events and off-chain consumers and create no
/// obligations.
pub fn refresh_display(env: &Env, ledger: &mut Ledger, market: &mut Market) {
    if market.long.size_open_interest == 0 && market.short.size_open_interest == 0 {
        // §4.13 — the market clears. Its EMA, display fields, and both
        // remainder groups are wiped so the next open cold-starts, and
        // **this market's** guaranteed receiver funding is released to LP
        // equity: with no positions left on either side, no receiver in it
        // can remain to claim it.
        //
        // Cumulative indices and `last_funding_checkpoint` are deliberately
        // untouched (§4.19). Rewinding an index would reprice a historical
        // position's baseline.
        market.skew_ema = 0;
        market.long_payer_remainders = RemainderGroup::default();
        market.short_payer_remainders = RemainderGroup::default();
        market.current_payer_side = PayerSide::None;
        market.current_payer_rate = 0;
        let owed = market.pending_receiver_funding;
        if owed > 0 {
            let released = ledger.release(env, Bucket::ReceiverFunding, owed);
            market.pending_receiver_funding =
                math::sub(env, market.pending_receiver_funding, released);
        }
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
    if funding_paid_to_receivers < 0 || funding_paid_to_lps < 0 || funding_received < 0 {
        panic_with_error!(env, PositionManagerError::InvariantViolation);
    }
    PendingFees {
        funding_paid_to_receivers,
        funding_paid_to_lps,
        funding_received,
        // §6.3 owns its own non-negativity check, which runs before the
        // per-window minimum is applied.
        borrow: crate::borrow::calculate_pending(env, ledger, position).due,
    }
}

/// §4.4 — reset the **three funding** baselines to the current index values
/// so the position's next accrual starts now: new size never owes or earns
/// funding from before it existed.
///
/// The borrow baseline is deliberately not here. §4.10 opens the borrow
/// window *after* the global rate refresh, because the window's monetary
/// minimum is quoted from the rate that applies going forward; see
/// `borrow::initialize_window`.
pub fn reset_debts(env: &Env, position: &mut Position, market: &Market) {
    let indices = market.funding_indices(position.is_long);
    position.funding_paid_to_receivers_debt =
        math::index_value_ceil(env, position.size, indices.receiver_backed_payer);
    position.funding_paid_to_lps_debt =
        math::index_value_ceil(env, position.size, indices.lp_backed_payer);
    position.funding_received_debt = math::index_value_floor(env, position.size, indices.receiver);
}

/// §4.12 — the pending amounts as of `now`, **without mutating anything**.
///
/// The global borrow index and the market's funding indices are advanced on
/// in-memory copies, the pending amounts are derived from them, and nothing
/// — no index, no remainder, no timestamp — is written back.
///
/// It reaches the same numbers by running the same code: the copies go
/// through `borrow::accrue` and `accrue` themselves rather than through a
/// parallel "preview" arithmetic that could drift. §4.12 requires a quote
/// immediately followed by settlement to produce identical amounts, and the
/// cheapest way to guarantee that is to have only one implementation.
///
/// Reading the last stored indices instead would understate fees after time
/// has elapsed; writing during a quote would make a view alter economics.
pub fn preview_pending_fees(
    env: &Env,
    ledger: &Ledger,
    position: &Position,
    market: &Market,
    now: u64,
) -> PendingFees {
    let mut ledger_copy = ledger.clone();
    let mut market_copy = market.clone();
    crate::borrow::accrue(env, &mut ledger_copy, now);
    accrue(env, &mut ledger_copy, &mut market_copy, now);
    pending_fees(env, &ledger_copy, position, &market_copy)
}
