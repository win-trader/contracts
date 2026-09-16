//! End-to-end suite derived from the specification rather than the contracts.
//!
//! Everything here was written against two sources only: the contract
//! interfaces in `shared` — the `PositionManager`, `VaultInterface`, and
//! `RequestRouter` traits, the `#[contracttype]` records, and the error
//! enums — and `docs/design/trading-fees-and-settlement-specification.md`.
//! No settlement, entry, mutation, risk, or fee implementation was read.
//!
//! That constraint is the point of the file. A test written while reading the
//! code it covers tends to assert what the code does; these assert what the
//! document says, so a disagreement between the two surfaces as a failure
//! instead of as a matching pair of mistakes. Where the document gives a
//! worked example with numbers (§11), the numbers are transcribed rather than
//! recomputed.
//!
//! Layout: harness, then the §11 worked examples, then the §9 invariants,
//! then the adversarial boundaries of §8 and §12.2, then the §7.17 LP path.

mod spec_harness;

use spec_harness::*;

use shared::constants::{PRICE_PRECISION, SHARE_SCALE};
use shared::{defaults, ActionOutcome, SettlementStatus};
use soroban_sdk::{testutils::Address as _, Address};

// ---------------------------------------------------------------------------
// §11 — the document's worked examples, transcribed
// ---------------------------------------------------------------------------

/// §11.1 — every number in the example, asserted as the example states it.
#[test]
fn the_worked_market_open_of_11_1_produces_the_documents_numbers() {
    let p = Protocol::new();
    let c = p.pm();
    let trader_before = p.cash(&p.trader);

    p.publish(COMMIT_PRICE);
    let action = c.create_market_open(&p.trader, &p.market, &p.open_payload(300));

    // "submitted collateral transferred to vault = $5,100.00"
    assert_eq!(
        c.get_pending_action(&action).escrowed_collateral,
        usd(5_100),
        "§11.1 — the whole submission is escrowed at creation"
    );
    assert_eq!(p.cash(&p.trader), trader_before - usd(5_100));

    // "position exposure created = $0 / risk capacity reserved = $0"
    let market = c.get_market(&p.market);
    assert_eq!(market.long.size_open_interest, 0, "§9.7 — a pending action is not exposure");
    assert_eq!(market.long.base_exposure, 0);
    assert_eq!(market.long.risk_units, 0);
    assert_eq!(p.snapshot().total_risk_units, 0, "§8.8 — no capacity is reserved");

    p.observe(6, FILL);
    assert_eq!(c.settle_market_open(&p.keeper, &action), ActionOutcome::Executed);

    let pos = c.get_position(&1);
    // "base exposure = floor($100,000 / $50,000) = 2 BTC"  (§2.6)
    assert_eq!(pos.base_exposure, 2 * PRICE_PRECISION, "§11.1 — 2 BTC of base");
    // "risk units = floor($100,000 * 10%) = $10,000"  (§2.7)
    assert_eq!(pos.risk_units, usd(10_000), "§11.1 — $10,000 of risk units");
    // "position collateral = $5,100.00 - $0.00 - $0.25 = $5,099.75"
    assert_eq!(
        pos.stored_collateral,
        usd(5_100) - REWARD,
        "§11.1 — escrow less the zero opening fee and the open reward"
    );
    assert_eq!(pos.size, SIZE);
    assert!(pos.is_long);

    // "the keeper receives $0.25"
    assert_eq!(p.cash(&p.keeper), REWARD, "§8.11 — exactly one open reward");
    // "the action escrow becomes zero"
    assert!(
        c.try_get_pending_action(&action).is_err(),
        "§5.6 — a terminal outcome removes the record"
    );
    // "LP, protocol, and referral opening-fee revenue are all zero"
    assert_eq!(c.protocol_claimable_total(), 0, "§11.1 — open_fee_bps is 0");
    assert_eq!(c.referral_claimable_total(), 0);

    p.assert_conserved("after the §11.1 open");
}

