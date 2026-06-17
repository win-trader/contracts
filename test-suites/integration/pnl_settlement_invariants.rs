//! Vault solvency invariants around realized vs unrealized PnL.
//!
//! Settles the question: once a position is closed, USDC has already moved
//! (via `vault.pay_profit` on a winning close or `vault.record_absorbed_collateral`
//! on a losing one). `total_assets` reflects that immediately, so the
//! `free_liquidity` formula must NOT deduct realized PnL a second time via
//! `net_global_trader_pnl`. Only OPEN trader winnings (current unrealized)
//! are a real outstanding liability.
//!
//! These tests assert the empty-market invariant: when every position is
//! closed, `total_assets - reserved_usdc - free_liquidity` (= `unclaimed_fees
//! + max(0, net_pnl)`) is bounded by fee dust — never by realized trader
//! profits.

use soroban_sdk::{symbol_short, Env};
use test_suites::testutils::{Fixture, TEST_TIMESTAMP, USDC_UNIT};

const MIN_POSITION_LIFETIME: u64 = 60;
/// Cap on `unclaimed_fees` we'd expect from a single open + close round-trip
/// at any sane fee config. 1% of notional is far above the actual non-LP
/// slice (typical revenue fees are a few bps) and far below any realistic
/// trader payout on a directional move, so it cleanly separates the two.
const FEE_DUST_BOUND: i128 = 100 * USDC_UNIT;

/// Regression for the realized-PnL double-deduction bug.
///
/// Before the fix, PM synced `realized + unrealized` to `vault.net_global_trader_pnl`,
/// so after a winning close `free_liquidity` was reduced by the trader's
/// profit even though the vault had already paid it out. With no positions
/// open, `unrealized = 0` and the implicit `total - reserved - free`
/// deduction must equal only `unclaimed_fees` — fee dust, not trader profits.
#[test]
fn test_free_liquidity_after_winning_close_does_not_double_deduct_realized() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let size = 10_000 * USDC_UNIT; // 10x leverage
    let collateral = 1_000 * USDC_UNIT;
    f.open_long(&f.trader, size, collateral);

    f.advance_time(TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 11);
    // +50% spot — at 10x leverage this is a ~$5,000 trader profit. Single
    // oracle source ⇒ the deviation gate does not bind.
    f.set_btc_price(75_000);

    let trader_balance_before = f.usdc.balance(&f.trader);
    f.decrease_position(&f.trader, &symbol_short!("BTC"), &size, &0_i128);
    let trader_balance_after = f.usdc.balance(&f.trader);

    let trader_received = trader_balance_after - trader_balance_before;
    assert!(
        trader_received > collateral,
        "Setup: trader must close at a profit. received={}, collateral={}",
        trader_received,
        collateral
    );
    let trader_profit = trader_received - collateral;
    assert!(
        trader_profit > FEE_DUST_BOUND,
        "Setup: profit must be large enough to dwarf fee dust. profit={}",
        trader_profit
    );

    let total_after = f.vault.total_assets();
    let reserved_after = f.vault.reserved_usdc();
    let free_after = f.vault.free_liquidity();

    assert_eq!(reserved_after, 0, "Full close: reserved_usdc must be 0");

    let implicit_deduction = total_after - reserved_after - free_after;
    assert!(
        implicit_deduction <= FEE_DUST_BOUND,
        "Realized trader profit is being double-deducted from free_liquidity. \
         total_assets={}, reserved={}, free_liquidity={}, \
         implicit deduction (unclaimed_fees + max(0, net_pnl))={}, \
         trader_profit={}, fee_dust_bound={}. \
         The bug pushes the deduction up by ~trader_profit; with no positions \
         open the only legitimate deduction is unclaimed_fees.",
        total_after,
        reserved_after,
        free_after,
        implicit_deduction,
        trader_profit,
        FEE_DUST_BOUND
    );
}

