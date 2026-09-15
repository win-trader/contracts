//! §7.0 — the common machinery of the two-phase action lifecycle.
//!
//! Every price-sensitive trader action is a commitment followed by a
//! settlement against a **strictly newer** observation. This module owns the
//! predicates both halves are held to, the escrow-bearing terminal helpers,
//! and the outcome vocabulary.
//!
//! The one structural rule worth stating up front: a settlement function
//! **returns** an outcome, it does not panic on an expected business
//! failure. A panic reverts, which would undo the keeper payment and the
//! escrow refund, leave the action pending, and hand the trader a free retry
//! after they had already seen the price (§8.9). Only genuinely unexpected
//! conditions — a wrong action kind, a broken aggregate, a failed transfer —
//! revert.

use soroban_sdk::{panic_with_error, Address, Env};

use shared::{
    ActionKind, ActionOutcome, FailureReason, GlobalConfig, Market, PendingAction, Position,
};

use crate::errors::PositionManagerError;
use crate::ledger::{self, Ledger};
use crate::risk::LiquidationAssessment;
use crate::{borrow, events, keeper, math, risk, storage};

// ---------------------------------------------------------------------------
// §7.0 predicates.
//
// Pure, total, and shared by every path, so "what makes an attempt eligible"
// is one definition rather than a condition repeated at six call sites.
// ---------------------------------------------------------------------------

/// §8.6 — the fill must be a *later observation* than the commitment, not
/// merely a later transaction. **Equality fails**: a price carrying the same
/// observation stamp contains no information created after the trader
/// committed, whatever the block clock says.
pub fn fresh_for_commit(fill_observed_at: u64, commit_observed_at: u64) -> bool {
    fill_observed_at > commit_observed_at
}

/// §8.5 — the execution delay, inclusive at the boundary.
pub fn delay_satisfied(now: u64, execute_after: u64) -> bool {
    now >= execute_after
}

/// §8.5 — the second timing gate, on every action that *removes* exposure.
///
/// Read from the position at settlement rather than frozen at creation, so a
/// size increase landing between commitment and settlement moves it forward.
/// It is measured from `last_size_increase_at` and applies uniformly to
/// decrease, close, take-profit, and stop-loss.
pub fn lifetime_satisfied(now: u64, position: &Position, config: &GlobalConfig) -> bool {
    now >= position
        .last_size_increase_at
        .saturating_add(config.min_position_lifetime)
}

/// §8.5 — `expired = now >= expires_at`. At exactly `expires_at` execution
/// is forbidden and cleanup is allowed; there is no timestamp at which both
/// succeed.
pub fn expired(now: u64, expires_at: u64) -> bool {
    now >= expires_at
}

/// §8.7 — the price bound on an action that *adds* exposure. `0` disables it.
pub fn entry_price_allowed(is_long: bool, price: i128, acceptable: i128) -> bool {
    acceptable == 0 || if is_long { price <= acceptable } else { price >= acceptable }
}

/// §8.7 — the price bound on an action that *removes* exposure.
pub fn exit_price_allowed(is_long: bool, price: i128, acceptable: i128) -> bool {
    acceptable == 0 || if is_long { price >= acceptable } else { price <= acceptable }
}

/// A limit entry's trigger, with its direction frozen at creation.
pub fn trigger_crossed(trigger_above: bool, price: i128, trigger_price: i128) -> bool {
    if trigger_above {
        price >= trigger_price
    } else {
        price <= trigger_price
    }
}

/// §7.11 / §7.12 — a take-profit crosses upward for a long and downward for
/// a short; a stop-loss is the mirror.
pub fn exit_trigger_crossed(is_long: bool, take_profit: bool, price: i128, trigger: i128) -> bool {
    // A long's take-profit and a short's stop-loss both fire on a rise.
    if is_long == take_profit {
        price >= trigger
    } else {
        price <= trigger
    }
}

// ---------------------------------------------------------------------------
// Loading.
// ---------------------------------------------------------------------------

/// Load a pending action and require it to be of `kind`.
///
/// A consumed ID fails here, before any transfer or reward (§8.13): removal
/// is what makes the ID unusable, so two keepers racing for one action
/// cannot both be paid. A kind mismatch is an unexpected condition and
/// reverts.
pub fn load(env: &Env, action_id: u64, kind: ActionKind) -> PendingAction {
    let action = storage::get_pending_action(env, action_id);
    if action.kind != kind {
        panic_with_error!(env, PositionManagerError::WrongActionKind);
    }
    action
}