/// §11.2 — the same order rejected by its own price bound. The transaction
/// completes, the keeper is paid, and the escrow is split with nothing left
/// over.
#[test]
fn the_slipped_entry_of_11_2_terminates_and_strands_no_cash() {
    let p = Protocol::new();
    let c = p.pm();
    let trader_before = p.cash(&p.trader);

    p.publish(COMMIT_PRICE);
    let action = c.create_market_open(&p.trader, &p.market, &p.open_payload(300));

    // "the first eligible fill is $50,200" — outside the $50,100 maximum.
    p.observe(6, usd(50_200));
    assert_eq!(
        c.settle_market_open(&p.keeper, &action),
        ActionOutcome::Failed,
        "§8.9 — an expected failure is a business outcome, not a revert"
    );

    // "keeper reward = $0.25 / owner refund = $5,100.00 - $0.25 = $5,099.75"
    assert_eq!(p.cash(&p.keeper), REWARD);
    assert_eq!(
        p.cash(&p.trader),
        trader_before - REWARD,
        "§11.2 — the trader is out exactly the keeper reward"
    );
    // "position created = no / exposure added = $0 / risk units added = $0"
    assert!(c.try_get_position(&1).is_err(), "§9.12 — no position, no opening fee");
    assert_eq!(c.get_market(&p.market).long.risk_units, 0);
    assert_eq!(c.protocol_claimable_total(), 0, "§9.12 — no fee on a failed entry");

    // "the keeper and refund transfers total the original $5,100, so no cash
    //  or claim is stranded" — §9.11's expected-entry-failure identity.
    assert!(c.try_get_pending_action(&action).is_err());
    p.assert_conserved("after the §11.2 failure");

    // "the failed action cannot retry at another price."
    p.observe(6, FILL);
    assert!(c.try_settle_market_open(&p.keeper, &action).is_err());
}

/// §9.13 and §11.5 — a close whose price PnL is negative charges no closing
/// fee, and the payout is the collateral less exactly the senior items the
/// interface will quote in advance.
#[test]
fn a_losing_close_charges_no_closing_fee() {
    let p = Protocol::new();
    let c = p.pm();
    let id = p.open_position();
    let pos = c.get_position(&id);
    let trader_before = p.cash(&p.trader);

    // §8.5 — the minimum position lifetime gates all four voluntary exits.
    p.wait(defaults::MIN_POSITION_LIFETIME);
    let close = c.create_close(&id, &0);

    // $49,000 on 2 BTC of base is a $2,000 price loss (§2.8), which leaves
    // effective collateral well above the maintenance margin, so this is a
    // close and not a liquidation.
    let exit = usd(49_000);
    p.observe(6, exit);

    // §4.12 — quote the senior charges through the read-only view before
    // settling, so the expected payout is built from the interface rather
    // than from a recomputation of the fee formulas.
    let fees = c.pending_fees(&id, &p.now());
    let raw_pnl = pos.base_exposure * exit / PRICE_PRECISION - pos.size;
    assert_eq!(raw_pnl, -usd(2_000), "§2.8 — floor(base * price) - size");

    assert_eq!(c.settle_close(&p.keeper, &close), ActionOutcome::Executed);

    // §3.8's waterfall with a zero closing fee: §9.13 bounds the fee by
    // payable price PnL, which is negative here.
    let expected = pos.stored_collateral + raw_pnl + fees.funding_received
        - fees.funding_paid_to_receivers
        - fees.funding_paid_to_lps
        - fees.borrow
        - REWARD;
    assert_eq!(
        p.cash(&p.trader) - trader_before,
        expected,
        "§11.5 — collateral less the loss, borrow, and the close reward; no closing fee"
    );
    assert_eq!(p.cash(&p.keeper), REWARD * 2, "§8.11 — one open reward, one close reward");
    p.assert_conserved("after a losing close");
}

// ---------------------------------------------------------------------------
// §9 — the invariants
// ---------------------------------------------------------------------------

/// §9.1 — the identity holds after every state transition in an ordinary
/// life, not merely at rest.
#[test]
fn cash_ownership_is_conserved_at_every_step_of_a_life() {
    let p = Protocol::new();
    let c = p.pm();
    p.assert_conserved("at deployment");

    p.publish(COMMIT_PRICE);
    let action = c.create_market_open(&p.trader, &p.market, &p.open_payload(300));
    p.assert_conserved("with an entry escrowed");

    p.observe(6, FILL);
    c.settle_market_open(&p.keeper, &action);
    p.assert_conserved("with the position open");

    c.add_collateral(&1, &usd(500));
    p.assert_conserved("after adding collateral");

    // §7.7 — adding collateral does not restart the lifetime clock, so the
    // wait below is measured from the open.
    p.wait(defaults::MIN_POSITION_LIFETIME);
    p.observe(1, usd(51_000));
    c.update_indices(&p.keeper, &p.market);
    p.assert_conserved("after a checkpoint at a moved price");

    let decrease = c.create_decrease(&1, &usd(40_000), &0);
    p.assert_conserved("with a decrease pending");

    p.observe(6, usd(51_000));
    c.settle_decrease(&p.keeper, &decrease);
    p.assert_conserved("after the decrease");

    p.wait(defaults::MIN_POSITION_LIFETIME);
    let close = c.create_close(&1, &0);
    p.observe(6, usd(51_000));
    c.settle_close(&p.keeper, &close);
    p.assert_conserved("after the close");

    assert!(c.try_get_position(&1).is_err());
}

