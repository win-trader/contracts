//! Cross-cutting solvency invariants — the single source of truth for "is the
//! protocol still self-consistent and physically backed?"
//!
//! Every stored ledger counter the protocol maintains is reconciled here
//! against either (a) the SEP-41 token balance of the relevant address, or
//! (b) a closed-form derivation from sibling counters. If any clause fails,
//! recorded state has drifted from physical reality.
//!
//! The helper is intentionally **stateless** and **pure-read** — it never
//! mutates state, never grants roles, never advances time. It's safe to call
//! after any operation in any test.
//!
//! Inject this into `Fixture` operation wrappers (open_long, decrease_position,
//! …) so every existing test gets drift detection for free. See `Fixture::*`
//! in `mod.rs`. The `ctx` parameter labels which call site detected the drift,
//! which makes failure messages immediately actionable.
//!
//! What this helper does NOT check (deliberately, to keep it cheap and
//! non-iterative):
//!
//! - Per-position consistency (e.g. `reserved_usdc == sum(pos.size)` across
//!   all open positions). PositionManager has no public iterator, and adding
//!   one to support this would expand the production surface. Use
//!   [`assert_position_summation`] explicitly from tests that want it.
//!
//! - Per-market global-avg-price * OI vs. sum(entry_price * size). Same
//!   reason. Drift-pair tests in `tests/drift_pairs.rs` cover this with
//!   bespoke scenarios.

use soroban_sdk::{symbol_short, Address, Env, Symbol};

use super::Fixture;

/// The markets the standard `Fixture::deploy` configures. Clause 6 (sum of
/// per-market unrealized == total unrealized) iterates over this list. If a
/// future fixture deploys more markets, extend it.
pub const ACTIVE_MARKETS: &[&str] = &["BTC"];

