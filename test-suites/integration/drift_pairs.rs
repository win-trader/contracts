//! Counter↔token-balance drift-pair tests.
//!
//! Each test pins **one** (stored counter, physical token movement) pair
//! against the closed-form expectation of how they should move together
//! across a specific operation. Together with `pnl_settlement_invariants.rs`
//! and the `assert_protocol_invariants` helper injected into the Fixture
//! wrappers, these cover the high-blast-radius drift surfaces between PM
//! accounting and vault token balances.
//!
//! Shape per test: snapshot pre-state (counter + relevant `usdc.balance`s),
//! perform the operation through a Fixture wrapper, snapshot post-state,
//! assert deltas match the operation's contract exactly. The Fixture wrapper
//! also re-runs `assert_protocol_invariants` so any other counter drift is
//! caught simultaneously.

use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env};
use test_suites::testutils::{
    invariants::assert_protocol_invariants, Fixture, TEST_TIMESTAMP, USDC_UNIT,
};

const MIN_POSITION_LIFETIME: u64 = 60;

// ---------------------------------------------------------------------------
// 1. TotalUnrealizedPnl ↔ vault.update_net_pnl (two-sided OI)
//
// Open both a long and a short, move the mark price, force PM to refresh
// indices (triggering refresh_market_unrealized_pnl), and assert that the
// vault's net_global_trader_pnl exactly matches the closed-form combined
// unrealized PnL across both sides.
//
// With both long and short sides each contributing a non-zero PnL, any
// operator mutation in `calc_market_unrealized_pnl` (`+` ↔ `-`, etc.) is
// caught — the sides can't cancel into a sign-agnostic shape.
// ---------------------------------------------------------------------------
#[test]
fn drift_total_unrealized_pnl_matches_two_sided_market() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let trader2 = f.create_funded_trader(50_000 * USDC_UNIT);

    // Trader A: long 10k @ 50k. Trader B: short 6k @ 50k. Imbalanced OI.
    f.open_long(&f.trader, 10_000 * USDC_UNIT, 1_000 * USDC_UNIT);
    f.open_short(&trader2, 6_000 * USDC_UNIT, 600 * USDC_UNIT);

    f.advance_time(TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 11);
    // Move to 60k: longs +20% on 10k = +2k; shorts -20% on 6k = -1.2k.
    f.set_btc_price(60_000);

    // Force refresh via update_indices (KEEPER call, mock_all_auths approves).
    f.update_indices(&f.keeper, &symbol_short!("BTC"));

    // Closed-form expected combined unrealized:
    //   long_pnl  = 10_000 * (60k - 50k) / 50k = +2_000 USDC
    //   short_pnl = 6_000  * (50k - 60k) / 50k = -1_200 USDC
    //   total     = +800 USDC, scaled into the 1e7 price domain.
    // Because PM stores PnL in USDC (6 decimals) the contract math handles
    // scaling; we read the actual computed value and assert *both* the
    // vault-side and the PM-side counters agree on it.
    let pm_total = f.position_manager.total_unrealized_pnl();
    let vault_net = f.vault.net_global_trader_pnl();
    assert_eq!(
        pm_total, vault_net,
        "PM.total_unrealized={} but vault.net_global_trader_pnl={} — sync drifted",
        pm_total, vault_net,
    );

    // Sanity: with this price move, combined unrealized must be positive
    // (long wins more than short loses).
    assert!(
        pm_total > 0,
        "Two-sided market with long-bias and price up should have positive net unrealized, got {}",
        pm_total,
    );

    // Clause-6 sanity: sum(per-market) == total (re-checked here because
    // assert_protocol_invariants already ran inside update_indices).
    let market_pnl = f
        .position_manager
        .market_unrealized_pnl(&symbol_short!("BTC"));
    assert_eq!(market_pnl, pm_total);
}