/// Empty-market invariant: a sequence of winning + losing closes must leave
/// the vault in a state where `total - reserved - free` is bounded by fee
/// dust accumulated across the lifecycle. Realized PnL — in either
/// direction, across any number of closes — must not contribute to the
/// deduction once the market is flat.
#[test]
fn test_alternating_win_loss_closes_leave_no_pnl_residue() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let size = 10_000 * USDC_UNIT;
    let collateral = 1_000 * USDC_UNIT;
    let mut t = TEST_TIMESTAMP;

    // Round 1: trader wins on a long (+30%).
    f.open_long(&f.trader, size, collateral);
    t += MIN_POSITION_LIFETIME + 11;
    f.advance_time(t);
    f.set_btc_price(65_000);
    f.decrease_position(&f.trader, &symbol_short!("BTC"), &size, &0_i128);

    // Reset price for the next entry. Round 2: trader loses on a long (-20%).
    t += 5;
    f.advance_time(t);
    f.set_btc_price(50_000);
    f.open_long(&f.trader, size, collateral);
    t += MIN_POSITION_LIFETIME + 11;
    f.advance_time(t);
    f.set_btc_price(40_000);
    f.decrease_position(&f.trader, &symbol_short!("BTC"), &size, &0_i128);

    // Reset, Round 3: trader wins again (+40%).
    t += 5;
    f.advance_time(t);
    f.set_btc_price(50_000);
    f.open_long(&f.trader, size, collateral);
    t += MIN_POSITION_LIFETIME + 11;
    f.advance_time(t);
    f.set_btc_price(70_000);
    f.decrease_position(&f.trader, &symbol_short!("BTC"), &size, &0_i128);

    let total = f.vault.total_assets();
    let reserved = f.vault.reserved_usdc();
    let free = f.vault.free_liquidity();
    assert_eq!(reserved, 0, "All positions closed: reserved must be 0");

    let implicit_deduction = total - reserved - free;
    // Three open + close cycles accumulate at most 3× the per-round dust.
    let three_round_dust = 3 * FEE_DUST_BOUND;
    assert!(
        implicit_deduction <= three_round_dust,
        "PnL residue is leaking into free_liquidity across closes. \
         total={}, reserved={}, free={}, deduction={}, allowed_dust={}. \
         Closed-trade PnL must net to zero in the deduction once OI is flat.",
        total,
        reserved,
        free,
        implicit_deduction,
        three_round_dust
    );
}

/// Partial closes at a profit: trader peels off half the position at a win,
/// then closes the rest at the same price. The first close realizes half
/// the unrealized profit; the second flattens the market. After step 2 the
/// invariant must hold — no PnL residue.
#[test]
fn test_partial_then_full_close_at_profit_leaves_no_pnl_residue() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let size = 20_000 * USDC_UNIT;
    let collateral = 2_000 * USDC_UNIT;
    f.open_long(&f.trader, size, collateral);

    f.advance_time(TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 11);
    f.set_btc_price(70_000); // +40%

    // Step 1: partial close — half the position.
    f.decrease_position(
        &f.trader,
        &symbol_short!("BTC"),
        &(size / 2),
        &0_i128,
    );

    // Step 2: close the rest at the same price.
    f.advance_time(TEST_TIMESTAMP + 2 * MIN_POSITION_LIFETIME + 22);
    f.set_btc_price(70_000);
    f.decrease_position(
        &f.trader,
        &symbol_short!("BTC"),
        &(size / 2),
        &0_i128,
    );

    let total = f.vault.total_assets();
    let reserved = f.vault.reserved_usdc();
    let free = f.vault.free_liquidity();
    assert_eq!(reserved, 0, "All positions closed: reserved must be 0");

    let implicit_deduction = total - reserved - free;
    assert!(
        implicit_deduction <= FEE_DUST_BOUND,
        "Realized PnL from the partial close is residual in free_liquidity. \
         total={}, reserved={}, free={}, deduction={}, dust={}.",
        total,
        reserved,
        free,
        implicit_deduction,
        FEE_DUST_BOUND
    );
}