/// Load a pending entry of either kind — the expiry-cleanup path, which
/// treats a market and a limit entry identically.
pub fn load_entry(env: &Env, action_id: u64) -> PendingAction {
    let action = storage::get_pending_action(env, action_id);
    if action.payload.open().is_none() {
        panic_with_error!(env, PositionManagerError::WrongActionKind);
    }
    action
}

/// §8.4 — the position a mutation action names, with the reverse reference
/// verified in both directions.
///
/// Checking only one direction would let an action outlive the reference
/// that blocks a second commitment, or a reference point at an action that
/// no longer exists.
pub fn load_target(env: &Env, action: &PendingAction) -> Position {
    let position_id = action
        .payload
        .position_id()
        .unwrap_or_else(|| panic_with_error!(env, PositionManagerError::WrongActionKind));
    let position = storage::get_position(env, position_id);
    if position.pending_mutation_action_id != Some(action.action_id) {
        panic_with_error!(env, PositionManagerError::WrongActionKind);
    }
    position
}

// ---------------------------------------------------------------------------
// §7.0 terminal helpers.
// ---------------------------------------------------------------------------

/// §7.0 `fail_entry_action` — the terminal path for an eligible entry
/// attempt that failed an expected check.
///
/// It pays the action's configured reward from escrow, refunds the whole
/// remainder to the **owner frozen in the action** (§8.10: the settlement
/// caller cannot redirect it), removes the record, and charges no opening
/// fee (§9.12). The escrow reaches zero before the record is removed, which
/// is §5.7's rule that no terminal action leaves positive escrow attached.
pub fn fail_entry_action(
    env: &Env,
    ledger: &mut Ledger,
    market: &Market,
    action: &mut PendingAction,
    caller: &Address,
    reward: i128,
    reason: FailureReason,
) -> ActionOutcome {
    keeper::pay_from_escrow(
        env,
        ledger,
        &mut action.escrowed_collateral,
        caller,
        reward,
    );
    let refund = action.escrowed_collateral;
    action.escrowed_collateral = 0;
    ledger::refund_escrow(env, ledger, &action.owner, refund);
    storage::remove_pending_action(env, action.action_id);
    storage::save_market(env, &action.market_id, market);
    borrow::refresh_rate(env, ledger, ledger::physical_cash(env));
    storage::save_ledger(env, ledger);
    events::emit_action_failed(
        env,
        action.action_id,
        &action.owner,
        &action.market_id,
        action.kind,
        reason,
        caller,
        reward,
        refund,
    );
    ActionOutcome::Failed
}

/// §7.0 — what a failing position mutation can safely pay its keeper.
///
/// `min(configured, headroom above minimum collateral, headroom above the
/// liquidation threshold)`. The `- 1` is deliberate: liquidation eligibility
/// is `effective <= threshold`, so leaving the position *at* the threshold
/// would make it liquidatable, which a failed voluntary attempt must not do.
fn payable_failure_reward(
    env: &Env,
    position: &Position,
    assessment: &LiquidationAssessment,
    min_collateral: i128,
    reward: i128,
) -> i128 {
    let above_minimum = core::cmp::max(math::sub(env, position.stored_collateral, min_collateral), 0);
    let above_threshold = core::cmp::max(
        math::sub(
            env,
            math::sub(env, assessment.effective_collateral, assessment.threshold),
            1,
        ),
        0,
    );
    core::cmp::min(reward, core::cmp::min(above_minimum, above_threshold))
}

