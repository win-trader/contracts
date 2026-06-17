//! Fee distribution integration tests.
//!
//! Validates the multi-contract flow for the three revenue streams charged by
//! the protocol:
//!
//! - Open fee: `size * open_fee_bps / BPS`, charged on every `increase_position`.
//!   Trader -> PM -> Vault. Non-LP slice (dev+staker bps of FeeSplits) accrues
//!   to `vault.unclaimed_fees`; LP slice stays in `vault.total_assets` implicitly.
//! - Close-time revenue fees (borrow + funding_protocol_cut): split via the
//!   same FeeSplits.
//! - Liquidation bounty: `min(collateral * liquidation_bounty_bps / BPS, pm_to_vault)`,
//!   paid PM -> liquidator. Separate from the revenue split.
//! - TP/SL execution escrow: a flat `tp_sl_execution_fee` USDC amount paid by
//!   the trader on first TP/SL set. Refunded on full UserClose / Deleverage;
//!   forfeited to vault on Liquidation; paid to the executor on OrderExecution.
//!
//! Fixture defaults: FeeSplits { lp_bps=9000, dev_bps=1000, staker_bps=0 };
//! FeeConfig { open_fee_bps=10, liquidation_bounty_bps=100, tp_sl_execution_fee=5_000_000 }.

use shared::constants::{
    self, BPS, DEFAULT_MIN_POSITION_LIFETIME, DEFAULT_TP_SL_EXECUTION_FEE, PRECISION,
};
use shared::FeeConfig;
use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env};
use test_suites::testutils::{Fixture, TEST_TIMESTAMP, USDC_UNIT};

const MIN_POSITION_LIFETIME: u64 = DEFAULT_MIN_POSITION_LIFETIME;
const DEFAULT_OPEN_FEE_BPS: i128 = constants::DEFAULT_OPEN_FEE_BPS as i128;
const DEFAULT_LIQ_BOUNTY_BPS: i128 = constants::DEFAULT_LIQUIDATION_BOUNTY_BPS as i128;
const DEFAULT_TP_SL_FEE: i128 = DEFAULT_TP_SL_EXECUTION_FEE;

// ---------------------------------------------------------------------------
// 1. User close (decrease_position): no bounty, no escrow payout, dev share accrues
// ---------------------------------------------------------------------------

#[test]
fn test_user_close_no_keeper_share() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let keeper_balance_before = f.usdc.balance(&f.keeper);

    let size = 50_000 * USDC_UNIT;
    let collateral = 5_000 * USDC_UNIT;
    f.open_long(&f.trader, size, collateral);

    f.advance_time(TEST_TIMESTAMP + 3_600);
    f.set_btc_price(50_000);
    f.update_indices(&f.keeper, &symbol_short!("BTC"));

    f.advance_time(TEST_TIMESTAMP + 3_600 + MIN_POSITION_LIFETIME + 1);
    f.set_btc_price(50_000);
    f.decrease_position(&f.trader, &symbol_short!("BTC"), &size, &0_i128);

    // Keeper balance unchanged: keepers no longer get a slice of revenue fees,
    // and there is no bounty path on user close.
    let keeper_balance_after = f.usdc.balance(&f.keeper);
    assert_eq!(
        keeper_balance_after, keeper_balance_before,
        "Keeper must NOT receive funds on user close (no bounty path)"
    );

    // Dev share (open_fee_bps non-LP slice + close-time non-LP slice) must be claimable.
    let recipient = Address::generate(&env);
    f.claim_fees(&f.admin, &recipient);
    let dev_claimed = f.usdc.balance(&recipient);
    assert!(
        dev_claimed > 0,
        "Dev share must be positive after user close: claimed={}",
        dev_claimed
    );
}

// ---------------------------------------------------------------------------
// 2. TP/SL execution: executor receives flat tp_sl_execution_fee (escrow)
// ---------------------------------------------------------------------------