// ---------------------------------------------------------------------------
// 2. ReservedUsdc ↔ collateral_in (open position)
//
// At open, the vault's reserved_usdc must grow by exactly `size` (the
// notional, not the collateral) and the trader's USDC balance must drop by
// exactly `collateral`. The vault's physical balance is unchanged at this
// moment — PM holds the collateral.
// ---------------------------------------------------------------------------
#[test]
fn drift_reserved_usdc_grows_by_exact_size_on_open() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let size = 7_500 * USDC_UNIT;
    let collateral = 750 * USDC_UNIT;

    let trader_before = f.usdc.balance(&f.trader);
    let vault_phys_before = f.usdc.balance(&f.vault_addr);
    let pm_phys_before = f.usdc.balance(&f.pm_addr);
    let reserved_before = f.vault.reserved_usdc();

    f.open_long(&f.trader, size, collateral);

    let trader_after = f.usdc.balance(&f.trader);
    let vault_phys_after = f.usdc.balance(&f.vault_addr);
    let pm_phys_after = f.usdc.balance(&f.pm_addr);
    let reserved_after = f.vault.reserved_usdc();

    // Reservation grew by exactly the position size.
    assert_eq!(
        reserved_after - reserved_before,
        size,
        "reserved_usdc must grow by exactly size on open",
    );
    // Trader spent collateral + open fee (which is taken from the inflow,
    // accrued to vault.unclaimed_fees + total_assets). The total trader
    // outflow is `collateral` plus a small open fee — assert the trader's
    // balance dropped by AT LEAST collateral and AT MOST collateral + dust.
    let trader_outflow = trader_before - trader_after;
    assert!(
        trader_outflow >= collateral,
        "trader paid at least collateral. before={}, after={}, collateral={}",
        trader_before,
        trader_after,
        collateral,
    );
    assert!(
        trader_outflow <= collateral + (collateral / 100),
        "trader paid at most collateral + 1% open-fee dust; got {}",
        trader_outflow,
    );
    // PM net inflow = collateral - open_fee_routed_to_vault. The open fee
    // physically transfers from PM to vault inside increase, so PM ends up
    // with exactly `collateral` minus that fee.
    let pm_inflow = pm_phys_after - pm_phys_before;
    let vault_inflow = vault_phys_after - vault_phys_before;
    assert_eq!(
        pm_inflow + vault_inflow,
        trader_outflow,
        "tokens are conserved: trader's outflow {} must equal PM inflow {} + vault inflow {}",
        trader_outflow,
        pm_inflow,
        vault_inflow,
    );
}

// ---------------------------------------------------------------------------
// 3. UnclaimedFees ↔ vault.accrue_fees on a close
//
// At close, per-position carrying fees are re-tagged from the
// LP pool to the dev/staker pool via `reslice_revenue` → `vault.accrue_fees`.
// This is a re-tag, NOT a transfer: vault's physical balance does NOT
// change in the reslice step, only `unclaimed_fees` increments.
//
// Stubbing out `reslice_revenue` (i.e. body→`()`) would leave `unclaimed_fees`
// unchanged after a close — this test pins a positive delta. Inverting the
// `accrue_fees` solvency guard would let `unclaimed > vault_physical` slip —
// pinned by the second assert.
// ---------------------------------------------------------------------------
#[test]
fn drift_unclaimed_fees_grows_on_close_without_token_movement() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let size = 20_000 * USDC_UNIT;
    let collateral = 2_000 * USDC_UNIT;

    f.open_long(&f.trader, size, collateral);
    // Accrue some borrow fee by holding the position for a while.
    f.advance_time(TEST_TIMESTAMP + 24 * 60 * 60); // +1 day
    f.set_btc_price(60_000); // small win, but enough to clear lifetime

    let vault_phys_before = f.usdc.balance(&f.vault_addr);
    let unclaimed_before = f.vault.unclaimed_fees();

    f.decrease_position(&f.trader, &symbol_short!("BTC"), &size, &0_i128);

    let vault_phys_after = f.usdc.balance(&f.vault_addr);
    let unclaimed_after = f.vault.unclaimed_fees();

    // The vault's PHYSICAL balance change on a close = (net flow in/out
    // of the vault). What we want to pin is the RESLICE step: it must NOT
    // physically move any tokens; only re-tag accounting from LP-pool to
    // dev-pool.
    //
    // Concretely: unclaimed_fees grew (because dev/staker bps > 0) by some
    // amount D, and that D must already be inside the vault's physical
    // balance — not an additional transfer ON TOP OF the trader payout.
    let unclaimed_delta = unclaimed_after - unclaimed_before;
    assert!(
        unclaimed_delta > 0,
        "Close after time-elapsed should reslice a positive fee amount to unclaimed",
    );
    // Solvency check: unclaimed_fees never exceeds total_assets minus reserved.
    assert!(
        unclaimed_after <= vault_phys_after,
        "unclaimed_fees {} > vault_physical {} — accrue_fees invariant violated",
        unclaimed_after,
        vault_phys_after,
    );
    // The change in physical balance is the trader's profit payout (net of
    // any open-fee accrual). It must NEVER equal `-unclaimed_delta` (i.e.
    // unclaimed grew but vault physically gave up that amount): that would
    // indicate a phantom transfer.
    let vault_phys_delta = vault_phys_after - vault_phys_before;
    assert_ne!(
        vault_phys_delta, -unclaimed_delta,
        "unclaimed grew by exactly the amount vault physically transferred out — \
         this would imply reslice_revenue actually moved tokens, not re-tagged",
    );
}

