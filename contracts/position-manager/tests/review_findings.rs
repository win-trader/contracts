//! Regression tests for the settlement-spec review findings.
//!
//! Each test states the behaviour the protocol must have and was written to
//! fail against the implementation the review examined. The finding each one
//! pins is named in its doc comment: `C` critical, `H` high, `M` medium.
//!
//! The window-integrator half of C1 — the sub-second downward crossing and the
//! §11.12 conformance vector — needs the private integrator and lives in
//! `src/window.rs`'s unit tests.

mod spec_harness;

use spec_harness::*;

use position_manager::PositionManagerError;
use request_router::RequestRouterError;
use shared::constants::{DEFAULT_UPGRADE_TIMELOCK, ROLE_ORACLE, ROLE_PAUSER, ROLE_UPGRADER};
use shared::{defaults, ActionOutcome, LpRequestStatus, PayerSide, RiskState, SettlementStatus};
use soroban_sdk::{testutils::Address as _, Address, BytesN, Symbol};
use vault::VaultError;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn grant(p: &Protocol, role: &str) -> Address {
    let who = Address::generate(&p.env);
    config_manager::ConfigManagerClient::new(&p.env, &p.config_manager).grant_role(
        &p.admin,
        &Symbol::new(&p.env, role),
        &who,
    );
    who
}

fn token(p: &Protocol) -> mock_token::MockTokenClient<'_> {
    mock_token::MockTokenClient::new(&p.env, &p.token)
}

/// The mock of a Stellar account that has removed its USDC trustline: every
/// transfer to it fails. The holder can do this to themselves at any time.
fn stop_receiving(p: &Protocol, who: &Address) {
    token(p).set_receive_blocked(&p.admin, who, &true);
}

fn contract_error(code: u32) -> soroban_sdk::Error {
    soroban_sdk::Error::from_contract_error(code)
}

/// A new LP buys in through the queue and receives shares.
fn deposit(p: &Protocol, lp: &Address, amount: i128) {
    p.mint(lp, amount);
    p.router_client().request_deposit(lp, &amount);
    p.observe(defaults::LP_REQUEST_DELAY_LOCAL, FILL);
    let executor = Address::generate(&p.env);
    assert_eq!(
        p.router_client().resolve_next(&executor).status,
        SettlementStatus::Settled
    );
}

/// Two `$100,000` longs, 4 BTC of base, opened at `FILL`.
fn two_longs(p: &Protocol) -> (u64, u64) {
    let a = p.open_position();
    let b = p.open(&p.trader.clone(), true, SIZE, SUBMITTED);
    (a, b)
}

// ---------------------------------------------------------------------------
// C1 — a funding sign change must never freeze a market
// ---------------------------------------------------------------------------

/// C1 — a short-dominated history under a book that is now long-dominated
/// makes the blended skew cross zero **upward**. §6.2.1's split divides by
/// `B`, which is negative in that direction, and a failed checkpoint never
/// advances: every later accrual on the market failed with it, so no close,
/// liquidation, ADL, or LP resolution could run again.
#[test]
fn c1_an_upward_funding_sign_change_does_not_freeze_the_market() {
    let p = Protocol::new();
    let c = p.pm();

    let shorter = Address::generate(&p.env);
    p.mint(&shorter, usd(100_000));
    p.open(&shorter, false, usd(100_000), usd(10_000));
    let long = p.open_position();
    p.open(&p.trader.clone(), true, usd(200_000), usd(20_000));
    assert_eq!(
        c.get_market(&p.market).current_payer_side,
        PayerSide::Short,
        "the EMA still remembers the short book, so shorts pay first"
    );

    // Live skew is +0.5 against an EMA of about -1, so the blended skew
    // crosses zero roughly 12.8 hours in. Checkpoint hourly across it.
    for hour in 1..=24 {
        p.observe(3_600, FILL);
        assert!(
            c.try_update_indices(&p.keeper, &p.market).is_ok(),
            "hour {hour}: a funding checkpoint must never revert on a sign change"
        );
    }
    assert_eq!(c.get_market(&p.market).current_payer_side, PayerSide::Long);

    // And the market still settles.
    let close = c.create_close(&long, &0);
    p.observe(6, FILL);
    assert_eq!(c.settle_close(&p.keeper, &close), ActionOutcome::Executed);
    p.assert_conserved("after trading through an upward funding sign change");
}

