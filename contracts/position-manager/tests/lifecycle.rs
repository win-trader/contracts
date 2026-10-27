//! Integration coverage for the §7/§8 two-phase action lifecycle.

mod common;

use common::{global_config, World, PRICE};
use shared::{defaults, ActionOutcome, OpenPayload};
use soroban_sdk::{testutils::Address as _, Address};

/// §7.1/§7.2 — the happy path, and the two things creation must *not* do.
///
/// Creating the commitment moves the trader's cash into the vault and
/// creates nothing else: no position, no exposure. Settlement against a
/// later observation is what produces the position.
#[test]
fn a_market_open_escrows_at_commitment_and_opens_at_settlement() {
    let w = World::new();
    let c = w.client();
    let before = w.balance(&w.trader);
    let collateral = 100_000_0000;

    let action_id = c.create_market_open(&w.trader, &w.market, &w.request(collateral, 0, 120));

    assert_eq!(
        w.balance(&w.trader),
        before - collateral,
        "creation transfers the full submitted collateral into escrow"
    );
    assert_eq!(
        c.non_lp_claims() >= collateral,
        true,
        "escrow is an explicit non-LP claim while it is pending (§9.11)"
    );
    let action = c.get_pending_action(&action_id);
    assert_eq!(action.escrowed_collateral, collateral);
    assert_eq!(c.accounting_snapshot(&w.physical()).open_position_count, 0);

    // A later observation, past the execution delay.
    w.observe(10, PRICE);
    assert_eq!(
        c.settle_market_open(&w.keeper, &action_id),
        ActionOutcome::Executed
    );

    let snapshot = c.accounting_snapshot(&w.physical());
    assert_eq!(snapshot.open_position_count, 1);
    let position = c.get_position(&1);
    assert_eq!(position.owner, w.trader);
    assert_eq!(position.size, 1_000_0000000);
    // The keeper reward is the only thing taken out of escrow at the default
    // parameters: `open_fee_bps` is zero (§10.4).
    let reward = global_config().keeper_rewards.open;
    assert_eq!(position.stored_collateral, collateral - reward);
    assert_eq!(w.balance(&w.keeper), reward);
}

/// §8.5/§8.6 — the two timing gates are independent, and neither consumes
/// the action or pays anyone.
///
/// The delay gate can be satisfied by the clock alone. The freshness gate
/// cannot: a transaction timestamp proves nothing about when the price was
/// observed, which is the whole reason the cursor exists.
#[test]
fn neither_timing_gate_consumes_the_action_or_pays_a_keeper() {
    let w = World::new();
    let c = w.client();
    let action_id = c.create_market_open(&w.trader, &w.market, &w.request(100_000_0000, 0, 120));

    // Too early: the execution delay has not elapsed.
    assert_eq!(
        c.settle_market_open(&w.keeper, &action_id),
        ActionOutcome::NotReady
    );

    // Late enough, but the feed has published nothing since the commitment,
    // so there is no observation carrying post-commitment information.
    w.wait(60);
    assert_eq!(
        c.settle_market_open(&w.keeper, &action_id),
        ActionOutcome::NotReady,
        "elapsed time is not a substitute for a new observation"
    );

    assert_eq!(w.balance(&w.keeper), 0, "a not-ready call pays nothing");
    assert_eq!(
        c.get_pending_action(&action_id).escrowed_collateral,
        100_000_0000,
        "and moves no escrow"
    );

    // One new observation is enough, even at an identical price.
    w.observe(1, PRICE);
    assert_eq!(
        c.settle_market_open(&w.keeper, &action_id),
        ActionOutcome::Executed
    );
}

/// §8.7/§8.9 — a slipped market-style attempt is a **successful
/// transaction** that records `Failed`.
///
/// This is the behaviour the whole phase turns on. The old implementation
/// panicked, which reverted the keeper payment and the refund together and
/// left the trader holding a free retry against the next observation.
#[test]
fn a_slipped_entry_terminates_pays_the_keeper_and_refunds_the_rest() {
    let w = World::new();
    let c = w.client();
    let collateral = 100_000_0000;
    let before = w.balance(&w.trader);
    // A long that refuses to pay more than the current price.
    let action_id =
        c.create_market_open(&w.trader, &w.market, &w.request(collateral, PRICE, 120));

    // The next observation is higher, so the bound fails.
    w.observe(10, PRICE + 1);
    assert_eq!(
        c.settle_market_open(&w.keeper, &action_id),
        ActionOutcome::Failed
    );

    let reward = global_config().keeper_rewards.open;
    assert_eq!(w.balance(&w.keeper), reward, "the keeper is paid for the attempt");
    assert_eq!(
        w.balance(&w.trader),
        before - reward,
        "and the trader gets everything else back — no opening fee (§9.12)"
    );
    assert_eq!(c.accounting_snapshot(&w.physical()).open_position_count, 0);
    // §8.13 — the ID is consumed. A second call cannot replay the payment.
    assert!(c.try_get_pending_action(&action_id).is_err());
    assert!(c.try_settle_market_open(&w.keeper, &action_id).is_err());
}

