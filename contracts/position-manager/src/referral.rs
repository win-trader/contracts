//! Referral program (doc §11.1).
//!
//! A referrer registers a unique **code**; a trader attaches to it. On a
//! winning close, a configurable slice of the closing fee is carved out of
//! the **protocol** remainder (keeper and LP shares untouched) and accrued to
//! the referrer as a claim bucket. The referrer **pulls** it later via
//! `claim`.
//!
//! Accounting: `Ledger::referral_claimable_total` is the invariant-bearing
//! aggregate (part of `non_lp_claims`, so NAV/solvency net it out); the
//! per-referrer `ReferralBalance` map is its distribution ledger. The two
//! always move together through `credit` / the `claim` debit, so they cannot
//! drift — and if they ever did, the `debit` verb fails safe (it refuses to
//! pay out cash the aggregate does not have) rather than over-paying.

use soroban_sdk::{panic_with_error, Address, Env, Symbol};

use shared::constants::BPS;

use crate::auth::{require_auth, require_initialized};
use crate::errors::PositionManagerError;
use crate::ledger::{self, Bucket, Ledger};
use crate::{borrow, events, math, storage};

/// Lockstep credit of a referral reward: the aggregate claim total and the
/// per-referrer balance move by the same amount, in one place.
fn credit(env: &Env, ledger: &mut Ledger, referrer: &Address, amount: i128) {
    if amount <= 0 {
        return;
    }
    ledger.credit(env, Bucket::Referral, amount);
    let balance = storage::get_referral_balance(env, referrer);
    storage::save_referral_balance(env, referrer, math::add(env, balance, amount));
}

/// §3.6 — carve the referral reward out of a collected **opening or
/// closing** fee and accrue it to the trader's referrer, if any.
///
/// Returns the carve-out so the caller subtracts it from the protocol
/// remainder; the LP share is computed off the full fee and is unaffected.
/// Returns `0` when the trader has no referrer or the share is disabled.
/// Changing the mapping affects only fees collected afterward — this reads
/// the referrer at collection time and accrues nothing retroactively.
pub fn accrue(
    env: &Env,
    ledger: &mut Ledger,
    owner: &Address,
    collected_fee: i128,
    position_id: u64,
) -> i128 {
    let referrer = match storage::get_referrer(env, owner) {
        Some(r) => r,
        None => return 0,
    };
    let bps = storage::get_global_config(env).referral_fee_share_bps;
    let cut = math::mul_div_floor(env, collected_fee, bps as i128, BPS);
    if cut <= 0 {
        return 0;
    }
    credit(env, ledger, &referrer, cut);
    events::emit_referral_accrued(env, &referrer, position_id, cut);
    cut
}

/// A referral code must be a non-empty symbol; the `Symbol` type already
/// bounds it to ≤ 32 characters of `[a-zA-Z0-9_]`.
fn validate_code(env: &Env, code: &Symbol) {
    if code == &Symbol::new(env, "") {
        panic_with_error!(env, PositionManagerError::ReferralCodeInvalid);
    }
}

/// Claim ownership of a referral code (first-come, owner immutable). One
/// address may own several codes; a taken code is rejected.
pub fn register_code(env: Env, owner: Address, code: Symbol) {
    require_initialized(&env);
    require_auth(&owner);
    validate_code(&env, &code);
    if storage::try_get_referral_code_owner(&env, &code).is_some() {
        panic_with_error!(&env, PositionManagerError::ReferralCodeTaken);
    }
    storage::save_referral_code_owner(&env, &code, &owner);
    events::emit_referral_code_registered(&env, &owner, &code);
}

/// Point the caller at a referrer via that referrer's code. Freely
/// re-settable (last write wins); a trader cannot refer themselves.
pub fn set_referrer(env: Env, trader: Address, code: Symbol) {
    require_initialized(&env);
    require_auth(&trader);
    let referrer = storage::try_get_referral_code_owner(&env, &code)
        .unwrap_or_else(|| panic_with_error!(&env, PositionManagerError::ReferralCodeNotFound));
    if referrer == trader {
        panic_with_error!(&env, PositionManagerError::SelfReferral);
    }
    storage::save_referrer(&env, &trader, &referrer);
    events::emit_referrer_set(&env, &trader, &referrer, &code);
}

/// Withdraw the caller's accrued referral rewards. Conservation-checked (the
/// same path as `claim_protocol`), so it is blocked during a cash shortfall.
pub fn claim(env: Env, referrer: Address) {
    require_initialized(&env);
    require_auth(&referrer);
    let mut ledger = storage::get_ledger(&env);
    borrow::accrue(&env, &mut ledger, env.ledger().timestamp());
    let amount = storage::get_referral_balance(&env, &referrer);
    if amount <= 0 {
        panic_with_error!(&env, PositionManagerError::InvalidAmount);
    }
    // Zero the per-referrer balance and debit the aggregate by the same
    // amount (inside `payout_checked`) — the two stay in lockstep.
    storage::save_referral_balance(&env, &referrer, 0);
    ledger::payout_checked(&env, &mut ledger, Bucket::Referral, &referrer, amount);
    borrow::refresh_rate(&env, &mut ledger, ledger::physical_cash(&env));
    storage::save_ledger(&env, &ledger);
    events::emit_referral_claimed(&env, &referrer, amount);
}