// ---------------------------------------------------------------------------
// H2 — no participant can veto a settlement by refusing to receive
// ---------------------------------------------------------------------------

/// H2 — only the FIFO head resolves, so a withdrawal whose payout reverts
/// blocked every LP behind it permanently.
#[test]
fn h2_an_lp_who_cannot_receive_does_not_freeze_the_withdrawal_queue() {
    let p = Protocol::new();
    let router = p.router_client();
    let (stuck, patient) = (Address::generate(&p.env), Address::generate(&p.env));
    deposit(&p, &stuck, usd(10_000));
    deposit(&p, &patient, usd(10_000));

    stop_receiving(&p, &stuck);
    router.request_withdrawal(&stuck, &(p.shares(&stuck) / 2));
    let second = router.request_withdrawal(&patient, &(p.shares(&patient) / 2));
    p.observe(defaults::LP_REQUEST_DELAY_LOCAL, FILL);

    let executor = Address::generate(&p.env);
    assert!(
        router.try_resolve_next(&executor).is_ok(),
        "an LP who cannot receive must not be able to block the queue"
    );
    assert_eq!(router.resolve_next(&executor).status, SettlementStatus::Settled);
    assert_eq!(router.get_request(&second).status, LpRequestStatus::Settled);
    assert!(p.cash(&patient) > 0, "the LP behind them was paid");

    // The undeliverable payout is held, not lost, and is claimable once the
    // owner can receive again.
    let held = router.lp_payout_claimable(&stuck);
    assert!(held > 0);
    assert_eq!(p.cash(&stuck), 0);
    assert!(router.try_claim_lp_payout(&stuck).is_err(), "still cannot receive");
    token(&p).set_receive_blocked(&p.admin, &stuck, &false);
    assert_eq!(router.claim_lp_payout(&stuck), held);
    assert_eq!(p.cash(&stuck), held);
    assert_eq!(router.lp_payout_claimable(&stuck), 0);
    p.assert_conserved("after the held LP payout was claimed");
}

/// H2 — liquidation paid the residual collateral straight to the owner, so
/// an owner who stopped receiving made their own position unliquidatable for
/// as long as any collateral remained.
#[test]
fn h2_liquidation_cannot_be_blocked_by_an_owner_who_stops_receiving() {
    let p = Protocol::new();
    let c = p.pm();
    let id = p.open_position();
    stop_receiving(&p, &p.trader);

    // 2 BTC marked down $2,000 each: a $4,000 loss leaves about $1,100 of
    // effective collateral against a $2,500 threshold. Liquidatable, with
    // residual collateral still owed to the owner.
    p.observe(1, usd(48_000));
    assert!(
        c.try_liquidate_position(&p.keeper, &id).is_ok(),
        "the risk-removal path must not depend on the owner's willingness to receive"
    );
    assert!(c.try_get_position(&id).is_err());

    // §12.1 — the residual is an explicit claim until the owner collects it.
    let held = c.unclaimed_payout(&p.trader);
    assert!(held > 0, "the residual collateral is held for the owner");
    assert_eq!(c.unclaimed_payout_total(), held);
    p.assert_conserved("with the residual held as a claim");
    assert_eq!(
        p.claims_from_parts(&[], &[]),
        p.snapshot().non_lp_claims,
        "§2.5 — the held payout is one of the six labels"
    );

    let before = p.cash(&p.trader);
    token(&p).set_receive_blocked(&p.admin, &p.trader, &false);
    assert_eq!(c.claim_payout(&p.trader), held);
    assert_eq!(p.cash(&p.trader), before + held);
    assert_eq!(c.unclaimed_payout_total(), 0);
    assert!(c.try_claim_payout(&p.trader).is_err(), "nothing left to claim");
    p.assert_conserved("after the owner claimed the held residual");
}