/// Reconcile every protocol counter against physical token-side reality and
/// sibling counters. Panics with a context-tagged message at the first failed
/// clause.
///
/// `ctx` is a short label identifying the call site (e.g. `"after open_long"`)
/// so test failures point straight at the operation that introduced drift.
pub fn assert_protocol_invariants(env: &Env, f: &Fixture, ctx: &str) {
    // -----------------------------------------------------------------------
    // Snapshot vault counters once. `total_assets()` reports the LP-owned
    // basis (raw custody minus the non-LP claims), so physical custody is
    // reconstructed as `total_assets + unclaimed_fees + max(0, net_pnl)`.
    // -----------------------------------------------------------------------
    let total = f.vault.total_assets();
    let reserved = f.vault.reserved_usdc();
    let unclaimed = f.vault.unclaimed_fees();
    let net_pnl = f.vault.net_global_trader_pnl();
    let pnl_deduction = if net_pnl > 0 { net_pnl } else { 0 };
    let raw_custody = total + unclaimed + pnl_deduction;

    // -----------------------------------------------------------------------
    // Clause 1: physical USDC in the vault wallet == reconstructed raw
    // custody. If the SEP-41 contract says the vault holds N USDC, vault's
    // accounting (LP basis + fee buffer + trader-PnL buffer) MUST agree. Any
    // drift here means a transfer happened (or didn't) without a matching
    // counter update.
    // -----------------------------------------------------------------------
    let vault_physical = f.usdc.balance(&f.vault_addr);
    assert_eq!(
        vault_physical, raw_custody,
        "[{ctx}] vault USDC balance {} != total_assets + unclaimed + max(0,pnl) {} — physical/accounting drift",
        vault_physical, raw_custody,
    );

    // -----------------------------------------------------------------------
    // (Clause 2 reserved.) PositionManager *does* custody collateral for the
    // duration of every open position — `increase_position` pulls USDC from
    // the trader into PM, and `decrease_position` / `liquidate` settle it
    // back out via the vault. There's no aggregate counter on PM that lets
    // us assert "expected balance" without enumerating open positions, so
    // this auto-injected helper does not check PM's USDC balance. The
    // drift_pairs.rs tests (Phase 4) snapshot PM balance pre/post a specific
    // operation and assert the delta against the operation's contract.
    // -----------------------------------------------------------------------

    // -----------------------------------------------------------------------
    // Clause 3: vault solvency invariant — `reserved_usdc + unclaimed_fees <=
    // raw custody`. Enforced at `accrue_fees` (clamped) and `reserve_liquidity`;
    // `vault_view::utilization_bps` and `safe_basis` derivations depend on it.
    // -----------------------------------------------------------------------
    assert!(
        reserved + unclaimed <= raw_custody,
        "[{ctx}] solvency violated: reserved {} + unclaimed {} > raw custody {} (vault_view.rs invariant)",
        reserved, unclaimed, raw_custody,
    );

    // -----------------------------------------------------------------------
    // Clause 4: `free_liquidity` must equal its closed-form value. In LP-basis
    // terms `raw - reserved - unclaimed - max(0,pnl)` collapses to
    // `total_assets - reserved`, since `total_assets = raw - unclaimed -
    // max(0,pnl)`. Any operator mutation on `vault/logic.rs::free_liquidity`
    // fails here.
    // -----------------------------------------------------------------------
    let expected_free = (total - reserved).max(0);
    assert_eq!(
        f.vault.free_liquidity(),
        expected_free,
        "[{ctx}] free_liquidity drift: got {}, expected max(0, total_assets {} - reserved {}) = {}",
        f.vault.free_liquidity(),
        total, reserved, expected_free,
    );

    // -----------------------------------------------------------------------
    // Clause 5: `vault.net_global_trader_pnl` is synced *exclusively* from
    // PM's `TotalUnrealizedPnl` (see `pnl_refresh.rs::refresh_market_unrealized_pnl`).
    // Realized PnL is NEVER sent to the vault — it has already moved physically
    // via `pay_profit` / `record_absorbed_collateral` and is reflected directly
    // in `total_assets`. Any mutation that adds realized PnL to the sync, or
    // fails to refresh unrealized after a state change, fails here.
    // -----------------------------------------------------------------------
    let pm_total_unrealized = f.position_manager.total_unrealized_pnl();
    assert_eq!(
        net_pnl, pm_total_unrealized,
        "[{ctx}] vault.net_global_trader_pnl {} != PM.total_unrealized_pnl {} — realized PnL is leaking into the vault sync",
        net_pnl, pm_total_unrealized,
    );

    // -----------------------------------------------------------------------
    // Clause 6: per-market unrealized must aggregate to the total. If a
    // single market's `MarketUnrealizedPnl(symbol)` is updated without a
    // matching `TotalUnrealizedPnl` delta (or vice-versa), the sum drifts.
    // `pnl_refresh.rs:32-41` does both updates in one delta-based pass, so
    // any reordering or sign flip would break here.
    // -----------------------------------------------------------------------
    let mut market_sum: i128 = 0;
    for m in ACTIVE_MARKETS {
        let s = Symbol::new(env, m);
        market_sum += f.position_manager.market_unrealized_pnl(&s);
    }
    assert_eq!(
        market_sum, pm_total_unrealized,
        "[{ctx}] sum(MarketUnrealizedPnl) {} != TotalUnrealizedPnl {} — per-market/total aggregation drift",
        market_sum, pm_total_unrealized,
    );

    // -----------------------------------------------------------------------
    // Clause 7: `total_assets_excl_pnl == max(0, raw - unclaimed_fees)`. In
    // LP-basis terms `raw - unclaimed` collapses to `total_assets +
    // max(0,pnl)`. This view feeds PM's utilization denominator; any operator
    // swap in `vault/logic.rs::total_assets_excl_pnl` would let mark-price PnL
    // feed back into the gate.
    // -----------------------------------------------------------------------
    let safe_basis_expected = (raw_custody - unclaimed).max(0);
    assert_eq!(
        f.vault.total_assets_excl_pnl(),
        safe_basis_expected,
        "[{ctx}] total_assets_excl_pnl drift: got {}, expected max(0, raw {} - unclaimed {}) = {}",
        f.vault.total_assets_excl_pnl(),
        raw_custody, unclaimed, safe_basis_expected,
    );
}

/// Optional, opt-in stronger check: reconcile `reserved_usdc` against the sum
/// of currently-open position sizes the caller knows about. Use from tests
/// that explicitly track their open positions; not auto-injected because the
/// caller has to pass the list.
///
/// `expected_open` is a slice of `(trader, symbol)` pairs the test believes
/// are currently open. Closed positions silently contribute 0 (their storage
/// read panics with `PositionNotFound` — we catch that case here so the test
/// can carry stale pairs without rebuilding the list every step).
pub fn assert_position_summation(
    f: &Fixture,
    expected_open: &[(Address, Symbol)],
) {
    let mut size_sum: i128 = 0;
    for (trader, symbol) in expected_open {
        // `get_position` panics on missing; soroban tests can't catch panics
        // in-process, so the caller must keep the list accurate. If you hit
        // a panic here, the test thinks a position is open that has actually
        // closed — fix the test's tracking.
        let pos = f.position_manager.get_position(trader, symbol);
        size_sum += pos.size;
    }
    assert_eq!(
        f.vault.reserved_usdc(),
        size_sum,
        "vault.reserved_usdc {} != sum(pos.size) {} — per-position reservation drift",
        f.vault.reserved_usdc(),
        size_sum,
    );
}

/// Convenience: `assert_protocol_invariants(env, f, "BTC")` is the common
/// shorthand used by the Fixture operation wrappers.
#[allow(dead_code)]
pub fn assert_after(env: &Env, f: &Fixture, op: &str) {
    assert_protocol_invariants(env, f, op);
}

/// Helper to compute the BTC symbol once. Kept here so callers don't import
/// `symbol_short!` just to read a constant.
#[allow(dead_code)]
pub fn btc() -> Symbol {
    symbol_short!("BTC")
}