/// §9.1 — `non_lp_claims` is the sum of the five labels the document names
/// and nothing else. A claim the totals forget is cash nobody can withdraw.
#[test]
fn non_lp_claims_equals_the_sum_of_its_five_labels() {
    let p = Protocol::new();
    let c = p.pm();

    let id = p.open_position();
    // A second trader with a pending entry, so escrow and position
    // collateral are both non-zero at the same instant.
    let other = Address::generate(&p.env);
    p.mint(&other, usd(10_000));
    p.publish(COMMIT_PRICE);
    let pending = c.create_market_open(&other, &p.market, &p.open_payload(300));

    assert_eq!(
        c.non_lp_claims(),
        p.claims_from_parts(&[id], &[pending]),
        "§9.1 — position collateral + escrow + receiver funding + protocol + referral"
    );
    assert_eq!(
        c.non_lp_claims(),
        c.get_position(&id).stored_collateral + usd(5_100),
        "with no fees collected yet, the two live labels are the whole of it"
    );
    p.assert_conserved("with a position and an escrow live together");
}

/// §9.11 — escrow is a trader-owned claim: it is not LP equity, not free
/// capital, and not position collateral. §9.7 — nor is it exposure.
#[test]
fn a_pending_action_reserves_no_exposure_and_no_lp_equity() {
    let p = Protocol::new();
    let c = p.pm();
    let before = p.snapshot();

    p.publish(COMMIT_PRICE);
    let action = c.create_market_open(&p.trader, &p.market, &p.open_payload(300));
    let after = p.snapshot();

    assert_eq!(
        after.physical_cash,
        before.physical_cash + usd(5_100),
        "the escrow is held inside the vault (§5.7)"
    );
    assert_eq!(
        after.cash_lp_equity, before.cash_lp_equity,
        "§9.11 — escrow is excluded from cash LP equity"
    );
    assert_eq!(
        after.non_lp_claims,
        before.non_lp_claims + usd(5_100),
        "§9.11 — and included in the non-LP claims"
    );
    assert_eq!(
        after.free_lp_capital, before.free_lp_capital,
        "§9.11 — excluded from free risk capacity"
    );
    assert_eq!(after.total_risk_units, before.total_risk_units, "§9.7");
    assert_eq!(after.open_position_count, before.open_position_count);

    // §9.11 — "cannot be used by a different action or position".
    assert!(
        c.try_settle_increase(&p.keeper, &action).is_err(),
        "§8.9 — a settlement call naming the wrong action kind reverts"
    );
    p.assert_conserved("with escrow pending");
}

/// §9.7 — aggregates equal the sum of live positions after every completed
/// mutation, and a terminal removal leaves no dust behind.
#[test]
fn exposure_aggregates_track_live_positions_and_keep_no_dust() {
    let p = Protocol::new();
    let c = p.pm();

    let first = p.open_position();
    let second_trader = Address::generate(&p.env);
    p.mint(&second_trader, usd(20_000));
    p.publish(COMMIT_PRICE);
    let mut order = p.open_payload(300);
    order.size = usd(40_000);
    order.submitted_collateral = usd(3_000);
    c.create_market_open(&second_trader, &p.market, &order);
    p.observe(6, FILL);
    c.settle_market_open(&p.keeper, &2);

    let a = c.get_position(&first);
    let b = c.get_position(&2);
    let market = c.get_market(&p.market);
    assert_eq!(market.long.size_open_interest, a.size + b.size, "§9.7 — size");
    assert_eq!(
        market.long.base_exposure,
        a.base_exposure + b.base_exposure,
        "§9.7 — base exposure"
    );
    assert_eq!(market.long.risk_units, a.risk_units + b.risk_units, "§9.7 — risk units");
    assert_eq!(
        market.long.stored_collateral_total,
        a.stored_collateral + b.stored_collateral
    );
    assert_eq!(
        p.snapshot().total_risk_units,
        a.risk_units + b.risk_units,
        "§9.7 — global.total_risk_units = sum(position.risk_units)"
    );
    assert_eq!(p.snapshot().open_position_count, 2);

    // "Full close removes every remaining unit of size, base exposure, and
    //  risk assigned to the position, so no terminal dust remains."
    p.wait(defaults::MIN_POSITION_LIFETIME);
    let close = c.create_close(&first, &0);
    p.observe(6, FILL);
    c.settle_close(&p.keeper, &close);

    let market = c.get_market(&p.market);
    assert_eq!(market.long.size_open_interest, b.size);
    assert_eq!(market.long.base_exposure, b.base_exposure);
    assert_eq!(market.long.risk_units, b.risk_units);
    assert_eq!(p.snapshot().open_position_count, 1);
    p.assert_conserved("after closing one of two");
}

