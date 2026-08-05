//! Time-based accrual checkpoints — doc §10.
//!
//! The borrow rate is constant between two checkpoints; a state change must
//! never reprice past time (§18.4). Funding is *not* piecewise-constant —
//! the §8.1 EMA rate decays continuously — so `checkpoint_market` advances
//! the funding indices with the exact closed-form integral of the rate over
//! the window. Checkpoint frequency therefore cannot change accrued value
//! beyond the decay table's quantization (§3, tolerance-bounded), and both
//! checkpoints carry their division remainders.

use soroban_sdk::Env;

use shared::constants::{BPS, INDEX_PRECISION, SECONDS_PER_DAY};
use shared::{Market, PayerSide};

use crate::ledger::Ledger;
use crate::{math, storage};

/// §10.1 — advance the global borrow index with the stored rate. A second
/// call at the same timestamp has no effect.
pub fn checkpoint_global(env: &Env, ledger: &mut Ledger, now: u64) {
    shared::bump_instance_ttl(env);
    if now <= ledger.last_global_checkpoint {
        return;
    }
    let elapsed = (now - ledger.last_global_checkpoint) as i128;

    let denominator = BPS * SECONDS_PER_DAY as i128;
    let numerator = math::add(
        env,
        math::mul(env, ledger.current_borrow_rate, elapsed),
        ledger.borrow_index_remainder,
    );
    ledger.borrow_index = math::add(env, ledger.borrow_index, numerator / denominator);
    ledger.borrow_index_remainder = numerator % denominator;

    ledger.last_global_checkpoint = now;
}

/// One accumulator advance: `(numerator + remainder) / divisor`, returning
/// the delta and the carried remainder. `divisor` must be positive.
fn accrue(env: &Env, numerator: i128, divisor: i128, remainder: i128) -> (i128, i128) {
    let total = math::add(env, numerator, remainder);
    (total / divisor, total % divisor)
}

/// §10.2 — advance the market's three funding indices and the global
/// receiver liability over the elapsed window (closed-form §8.1 integral),
/// then advance the skew EMA itself. The window's payer is the side
/// `∫ I dt` points at — possibly the lighter one. A second call at the same
/// timestamp has no effect.
pub fn checkpoint_market(env: &Env, ledger: &mut Ledger, market: &mut Market, now: u64) {
    if now <= market.last_funding_checkpoint {
        return;
    }
    let elapsed = now - market.last_funding_checkpoint;
    let half_life = storage::global_config(env).funding_half_life_seconds;
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
        let (receiver_delta, receiver_rem) = accrue(
            env,
            weight_receiver,
            denominator,
            market.receiver_payer_remainder,
        );
        let (lp_delta, lp_rem) = accrue(env, weight_lp, denominator, market.lp_payer_remainder);

        // The liability and the receiver credit both derive from the exact
        // amount the payer index will collect (§8.3), so a credit can never
        // outrun its backing accrual.
        let receiver_cash = math::mul(env, payer_size, receiver_delta);
        let (liability_delta, pending_rem) = accrue(
            env,
            receiver_cash,
            INDEX_PRECISION,
            market.pending_remainder,
        );
        ledger.pending_receiver_funding_total = math::add(
            env,
            ledger.pending_receiver_funding_total,
            liability_delta,
        );
        market.pending_remainder = pending_rem;
        let (credit_delta, credit_rem) = if receiver_size > 0 {
            accrue(
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
    market.current_payer_side = if window.integral_now > 0 {
        PayerSide::Long
    } else if window.integral_now < 0 {
        PayerSide::Short
    } else {
        PayerSide::None
    };
    market.current_payer_rate = math::rate_from_integral(
        env,
        market.config.max_funding_rate_bps_day,
        window.integral_now,
    );
    market.last_funding_checkpoint = now;
}
