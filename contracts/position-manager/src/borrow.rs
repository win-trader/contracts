//! The borrow clock — doc §9.2 and §10.1: the global index that accrues
//! the vault-capacity rent on risk units, and the utilization-driven rate
//! that feeds it.
//!
//! The rate is piecewise-constant between mutations: `accrue` advances the
//! index with the *stored* rate over elapsed time (division remainder
//! carried exactly), and `refresh_rate` re-derives the rate only after the
//! mutation that changed it — so a state change can never reprice time
//! that already passed (§18.4).

use soroban_sdk::{Address, Env};

use shared::constants::{BPS, INDEX_PRECISION, SECONDS_PER_DAY};

use shared::Position;
use soroban_sdk::panic_with_error;

use crate::errors::PositionManagerError;
use crate::ledger::Ledger;
use crate::{events, math, storage};

/// §10.1 — advance the global borrow index with the stored rate. A second
/// call at the same timestamp has no effect.
/// `actor` is `None` for §4.12's read-only quote, which runs this same code
/// on copies. A quote must write nothing, and an emitted event is a written
/// thing: passing the actor is how a caller says "this is a real checkpoint
/// and here is who to credit for it".
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
    // §12.6 — only when the index actually moved. A sub-unit window carries
    // entirely in the remainder and changes nothing a consumer could act on,
    // and every trader action checkpoints, so emitting regardless would put
    // an empty event on every transaction.
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

/// §6.14 — recompute the stored borrow rate from current utilization along
/// the fixed quadratic curve. Call after any mutation that changes risk
/// units, physical cash, or any non-LP claim affecting cash LP equity
/// (§4.9 step 9).
///
/// `u` is converted to `INDEX_PRECISION` **before** squaring. Written as
/// `utilization / BPS` the integer division would collapse to `0` or `1`,
/// and the curve would be a step function. Both addends are formed at
/// `INDEX_PRECISION` before they are summed: the base rate is a plain bps
/// number scaled explicitly, and the variable term is a bps number
/// multiplied by an already-scaled factor.
///
/// The square is exact at every utilization the curve can reach — `u` is a
/// multiple of `1e10` and its square a multiple of `1e6` — so this function
/// introduces no approximation at all.
pub fn refresh_rate(env: &Env, ledger: &mut Ledger, physical_cash: i128) {
    let equity = ledger.cash_lp_equity(env, physical_cash);
    ledger.current_borrow_rate = rate_at(env, ledger.total_risk_units, equity);
}

/// §6.14 — the curve itself, as a pure function of the risk units and cash
/// LP equity it is measured at.
///
/// Split out of `refresh_rate` because §7.2's `projected_minimum_borrow`
/// must quote the **post-settlement** rate: the position's borrow window
/// opens after the health checks, and quoting it from the pre-action
/// utilization would let a position be admitted exactly at initial margin
/// and be below it the moment its window exists.
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

/// §6.3 — what a position owes for its active borrow window.
///
/// `actual` and `minimum` are carried alongside `due` because §6.7's two
/// paths differ in what they may collect: the surviving path requires
/// `due` in full, while terminal settlement may collect less and reports
/// the shortfall.
#[derive(Clone, Copy, Debug)]
pub struct PendingBorrow {
    /// Index growth since the window's baseline.
    #[allow(dead_code)]
    pub actual: i128,
    /// The window's stored monetary minimum.
    #[allow(dead_code)]
    pub minimum: i128,
    /// What is actually owed: the greater of the two.
    pub due: i128,
}

/// §6.3 — the pending borrow obligation.
///
/// The non-negativity check runs **before** the minimum is applied. A
/// minimum may raise a valid obligation; it may never conceal a broken
/// baseline, which is what a decreasing index or a corrupted
/// `borrow_debt` would look like.
pub fn calculate_pending(env: &Env, ledger: &Ledger, position: &Position) -> PendingBorrow {
    let cumulative_value = math::mul_div_ceil(
        env,
        position.risk_units,
        ledger.borrow_index,
        INDEX_PRECISION,
    );
    let raw_actual = math::sub(env, cumulative_value, position.borrow_debt);
    if raw_actual < 0 {
        panic_with_error!(env, PositionManagerError::InvariantViolation);
    }
    PendingBorrow {
        actual: raw_actual,
        minimum: position.stored_minimum_borrow_fee,
        due: core::cmp::max(raw_actual, position.stored_minimum_borrow_fee),
    }
}

/// §4.10 / §6.7 — open a borrow window for the position's **resulting**
/// risk units.
///
/// Runs after the exposure mutation and after the global rate refresh, in
/// that order: the baseline stops the new exposure from paying historical
/// index growth, and the monetary minimum is quoted from the rate that
/// applies going forward. Quoting it before the refresh would price the
/// window at the pre-mutation utilization.
///
/// Nothing carries forward from the previous window — no tranche, no
/// proportional remainder, no old minimum (§3.3.3).
pub fn initialize_window(env: &Env, ledger: &Ledger, position: &mut Position) {
    let config = storage::get_global_config(env);
    position.borrow_debt = math::mul_div_ceil(
        env,
        position.risk_units,
        ledger.borrow_index,
        INDEX_PRECISION,
    );
    // §6.7 — a full-precision checked operation; the grouped expression
    // does not authorize an overflowing intermediate.
    position.stored_minimum_borrow_fee = math::mul_mul_div_ceil(
        env,
        position.risk_units,
        ledger.current_borrow_rate,
        config.min_borrow_fee_seconds as i128,
        math::mul(env, INDEX_PRECISION, BPS * SECONDS_PER_DAY as i128),
    );
}

/// §7.2 / §7.8 — the monetary minimum `initialize_window` will quote for a
/// window of `risk_units` opened at `rate`.
///
/// Admission needs this before the window exists: the floor is part of
/// pending borrow from the window's first second (§2.9), so a position
/// admitted exactly at initial margin would be below it the moment its
/// window is quoted. Every input is known at preflight — the caller pairs
/// it with `rate_at` evaluated on the projected post-settlement book.
///
/// The same expression as `initialize_window`'s, deliberately: if the two
/// could disagree the preflight would be checking a different number from
/// the one the position is charged.
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
