use soroban_sdk::{panic_with_error, Address, Env, Symbol, U256};

use shared::constants::{BPS, INDEX_PRECISION, SECONDS_PER_DAY};
use shared::{Market, PayerSide, Position, RemainderGroup};

use crate::errors::PositionManagerError;
use crate::ledger::{Bucket, Ledger};
use crate::window::{self, Segment};
use crate::{events, math, storage};

pub fn accrue(
    env: &Env,
    ledger: &mut Ledger,
    market_id: &Symbol,
    actor: Option<&Address>,
    market: &mut Market,
    now: u64,
) {
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

    for (index, segment) in window.segments().enumerate() {
        let accrued = accrue_segment(env, ledger, market, segment);
        if let (Some(a), Some(actor)) = (accrued, actor) {
            events::emit_funding_checkpoint(
                env,
                market_id,
                actor,
                index as u32,
                if segment.long_pays() {
                    PayerSide::Long
                } else {
                    PayerSide::Short
                },
                a.receiver_backed_delta,
                a.lp_backed_delta,
                a.receiver_delta,
                a.liability_delta,
                window.ema_after,
                elapsed,
            );
        }
    }

    market.skew_ema = window.ema_after;
    market.current_payer_side = window.payer_side_at_end;
    market.current_payer_rate = window.displayed_rate_at_end;
    market.last_funding_checkpoint = now;
}

struct SegmentAccrual {
    receiver_backed_delta: i128,
    lp_backed_delta: i128,
    receiver_delta: i128,
    liability_delta: i128,
}

fn accrue_segment(
    env: &Env,
    ledger: &mut Ledger,
    market: &mut Market,
    segment: &Segment,
) -> Option<SegmentAccrual> {
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
    if payer_size == 0 || segment.funding_weight == zero {
        return None;
    }

    let carries = market.payer_remainders(long_pays).clone();

    let receiver_weight = if receiver_size == 0 || receiver_base == 0 {
        zero
    } else if receiver_base >= payer_base {
        segment.funding_weight.clone()
    } else {
        math::wide_mul_div_floor(env, &segment.funding_weight, receiver_base, payer_base)
    };
    let lp_weight = segment.funding_weight.sub(&receiver_weight);

    let denominator = math::mul(env, INDEX_PRECISION, BPS * SECONDS_PER_DAY as i128);
    let (receiver_payer_delta, receiver_payer_rem) = math::carried_div(
        env,
        &receiver_weight,
        denominator,
        carries.receiver_payer_remainder,
    );
    let (lp_payer_delta, lp_payer_rem) =
        math::carried_div(env, &lp_weight, denominator, carries.lp_payer_remainder);

    let receiver_backing_scaled = math::widen_mul(env, payer_size, receiver_payer_delta);
    let (liability_delta, liability_rem) = math::carried_div(
        env,
        &receiver_backing_scaled,
        INDEX_PRECISION,
        carries.receiver_liability_remainder,
    );
    ledger.credit(env, Bucket::ReceiverFunding, liability_delta);
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

    Some(SegmentAccrual {
        receiver_backed_delta: receiver_payer_delta,
        lp_backed_delta: lp_payer_delta,
        receiver_delta: credit_delta,
        liability_delta,
    })
}

// A distribution carry is only valid for the receiver size it was produced under (§4.5.1).
pub fn reset_receiver_distribution_remainder(market: &mut Market, changed_side_is_long: bool) {
    market
        .payer_remainders_mut(!changed_side_is_long)
        .distribution_remainder = 0;
}

pub fn cold_start(env: &Env, market: &mut Market) {
    market.skew_ema = math::skew_frac(env, market.long.base_exposure, market.short.base_exposure);
}

pub fn verify_no_final_receiver_residue(env: &Env, ledger: &Ledger) {
    if ledger.open_position_count == 0 && ledger.pending_receiver_funding_total != 0 {
        panic_with_error!(env, PositionManagerError::InvariantViolation);
    }
}

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

#[derive(Clone, Copy, Debug)]
pub struct PendingFees {
    pub funding_paid_to_receivers: i128,
    pub funding_paid_to_lps: i128,
    pub funding_received: i128,
    pub borrow: i128,
}

pub fn refresh_display(env: &Env, ledger: &mut Ledger, market: &mut Market) {
    if market.long.size_open_interest == 0 && market.short.size_open_interest == 0 {
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

pub fn pending_fees(
    env: &Env,
    ledger: &Ledger,
    position: &Position,
    market: &Market,
) -> PendingFees {
    let indices = market.funding_indices(position.is_long);
    // Round once from the index delta; rounding both ends can over-credit receivers.
    let receiver_payer_delta = index_delta(
        env,
        indices.receiver_backed_payer,
        position.receiver_payer_index_snapshot,
    );
    let lp_payer_delta = index_delta(env, indices.lp_backed_payer, position.lp_payer_index_snapshot);
    let receiver_delta = index_delta(env, indices.receiver, position.receiver_index_snapshot);
    PendingFees {
        funding_paid_to_receivers: math::index_value_ceil(env, position.size, receiver_payer_delta),
        funding_paid_to_lps: math::index_value_ceil(env, position.size, lp_payer_delta),
        funding_received: math::index_value_floor(env, position.size, receiver_delta),
        borrow: crate::borrow::calculate_pending(env, ledger, position),
    }
}

pub fn index_delta(env: &Env, current: i128, snapshot: i128) -> i128 {
    let delta = math::sub(env, current, snapshot);
    if delta < 0 {
        panic_with_error!(env, PositionManagerError::InvariantViolation);
    }
    delta
}

pub fn snapshot_funding_indices(position: &mut Position, market: &Market) {
    let indices = market.funding_indices(position.is_long);
    position.receiver_payer_index_snapshot = indices.receiver_backed_payer;
    position.lp_payer_index_snapshot = indices.lp_backed_payer;
    position.receiver_index_snapshot = indices.receiver;
}

pub fn preview_pending_fees(
    env: &Env,
    ledger: &Ledger,
    position: &Position,
    market: &Market,
    now: u64,
) -> PendingFees {
    let mut ledger_copy = ledger.clone();
    let mut market_copy = market.clone();
    crate::borrow::accrue(env, &mut ledger_copy, None, now);
    accrue(
        env,
        &mut ledger_copy,
        &position.market,
        None,
        &mut market_copy,
        now,
    );
    pending_fees(env, &ledger_copy, position, &market_copy)
}