/// §9.5 — no cumulative index may decrease, including across a checkpoint
/// taken while the skew EMA moves in the other direction.
#[test]
fn cumulative_indices_never_decrease() {
    let p = Protocol::new();
    let c = p.pm();
    p.open_position();

    let mut previous = c.get_market(&p.market);
    for step in 0..6 {
        // Alternate the price so the EMA has to move in both signed
        // directions while the payment indices stay monotonic.
        let price = if step % 2 == 0 { usd(52_000) } else { usd(48_500) };
        p.observe(600, price);
        c.update_indices(&p.keeper, &p.market);
        let now = c.get_market(&p.market);
        for (label, then, next) in [
            ("receiver_backed_long", previous.receiver_backed_index_long, now.receiver_backed_index_long),
            ("receiver_backed_short", previous.receiver_backed_index_short, now.receiver_backed_index_short),
            ("lp_backed_long", previous.lp_backed_index_long, now.lp_backed_index_long),
            ("lp_backed_short", previous.lp_backed_index_short, now.lp_backed_index_short),
            ("receiver_long", previous.receiver_index_long, now.receiver_index_long),
            ("receiver_short", previous.receiver_index_short, now.receiver_index_short),
        ] {
            assert!(next >= then, "§9.5 — {label} decreased at step {step}");
            assert!(next >= 0, "§9.5 — every cumulative index is unsigned");
        }
        assert!(
            now.last_funding_checkpoint >= previous.last_funding_checkpoint,
            "§9.5 — elapsed time is non-negative"
        );
        assert!(
            c.pending_receiver_funding_total() >= 0,
            "§9.4 — the guaranteed receiver liability never goes negative"
        );
        previous = now;
    }
    p.assert_conserved("after six checkpoints");
}

// ---------------------------------------------------------------------------
// §8 and §12.2 — the adversarial boundaries
// ---------------------------------------------------------------------------

/// §8.6 — `fill_observed_at > commit_observed_at`; equality fails. Waiting
/// out the delay proves nothing if no new observation arrived.
#[test]
fn an_observation_equal_to_the_commit_cursor_is_not_fresh() {
    let p = Protocol::new();
    let c = p.pm();

    p.publish(FILL);
    let action = c.create_market_open(&p.trader, &p.market, &p.open_payload(300));
    let cursor = c.get_pending_action(&action).commit_observed_at;

    // Past `execute_after`, with the feed still holding the very observation
    // the commitment was made against.
    p.wait(30);
    assert!(p.now() >= c.get_pending_action(&action).execute_after);
    assert_eq!(
        c.settle_market_open(&p.keeper, &action),
        ActionOutcome::NotReady,
        "§8.6 — the delay is necessary but not sufficient"
    );
    assert_eq!(p.cash(&p.keeper), 0, "§8.6 — no keeper reward is paid");
    assert_eq!(
        c.get_pending_action(&action).escrowed_collateral,
        usd(5_100),
        "§8.6 — no escrow moves"
    );
    assert_eq!(c.get_market(&p.market).long.risk_units, 0, "§8.6 — no exposure changes");

    // One strictly newer observation is all it takes.
    p.observe(1, FILL);
    assert!(c.get_pending_action(&action).commit_observed_at == cursor);
    assert_eq!(c.settle_market_open(&p.keeper, &action), ActionOutcome::Executed);
    p.assert_conserved("after the fresh fill");
}

