use soroban_sdk::{Address, Env};

use shared::constants::{BPS, INDEX_PRECISION, SECONDS_PER_DAY};

use shared::Position;

use crate::ledger::Ledger;
use crate::{events, math, storage};

pub fn accrue(env: &Env, ledger: &mut Ledger, actor: Option<&Address>, now: u64) {
    if now <= ledger.last_global_checkpoint {
        return;
    }
    let elapsed = now - ledger.last_global_checkpoint;

    let denominator = BPS * SECONDS_PER_DAY as i128;
    let numerator = math::add(
        env,
        math::mul(env, ledger.current_borrow_rate, elapsed as i128),
        ledger.borrow_index_remainder,
    );
    let delta = numerator / denominator;
    ledger.borrow_index = math::add(env, ledger.borrow_index, delta);
    ledger.borrow_index_remainder = numerator % denominator;

    ledger.last_global_checkpoint = now;
    if let (Some(actor), true) = (actor, delta != 0) {
        events::emit_borrow_checkpoint(
            env,
            actor,
            elapsed,
            delta,
            ledger.current_borrow_rate,
            ledger.borrow_index,
        );
    }
}

pub fn refresh_rate(env: &Env, ledger: &mut Ledger, physical_cash: i128) {
    let equity = ledger.cash_lp_equity(env, physical_cash);
    ledger.current_borrow_rate = rate_at(env, ledger.total_risk_units, equity);
}

pub fn rate_at(env: &Env, total_risk_units: i128, cash_lp_equity: i128) -> i128 {
    let config = storage::get_global_config(env);
    let utilization = math::utilization_bps(env, total_risk_units, cash_lp_equity);
    let u = math::mul_div_floor(env, utilization, INDEX_PRECISION, BPS);
    let variable_factor = math::mul_div_floor(env, u, u, INDEX_PRECISION);
    math::add(
        env,
        math::mul(env, config.base_borrow_rate_bps_day, INDEX_PRECISION),
        math::mul(env, config.max_variable_borrow_bps_day, variable_factor),
    )
}

pub fn calculate_pending(env: &Env, ledger: &Ledger, position: &Position) -> i128 {
    let delta = crate::funding::index_delta(env, ledger.borrow_index, position.borrow_index_snapshot);
    let actual = math::mul_div_ceil(env, position.risk_units, delta, INDEX_PRECISION);
    core::cmp::max(actual, position.stored_minimum_borrow_fee)
}

pub fn initialize_window(env: &Env, ledger: &Ledger, position: &mut Position) {
    let config = storage::get_global_config(env);
    position.borrow_index_snapshot = ledger.borrow_index;
    position.stored_minimum_borrow_fee = math::mul_mul_div_ceil(
        env,
        position.risk_units,
        ledger.current_borrow_rate,
        config.min_borrow_fee_seconds as i128,
        math::mul(env, INDEX_PRECISION, BPS * SECONDS_PER_DAY as i128),
    );
}

pub fn projected_minimum(env: &Env, rate: i128, risk_units: i128) -> i128 {
    let config = storage::get_global_config(env);
    math::mul_mul_div_ceil(
        env,
        risk_units,
        rate,
        config.min_borrow_fee_seconds as i128,
        math::mul(env, INDEX_PRECISION, BPS * SECONDS_PER_DAY as i128),
    )
}