#[test]
fn test_tp_sl_keeper_gets_share() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let executor = Address::generate(&env);
    let executor_balance_before = f.usdc.balance(&executor);

    // Open long with TP set on increase_position — escrow charged at open.
    let size = 50_000 * USDC_UNIT;
    let collateral = 5_000 * USDC_UNIT;
    let tp_price: i128 = 55_000 * PRECISION;
    f.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &size,
        &collateral,
        &true,
        &tp_price,
        &0,
        &0i128,
    );

    // Confirm escrow was paid at open (escrow != 0 -> the escrow path fires).
    let pos = f
        .position_manager
        .get_position(&f.trader, &symbol_short!("BTC"));
    assert_eq!(
        pos.execution_fee_escrow, DEFAULT_TP_SL_FEE,
        "Position must record tp_sl_execution_fee escrow on open with TP"
    );

    f.advance_time(TEST_TIMESTAMP + 3_600);
    f.set_btc_price(56_000); // above TP triggers

    // Permissionless executor (no KEEPER role) calls execute_order.
    f.execute_order(&executor, &f.trader, &symbol_short!("BTC"));

    // Executor must receive the escrowed tp_sl_execution_fee (flat amount).
    let executor_balance_after = f.usdc.balance(&executor);
    let executor_received = executor_balance_after - executor_balance_before;
    assert_eq!(
        executor_received, DEFAULT_TP_SL_FEE,
        "Executor must receive the flat tp_sl_execution_fee escrow on order execution: \
         got {}, expected {}",
        executor_received, DEFAULT_TP_SL_FEE
    );
}

// ---------------------------------------------------------------------------
// 3. Liquidation: liquidator receives bounty (collateral * bps / BPS)
// ---------------------------------------------------------------------------

#[test]
fn test_liquidation_keeper_gets_share() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    // Permissionless liquidator address, never granted KEEPER.
    let liquidator = Address::generate(&env);
    let liq_balance_before = f.usdc.balance(&liquidator);

    let size = 20_000 * USDC_UNIT;
    let collateral = 2_000 * USDC_UNIT;
    f.open_long(&f.trader, size, collateral);

    f.advance_time(TEST_TIMESTAMP + 3_600);
    f.set_btc_price(44_000); // crash, position liquidatable

    f.liquidate(&liquidator, &f.trader, &symbol_short!("BTC"));

    // Bounty = collateral * liquidation_bounty_bps / BPS. pm_to_vault on this
    // crash exceeds the bounty, so the clamp does not bind.
    let liq_balance_after = f.usdc.balance(&liquidator);
    let received = liq_balance_after - liq_balance_before;
    let expected_bounty = collateral * DEFAULT_LIQ_BOUNTY_BPS / BPS;
    assert_eq!(
        received, expected_bounty,
        "Liquidator must receive bounty == collateral * liquidation_bounty_bps / BPS: \
         got {}, expected {}",
        received, expected_bounty
    );
}

// ---------------------------------------------------------------------------
// 4. ADL (deleverage_position): no keeper share, no bounty, escrow refunded
// ---------------------------------------------------------------------------

#[test]
fn test_adl_no_keeper_share() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    // Lower ADL utilization threshold so the test trigger is reachable.
    f.config_manager.update_protocol_limits(
        &f.admin,
        &config_manager::ProtocolLimits {
            min_collateral: 1_000_000,
            cooldown_duration: 60,
            min_position_lifetime: 60,
            max_utilization_ratio: 8_500,
            funding_cut_bps: 500,
            adl_pnl_bps: 9_000,
            adl_utilization_bps: 3_000,
            liquidation_threshold_bps: 200,
        },
    );
    f.config_manager.update_borrow_rate_config(
        &f.admin,
        &config_manager::BorrowRateConfig {
            base_borrow_rate_bps: 100,
            slope1_bps: 500,
            slope2_bps: 5_000,
            optimal_utilization_bps: 8_000,
            base_funding_rate_bps: 100,
        },
    );

    let keeper_balance_before = f.usdc.balance(&f.keeper);

    let trader = f.create_funded_trader(50_000 * USDC_UNIT);
    f.open_long(&trader, 400_000 * USDC_UNIT, 40_000 * USDC_UNIT);

    f.advance_time(TEST_TIMESTAMP + 3_600);
    f.set_btc_price(50_100);

    f.deleverage_position(&f.keeper, &trader, &symbol_short!("BTC"));

    // Keeper balance unchanged: ADL pays no bounty and there is no TP/SL
    // escrow on this position (none was set).
    let keeper_balance_after = f.usdc.balance(&f.keeper);
    assert_eq!(
        keeper_balance_after, keeper_balance_before,
        "Keeper must NOT receive funds on ADL (no bounty, no escrow payout)"
    );

    // Dev share still accrues from open_fee + close-time fees.
    let recipient = Address::generate(&env);
    f.claim_fees(&f.admin, &recipient);
    let dev_claimed = f.usdc.balance(&recipient);
    assert!(
        dev_claimed > 0,
        "Dev share must be positive after ADL: claimed={}",
        dev_claimed
    );
}

