use soroban_sdk::{panic_with_error, Address, Env};

use shared::{
    ActionKind, ActionOutcome, FailureReason, GlobalConfig, Market, PendingAction, Position,
};

use crate::errors::PositionManagerError;
use crate::ledger::{self, Ledger};
use crate::risk::LiquidationAssessment;
use crate::{borrow, events, keeper, math, risk, storage};

pub fn fresh_for_commit(fill_observed_at: u64, commit_observed_at: u64, committed_at: u64) -> bool {
    // Sampled after the commit, not just newer than the stamp seen at commit.
    fill_observed_at > commit_observed_at && fill_observed_at > committed_at
}

pub fn delay_satisfied(now: u64, execute_after: u64) -> bool {
    now >= execute_after
}

pub fn lifetime_satisfied(now: u64, position: &Position, config: &GlobalConfig) -> bool {
    now >= position
        .last_size_increase_at
        .saturating_add(config.min_position_lifetime)
}

pub fn expired(now: u64, expires_at: u64) -> bool {
    now >= expires_at
}

pub fn entry_price_allowed(is_long: bool, price: i128, acceptable: i128) -> bool {
    acceptable == 0 || if is_long { price <= acceptable } else { price >= acceptable }
}

pub fn exit_price_allowed(is_long: bool, price: i128, acceptable: i128) -> bool {
    acceptable == 0 || if is_long { price >= acceptable } else { price <= acceptable }
}

pub fn trigger_crossed(trigger_above: bool, price: i128, trigger_price: i128) -> bool {
    if trigger_above {
        price >= trigger_price
    } else {
        price <= trigger_price
    }
}

pub fn exit_trigger_crossed(is_long: bool, take_profit: bool, price: i128, trigger: i128) -> bool {
    if is_long == take_profit {
        price >= trigger
    } else {
        price <= trigger
    }
}

pub fn load(env: &Env, action_id: u64, kind: ActionKind) -> PendingAction {
    let action = storage::get_pending_action(env, action_id);
    if action.kind != kind {
        panic_with_error!(env, PositionManagerError::WrongActionKind);
    }
    action
}

pub fn load_entry(env: &Env, action_id: u64) -> PendingAction {
    let action = storage::get_pending_action(env, action_id);
    if action.payload.open().is_none() {
        panic_with_error!(env, PositionManagerError::WrongActionKind);
    }
    action
}

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
            borrow_index_snapshot: 0,
            stored_minimum_borrow_fee: 0,
            receiver_payer_index_snapshot: 0,
            lp_payer_index_snapshot: 0,
            receiver_index_snapshot: 0,
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

    #[test]
    fn a_failing_mutation_pays_the_tightest_of_its_three_bounds() {
        let e = Env::default();
        let (reward, min_collateral) = (250i128, 1_000i128);

        let p = position(&e, 10_000);
        let a = assessment(10_000, 100);
        assert_eq!(
            payable_failure_reward(&e, &p, &a, min_collateral, reward),
            reward
        );

        let p = position(&e, 1_100);
        let a = assessment(10_000, 100);
        assert_eq!(payable_failure_reward(&e, &p, &a, min_collateral, reward), 100);

        let p = position(&e, 10_000);
        let a = assessment(5_100, 5_000);
        assert_eq!(payable_failure_reward(&e, &p, &a, min_collateral, reward), 99);
    }

    #[test]
    fn a_position_with_no_headroom_pays_a_zero_reward() {
        let e = Env::default();
        let p = position(&e, 1_000);
        let a = assessment(5_000, 5_000 - 1);
        assert_eq!(payable_failure_reward(&e, &p, &a, 1_000, 250), 0);
        let a = assessment(1_000_000, 0);
        assert_eq!(payable_failure_reward(&e, &p, &a, 1_000, 250), 0);
    }

    #[test]
    fn a_fill_on_the_commitment_observation_is_not_fresh() {
        assert!(!fresh_for_commit(100, 100, 90));
        assert!(!fresh_for_commit(99, 100, 90));
        assert!(fresh_for_commit(101, 100, 90));
    }

    #[test]
    fn a_fill_sampled_before_the_commitment_is_not_fresh() {
        assert!(!fresh_for_commit(95, 70, 100), "newer than the cursor, older than the commit");
        assert!(!fresh_for_commit(100, 70, 100), "the commit's own second is not after it");
        assert!(fresh_for_commit(101, 70, 100));
    }

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
