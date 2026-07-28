//! Time-based accrual checkpoints — doc §10.
//!
//! A rate is constant between two checkpoints; a state change must never
//! reprice past time (§18.4). Both checkpoints carry their division
//! remainders so checkpoint frequency cannot change accrued value (§3).

use soroban_sdk::Env;

use shared::constants::{BPS, INDEX_PRECISION, SECONDS_PER_DAY};
use shared::{Market, PayerSide};

use crate::ledger::Ledger;
use crate::math;

/// §10.1 — advance the global borrow index with the stored rate and grow the
/// guaranteed receiver liability with the stored global flow. A second call
/// at the same timestamp has no effect.
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

    let receiver_numerator = math::add(
        env,
        math::mul(env, ledger.receiver_flow_per_second, elapsed),
        ledger.receiver_accrual_remainder,
    );
    ledger.pending_receiver_funding_total = math::add(
        env,
        ledger.pending_receiver_funding_total,
        receiver_numerator / INDEX_PRECISION,
    );
    ledger.receiver_accrual_remainder = receiver_numerator % INDEX_PRECISION;

    ledger.last_global_checkpoint = now;
}

/// One index advance: `(flow * elapsed + remainder) / base`, returning the
/// delta and the carried remainder.
pub fn accrue_index(
    env: &Env,
    flow: i128,
    elapsed: i128,
    base: i128,
    remainder: i128,
) -> (i128, i128) {
    if flow == 0 || base == 0 {
        return (0, remainder);
    }
    let numerator = math::add(env, math::mul(env, flow, elapsed), remainder);
    (numerator / base, numerator % base)
}

/// §10.2 — advance the market's three funding indices for the elapsed
/// interval using the stored flows. A second call at the same timestamp has
/// no effect.
pub fn checkpoint_market(env: &Env, market: &mut Market, now: u64) {
    if now <= market.last_funding_checkpoint {
        return;
    }
    let elapsed = (now - market.last_funding_checkpoint) as i128;
    if market.current_payer_side != PayerSide::None {
        let long_pays = market.current_payer_side == PayerSide::Long;
        let (payer_size, receiver_size) = if long_pays {
            (
                market.long.size_open_interest,
                market.short.size_open_interest,
            )
        } else {
            (
                market.short.size_open_interest,
                market.long.size_open_interest,
            )
        };
        let (receiver_delta, receiver_rem) = accrue_index(
            env,
            market.receiver_flow_per_second,
            elapsed,
            payer_size,
            market.receiver_payer_remainder,
        );
        let (lp_delta, lp_rem) = accrue_index(
            env,
            market.lp_flow_per_second,
            elapsed,
            payer_size,
            market.lp_payer_remainder,
        );
        let (credit_delta, credit_rem) = accrue_index(
            env,
            market.receiver_flow_per_second,
            elapsed,
            receiver_size,
            market.receiver_index_remainder,
        );
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
    market.last_funding_checkpoint = now;
}
