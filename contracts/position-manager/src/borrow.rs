//! The borrow clock — doc §9.2 and §10.1: the global index that accrues
//! the vault-capacity rent on risk units, and the utilization-driven rate
//! that feeds it.
//!
//! The rate is piecewise-constant between mutations: `accrue` advances the
//! index with the *stored* rate over elapsed time (division remainder
//! carried exactly), and `refresh_rate` re-derives the rate only after the
//! mutation that changed it — so a state change can never reprice time
//! that already passed (§18.4).

use soroban_sdk::Env;

use shared::constants::{BPS, INDEX_PRECISION, SECONDS_PER_DAY};

use crate::ledger::Ledger;
use crate::{math, storage};

/// §10.1 — advance the global borrow index with the stored rate. A second
/// call at the same timestamp has no effect.
pub fn accrue(env: &Env, ledger: &mut Ledger, now: u64) {
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

/// Placeholder while the borrow curve is rebuilt (P1-07). The configurable
/// power curve is deleted and §6.14's fixed square lands in P4-02; until
/// then every window prices at the base rate, so no stale curve survives
/// the deletion and the variable term contributes nothing.
pub fn refresh_rate(env: &Env, ledger: &mut Ledger, _physical_cash: i128) {
    let config = storage::get_global_config(env);
    ledger.current_borrow_rate = math::mul(env, config.base_borrow_rate_bps_day, INDEX_PRECISION);
}