/// H2 — a failed entry refunds its escrow. A refund that reverts undid the
/// whole failure, left the order pending, and handed the trader a free retry
/// at the next price.
#[test]
fn h2_a_failed_entry_is_terminal_even_when_its_refund_cannot_be_delivered() {
    let p = Protocol::new();
    let c = p.pm();
    p.publish(COMMIT_PRICE);
    let action = c.create_market_open(&p.trader, &p.market, &p.open_payload(300));
    stop_receiving(&p, &p.trader);

    p.observe(6, usd(50_200)); // above the $50,100 acceptable maximum
    assert_eq!(
        c.try_settle_market_open(&p.keeper, &action),
        Ok(Ok(ActionOutcome::Failed)),
        "the first eligible attempt is terminal whatever the owner does"
    );
    assert!(c.try_get_pending_action(&action).is_err());
    assert_eq!(
        c.unclaimed_payout(&p.trader),
        SUBMITTED - REWARD,
        "the refund is held for the owner rather than reverting the failure"
    );
    p.assert_conserved("after failing an entry whose owner cannot receive");
}

// ---------------------------------------------------------------------------
// H3 — a fill must be observed after the commitment
// ---------------------------------------------------------------------------

/// H3 — a feed stamps a price with its sampling time and publishes it later.
/// Comparing the fill only against the last stamp seen at commit accepted an
/// observation sampled *before* the commitment, at a price the trader already
/// knew.
#[test]
fn h3_an_observation_sampled_before_the_commitment_cannot_fill_it() {
    let p = Protocol::new();
    let c = p.pm();
    let feed = mock_oracle::MockOracleClient::new(&p.env, &p.feed);
    let committed_at = p.now();

    feed.set_price_at(&p.market, &FILL, &(committed_at - 30));
    let mut order = p.open_payload(300);
    order.acceptable_price = 0;
    let action = c.create_market_open(&p.trader, &p.market, &order);

    // Published after the commitment, sampled ten seconds before it.
    p.wait(6);
    feed.set_price_at(&p.market, &FILL, &(committed_at - 10));
    assert_eq!(
        c.settle_market_open(&p.keeper, &action),
        ActionOutcome::NotReady,
        "an observation from before the commitment carries no new information"
    );

    // A genuinely later observation settles it.
    p.observe(1, FILL);
    assert_eq!(c.settle_market_open(&p.keeper, &action), ActionOutcome::Executed);
}

/// H3 — a future-dated stamp is never inside the age bound's meaning, and as a
/// commitment cursor it would hold every fill `NotReady` until the clock
/// caught up.
#[test]
fn h3_a_future_dated_observation_is_refused() {
    let p = Protocol::new();
    let c = p.pm();
    mock_oracle::MockOracleClient::new(&p.env, &p.feed).set_price_at(
        &p.market,
        &FILL,
        &(p.now() + 3_600),
    );
    assert_eq!(
        c.try_create_market_open(&p.trader, &p.market, &p.open_payload(300)),
        Err(Ok(contract_error(PositionManagerError::PriceUnavailable as u32)))
    );
}

// ---------------------------------------------------------------------------
// M4 — a half-life change must not reprice elapsed time
// ---------------------------------------------------------------------------

/// M4 — `apply_global_config` checkpointed borrow but not funding, so a new
/// `funding_half_life_seconds` priced every market's open window since its
/// last checkpoint — §9.6's retroactive repricing.
#[test]
fn m4_a_funding_half_life_change_does_not_reprice_the_elapsed_window() {
    let indices_after_timelock = |apply_change: bool| -> (i128, i128) {
        let p = Protocol::new();
        let c = p.pm();
        // Long-first then a smaller short: live skew +0.5 under an EMA of
        // +1, so the EMA — and therefore the half-life — drives funding.
        p.open(&p.trader.clone(), true, usd(300_000), usd(20_000));
        let shorter = Address::generate(&p.env);
        p.mint(&shorter, usd(100_000));
        p.open(&shorter, false, usd(100_000), usd(10_000));

        if apply_change {
            let mut faster = defaults::global_config();
            faster.funding_half_life_seconds = 3_600;
            c.propose_global_config(&p.admin, &faster);
        }
        p.observe(defaults::CONFIG_TIMELOCK_SECONDS, FILL);
        if apply_change {
            c.apply_global_config(&p.keeper);
        }
        c.update_indices(&p.keeper, &p.market);
        let m = c.get_market(&p.market);
        (m.lp_backed_index_long, m.receiver_backed_index_long)
    };

    assert_eq!(
        indices_after_timelock(true),
        indices_after_timelock(false),
        "the 48 hours before the change are priced under the half-life that governed them"
    );
}

// ---------------------------------------------------------------------------
// M5 — a decrease may not leave dust
// ---------------------------------------------------------------------------