// ---------------------------------------------------------------------------
// 5. Admin claims dev fees — non-zero after open + close lifecycle
// ---------------------------------------------------------------------------

#[test]
fn test_admin_claims_dev_fees() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let size = 50_000 * USDC_UNIT;
    let collateral = 5_000 * USDC_UNIT;
    f.open_long(&f.trader, size, collateral);

    f.advance_time(TEST_TIMESTAMP + 86_400);
    f.set_btc_price(50_000);
    f.update_indices(&f.keeper, &symbol_short!("BTC"));
    f.decrease_position(&f.trader, &symbol_short!("BTC"), &size, &0_i128);

    let recipient = Address::generate(&env);
    let recipient_balance_before = f.usdc.balance(&recipient);
    f.claim_fees(&f.admin, &recipient);

    let dev_claimed = f.usdc.balance(&recipient) - recipient_balance_before;
    assert!(
        dev_claimed > 0,
        "Admin must receive positive dev fees via claim_fees: claimed={}",
        dev_claimed
    );

    // Lower bound: dev_claimed must include at least the non-LP slice of the
    // open fee (open_fee_bps * size / BPS, then dev_bps slice of that).
    let open_fee = size * DEFAULT_OPEN_FEE_BPS / BPS;
    let fee_splits = f.config_manager.get_fee_splits();
    let non_lp_bps = (fee_splits.dev_bps + fee_splits.staker_bps) as i128;
    let open_fee_dev_slice = open_fee * non_lp_bps / BPS;
    assert!(
        dev_claimed >= open_fee_dev_slice,
        "dev_claimed must include at least the open-fee non-LP slice: \
         dev_claimed={}, open_fee_dev_slice={}",
        dev_claimed,
        open_fee_dev_slice
    );

    // Second claim must panic — unclaimed_fees was zeroed.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        f.claim_fees(&f.admin, &recipient);
    }));
    assert!(
        result.is_err(),
        "Second claim_fees must fail when unclaimed_fees is 0"
    );
}

// ---------------------------------------------------------------------------
// 6. Zero fees: no distribution, no panics
// ---------------------------------------------------------------------------

#[test]
fn test_zero_fees_no_distribution() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    // Disable open fee + TP/SL escrow so the position lifecycle generates only
    // the negligible borrow fee that ~60 seconds will accrue.
    f.config_manager.set_fee_config(
        &f.admin,
        &FeeConfig {
            open_fee_bps: 0,
            liquidation_bounty_bps: DEFAULT_LIQ_BOUNTY_BPS as u32,
            tp_sl_execution_fee: 0,
        },
    );

    let keeper_balance_before = f.usdc.balance(&f.keeper);

    let size = 10_000 * USDC_UNIT;
    let collateral = 1_000 * USDC_UNIT;
    f.open_long(&f.trader, size, collateral);

    f.advance_time(TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 1);
    f.set_btc_price(50_000);

    let trader_balance_before_close = f.usdc.balance(&f.trader);

    f.decrease_position(&f.trader, &symbol_short!("BTC"), &size, &0_i128);

    let keeper_balance_after = f.usdc.balance(&f.keeper);
    assert_eq!(
        keeper_balance_after, keeper_balance_before,
        "Keeper must not receive fees when there are zero/minimal fees"
    );

    let trader_returned = f.usdc.balance(&f.trader) - trader_balance_before_close;
    assert!(
        trader_returned >= collateral - USDC_UNIT,
        "Trader should get back ~full collateral with minimal time and zero open fee: returned={}",
        trader_returned
    );
}