/// §8.13 and §9.14 — a consumed action id is gone, and the keeper payment
/// that went with it cannot be replayed by a second caller.
#[test]
fn a_consumed_action_id_cannot_be_settled_or_cleaned_again() {
    let p = Protocol::new();
    let c = p.pm();
    let rival = Address::generate(&p.env);

    p.publish(COMMIT_PRICE);
    let action = c.create_market_open(&p.trader, &p.market, &p.open_payload(300));
    p.observe(6, FILL);
    assert_eq!(c.settle_market_open(&p.keeper, &action), ActionOutcome::Executed);
    let paid = p.cash(&p.keeper);

    // "A second call using a consumed ID fails before any transfer or
    //  reward" — and every other consumer of the same record fails too.
    p.observe(6, FILL);
    assert!(c.try_settle_market_open(&rival, &action).is_err());
    assert!(c.try_clean_expired_entry(&rival, &action).is_err());
    assert!(c.try_cancel_limit_open(&action).is_err());
    assert!(c.try_get_pending_action(&action).is_err());
    assert_eq!(p.cash(&rival), 0, "§9.14 — two callers racing cannot both be paid");
    assert_eq!(p.cash(&p.keeper), paid, "§8.13 — and the winner is not paid twice");

    // §8.13 — ids are monotonic and never reused, so the next action takes a
    // fresh one rather than the consumed one.
    p.publish(COMMIT_PRICE);
    let next = c.create_market_open(&p.trader, &p.market, &p.open_payload(300));
    assert!(next > action, "§8.13 — action ids increase monotonically");
    p.assert_conserved("after the replay attempts");
}

/// §8.10 — "refunds always go to the owner frozen in the action. The
/// settlement caller cannot redirect them."
#[test]
fn a_refund_goes_to_the_committed_owner_not_the_caller() {
    let p = Protocol::new();
    let c = p.pm();
    let stranger = Address::generate(&p.env);
    let trader_before = p.cash(&p.trader);

    p.publish(COMMIT_PRICE);
    let action = c.create_market_open(&p.trader, &p.market, &p.open_payload(60));

    // §8.5 — at or after `expires_at`, cleanup is the only valid path, and
    // it is permissionless.
    p.wait(60);
    c.clean_expired_entry(&stranger, &action);

    assert_eq!(
        p.cash(&stranger),
        REWARD,
        "§8.11 — the caller receives the expiry reward and nothing else"
    );
    assert_eq!(
        p.cash(&p.trader),
        trader_before - REWARD,
        "§8.10 — the remaining escrow returns to the committed owner"
    );
    assert!(c.try_get_position(&1).is_err(), "§9.12 — expiry creates no position");
    p.assert_conserved("after an expiry cleanup by a stranger");
}

/// §8.5 — "at exactly `expires_at`, execution is forbidden and expiry
/// cleanup is allowed. There is no timestamp at which both can succeed."
#[test]
fn expiry_closes_execution_at_the_same_instant_it_opens_cleanup() {
    // One tick early: cleanup is not yet available.
    let p = Protocol::new();
    let c = p.pm();
    p.publish(COMMIT_PRICE);
    let action = c.create_market_open(&p.trader, &p.market, &p.open_payload(60));
    p.wait(59);
    assert!(
        c.try_clean_expired_entry(&p.keeper, &action).is_err(),
        "§8.5 — `expired` is `now >= expires_at`, so 59 is too early"
    );
    p.wait(1);
    c.clean_expired_entry(&p.keeper, &action);
    assert_eq!(p.cash(&p.keeper), REWARD);

    // At exactly `expires_at`, with a perfectly good fresh observation,
    // execution must not produce a position.
    let q = Protocol::new();
    let d = q.pm();
    q.publish(COMMIT_PRICE);
    let entry = d.create_market_open(&q.trader, &q.market, &q.open_payload(60));
    q.observe(60, FILL);
    let outcome = d.try_settle_market_open(&q.keeper, &entry);
    assert!(
        outcome.is_err() || d.try_get_position(&1).is_err(),
        "§8.5 — execution is closed at exactly expires_at"
    );
    q.assert_conserved("at exactly expires_at");
}