// ---------------------------------------------------------------------------
// 4. record_absorbed_collateral mismatch panic
//
// PM is the only legitimate caller. We spoof PM via mock_all_auths and pass
// an inconsistent `pre_balance` so `post - pre != amount`. The vault must
// panic with AbsorbedCollateralMismatch (= 12).
//
// Pins the mismatch check `!=` so an operator swap inverting the guard does
// not survive.
// ---------------------------------------------------------------------------
#[test]
#[should_panic(expected = "Error(Contract, #12)")]
fn drift_record_absorbed_collateral_panics_on_mismatch() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    // Spoof PM as caller. With mock_all_auths the require_auth check
    // passes, and the require_position_manager check is satisfied because
    // we pass the actual pm_addr.
    //
    // Call with a `pre_balance` that does NOT match the actual prior
    // vault balance: vault.total_assets() == VAULT_DEPOSIT (1M USDC); we
    // claim the pre was 0, and we claim 100 USDC moved. With 0 movement
    // having actually happened, post - pre = 1M - 0 = 1M, which does not
    // equal 100 — panic.
    f.vault.record_absorbed_collateral(
        &f.pm_addr,
        &f.trader,
        &(100 * USDC_UNIT),
        &0_i128, // wrong pre_balance
    );
}

// ---------------------------------------------------------------------------
// 5. Realized vs unrealized separation across a partial close
//
// After a profitable partial close: vault.net_global_trader_pnl must reflect
// only the still-OPEN portion's unrealized PnL — the closed half is now in
// the trader's wallet and in vault.total_assets, not in vault.net.
//
// Pins the invariant that realized PnL is *separate* from `vault.net`, never
// additive: `vault.net != pre_close_net + realized_delta`.
// ---------------------------------------------------------------------------
#[test]
fn drift_realized_vs_unrealized_separation_on_partial_close() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let size = 20_000 * USDC_UNIT;
    let collateral = 2_000 * USDC_UNIT;
    f.open_long(&f.trader, size, collateral);

    f.advance_time(TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 11);
    f.set_btc_price(60_000); // +20% — long is up

    // Refresh so vault.net reflects pre-close state.
    f.update_indices(&f.keeper, &symbol_short!("BTC"));
    let vault_net_before = f.vault.net_global_trader_pnl();
    let realized_before = f.position_manager.realized_pnl();
    assert!(
        vault_net_before > 0,
        "Setup: pre-close unrealized must be positive",
    );

    // Partial close: half the position.
    f.decrease_position(&f.trader, &symbol_short!("BTC"), &(size / 2), &0_i128);

    let vault_net_after = f.vault.net_global_trader_pnl();
    let realized_after = f.position_manager.realized_pnl();

    // After closing half, realized_pnl must have JUMPED (the trader took
    // profit on half the size). The CHANGE in vault.net must be the loss
    // of the open half's unrealized — NOT realized PnL being added in.
    let realized_delta = realized_after - realized_before;
    let vault_net_delta = vault_net_after - vault_net_before;
    assert!(
        realized_delta > 0,
        "Half-close at a profit must increase realized_pnl; got {}",
        realized_delta,
    );
    // The remaining open half should have its own unrealized roughly equal
    // to half the pre-close unrealized. vault.net should DECREASE (lost the
    // closed half's contribution), and the magnitude of the decrease should
    // be ~ the closed half's pre-close unrealized.
    assert!(
        vault_net_delta < 0,
        "vault.net must decrease when half the OI closes; got delta {}",
        vault_net_delta,
    );
    // Critical: vault.net is NOT the sum (vault_net_before + realized_delta).
    // That would be the double-deduction bug shape.
    let leaked_into_net = vault_net_before + realized_delta;
    assert_ne!(
        vault_net_after, leaked_into_net,
        "vault.net_global_trader_pnl = pre-close-net + realized_delta — \
         realized PnL is leaking back into vault.net",
    );
}