/// §7.6/§8.5 — at exactly `expires_at`, execution is closed and cleanup is
/// open. There is no timestamp at which both succeed.
#[test]
fn expiry_closes_execution_and_opens_cleanup_at_the_same_instant() {
    let w = World::new();
    let c = w.client();
    let collateral = 100_000_0000;
    let before = w.balance(&w.trader);
    let action_id = c.create_market_open(&w.trader, &w.market, &w.request(collateral, 0, 60));

    // Before expiry, cleanup is refused.
    w.observe(10, PRICE);
    assert!(c.try_clean_expired_entry(&w.keeper, &action_id).is_err());

    // At and after it, execution is refused and cleanup is the only path.
    w.observe(50, PRICE + 1);
    assert_eq!(
        c.settle_market_open(&w.keeper, &action_id),
        ActionOutcome::Expired
    );
    c.clean_expired_entry(&w.keeper, &action_id);

    let reward = global_config().keeper_rewards.expiry;
    assert_eq!(w.balance(&w.keeper), reward);
    assert_eq!(w.balance(&w.trader), before - reward);
    assert!(c.try_get_pending_action(&action_id).is_err());
}

/// §7.3/§7.4/§7.5 — an untriggered limit order is not an attempt. It keeps
/// resting, pays nothing, and the owner can still withdraw it.
#[test]
fn an_untriggered_limit_order_rests_and_can_still_be_cancelled() {
    let w = World::new();
    let c = w.client();
    let collateral = 100_000_0000;
    let before = w.balance(&w.trader);
    // Fill only if the price falls to $95,000.
    let trigger = 95_000_0000000;
    let action_id = c.create_limit_open(
        &w.trader,
        &w.market,
        &w.request(collateral, 0, 3_600),
        &trigger,
    );

    w.observe(10, PRICE);
    assert_eq!(
        c.settle_limit_open(&w.keeper, &action_id),
        ActionOutcome::Pending,
        "an uncrossed trigger is not an execution attempt"
    );
    assert_eq!(w.balance(&w.keeper), 0);
    assert_eq!(c.get_pending_action(&action_id).escrowed_collateral, collateral);

    let refund = c.cancel_limit_open(&action_id);
    assert_eq!(refund, collateral, "cancellation refunds the complete escrow");
    assert_eq!(w.balance(&w.trader), before, "and charges nothing at all");
}

/// §7.4 — once the trigger crosses, the limit entry settles exactly like a
/// market open, paying its own reward.
#[test]
fn a_crossed_limit_order_settles_and_pays_the_limit_reward() {
    let w = World::new();
    let c = w.client();
    let collateral = 100_000_0000;
    let trigger = 95_000_0000000;
    let action_id = c.create_limit_open(
        &w.trader,
        &w.market,
        &w.request(collateral, 0, 3_600),
        &trigger,
    );

    w.observe(10, 94_000_0000000);
    assert_eq!(
        c.settle_limit_open(&w.keeper, &action_id),
        ActionOutcome::Executed
    );
    assert_eq!(
        w.balance(&w.keeper),
        global_config().keeper_rewards.limit_order,
        "the limit reward, not the market-open reward (§8.11)"
    );
    let position = c.get_position(&1);
    // The fill is priced at the observation, not at the trigger.
    assert_eq!(
        position.base_exposure,
        1_000_0000000i128 * 10_000_000 / 94_000_0000000
    );
}