/// M5 — §5.5: a surviving position always has positive size **and** base
/// exposure. Decreasing by `size - 1` left one unit of size with zero base and
/// zero risk units: no borrow, never liquidatable or deleverageable, and a
/// permanent obstacle to deregistering the market.
#[test]
fn m5_a_decrease_cannot_leave_a_zero_base_zero_risk_survivor() {
    let p = Protocol::new();
    let c = p.pm();
    let id = p.open_position();
    p.observe(defaults::MIN_POSITION_LIFETIME, FILL);
    let size = c.get_position(&id).size;

    assert_eq!(
        c.try_create_decrease(&id, &(size - 1), &0),
        Err(Ok(contract_error(PositionManagerError::InvalidAmount as u32))),
        "the surviving remainder would round to zero base and zero risk"
    );
    // A decrease that leaves a real position is still accepted.
    assert!(c.try_create_decrease(&id, &(size / 2), &0).is_ok());
}

// ---------------------------------------------------------------------------
// M6 — a withdrawal must not itself restrict a side
// ---------------------------------------------------------------------------

/// M6 — the withdrawal gate checked side states *before* the payout. LP
/// equity is every side's denominator, so a large withdrawal could push a
/// side into `ADL` or `HardCap` on its way out and haircut the traders on it.
#[test]
fn m6_a_withdrawal_that_would_push_a_side_into_adl_fails() {
    let p = Protocol::new();
    let lp = Address::generate(&p.env);
    deposit(&p, &lp, usd(500_000));
    two_longs(&p);

    // $56,000 of long profit against about $1.5M of equity: 373 bps, Normal.
    let marked = usd(64_000);
    p.publish(marked);
    assert_eq!(p.snapshot().deleveraging_side_count, 0);

    // Paying out ~$433,000 leaves ~$1.07M of equity: 525 bps, ADL.
    p.router_client()
        .request_withdrawal(&lp, &(p.shares(&lp) * 9 / 10));
    p.observe(defaults::LP_REQUEST_DELAY_LOCAL, marked);
    let executor = Address::generate(&p.env);
    assert_eq!(
        p.router_client().resolve_next(&executor).status,
        SettlementStatus::Failed,
        "the withdrawal would put the long side into ADL"
    );
}

// ---------------------------------------------------------------------------
// M7 — a terminal exit must not revert over its own keeper reward
// ---------------------------------------------------------------------------

/// M7 — health counts payable profit, but settlement credits it only up to
/// cash LP equity. A healthy winner whose stored collateral had been worn
/// away could not pay its close reward, so every exit reverted — while it was
/// too healthy to liquidate.
///
/// The gap opens once a side has latched `HardCap` and LP equity then falls:
/// the stored payout factor keeps payable profit at its latch-time size (§6.5),
/// while the payment-time cash limit shrinks with the equity. The test drains
/// equity by burning vault cash, standing in for losses elsewhere in the vault.
#[test]
fn m7_a_healthy_winner_can_close_when_the_vault_cannot_pay_its_profit() {
    let p = Protocol::new();
    let c = p.pm();
    let (id, _) = two_longs(&p);

    // A week of funding ($800 a day at 80 bps on $100,000) and borrow wears
    // the $5,099.75 of collateral away...
    p.accrue(7 * 86_400, 86_400, FILL);
    // ...while the price runs and the side latches with a stored factor.
    let marked = usd(85_000);
    p.latch_risk_state(marked);
    let side = c.get_market(&p.market).long;
    assert_eq!(side.risk_state, RiskState::HardCap);

    // LP equity then disappears: nothing is left to pay the profit from.
    let equity = p.snapshot().cash_lp_equity;
    token(&p).burn(&p.vault, &equity);
    assert_eq!(p.snapshot().cash_lp_equity, 0);

    let close = c.create_close(&id, &0);
    p.observe(6, marked);
    assert_eq!(
        c.try_settle_close(&p.keeper, &close),
        Ok(Ok(ActionOutcome::Executed)),
        "a terminal exit caps its reward at what the position holds"
    );
    assert!(c.try_get_position(&id).is_err());
}

// ---------------------------------------------------------------------------
// M8 — governance must not bypass its own timelock
// ---------------------------------------------------------------------------