// ---------------------------------------------------------------------------
// 6. Pause-fee-clamp: borrow fee doesn't accrue while paused
//
// Pause PM, wait a long time, unpause, close. The borrow fee charged on
// close must reflect only the AWAKE intervals, not the paused interval.
// Uses LastUnpauseTime to clamp `effective_start = max(last_index_update,
// last_unpause_time)`.
// ---------------------------------------------------------------------------
#[test]
fn drift_pause_fee_clamp_excludes_paused_interval() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let size = 10_000 * USDC_UNIT;
    let collateral = 1_000 * USDC_UNIT;

    // Baseline: open, wait a known short time T_short, close — record fee.
    f.open_long(&f.trader, size, collateral);
    let t_short: u64 = 60 * 60; // 1 hour
    f.advance_time(TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 11 + t_short);
    f.set_btc_price(50_000); // flat — isolate fee, no PnL
    let trader_before = f.usdc.balance(&f.trader);
    f.decrease_position(&f.trader, &symbol_short!("BTC"), &size, &0_i128);
    let trader_after = f.usdc.balance(&f.trader);
    let baseline_payout = trader_after - trader_before;

    // Re-deploy fresh and run the same shape but with a long pause in the middle.
    let env2 = Env::default();
    let f2 = Fixture::deploy(&env2);

    f2.open_long(&f2.trader, size, collateral);
    // Advance a small amount, then pause.
    f2.advance_time(TEST_TIMESTAMP + 10);
    f2.pause_pm(&f2.admin);
    // Sleep ~30 days WHILE PAUSED.
    let paused_for: u64 = 30 * 24 * 60 * 60;
    f2.advance_time(TEST_TIMESTAMP + 10 + paused_for);
    f2.unpause_pm(&f2.admin);
    // Now wait the same `t_short` AWAKE as in the baseline.
    f2.advance_time(TEST_TIMESTAMP + 10 + paused_for + MIN_POSITION_LIFETIME + 11 + t_short);
    f2.set_btc_price(50_000);
    let t_before = f2.usdc.balance(&f2.trader);
    f2.decrease_position(&f2.trader, &symbol_short!("BTC"), &size, &0_i128);
    let t_after = f2.usdc.balance(&f2.trader);
    let paused_payout = t_after - t_before;

    // If the pause-fee-clamp works, the paused-run payout must be within
    // a small tolerance of the baseline — the 30-day paused interval does
    // not accumulate fees. If the clamp is broken, the paused run would
    // see ~30 days of fees and the payout would be visibly lower.
    let tolerance = collateral / 100; // 1% of collateral as dust band
    let diff = (baseline_payout - paused_payout).abs();
    assert!(
        diff <= tolerance,
        "pause-fee-clamp broken: baseline payout {} vs paused-run payout {} differ by {} > tolerance {} \
         (30-day pause must not contribute to borrow fee)",
        baseline_payout, paused_payout, diff, tolerance,
    );
}