/// §12.2 — "a pause stops the vault taking on risk. It never stops anyone
/// shedding it." A pending entry drains terminally rather than waiting for an
/// unpause that may never come.
#[test]
fn a_pause_drains_the_risk_adding_queue_and_leaves_exits_open() {
    let p = Protocol::new();
    let c = p.pm();
    let id = p.open_position();

    // A second entry, committed before the pause.
    p.publish(COMMIT_PRICE);
    let pending = c.create_market_open(&p.trader, &p.market, &p.open_payload(300));
    let trader_before = p.cash(&p.trader);
    let keeper_before = p.cash(&p.keeper);

    c.pause(&p.admin);

    // "Create any entry order or increase: rejected at creation."
    assert!(c
        .try_create_market_open(&p.trader, &p.market, &p.open_payload(300))
        .is_err());
    assert!(c.try_create_increase(&id, &usd(10_000), &usd(1_000), &0).is_err());

    // "Settle a pending entry or increase: terminates as an expected
    //  failure" — reward paid, escrow refunded, no opening fee, no position.
    p.observe(6, FILL);
    assert_eq!(c.settle_market_open(&p.keeper, &pending), ActionOutcome::Failed);
    assert_eq!(p.cash(&p.keeper), keeper_before + REWARD);
    assert_eq!(p.cash(&p.trader), trader_before + usd(5_100) - REWARD);
    assert!(c.try_get_position(&2).is_err());

    // "Add collateral: allowed. Decrease, close, take-profit, stop-loss:
    //  allowed. Claim protocol revenue: rejected."
    c.add_collateral(&id, &usd(100));
    assert!(c.try_claim_protocol(&p.admin, &p.admin, &1).is_err());
    p.wait(defaults::MIN_POSITION_LIFETIME);
    let close = c.create_close(&id, &0);
    p.observe(6, FILL);
    assert_eq!(
        c.settle_close(&p.keeper, &close),
        ActionOutcome::Executed,
        "§12.2 — a pause never blocks an exit"
    );
    p.assert_conserved("after the pause drained the queue");
}

/// §12.2 — "accrual never pauses". The clocks advance across a pause exactly
/// as they would otherwise, which is only fair because exits stay open.
#[test]
fn accrual_keeps_running_across_a_pause() {
    let p = Protocol::new();
    let c = p.pm();
    let id = p.open_position();

    c.pause(&p.admin);
    let before = c.pending_fees(&id, &p.now());
    p.observe(3_600, FILL);
    let after = c.pending_fees(&id, &p.now());

    assert!(
        after.borrow > before.borrow,
        "§12.2 — the global borrow index advances across a pause"
    );
    c.update_indices(&p.keeper, &p.market);
    assert_eq!(
        c.get_market(&p.market).last_funding_checkpoint,
        p.now(),
        "§12.2 — both checkpoint clocks advance"
    );
    p.assert_conserved("after accruing through a pause");
}

/// §8.12 — "individual position is liquidatable => liquidation before every
/// voluntary mutation". The displaced close is not consumed and pays nothing.
#[test]
fn liquidation_takes_precedence_over_an_eligible_close() {
    let p = Protocol::new();
    let c = p.pm();
    let id = p.open_position();

    p.wait(defaults::MIN_POSITION_LIFETIME);
    let close = c.create_close(&id, &0);
    let keeper_before = p.cash(&p.keeper);

    // 2 BTC of base against $5,099.75 of collateral: a $47,600 print is a
    // $4,800 loss, which is below the $2,500 maintenance margin (§9.10).
    p.observe(6, usd(47_600));
    assert_eq!(
        c.settle_close(&p.keeper, &close),
        ActionOutcome::RequiresLiquidation,
        "§8.9 — the safety outcome is explicitly non-terminal"
    );
    assert_eq!(p.cash(&p.keeper), keeper_before, "§8.11 — a displaced action pays nothing");
    assert_eq!(
        c.get_pending_action(&close).action_id,
        close,
        "§7.0 — RequiresLiquidation does not consume the action"
    );
    assert!(c.try_get_position(&id).is_ok());

    // §8.12's removal sequence: the position goes, its pending mutation goes
    // with it, and only the liquidation reward is paid.
    c.liquidate_position(&p.keeper, &id);
    assert!(c.try_get_position(&id).is_err());
    assert!(
        c.try_get_pending_action(&close).is_err(),
        "§8.12 — the superseded mutation is removed, not left dangling"
    );
    assert!(
        p.cash(&p.keeper) - keeper_before <= REWARD,
        "§9.10 — at most the configured fixed liquidation reward, never a percentage"
    );
    assert_eq!(c.get_market(&p.market).long.risk_units, 0, "§9.7 — no terminal dust");
    p.assert_conserved("after the liquidation");
}

