use soroban_sdk::{panic_with_error, Address, Env, Symbol};

use shared::constants::BPS;

use crate::errors::PositionManagerError;
use crate::ledger::{self, Bucket, Ledger};
use crate::{borrow, events, math, storage};

fn credit(env: &Env, ledger: &mut Ledger, referrer: &Address, amount: i128) {
    if amount <= 0 {
        return;
    }
    ledger.credit(env, Bucket::Referral, amount);
    let balance = storage::get_referral_balance(env, referrer);
    storage::save_referral_balance(env, referrer, math::add(env, balance, amount));
}

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

fn validate_code(env: &Env, code: &Symbol) {
    if code == &Symbol::new(env, "") {
        panic_with_error!(env, PositionManagerError::ReferralCodeInvalid);
    }
}

pub fn register_code(env: Env, owner: Address, code: Symbol) {
    owner.require_auth();
    validate_code(&env, &code);
    if storage::try_get_referral_code_owner(&env, &code).is_some() {
        panic_with_error!(&env, PositionManagerError::ReferralCodeTaken);
    }
    storage::save_referral_code_owner(&env, &code, &owner);
    events::emit_referral_code_registered(&env, &owner, &code);
}

pub fn set_referrer(env: Env, trader: Address, code: Symbol) {
    trader.require_auth();
    let referrer = storage::try_get_referral_code_owner(&env, &code)
        .unwrap_or_else(|| panic_with_error!(&env, PositionManagerError::ReferralCodeNotFound));
    if referrer == trader {
        panic_with_error!(&env, PositionManagerError::SelfReferral);
    }
    storage::save_referrer(&env, &trader, &referrer);
    events::emit_referrer_set(&env, &trader, &referrer, &code);
}

pub fn claim(env: Env, referrer: Address) {
    referrer.require_auth();
    let mut ledger = storage::get_ledger(&env);
    borrow::accrue(&env, &mut ledger, Some(&referrer), env.ledger().timestamp());
    let amount = storage::get_referral_balance(&env, &referrer);
    if amount <= 0 {
        panic_with_error!(&env, PositionManagerError::InvalidAmount);
    }
    storage::save_referral_balance(&env, &referrer, 0);
    ledger::payout_checked(&env, &mut ledger, Bucket::Referral, &referrer, amount);
    borrow::refresh_rate(&env, &mut ledger, ledger::physical_cash(&env));
    storage::save_ledger(&env, &ledger);
    events::emit_referral_claimed(&env, &referrer, amount);
}