// ---------------------------------------------------------------------------
// 7. Revenue fee split: LP slice stays in pool, dev slice accrues
// ---------------------------------------------------------------------------

/// Verifies the LP vs dev split on the open-fee revenue stream: with FeeSplits
/// {9000/1000/0}, `vault.unclaimed_fees` grows by exactly `dev_bps + staker_bps`
/// of the open fee, and `total_assets - unclaimed_fees` grows by the LP slice.
/// Isolates the open-fee split from close-time funding-no-counterparty drift.
#[test]
fn test_fee_split_bps_precision() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let total_assets_before = f.vault.total_assets();

    let size = 100_000 * USDC_UNIT;
    let collateral = 10_000 * USDC_UNIT;
    f.open_long(&f.trader, size, collateral);

    // Drain the unclaimed_fees produced by the open fee. After the claim,
    // `total_assets - total_assets_before` is the LP residue from the open fee
    // alone (no close-time fees have fired and no PnL has settled).
    let dev_recipient = Address::generate(&env);
    f.claim_fees(&f.admin, &dev_recipient);
    let dev_claimed = f.usdc.balance(&dev_recipient);

    assert!(
        dev_claimed > 0,
        "Dev share of open fee must be positive"
    );

    // Open fee = size * open_fee_bps / BPS. With defaults (10 bps) and 100k
    // size that's 100 USDC. Dev gets 10%, LP gets 90%.
    let expected_open_fee = size * DEFAULT_OPEN_FEE_BPS / BPS;
    let splits = f.config_manager.get_fee_splits();
    let non_lp_bps = (splits.dev_bps + splits.staker_bps) as i128;
    let expected_dev = expected_open_fee * non_lp_bps / BPS;
    let expected_lp = expected_open_fee - expected_dev;

    assert_eq!(
        dev_claimed, expected_dev,
        "Dev claim must equal open_fee * (dev_bps + staker_bps) / BPS"
    );

    let total_assets_after = f.vault.total_assets();
    let lp_residue_in_vault = total_assets_after - total_assets_before;
    assert_eq!(
        lp_residue_in_vault, expected_lp,
        "LP residue must equal open_fee * lp_bps / BPS"
    );

    // Sanity: with default 9000/1000/0, LP slice is exactly 9x dev slice.
    let expected_ratio = (splits.lp_bps as i128) / non_lp_bps;
    assert_eq!(
        lp_residue_in_vault, dev_claimed * expected_ratio,
        "LP residue must be exactly {}x dev share under default FeeSplits",
        expected_ratio
    );
}

// ---------------------------------------------------------------------------
// 8. ADVERSARIAL: Multiple closes accumulate dev fees correctly
// ---------------------------------------------------------------------------

#[test]
fn test_multiple_closes_accumulate_dev_fees() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let trader1 = f.create_funded_trader(10_000 * USDC_UNIT);
    f.open_long(&trader1, 20_000 * USDC_UNIT, 2_000 * USDC_UNIT);

    f.advance_time(TEST_TIMESTAMP + 3_600);
    f.set_btc_price(50_000);
    f.update_indices(&f.keeper, &symbol_short!("BTC"));
    f.decrease_position(&trader1, &symbol_short!("BTC"), &(20_000 * USDC_UNIT), &0_i128);

    let trader2 = f.create_funded_trader(10_000 * USDC_UNIT);
    f.open_long(&trader2, 20_000 * USDC_UNIT, 2_000 * USDC_UNIT);

    f.advance_time(TEST_TIMESTAMP + 7_200);
    f.set_btc_price(50_000);
    f.update_indices(&f.keeper, &symbol_short!("BTC"));
    f.decrease_position(&trader2, &symbol_short!("BTC"), &(20_000 * USDC_UNIT), &0_i128);

    let recipient = Address::generate(&env);
    f.claim_fees(&f.admin, &recipient);
    let total_dev_claimed = f.usdc.balance(&recipient);

    assert!(
        total_dev_claimed > 0,
        "Accumulated dev fees from two closes must be positive: claimed={}",
        total_dev_claimed
    );
}

// ---------------------------------------------------------------------------
// 9. ADVERSARIAL: Non-admin cannot claim dev fees
// ---------------------------------------------------------------------------