/// M8 — raising maintenance margin makes open positions liquidatable, which
/// is exactly what the timelock exists to make observable in advance.
#[test]
fn m8_raising_maintenance_margin_waits_out_the_timelock() {
    let p = Protocol::new();
    let c = p.pm();
    let mut stricter = defaults::market_config();
    stricter.maintenance_margin_bps = 400;
    c.propose_market_config(&p.admin, &p.market, &stricter);
    assert_eq!(
        c.get_market(&p.market).config.maintenance_margin_bps,
        defaults::MAINTENANCE_MARGIN_BPS,
        "the raise is proposed, not applied"
    );
}

/// M8 — the price feed decides every fill and every liquidation. Replacing
/// it instantly let a single key reprice the whole book; §12.3 exempts only
/// pausing and conservative bounds from the timelock.
#[test]
fn m8_a_price_feed_change_waits_out_the_timelock() {
    let p = Protocol::new();
    let c = p.pm();
    let oracle = grant(&p, ROLE_ORACLE);
    let replacement = p.env.register(mock_oracle::MockOracle, ());

    c.propose_price_feed(&oracle, &replacement);
    assert_eq!(c.price_feed(), p.feed, "the change is proposed, not applied");
    assert!(
        c.try_apply_price_feed(&p.keeper).is_err(),
        "not before the timelock"
    );

    p.wait(defaults::CONFIG_TIMELOCK_SECONDS);
    c.apply_price_feed(&p.keeper);
    assert_eq!(c.price_feed(), replacement, "and applied once it has elapsed");
}

/// M8 — a proposal can be withdrawn before it lands.
#[test]
fn m8_a_pending_proposal_can_be_cancelled() {
    let p = Protocol::new();
    let c = p.pm();
    let oracle = grant(&p, ROLE_ORACLE);
    let replacement = p.env.register(mock_oracle::MockOracle, ());

    let mut changed = defaults::global_config();
    changed.min_position_lifetime = 120;
    c.propose_global_config(&p.admin, &changed);
    let mut stricter = defaults::market_config();
    stricter.maintenance_margin_bps = 400;
    c.propose_market_config(&p.admin, &p.market, &stricter);
    c.propose_price_feed(&oracle, &replacement);

    c.cancel_global_config(&p.admin);
    c.cancel_market_config(&p.admin, &p.market);
    c.cancel_price_feed(&oracle);

    p.observe(defaults::CONFIG_TIMELOCK_SECONDS, FILL);
    assert!(c.try_apply_global_config(&p.keeper).is_err());
    assert!(c.try_apply_market_config(&p.keeper, &p.market).is_err());
    assert!(c.try_apply_price_feed(&p.keeper).is_err());
    assert!(c.try_cancel_global_config(&p.admin).is_err(), "nothing left to cancel");
}

/// M8 — an upgrade can change anything a configuration change can, so its
/// delay may not be shorter than the configuration timelock. The upgrade
/// timelock defaulted to 24h against a 48h configuration timelock.
#[test]
fn m8_an_upgrade_waits_at_least_the_configuration_timelock() {
    let p = Protocol::new();
    let auth = grant(&p, ROLE_UPGRADER);
    let wasm = BytesN::from_array(&p.env, &[9; 32]);
    p.pm().propose_upgrade(&auth, &wasm);
    p.vault_client().propose_upgrade(&auth, &wasm);
    p.router_client().propose_upgrade(&auth, &wasm);

    // The defaults put the upgrade timelock (24h) under the configuration
    // timelock (48h), which is the gap this test covers.
    const _: () = assert!(defaults::CONFIG_TIMELOCK_SECONDS > DEFAULT_UPGRADE_TIMELOCK);
    p.wait(DEFAULT_UPGRADE_TIMELOCK + 1);

    assert_eq!(
        position_manager::PositionManagerContractClient::new(&p.env, &p.pm)
            .try_upgrade(&wasm, &auth),
        Err(Ok(contract_error(
            PositionManagerError::UpgradeTimelockNotElapsed as u32
        )))
    );
    assert_eq!(
        vault::VaultContractClient::new(&p.env, &p.vault).try_upgrade(&wasm, &auth),
        Err(Ok(contract_error(VaultError::UpgradeTimelockNotElapsed as u32)))
    );
    assert_eq!(
        request_router::RequestRouterContractClient::new(&p.env, &p.router)
            .try_upgrade(&wasm, &auth),
        Err(Ok(contract_error(
            RequestRouterError::UpgradeTimelockNotElapsed as u32
        )))
    );
}