/// §8.4 and §8.13 — "one position stores at most one ordinary pending
/// mutation reference", so conflicting mutations cannot be committed against
/// the same pre-action state.
#[test]
fn a_position_holds_one_pending_mutation_at_a_time() {
    let p = Protocol::new();
    let c = p.pm();
    let id = p.open_position();
    p.wait(defaults::MIN_POSITION_LIFETIME);

    let close = c.create_close(&id, &0);
    assert_eq!(c.get_position(&id).pending_mutation_action_id, Some(close));
    assert!(c.try_create_close(&id, &0).is_err(), "§8.4 — the slot is occupied");
    assert!(c.try_create_decrease(&id, &usd(10_000), &0).is_err());
    assert!(c.try_create_increase(&id, &usd(10_000), &usd(1_000), &0).is_err());

    p.observe(6, FILL);
    c.settle_close(&p.keeper, &close);
    assert!(c.try_get_position(&id).is_err());
    p.assert_conserved("after the slot cleared with the position");
}

/// §7.5 and §8.13 — an owner cancellation refunds the complete escrow, pays
/// no reward and charges no fee, and consumes the same record execution
/// would have used.
#[test]
fn an_owner_cancellation_refunds_everything_and_consumes_the_record() {
    let p = Protocol::new();
    let c = p.pm();
    let trader_before = p.cash(&p.trader);

    p.publish(COMMIT_PRICE);
    // A limit entry triggered well above the market, so it rests.
    let action = c.create_limit_open(&p.trader, &p.market, &p.open_payload(3_600), &usd(60_000));
    p.observe(6, FILL);
    assert_eq!(
        c.settle_limit_open(&p.keeper, &action),
        ActionOutcome::Pending,
        "§7.4 — an untriggered observation does not consume the order"
    );
    assert_eq!(p.cash(&p.keeper), 0, "§8.11 — no reward for an untriggered conditional");

    let refund = c.cancel_limit_open(&action);
    assert_eq!(refund, usd(5_100), "§9.12 — the complete escrow, no fee, no reward");
    assert_eq!(p.cash(&p.trader), trader_before);
    assert_eq!(p.cash(&p.keeper), 0);
    assert!(c.try_settle_limit_open(&p.keeper, &action).is_err(), "§8.13");
    p.assert_conserved("after the cancellation");
}

// ---------------------------------------------------------------------------
// §7.17 — the LP path
// ---------------------------------------------------------------------------

/// §7.17 — a deposit converts at the document's formula, having paid the
/// resolve reward out of escrow first, and the round trip returns the LP's
/// stake less the two rewards.
#[test]
fn an_lp_deposit_and_withdrawal_round_trip_at_the_documents_conversion() {
    let p = Protocol::new();
    let lp = Address::generate(&p.env);
    let executor = Address::generate(&p.env);
    let deposit = usd(10_000);
    p.mint(&lp, deposit);

    let id = p.router_client().request_deposit(&lp, &deposit);
    assert_eq!(p.cash(&lp), 0, "§7.17 — the escrow leaves the owner at creation");
    assert_eq!(
        p.router_client().resolve_next(&executor).status,
        SettlementStatus::NotReady,
        "§7.17 — only a matured head resolves, and a premature call is not a revert"
    );

    // The feed has to stay alive across the delay: §7.17 prices every active
    // market from it inside the settling transaction, and the read-only quote
    // below refuses an observation older than `max_price_age_seconds`.
    p.observe(defaults::LP_REQUEST_DELAY_LOCAL, FILL);
    // "Successful deposit settlement uses the pre-deposit state."
    let nav = p.vault_client().accounting_snapshot().vault_nav;
    let supply = p.vault_client().total_share_supply();
    let reward = p.vault_client().lp_resolve_reward();
    let result = p.router_client().resolve_next(&executor);

    assert_eq!(result.status, SettlementStatus::Settled);
    assert_eq!(result.reward, reward, "§7.17 — paid from escrow before conversion");
    assert_eq!(p.cash(&executor), reward);
    let expected_shares = (deposit - reward) * (supply + SHARE_SCALE) / (nav + 1);
    assert_eq!(
        p.shares(&lp),
        expected_shares,
        "§7.17 — mul_div_floor(deposit_assets, supply + SHARE_SCALE, nav + 1)"
    );
    assert_eq!(result.amount, expected_shares);
    assert_eq!(
        p.router_client().get_request(&id).status,
        shared::LpRequestStatus::Settled
    );
    p.assert_conserved("after the deposit settled");

    // And part of the way back out. Withdrawing the *whole* supply takes
    // §7.17's terminal-vault path instead — "the final LP may withdraw all
    // residual cash LP equity so conversion rounding cannot strand ownerless
    // assets" — which is a sweep, not the conversion this test is about.
    let held = p.shares(&lp);
    let leaving = held / 2;
    p.router_client().request_withdrawal(&lp, &leaving);
    p.observe(defaults::LP_REQUEST_DELAY_LOCAL, FILL);
    // §7.17 — escrowed shares stay in total supply until settlement, so this
    // read is the pre-withdrawal state the conversion is stated over.
    let nav = p.vault_client().accounting_snapshot().vault_nav;
    let supply = p.vault_client().total_share_supply();
    let out = p.router_client().resolve_next(&executor);

    assert_eq!(out.status, SettlementStatus::Settled);
    let assets = leaving * (nav + 1) / (supply + SHARE_SCALE);
    assert_eq!(out.amount, assets, "§7.17 — mul_div_floor(shares, nav + 1, supply + SHARE_SCALE)");
    assert_eq!(
        p.cash(&lp),
        assets - reward,
        "§7.17 — the reward comes out of the assets the withdrawal releases"
    );
    assert_eq!(p.cash(&executor), reward * 2);
    assert_eq!(
        p.shares(&lp),
        held - leaving,
        "§7.17 — the escrowed shares are burned in full, and only those"
    );
    p.assert_conserved("after the withdrawal settled");
}