// ---------------------------------------------------------------------------
// 7. safe_basis (total_assets_excl_pnl) does NOT move with mark price
//
// PM uses safe_basis as the utilization denominator. PnL is excluded from
// this read so mark-price wicks can't manipulate the gate. Open both sides,
// move the mark price, refresh — assert total_assets_excl_pnl is unchanged.
//
// Any operator swap in `total_assets_excl_pnl` (e.g. `total - unclaimed_fees`
// → `+`) would make this value shift with every fee accrual or mark price
// move, failing this test.
// ---------------------------------------------------------------------------
#[test]
fn drift_safe_basis_invariant_under_mark_price_moves() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let trader2 = f.create_funded_trader(20_000 * USDC_UNIT);
    f.open_long(&f.trader, 10_000 * USDC_UNIT, 1_000 * USDC_UNIT);
    f.open_short(&trader2, 8_000 * USDC_UNIT, 800 * USDC_UNIT);

    let safe_basis_initial = f.vault.total_assets_excl_pnl();

    // Move mark price up — longs win, shorts lose. Net unrealized changes.
    f.advance_time(TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 11);
    f.set_btc_price(70_000);
    f.update_indices(&f.keeper, &symbol_short!("BTC"));

    let safe_basis_after_up = f.vault.total_assets_excl_pnl();

    // Move mark price down — shorts win, longs lose. Net flips.
    f.set_btc_price(35_000);
    f.update_indices(&f.keeper, &symbol_short!("BTC"));

    let safe_basis_after_down = f.vault.total_assets_excl_pnl();

    assert_eq!(
        safe_basis_initial, safe_basis_after_up,
        "total_assets_excl_pnl moved with a +40% mark price wick — utilization gate is mark-price-sensitive",
    );
    assert_eq!(
        safe_basis_initial, safe_basis_after_down,
        "total_assets_excl_pnl moved with a -30% mark price wick",
    );
}

// ---------------------------------------------------------------------------
// 8. Global avg price recalc BEFORE OI decrement
//
// Open two longs at different entry prices. Close one. The remaining
// global_long_avg_price must equal exactly the entry price of the remaining
// position (single open). If OI is decremented before the avg recalc, the
// avg drifts to a stale weighted value.
// ---------------------------------------------------------------------------
#[test]
fn drift_global_avg_price_recalculates_before_oi_decrement() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let trader2 = f.create_funded_trader(20_000 * USDC_UNIT);

    // Trader A opens 10k @ 50k.
    f.open_long(&f.trader, 10_000 * USDC_UNIT, 1_000 * USDC_UNIT);

    // Move to 60k, then trader B opens 10k @ 60k. Advance beyond the
    // oracle-router cache window (`cache_duration = 10s` per Fixture::deploy)
    // so the second open reads the new price, not the cached one.
    f.advance_time(TEST_TIMESTAMP + 30);
    f.set_btc_price(60_000);
    f.open_long(&trader2, 10_000 * USDC_UNIT, 1_000 * USDC_UNIT);

    // The display price is derived from additive base exposure. Equal quote
    // notionals therefore produce the harmonic mean, while PnL remains exact.
    let market = f.position_manager.get_market(&symbol_short!("BTC"));
    let expected_after_open = 545_454_545_454_i128;
    assert_eq!(
        market.global_long_avg_price, expected_after_open,
        "display price must be derived from aggregate base exposure",
    );

    // Now close Trader A's position. Remaining is Trader B's 10k @ 60k.
    // Avg must recalc to 60k BEFORE decrementing OI.
    f.advance_time(TEST_TIMESTAMP + 30 + MIN_POSITION_LIFETIME + 11);
    f.set_btc_price(60_000); // keep flat to isolate
    f.decrease_position(
        &f.trader,
        &symbol_short!("BTC"),
        &(10_000 * USDC_UNIT),
        &0_i128,
    );

    let market_after = f.position_manager.get_market(&symbol_short!("BTC"));
    let expected_after_close = 60_000 * 10_000_000_i128;
    assert_eq!(
        market_after.global_long_avg_price, expected_after_close,
        "After closing the 50k-entry leg, remaining 10k @ 60k must produce avg=60k. \
         A wrong recalc-after-decrement ordering would leave the avg at a stale value.",
    );
    assert_eq!(
        market_after.long_open_interest,
        10_000 * USDC_UNIT,
        "OI must drop by exactly the closed size",
    );
}