#[test]
fn test_non_admin_cannot_claim_dev_fees() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    f.open_long(&f.trader, 20_000 * USDC_UNIT, 2_000 * USDC_UNIT);
    f.advance_time(TEST_TIMESTAMP + 3_600 + MIN_POSITION_LIFETIME + 1);
    f.set_btc_price(50_000);
    f.update_indices(&f.keeper, &symbol_short!("BTC"));
    f.decrease_position(&f.trader, &symbol_short!("BTC"), &(20_000 * USDC_UNIT), &0_i128);

    let random = Address::generate(&env);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        f.claim_fees(&random, &random);
    }));
    assert!(
        result.is_err(),
        "Non-admin must be rejected from claiming dev fees"
    );
}

// ---------------------------------------------------------------------------
// 10. ADVERSARIAL: LP share stays in vault pool, not in unclaimed_fees
// ---------------------------------------------------------------------------

#[test]
fn test_lp_share_stays_in_vault_pool() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let total_assets_before = f.vault.total_assets();

    let size = 50_000 * USDC_UNIT;
    let collateral = 5_000 * USDC_UNIT;
    f.open_long(&f.trader, size, collateral);

    f.advance_time(TEST_TIMESTAMP + 86_400);
    f.set_btc_price(50_000);
    f.update_indices(&f.keeper, &symbol_short!("BTC"));
    f.decrease_position(&f.trader, &symbol_short!("BTC"), &size, &0_i128);

    let recipient = Address::generate(&env);
    f.claim_fees(&f.admin, &recipient);
    let dev_claimed = f.usdc.balance(&recipient);

    // FeeSplits has no keeper field — only lp / dev / staker.
    let splits = f.config_manager.get_fee_splits();
    let lp_bps = splits.lp_bps as i128;
    let non_lp_bps = (splits.dev_bps + splits.staker_bps) as i128;

    // After draining the dev slice, vault.total_assets grew exactly by the
    // LP residue (PnL was 0 on this close so no payouts moved capital out).
    let total_assets_after = f.vault.total_assets();
    let lp_residue_in_vault = total_assets_after - total_assets_before;

    // With defaults (9000/1000/0) the LP residue is 9x the dev share.
    let expected_ratio = lp_bps / non_lp_bps;
    assert!(
        lp_residue_in_vault >= dev_claimed * expected_ratio - USDC_UNIT,
        "LP residue ({}) must be ~{}x dev share ({}) under default FeeSplits",
        lp_residue_in_vault,
        expected_ratio,
        dev_claimed
    );
    assert!(
        lp_residue_in_vault > 0,
        "LP residue must be positive — open fee and close fees both have an LP slice"
    );
}

// ---------------------------------------------------------------------------
// 11. End-to-end: Liquidation bounty zero-bps edge case
// ---------------------------------------------------------------------------

/// Admin disables liquidation bounty (bps=0). Liquidator gets nothing from
/// the bounty path even though pm_to_vault is non-zero — bounty is gated on
/// bps alone, not on availability.
#[test]
fn test_liquidation_bounty_zero_bps_pays_nothing() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    f.config_manager.set_fee_config(
        &f.admin,
        &FeeConfig {
            open_fee_bps: DEFAULT_OPEN_FEE_BPS as u32,
            liquidation_bounty_bps: 0,
            tp_sl_execution_fee: DEFAULT_TP_SL_FEE,
        },
    );

    let liquidator = Address::generate(&env);
    let liq_balance_before = f.usdc.balance(&liquidator);

    f.open_long(&f.trader, 20_000 * USDC_UNIT, 2_000 * USDC_UNIT);

    f.advance_time(TEST_TIMESTAMP + 3_600);
    f.set_btc_price(44_000);

    f.liquidate(&liquidator, &f.trader, &symbol_short!("BTC"));

    let liq_balance_after = f.usdc.balance(&liquidator);
    assert_eq!(
        liq_balance_after, liq_balance_before,
        "Zero bounty_bps must produce zero payout to liquidator regardless of pm_to_vault"
    );
}

// ---------------------------------------------------------------------------
// 12. End-to-end: TP/SL escrow refunded on full UserClose
// ---------------------------------------------------------------------------