/// §7.17 and §9.1 — LP request escrow is "held by the request contract, not
/// the vault", so it is not vault cash and mints no claim against it.
#[test]
fn an_lp_request_escrow_is_held_outside_the_vault() {
    let p = Protocol::new();
    let lp = Address::generate(&p.env);
    let deposit = usd(10_000);
    p.mint(&lp, deposit);

    let physical_before = p.physical();
    let claims_before = p.pm().non_lp_claims();
    p.router_client().request_deposit(&lp, &deposit);

    assert_eq!(p.physical(), physical_before, "§7.17 — vault cash is untouched");
    assert_eq!(p.cash(&p.router), deposit, "§7.17 — the router holds it");
    assert_eq!(
        p.pm().non_lp_claims(),
        claims_before,
        "§9.1 — a claim on cash the vault does not hold would be a fabricated label"
    );
    p.assert_conserved("with an LP request escrowed at the router");
}

/// §7.17 — "only the FIFO head can resolve", and resolution is
/// permissionless, so a queue can always be cleared by whoever is behind it.
#[test]
fn only_the_fifo_head_resolves() {
    let p = Protocol::new();
    let first = Address::generate(&p.env);
    let second = Address::generate(&p.env);
    p.mint(&first, usd(10_000));
    p.mint(&second, usd(10_000));

    let a = p.router_client().request_deposit(&first, &usd(10_000));
    let b = p.router_client().request_deposit(&second, &usd(10_000));
    assert!(b > a, "request ids are monotonic");
    assert_eq!(p.router_client().next_request_to_resolve(), a);

    p.wait(defaults::LP_REQUEST_DELAY_LOCAL);
    // The owner of the *second* request clears the queue themselves, which is
    // the path §7.17 says funds itself.
    let head = p.router_client().resolve_next(&second);
    assert_eq!(head.status, SettlementStatus::Settled);
    assert!(p.shares(&first) > 0, "the head settled for its own owner");
    assert_eq!(p.shares(&second), 0, "and not for the caller");
    assert_eq!(p.router_client().next_request_to_resolve(), b);

    let own = p.router_client().resolve_next(&second);
    assert_eq!(own.status, SettlementStatus::Settled);
    assert!(p.shares(&second) > 0);
    p.assert_conserved("after the queue drained");
}

/// §8.6 — `max_price_age_seconds` bounds how old an accepted observation may
/// be. The document says an observation "may be" that old before an action
/// refuses it, which makes the bound inclusive; §8.5 spells out its two
/// sibling comparisons explicitly and this one was left implicit. Pinned here
/// so a later change to it is a decision rather than a drift.
#[test]
fn an_observation_is_refused_only_once_it_is_older_than_the_limit() {
    let p = Protocol::new();
    let c = p.pm();
    let limit = defaults::MAX_PRICE_AGE_SECONDS;

    p.publish(FILL);
    p.wait(limit);
    assert!(
        c.try_accounting_snapshot(&p.physical()).is_ok(),
        "§8.6 — an observation exactly at the limit is still acceptable"
    );

    p.wait(1);
    assert!(
        c.try_accounting_snapshot(&p.physical()).is_err(),
        "§8.6 — one second past it, the feed's answer is too old to act on"
    );
}
