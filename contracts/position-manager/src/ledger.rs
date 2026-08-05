//! The global accounting ledger — doc §4 "Sources of truth" and §5.1
//! "Global state" as one aggregate.
//!
//! The `Ledger` is loaded once at the start of a state-changing entry point,
//! mutated in memory, and stored once at the end (the Blend `Pool` cache
//! pattern). Business logic never touches ledger storage keys directly, so
//! every mutation of a claim total is visible in this module's call graph.
//!
//! Trust note: the ledger stays cached across the external calls an action
//! makes (vault transfers, oracle reads). Neither the vault nor the
//! governance-chosen collateral token calls back into this contract, so no
//! reentrant read can observe the stale stored copy.
//!
//! Physical cash is *never* stored here: it is always
//! `collateral_token.balanceOf(vault)` (§4.1), read via the vault. Cash LP
//! equity, free capital, and NAV are always derived (§4.3).

use soroban_sdk::{contracttype, panic_with_error, Env};

use shared::{MarketSide, Position, VaultClient};

use crate::errors::PositionManagerError;
use crate::{math, storage};

/// §5.1 global state: the five non-LP claim totals, the risk counters, and
/// the global borrow accrual. The receiver-funding liability total is fed
/// per-market by `checkpoint::checkpoint_market` (§8.3).
#[contracttype]
#[derive(Clone, Debug)]
pub struct Ledger {
    // -- Non-LP claims (§4.2): each is a label on the one vault balance. --
    pub position_collateral_total: i128,
    pub pending_receiver_funding_total: i128,
    pub execution_budget_total: i128,
    pub protocol_claimable_total: i128,
    pub risk_keeper_reserve_total: i128,
    // -- Risk counters. --
    pub total_risk_units: i128,
    pub open_position_count: u64,
    pub lp_blocked_side_count: u32,
    // -- Global borrow accrual (§9.2). --
    pub borrow_index: i128,
    pub borrow_index_remainder: i128,
    /// `INDEX_PRECISION`-scaled bps/day rate for the current interval.
    pub current_borrow_rate: i128,
    pub last_global_checkpoint: u64,
}

impl Ledger {
    pub fn new(now: u64, initial_borrow_rate: i128) -> Self {
        Ledger {
            position_collateral_total: 0,
            pending_receiver_funding_total: 0,
            execution_budget_total: 0,
            protocol_claimable_total: 0,
            risk_keeper_reserve_total: 0,
            total_risk_units: 0,
            open_position_count: 0,
            lp_blocked_side_count: 0,
            borrow_index: 0,
            borrow_index_remainder: 0,
            current_borrow_rate: initial_borrow_rate,
            last_global_checkpoint: now,
        }
    }

    /// §4.2 — complete non-LP claims on the vault's physical cash.
    pub fn non_lp_claims(&self, env: &Env) -> i128 {
        let mut total = self.position_collateral_total;
        total = math::add(env, total, self.pending_receiver_funding_total);
        total = math::add(env, total, self.execution_budget_total);
        total = math::add(env, total, self.protocol_claimable_total);
        math::add(env, total, self.risk_keeper_reserve_total)
    }

    /// §4.3 — `max(physical_cash - non_lp_claims, 0)`, never stored.
    pub fn cash_lp_equity(&self, env: &Env, physical_cash: i128) -> i128 {
        let claims = self.non_lp_claims(env);
        core::cmp::max(
            math::sub(env, physical_cash, core::cmp::min(physical_cash, claims)),
            0,
        )
    }
}

/// Read `collateral_token.balanceOf(vault)` — the only authoritative cash
/// balance (§4.1). Costs a vault + token hop; actions read it once per
/// distinct balance state, not per use.
pub fn physical_cash(env: &Env) -> i128 {
    VaultClient::new(env, &storage::vault(env)).physical_cash()
}

// ---------------------------------------------------------------------------
// Stored-collateral choke point.
//
// Every mutation of position collateral moves three legs together — the
// position's stored collateral, its market side's collateral aggregate, and
// the global claim total — so aggregate conservation (§18.2) cannot be
// broken by a call site updating one leg and forgetting another.
// ---------------------------------------------------------------------------

/// Add `amount` to a position's stored collateral (all three legs).
pub fn add_stored_collateral(
    env: &Env,
    ledger: &mut Ledger,
    position: &mut Position,
    side: &mut MarketSide,
    amount: i128,
) {
    if amount == 0 {
        return;
    }
    if amount < 0 {
        panic_with_error!(env, PositionManagerError::InvariantViolation);
    }
    position.stored_collateral = math::add(env, position.stored_collateral, amount);
    side.stored_collateral_total = math::add(env, side.stored_collateral_total, amount);
    ledger.position_collateral_total = math::add(env, ledger.position_collateral_total, amount);
}

/// Take up to `amount` from a position's stored collateral (all three legs).
/// Returns the amount actually collected — never more than the position has.
pub fn collect_stored_collateral(
    env: &Env,
    ledger: &mut Ledger,
    position: &mut Position,
    side: &mut MarketSide,
    amount: i128,
) -> i128 {
    let collected = core::cmp::min(position.stored_collateral, amount);
    if collected <= 0 {
        return 0;
    }
    position.stored_collateral = math::sub(env, position.stored_collateral, collected);
    side.stored_collateral_total = math::sub(env, side.stored_collateral_total, collected);
    ledger.position_collateral_total = math::sub(env, ledger.position_collateral_total, collected);
    collected
}