/// §8.5 — the lifetime gate applies to a close, is read at settlement, and
/// is non-terminal.
#[test]
fn a_close_before_the_minimum_lifetime_is_not_ready_rather_than_an_error() {
    let w = World::new();
    let c = w.client();
    let action_id = c.create_market_open(&w.trader, &w.market, &w.request(100_000_0000, 0, 120));
    w.observe(10, PRICE);
    c.settle_market_open(&w.keeper, &action_id);

    let close = c.create_close(&1, &0);
    w.observe(10, PRICE);
    assert_eq!(
        c.settle_close(&w.keeper, &close),
        ActionOutcome::NotReady,
        "the minimum position lifetime has not elapsed"
    );
    assert_eq!(w.balance(&w.keeper), global_config().keeper_rewards.open);

    w.observe(defaults::MIN_POSITION_LIFETIME, PRICE);
    assert_eq!(
        c.settle_close(&w.keeper, &close),
        ActionOutcome::Executed
    );
    assert!(c.try_get_position(&1).is_err());
}

/// §7.7 — adding collateral is immediate and charges nothing.
#[test]
fn adding_collateral_is_immediate_and_free() {
    let w = World::new();
    let c = w.client();
    let action_id = c.create_market_open(&w.trader, &w.market, &w.request(100_000_0000, 0, 120));
    w.observe(10, PRICE);
    c.settle_market_open(&w.keeper, &action_id);

    let before = c.get_position(&1).stored_collateral;
    w.observe(10, PRICE);
    c.add_collateral(&1, &25_000_0000);

    let position = c.get_position(&1);
    assert_eq!(
        position.stored_collateral,
        before + 25_000_0000,
        "the whole amount lands in collateral — no fee, no reward"
    );
    assert_eq!(position.pending_mutation_action_id, None);
}

/// §7.9 — a surviving decrease leaves realized profit **in** the position.
///
/// There is no collateral-withdrawal leg, and that is what makes the §7.9
/// guards local: a survivor may not realize profit the vault could not pay,
/// because unlike a terminal settlement it has no result in which to report
/// the shortfall.
#[test]
fn a_decrease_leaves_realized_profit_in_the_position() {
    let w = World::new();
    let c = w.client();
    let action_id = c.create_market_open(&w.trader, &w.market, &w.request(100_000_0000, 0, 120));
    w.observe(10, PRICE);
    c.settle_market_open(&w.keeper, &action_id);
    let after_open = c.get_position(&1).stored_collateral;
    let trader_before = w.balance(&w.trader);

    // 10% up on 10x leverage: $100 of profit on the full $1,000 of notional,
    // so half the size realizes about $50.
    w.observe(defaults::MIN_POSITION_LIFETIME, 110_000_0000000);
    let decrease = c.create_decrease(&1, &500_0000000, &0);
    w.observe(10, 110_000_0000000);
    assert_eq!(
        c.settle_decrease(&w.keeper, &decrease),
        ActionOutcome::Executed
    );

    let position = c.get_position(&1);
    assert_eq!(position.size, 500_0000000, "half the notional is gone");
    assert!(
        position.stored_collateral > after_open,
        "realized profit stays as collateral, it is not paid out"
    );
    assert_eq!(
        w.balance(&w.trader),
        trader_before,
        "the trader receives no cash from a decrease (§7.9)"
    );
    assert_eq!(position.pending_mutation_action_id, None);
}

/// §8.7 — the one price failure that is **not** terminal.
///
/// A take-profit is a standing instruction, not a one-attempt commitment:
/// the trader asked to exit at a price, not to exit once. A crossed trigger
/// outside its exit bound leaves the instruction attached and pays nothing,
/// and it may execute on a later qualifying observation.
#[test]
fn a_crossed_trigger_outside_its_bound_stays_attached() {
    let w = World::new();
    let c = w.client();
    let action_id = c.create_market_open(&w.trader, &w.market, &w.request(100_000_0000, 0, 120));
    w.observe(10, PRICE);
    c.settle_market_open(&w.keeper, &action_id);
    let keeper_before = w.balance(&w.keeper);

    // Exit at $110,000, but refuse to sell below $120,000 — a bound the
    // trigger price itself cannot satisfy.
    c.set_take_profit(&1, &110_000_0000000, &120_000_0000000);

    w.observe(defaults::MIN_POSITION_LIFETIME, 111_000_0000000);
    assert_eq!(
        c.execute_take_profit(&w.keeper, &1),
        ActionOutcome::Pending,
        "crossed, but outside the exit bound"
    );
    assert_eq!(w.balance(&w.keeper), keeper_before, "and pays nothing");
    assert!(c.get_position(&1).take_profit != shared::Trigger::None);

    // A later observation that satisfies both executes normally.
    w.observe(10, 125_000_0000000);
    assert_eq!(
        c.execute_take_profit(&w.keeper, &1),
        ActionOutcome::Executed
    );
    assert_eq!(
        w.balance(&w.keeper),
        keeper_before + global_config().keeper_rewards.tp,
        "and pays only the take-profit reward (§8.11)"
    );
    assert!(c.try_get_position(&1).is_err());
}