/// §7.0 `fail_position_action` — the terminal path for an eligible increase,
/// decrease, or close that failed an expected check.
///
/// **The reward is variable; the finality is not.** The keeper receives
/// whatever the position can pay without dropping below minimum collateral
/// or into liquidation, which may be zero, and the action terminates either
/// way.
///
/// Refusing to terminate on an unpayable reward would leave an eligible
/// action pending forever while `pending_mutation_action_id` blocks every
/// further increase, decrease, and close — and a position mutation has
/// neither a cancel operation nor an expiry, so the only remaining exits
/// would be `add_collateral` and liquidation. It would also break
/// first-attempt finality: an action that survives an eligible attempt is a
/// free retry.
///
/// The caller must already have returned `RequiresLiquidation` for a
/// liquidatable position; reaching here with one is an invariant break, not
/// a business outcome.
#[allow(clippy::too_many_arguments)]
pub fn fail_position_action(
    env: &Env,
    ledger: &mut Ledger,
    market: &mut Market,
    position: &mut Position,
    action: &mut PendingAction,
    caller: &Address,
    reward: i128,
    reason: FailureReason,
    assessment: &LiquidationAssessment,
    price: i128,
) -> ActionOutcome {
    if assessment.liquidatable {
        panic_with_error!(env, PositionManagerError::InvariantViolation);
    }
    let min_collateral = storage::get_global_config(env).min_collateral;
    let payable = payable_failure_reward(env, position, assessment, min_collateral, reward);
    let paid = if payable > 0 {
        let is_long = position.is_long;
        ledger::payout_collateral(
            env,
            ledger,
            position,
            market.side_mut(is_long),
            caller,
            payable,
        )
    } else {
        0
    };

    let refund = action.escrowed_collateral;
    action.escrowed_collateral = 0;
    ledger::refund_escrow(env, ledger, &action.owner, refund);

    position.pending_mutation_action_id = None;
    storage::remove_pending_action(env, action.action_id);

    let physical = ledger::physical_cash(env);
    let equity = ledger.cash_lp_equity(env, physical);
    risk::evaluate_market_risk(env, ledger, &action.market_id, caller, market, price, equity);
    storage::save_market(env, &action.market_id, market);
    storage::save_position(env, position);
    borrow::refresh_rate(env, ledger, physical);
    storage::save_ledger(env, ledger);

    events::emit_action_failed(
        env,
        action.action_id,
        &action.owner,
        &action.market_id,
        action.kind,
        reason,
        caller,
        paid,
        refund,
    );
    ActionOutcome::Failed
}