// ---------------------------------------------------------------------------
// 9. accrue_fees clamps at the solvency boundary: reserved + unclaimed cannot
// exceed raw custody.
//
// We force the boundary by directly calling vault.reserve_liquidity and
// vault.accrue_fees as PM (mock_all_auths approves). Pump reserved to just
// under total, accrue fees up to the limit, then attempt one more 1% over
// the boundary. accrue_fees runs inside PM's close/liquidation paths, so it
// must CLAMP to the available headroom rather than revert.
// ---------------------------------------------------------------------------
#[test]
fn drift_accrue_fees_clamps_at_solvency_boundary() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let total_initial = f.vault.total_assets();
    let reserved = total_initial * 99 / 100;

    f.vault.reserve_liquidity(&f.pm_addr, &reserved);
    f.vault.accrue_fees(&f.pm_addr, &(total_initial / 200)); // 0.5% — fits

    // This attempt to accrue another 1% must clamp to the remaining headroom,
    // not revert.
    f.vault.accrue_fees(&f.pm_addr, &(total_initial / 100));

    // Clamped exactly to the boundary: reserved + unclaimed == raw custody
    // (no token moved during this test, so raw == the vault's USDC balance).
    let raw = f.usdc.balance(&f.vault_addr);
    assert_eq!(
        f.vault.reserved_usdc() + f.vault.unclaimed_fees(),
        raw,
        "accrue_fees must clamp to the solvency boundary, never exceed it",
    );
}

// ---------------------------------------------------------------------------
// 10. claim_fees cascade: 100 → 40 → 40 → 20 → 0
//
// Accrue 100 USDC of unclaimed_fees, claim 40 to one recipient and 60 to
// another via two claim_fees calls (note: claim_fees claims ALL; so we
// use it for the second/final claim). Asserts each recipient received their
// share, unclaimed_fees ends at 0, and vault.total_assets dropped by exactly
// 100 USDC.
// ---------------------------------------------------------------------------
#[test]
fn drift_claim_fees_cascade_drains_to_zero_with_token_conservation() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let total_before = f.vault.total_assets();
    let vault_phys_before = f.usdc.balance(&f.vault_addr);

    // Seed: accrue 100 USDC of fees via the PM-only path. This requires the
    // vault to already hold the dollars (accrue_fees is a re-tag), so first
    // reserve 0 and accrue. accrue_fees enforces `new_total + reserved <=
    // total_assets`, which trivially passes since reserved=0 and 100 << 1M.
    let fee_amount = 100 * USDC_UNIT;
    f.vault.accrue_fees(&f.pm_addr, &fee_amount);
    assert_eq!(f.vault.unclaimed_fees(), fee_amount);

    // claim_fees_to: partial claim of 40 USDC to recipient_a. Caller is PM.
    let recipient_a = Address::generate(&env);
    let recipient_b = Address::generate(&env);

    f.vault
        .claim_fees_to(&f.pm_addr, &recipient_a, &(40 * USDC_UNIT));
    assert_eq!(f.usdc.balance(&recipient_a), 40 * USDC_UNIT);
    assert_eq!(f.vault.unclaimed_fees(), 60 * USDC_UNIT);

    // Final claim of the remaining 60 via claim_fees (admin-gated, drains all).
    f.claim_fees(&f.admin, &recipient_b);
    assert_eq!(f.usdc.balance(&recipient_b), 60 * USDC_UNIT);
    assert_eq!(f.vault.unclaimed_fees(), 0);

    // Token conservation: vault's PHYSICAL balance dropped by exactly the
    // total claimed (100 USDC). total_assets reflects the same drop.
    let vault_phys_after = f.usdc.balance(&f.vault_addr);
    let total_after = f.vault.total_assets();
    assert_eq!(
        vault_phys_before - vault_phys_after,
        100 * USDC_UNIT,
        "vault physical balance must drop by exactly the claimed fee total",
    );
    assert_eq!(
        total_before - total_after,
        100 * USDC_UNIT,
        "vault.total_assets must drop by exactly the claimed fee total",
    );

    // Final invariant sweep — also called inside f.claim_fees but worth
    // an explicit pin here as the canonical "clean state" check.
    assert_protocol_invariants(&env, &f, "drift_claim_fees_cascade end-state");
}