/// §7.14/§7.0 — ADL is permissionless, and the state gate is what bounds it.
///
/// A side in `Normal` cannot be deleveraged whoever asks, which is the
/// property an allowlist was previously being asked to provide.
#[test]
fn adl_is_refused_on_an_unrestricted_side_whoever_calls_it() {
    let w = World::new();
    let c = w.client();
    let action_id = c.create_market_open(&w.trader, &w.market, &w.request(100_000_0000, 0, 120));
    w.observe(10, PRICE);
    c.settle_market_open(&w.keeper, &action_id);

    // A modest winner on a side nowhere near `adl_pnl_factor_bps` of equity.
    w.observe(defaults::MIN_POSITION_LIFETIME, 110_000_0000000);
    let anyone = Address::generate(&w.env);
    assert!(
        c.try_execute_adl(&anyone, &1).is_err(),
        "the side is Normal, so no caller may deleverage it"
    );
    assert!(c.try_get_position(&1).is_ok());
}

/// §9.11/§12.2 — a pause stops the vault taking on risk and never stops
/// anyone shedding it, and it **drains** the risk-adding queue rather than
/// freezing it.
///
/// An entry committed before the pause takes the ordinary expected-failure
/// route when it becomes eligible: it terminates, pays its reward, and
/// refunds the escrow. Nothing is left waiting for an unpause that may never
/// come.
#[test]
fn a_pause_drains_the_entry_queue_and_leaves_exits_open() {
    let w = World::new();
    let c = w.client();
    let open = c.create_market_open(&w.trader, &w.market, &w.request(100_000_0000, 0, 120));
    w.observe(10, PRICE);
    c.settle_market_open(&w.keeper, &open);

    // A second entry, committed while the vault is still live.
    let trader_before = w.balance(&w.trader);
    let pending = c.create_market_open(&w.trader, &w.market, &w.request(100_000_0000, 0, 120));
    c.pause(&w.admin);

    // Creating new risk is refused outright.
    assert!(c
        .try_create_market_open(&w.trader, &w.market, &w.request(100_000_0000, 0, 120))
        .is_err());

    // The already-committed one drains.
    w.observe(10, PRICE);
    assert_eq!(
        c.settle_market_open(&w.keeper, &pending),
        ActionOutcome::Failed
    );
    let reward = global_config().keeper_rewards.open;
    assert_eq!(
        w.balance(&w.trader),
        trader_before - reward,
        "escrow refunded in full, no opening fee charged"
    );

    // And the existing position can still be closed.
    let close = c.create_close(&1, &0);
    w.observe(defaults::MIN_POSITION_LIFETIME, PRICE);
    assert_eq!(c.settle_close(&w.keeper, &close), ActionOutcome::Executed);
}

/// §8.8 — capacity is evaluated from the post-settlement state, and running
/// out of it is an expected terminal failure rather than an error.
///
/// Creation reserves nothing, so a commitment this large is accepted and
/// only discovers the shortage when it settles. That is the design: pending
/// orders that reserved capacity would let a trader deny it to everyone else
/// for free.
#[test]
fn an_entry_beyond_capacity_fails_terminally_rather_than_reverting() {
    let w = World::new();
    let c = w.client();
    // `risk_capacity_limit_bps` is 8,500 and the market risk factor is
    // 1,000 bps, so notional above 8.5x cash LP equity exhausts the
    // register. The collateral is sized to clear the initial margin, which
    // is the only bound creation applies.
    let equity = c.accounting_snapshot(&w.physical()).cash_lp_equity;
    let size = equity * 9;
    let collateral = size / 10;
    w.mint(&w.trader, collateral * 2);
    let trader_before = w.balance(&w.trader);
    let oversized = OpenPayload {
        size,
        submitted_collateral: collateral,
        ..w.request(collateral, 0, 120)
    };
    let action_id = c.create_market_open(&w.trader, &w.market, &oversized);

    w.observe(10, PRICE);
    assert_eq!(
        c.settle_market_open(&w.keeper, &action_id),
        ActionOutcome::Failed
    );
    assert_eq!(
        c.accounting_snapshot(&w.physical()).open_position_count,
        0,
        "no position is created"
    );
    assert_eq!(
        w.balance(&w.trader),
        trader_before - global_config().keeper_rewards.open,
        "the keeper is paid and everything else is refunded"
    );
}