/// §8.12 steps 4-6 — a forced safety action removed the position, so its
/// pending voluntary mutation is superseded.
///
/// The complete added-collateral escrow goes back to the owner and **no**
/// reward is paid for it: the caller receives only the forced action's
/// reward (§8.11). Cleanup inside a forced action is not a second keeper
/// action.
pub fn supersede_pending_mutation(
    env: &Env,
    ledger: &mut Ledger,
    actor: &Address,
    position: &mut Position,
) {
    let Some(action_id) = position.pending_mutation_action_id else {
        return;
    };
    position.pending_mutation_action_id = None;
    // The reference can only be cleared together with the record it names,
    // so a missing record here is a broken invariant rather than a race.
    let mut action = storage::get_pending_action(env, action_id);
    let refund = action.escrowed_collateral;
    action.escrowed_collateral = 0;
    ledger::refund_escrow(env, ledger, &action.owner, refund);
    storage::remove_pending_action(env, action_id);
    events::emit_action_superseded(
        env,
        &position.market,
        actor,
        action_id,
        position.id,
        &action.owner,
        refund,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    use shared::Trigger;
    use soroban_sdk::{testutils::Address as _, Address, Symbol};

    fn position(env: &Env, stored_collateral: i128) -> Position {
        Position {
            id: 1,
            owner: Address::generate(env),
            market: Symbol::new(env, "BTC"),
            is_long: true,
            size: 0,
            base_exposure: 0,
            stored_collateral,
            risk_units: 0,
            borrow_debt: 0,
            stored_minimum_borrow_fee: 0,
            funding_paid_to_receivers_debt: 0,
            funding_paid_to_lps_debt: 0,
            funding_received_debt: 0,
            opened_at: 0,
            last_size_increase_at: 0,
            pending_mutation_action_id: None,
            take_profit: Trigger::None,
            stop_loss: Trigger::None,
        }
    }

    fn assessment(effective_collateral: i128, threshold: i128) -> LiquidationAssessment {
        LiquidationAssessment {
            liquidatable: effective_collateral <= threshold,
            insolvent: effective_collateral < 0,
            effective_collateral,
            threshold,
            payable_pnl: 0,
        }
    }

    /// §7.0 — the reward is the *smallest* of the three bounds, so whichever
    /// constraint binds is the one that decides the payment.
    #[test]
    fn a_failing_mutation_pays_the_tightest_of_its_three_bounds() {
        let e = Env::default();
        let (reward, min_collateral) = (250i128, 1_000i128);

        // Nothing binds: the configured reward is paid in full.
        let p = position(&e, 10_000);
        let a = assessment(10_000, 100);
        assert_eq!(
            payable_failure_reward(&e, &p, &a, min_collateral, reward),
            reward
        );

        // Minimum collateral binds: only $100 of headroom above it.
        let p = position(&e, 1_100);
        let a = assessment(10_000, 100);
        assert_eq!(payable_failure_reward(&e, &p, &a, min_collateral, reward), 100);

        // The liquidation threshold binds, and the `- 1` is load-bearing:
        // eligibility is `effective <= threshold`, so paying the position
        // down *to* the threshold would make it liquidatable — which a
        // failed voluntary attempt must never do.
        let p = position(&e, 10_000);
        let a = assessment(5_100, 5_000);
        assert_eq!(payable_failure_reward(&e, &p, &a, min_collateral, reward), 99);
    }

    /// §7.0 — "the reward is variable, the finality is not". A position with
    /// no headroom at all pays zero, and the caller still terminates the
    /// action.
    #[test]
    fn a_position_with_no_headroom_pays_a_zero_reward() {
        let e = Env::default();
        let p = position(&e, 1_000);
        let a = assessment(5_000, 5_000 - 1);
        assert_eq!(payable_failure_reward(&e, &p, &a, 1_000, 250), 0);
        // Sitting exactly at minimum collateral is enough on its own.
        let a = assessment(1_000_000, 0);
        assert_eq!(payable_failure_reward(&e, &p, &a, 1_000, 250), 0);
    }

    /// §8.6 — equality fails. The boundary is the whole point of the
    /// predicate: a stamp equal to the commitment cursor is the same
    /// observation the trader already saw.
    #[test]
    fn a_fill_on_the_commitment_observation_is_not_fresh() {
        assert!(!fresh_for_commit(100, 100));
        assert!(!fresh_for_commit(99, 100));
        assert!(fresh_for_commit(101, 100));
    }

    /// §8.5 — the delay boundary is inclusive and the expiry boundary is
    /// not, so there is no timestamp at which both execution and cleanup
    /// are permitted.
    #[test]
    fn execution_and_cleanup_never_overlap() {
        let (execute_after, expires_at) = (10u64, 20u64);
        for now in 0u64..30 {
            let executable = delay_satisfied(now, execute_after) && !expired(now, expires_at);
            let cleanable = expired(now, expires_at);
            assert!(!(executable && cleanable), "both at {now}");
        }
        assert!(delay_satisfied(10, execute_after), "inclusive delay");
        assert!(expired(20, expires_at), "inclusive expiry");
    }

    /// §8.7's table, both directions, plus the zero sentinel.
    #[test]
    fn price_bounds_are_mirrored_between_entry_and_exit() {
        assert!(entry_price_allowed(true, 100, 100), "long buys at or below");
        assert!(!entry_price_allowed(true, 101, 100));
        assert!(entry_price_allowed(false, 100, 100), "short sells at or above");
        assert!(!entry_price_allowed(false, 99, 100));

        assert!(exit_price_allowed(true, 100, 100), "long exits at or above");
        assert!(!exit_price_allowed(true, 99, 100));
        assert!(exit_price_allowed(false, 100, 100), "short exits at or below");
        assert!(!exit_price_allowed(false, 101, 100));

        for is_long in [true, false] {
            assert!(entry_price_allowed(is_long, 1, 0), "zero disables");
            assert!(exit_price_allowed(is_long, 1, 0), "zero disables");
        }
    }

    /// §7.11/§7.12 — a long takes profit on the way up and stops out on the
    /// way down; a short is the mirror of both.
    #[test]
    fn exit_triggers_fire_on_the_right_side_for_each_direction() {
        assert!(exit_trigger_crossed(true, true, 110, 110), "long TP");
        assert!(!exit_trigger_crossed(true, true, 109, 110));
        assert!(exit_trigger_crossed(true, false, 90, 90), "long SL");
        assert!(!exit_trigger_crossed(true, false, 91, 90));
        assert!(exit_trigger_crossed(false, true, 90, 90), "short TP");
        assert!(!exit_trigger_crossed(false, true, 91, 90));
        assert!(exit_trigger_crossed(false, false, 110, 110), "short SL");
        assert!(!exit_trigger_crossed(false, false, 109, 110));
    }
}