/// Position opened with TP set -> escrow paid trader -> PM at open. Full
/// UserClose refunds the escrow back to the trader.
#[test]
fn test_tp_sl_escrow_refunded_on_full_user_close() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let size = 50_000 * USDC_UNIT;
    let collateral = 5_000 * USDC_UNIT;
    let tp_price: i128 = 55_000 * PRECISION;

    let trader_balance_before_open = f.usdc.balance(&f.trader);

    f.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &size,
        &collateral,
        &true,
        &tp_price,
        &0,
        &0i128,
    );

    // After open, trader has paid: collateral + open_fee + escrow.
    let trader_after_open = f.usdc.balance(&f.trader);
    let paid_at_open = trader_balance_before_open - trader_after_open;
    let open_fee = size * DEFAULT_OPEN_FEE_BPS / BPS;
    let expected_paid = collateral + open_fee + DEFAULT_TP_SL_FEE;
    assert_eq!(
        paid_at_open, expected_paid,
        "Open with TP must charge collateral + open_fee + tp_sl_execution_fee"
    );

    // Close at flat price (PnL=0) after min_lifetime.
    f.advance_time(TEST_TIMESTAMP + MIN_POSITION_LIFETIME + 10);
    f.set_btc_price(50_000);
    f.decrease_position(&f.trader, &symbol_short!("BTC"), &size, &0_i128);

    // Trader must receive: collateral (minus minor borrow fee for 70s) + escrow refund.
    let trader_after_close = f.usdc.balance(&f.trader);
    let net_paid = trader_balance_before_open - trader_after_close;
    // Net cost = open_fee + borrow_fee. Escrow was fully refunded.
    assert!(
        net_paid >= open_fee,
        "Net cost must be at least the open fee: net_paid={}, open_fee={}",
        net_paid,
        open_fee
    );
    // Generous upper bound — short close should not drain anywhere near the escrow.
    assert!(
        net_paid < open_fee + DEFAULT_TP_SL_FEE,
        "Trader must keep the refunded escrow: net_paid={}, open_fee+escrow={}",
        net_paid,
        open_fee + DEFAULT_TP_SL_FEE
    );
}

// ---------------------------------------------------------------------------
// 13. End-to-end: TP/SL escrow forfeited on liquidation
// ---------------------------------------------------------------------------

#[test]
fn test_tp_sl_escrow_forfeited_on_liquidation() {
    let env = Env::default();
    let f = Fixture::deploy(&env);

    let size = 20_000 * USDC_UNIT;
    let collateral = 2_000 * USDC_UNIT;
    let tp_price: i128 = 60_000 * PRECISION;

    f.increase_position(
        &f.trader,
        &symbol_short!("BTC"),
        &size,
        &collateral,
        &true,
        &tp_price,
        &0,
        &0i128,
    );

    let pos = f
        .position_manager
        .get_position(&f.trader, &symbol_short!("BTC"));
    assert_eq!(
        pos.execution_fee_escrow, DEFAULT_TP_SL_FEE,
        "Setup: position must record escrow before liquidation"
    );

    let trader_balance_pre_crash = f.usdc.balance(&f.trader);

    let liquidator = Address::generate(&env);
    let liq_balance_before = f.usdc.balance(&liquidator);

    f.advance_time(TEST_TIMESTAMP + 3_600);
    f.set_btc_price(44_000); // crash

    f.liquidate(&liquidator, &f.trader, &symbol_short!("BTC"));

    // Trader gets NO escrow refund on liquidation.
    let trader_balance_post = f.usdc.balance(&f.trader);
    assert_eq!(
        trader_balance_post, trader_balance_pre_crash,
        "Trader must not be refunded the escrow on liquidation"
    );

    // Liquidator receives only the bounty (NOT the escrow).
    let liq_received = f.usdc.balance(&liquidator) - liq_balance_before;
    let expected_bounty = collateral * DEFAULT_LIQ_BOUNTY_BPS / BPS;
    assert_eq!(
        liq_received, expected_bounty,
        "Liquidator must receive only the bounty, not the forfeited escrow: \
         got {}, expected bounty {}",
        liq_received, expected_bounty
    );
}