/// M8 — applying is permissionless, so a proposal nobody applied stayed a
/// standing capability for anyone to land at any later moment.
#[test]
fn m8_a_configuration_proposal_expires_if_it_is_not_applied() {
    let p = Protocol::new();
    let c = p.pm();
    let mut changed = defaults::global_config();
    changed.min_position_lifetime = 120;
    c.propose_global_config(&p.admin, &changed);

    p.observe(defaults::CONFIG_TIMELOCK_SECONDS * 30, FILL);
    assert_eq!(
        c.try_apply_global_config(&p.keeper),
        Err(Ok(contract_error(PositionManagerError::ConfigProposalExpired as u32))),
        "a months-old proposal must not be applicable by anyone"
    );
    assert_eq!(
        c.global_config().min_position_lifetime,
        defaults::MIN_POSITION_LIFETIME
    );
}

// ---------------------------------------------------------------------------
// M9 — pause semantics (§12.2, §12.3)
// ---------------------------------------------------------------------------

/// M9 — §12.3: `pause_authority` may set `paused` and may not clear it. The
/// vault let the pause key unpause.
#[test]
fn m9_the_vault_pause_key_cannot_unpause() {
    let p = Protocol::new();
    let pauser = grant(&p, ROLE_PAUSER);
    p.vault_client().pause(&pauser);
    assert!(p.vault_client().try_unpause(&pauser).is_err());
}

/// M9 — §12.3: enabling a market re-admits risk, which is the unpause
/// authority's power, not the pause key's.
#[test]
fn m9_the_pause_key_cannot_re_enable_a_market() {
    let p = Protocol::new();
    let c = p.pm();
    let pauser = grant(&p, ROLE_PAUSER);
    c.disable_market(&pauser, &p.market);
    assert!(c.try_enable_market(&pauser, &p.market).is_err());
}

/// M9 — §12.2: while paused, LP resolution is rejected and "pending requests
/// wait". A paused vault instead failed them, charging each deposit the
/// resolve reward for an outage its owner did not cause.
#[test]
fn m9_a_paused_vault_leaves_lp_requests_waiting() {
    let p = Protocol::new();
    let lp = Address::generate(&p.env);
    p.mint(&lp, usd(1_000));
    let id = p.router_client().request_deposit(&lp, &usd(1_000));

    p.vault_client().pause(&p.admin);
    p.observe(defaults::LP_REQUEST_DELAY_LOCAL, FILL);
    let executor = Address::generate(&p.env);
    assert_eq!(
        p.router_client().resolve_next(&executor).status,
        SettlementStatus::NotReady
    );
    assert_eq!(p.router_client().get_request(&id).status, LpRequestStatus::Pending);
    assert_eq!(p.cash(&executor), 0, "nothing is charged while paused");
}

/// M9 — the position manager's pause is the protocol's pause (§12.2), and it
/// did not reach LP resolution at all.
#[test]
fn m9_the_protocol_pause_also_holds_lp_resolution() {
    let p = Protocol::new();
    let lp = Address::generate(&p.env);
    p.mint(&lp, usd(1_000));
    let id = p.router_client().request_deposit(&lp, &usd(1_000));

    p.pm().pause(&p.admin);
    p.observe(defaults::LP_REQUEST_DELAY_LOCAL, FILL);
    let executor = Address::generate(&p.env);
    assert_eq!(
        p.router_client().resolve_next(&executor).status,
        SettlementStatus::NotReady
    );
    assert_eq!(p.router_client().get_request(&id).status, LpRequestStatus::Pending);
}

/// M9 — §7.17: a deposit adds LP equity and lowers every side's factor, so it
/// is not gated on side risk state. Creation refused it while a side was
/// latched — turning away rescue capital in exactly the state that needs it.
#[test]
fn m9_a_deposit_request_is_accepted_while_a_side_is_latched() {
    let p = Protocol::new();
    two_longs(&p);
    p.latch_risk_state(usd(85_000));
    assert_eq!(p.pm().get_market(&p.market).long.risk_state, RiskState::HardCap);

    let lp = Address::generate(&p.env);
    p.mint(&lp, usd(1_000));
    assert!(p.router_client().try_request_deposit(&lp, &usd(1_000)).is_ok());
}
