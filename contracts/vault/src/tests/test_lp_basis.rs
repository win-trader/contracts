#![cfg(test)]

//! Tests for LP-basis share pricing, lockup-propagation limits, the
//! `pay_profit` liquidity gate, PnL sync freshness, and fee-accrual clamping.
//!
//! Share conversions price against `lp_total_assets = raw - unclaimed_fees -
//! max(0, net_pnl)`, so a withdrawing LP can only ever extract their pro-rata
//! slice of LP equity — never the fee buffer or the trader-PnL backing.

use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
    Address, Env, String,
};

const DECIMALS: u32 = 7;
const ONE_USDC: i128 = 10_000_000;

struct TestFixture {
    env: Env,
    admin: Address,
    token_client: mock_token::MockTokenClient<'static>,
    config_client: config_manager::ConfigManagerClient<'static>,
    vault_id: Address,
    vault_client: crate::VaultContractClient<'static>,
    position_manager: Address,
}

fn setup() -> TestFixture {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let position_manager = Address::generate(&env);

    let token_id = env.register(mock_token::MockToken, ());
    let token_client = mock_token::MockTokenClient::new(&env, &token_id);
    token_client.initialize(
        &admin,
        &DECIMALS,
        &String::from_str(&env, "USD Coin"),
        &String::from_str(&env, "USDC"),
    );

    let config_id = env.register(config_manager::ConfigManagerContract, (admin.clone(),));
    let config_client = config_manager::ConfigManagerClient::new(&env, &config_id);
    set_cooldown(&env, &config_client, &admin, 0);

    let vault_id = env.register(
        crate::VaultContract,
        (token_id.clone(), config_id.clone(), position_manager.clone()),
    );
    let vault_client = crate::VaultContractClient::new(&env, &vault_id);

    // SAFETY: transmute lifetimes -- fixture owns the Env so clients remain valid.
    let token_client = unsafe { core::mem::transmute(token_client) };
    let config_client = unsafe { core::mem::transmute(config_client) };
    let vault_client = unsafe { core::mem::transmute(vault_client) };

    TestFixture {
        env,
        admin,
        token_client,
        config_client,
        vault_id,
        vault_client,
        position_manager,
    }
}

fn set_cooldown(
    env: &Env,
    config_client: &config_manager::ConfigManagerClient,
    admin: &Address,
    cooldown_duration: u64,
) {
    let _ = env;
    config_client.update_protocol_limits(
        admin,
        &config_manager::ProtocolLimits {
            min_collateral: 1,
            cooldown_duration,
            min_position_lifetime: 0,
            max_utilization_ratio: 10_000,
            funding_cut_bps: 0,
            adl_pnl_bps: 9_000,
            adl_utilization_bps: 9_500,
            liquidation_threshold_bps: 200,
        },
    );
}

fn set_ts(fix: &TestFixture, timestamp: u64) {
    fix.env.ledger().set(LedgerInfo {
        timestamp,
        protocol_version: 23,
        sequence_number: 100,
        network_id: [0u8; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 100,
        min_persistent_entry_ttl: 100,
        max_entry_ttl: 10_000_000,
    });
}

fn deposit_usdc(fix: &TestFixture, addr: &Address, amount: i128) -> i128 {
    fix.token_client.mint(addr, &amount);
    fix.vault_client.deposit(&amount, addr, addr, addr)
}

/// Simulate PM pushing fee revenue: mint USDC straight into the vault's
/// wallet, then book it via `accrue_fees` (mirrors PM's `recv_revenue`).
fn push_fees(fix: &TestFixture, amount: i128) {
    fix.token_client.mint(&fix.vault_id, &amount);
    fix.vault_client.accrue_fees(&fix.position_manager, &amount);
}

// ===========================================================================
// LP-basis pricing: the fee buffer cannot be skimmed by the first withdrawer
// ===========================================================================

#[test]
fn test_withdraw_cannot_skim_fee_buffer() {
    let fix = setup();
    let lp_a = Address::generate(&fix.env);
    let lp_b = Address::generate(&fix.env);

    let stake = 1_000 * ONE_USDC;
    deposit_usdc(&fix, &lp_a, stake);
    deposit_usdc(&fix, &lp_b, stake);

    // 600 USDC of revenue arrives; the dev+staker slice is booked as
    // unclaimed fees and is NOT LP equity.
    push_fees(&fix, 600 * ONE_USDC);

    // A's claim must stay at their 1000 USDC of LP equity (± conversion
    // dust in the vault's favor), not 1300 at the raw-balance rate.
    let max_w = fix.vault_client.max_withdraw(&lp_a);
    assert!(
        max_w <= stake,
        "max_withdraw must not exceed the LP's equity: got {max_w}, stake {stake}"
    );
    assert!(
        max_w >= stake - 10,
        "max_withdraw must be within dust of the LP's equity: got {max_w}"
    );

    // Withdrawing at the old inflated rate must fail outright.
    let skim = fix
        .vault_client
        .try_withdraw(&(1_300 * ONE_USDC), &lp_a, &lp_a, &lp_a);
    assert!(skim.is_err(), "withdrawing above LP equity must fail");

    // A exits in full at the fair rate.
    fix.vault_client.withdraw(&max_w, &lp_a, &lp_a, &lp_a);

    // The fee buffer is intact and claimable in full.
    assert_eq!(fix.vault_client.unclaimed_fees(), 600 * ONE_USDC);
    let recipient = Address::generate(&fix.env);
    fix.vault_client.claim_fees(&fix.admin, &recipient);
    assert_eq!(fix.token_client.balance(&recipient), 600 * ONE_USDC);

    // B is unaffected: their full stake is still withdrawable (± dust).
    let max_b = fix.vault_client.max_withdraw(&lp_b);
    assert!(
        max_b >= stake - 10,
        "remaining LP must keep their full equity: got {max_b}, stake {stake}"
    );
}

#[test]
fn test_share_price_excludes_positive_trader_pnl() {
    let fix = setup();
    let lp = Address::generate(&fix.env);

    let stake = 1_000 * ONE_USDC;
    deposit_usdc(&fix, &lp, stake);

    // Traders are up 400: that liability is excluded from the share price,
    // so a new deposit of 600 buys shares at the same 1:1 LP rate as if the
    // PnL buffer did not exist.
    fix.vault_client
        .update_net_pnl(&fix.position_manager, &(400 * ONE_USDC));

    // LP equity is total (1000) minus the PnL liability (400) = 600.
    let max_w = fix.vault_client.max_withdraw(&lp);
    assert!(
        max_w <= 600 * ONE_USDC && max_w >= 600 * ONE_USDC - 10,
        "withdrawable equity must exclude the trader-PnL backing: got {max_w}"
    );

    let lp2 = Address::generate(&fix.env);
    let shares_lp1 = fix.vault_client.balance(&lp);
    let deposit2 = 600 * ONE_USDC;
    let shares_lp2 = deposit_usdc(&fix, &lp2, deposit2);
    // lp1 holds 1000 staked at equity 600; lp2's 600 must buy equity
    // pro-rata: shares2 / shares1 == 600 / 600 == 1 (± conversion dust).
    let ratio_num = shares_lp2 * 1_000_000;
    let ratio_den = shares_lp1;
    let ratio = ratio_num / ratio_den;
    assert!(
        (999_000..=1_001_000).contains(&ratio),
        "new deposit must be priced on LP equity, got share ratio {ratio} ppm"
    );
}

// ===========================================================================
// Lockup propagation: dust transfers cannot extend an existing holder's lock
// ===========================================================================

#[test]
fn test_dust_transfer_does_not_extend_existing_holder_lockup() {
    let fix = setup();
    set_cooldown(&fix.env, &fix.config_client, &fix.admin, 300);

    let victim = Address::generate(&fix.env);
    let attacker = Address::generate(&fix.env);

    set_ts(&fix, 1_000);
    deposit_usdc(&fix, &victim, 100 * ONE_USDC);
    assert_eq!(fix.vault_client.lockup_expires_at(&victim), 1_300);

    // Victim's lockup elapses.
    set_ts(&fix, 1_301);

    // Attacker deposits (fresh lockup until 1601) and dusts the victim.
    deposit_usdc(&fix, &attacker, ONE_USDC);
    fix.vault_client.transfer(&attacker, &victim, &1);

    // The victim's expiry is untouched and they can withdraw immediately.
    assert_eq!(
        fix.vault_client.lockup_expires_at(&victim),
        1_300,
        "an inbound dust transfer must not extend an existing holder's lockup"
    );
    let max_w = fix.vault_client.max_withdraw(&victim);
    fix.vault_client.withdraw(&max_w, &victim, &victim, &victim);
}

#[test]
fn test_transfer_to_fresh_address_still_inherits_lockup() {
    let fix = setup();
    set_cooldown(&fix.env, &fix.config_client, &fix.admin, 300);

    let depositor = Address::generate(&fix.env);
    let fresh = Address::generate(&fix.env);

    set_ts(&fix, 1_000);
    let shares = deposit_usdc(&fix, &depositor, 100 * ONE_USDC);

    // Shifting shares to a fresh address must not shed the cooldown.
    fix.vault_client.transfer(&depositor, &fresh, &shares);
    assert_eq!(
        fix.vault_client.lockup_expires_at(&fresh),
        1_300,
        "a fresh recipient must inherit the sender's lockup"
    );
    let attempt = fix
        .vault_client
        .try_withdraw(&ONE_USDC, &fresh, &fresh, &fresh);
    assert!(attempt.is_err(), "fresh recipient must observe the cooldown");
}

// ===========================================================================
// pay_profit gate: the payout extinguishes its own share of the liability
// ===========================================================================

#[test]
fn test_pay_profit_fully_backed_winner_succeeds() {
    let fix = setup();
    let lp = Address::generate(&fix.env);
    let trader = Address::generate(&fix.env);

    deposit_usdc(&fix, &lp, 1_000 * ONE_USDC);
    fix.vault_client
        .update_net_pnl(&fix.position_manager, &(600 * ONE_USDC));

    // total=1000, net_pnl=600: the 600 payout is backed (1000 >= 600) and
    // must not be double-counted against the still-synced liability.
    fix.vault_client
        .pay_profit(&fix.position_manager, &trader, &(600 * ONE_USDC));
    assert_eq!(fix.token_client.balance(&trader), 600 * ONE_USDC);
}

// ===========================================================================
// PnL sync freshness on the LP exit path
// ===========================================================================

#[test]
fn test_withdraw_with_stale_pnl_sync_panics_while_reserved() {
    let fix = setup();
    let lp = Address::generate(&fix.env);

    set_ts(&fix, 1_000);
    deposit_usdc(&fix, &lp, 1_000 * ONE_USDC);
    fix.vault_client
        .reserve_liquidity(&fix.position_manager, &(100 * ONE_USDC));
    fix.vault_client
        .update_net_pnl_full_sync(&fix.position_manager, &0);

    // Just inside the freshness window: allowed.
    set_ts(&fix, 1_000 + crate::logic::PNL_SYNC_MAX_AGE_SECS);
    fix.vault_client.withdraw(&(10 * ONE_USDC), &lp, &lp, &lp);

    // Beyond the window: the synced PnL can no longer be trusted.
    set_ts(&fix, 1_001 + crate::logic::PNL_SYNC_MAX_AGE_SECS);
    let stale = fix
        .vault_client
        .try_withdraw(&(10 * ONE_USDC), &lp, &lp, &lp);
    assert!(stale.is_err(), "stale PnL sync must block withdraw");

    // A fresh full-book sync unblocks the exit.
    fix.vault_client
        .update_net_pnl_full_sync(&fix.position_manager, &0);
    fix.vault_client.withdraw(&(10 * ONE_USDC), &lp, &lp, &lp);
}

#[test]
fn test_partial_pnl_update_does_not_refresh_lp_exit_freshness() {
    let fix = setup();
    let lp = Address::generate(&fix.env);

    set_ts(&fix, 1_000);
    deposit_usdc(&fix, &lp, 1_000 * ONE_USDC);
    fix.vault_client
        .reserve_liquidity(&fix.position_manager, &(100 * ONE_USDC));
    fix.vault_client
        .update_net_pnl_full_sync(&fix.position_manager, &0);

    set_ts(&fix, 1_001 + crate::logic::PNL_SYNC_MAX_AGE_SECS);

    // A partial, single-market update changes the amount but must not make
    // the whole-book freshness gate pass.
    fix.vault_client.update_net_pnl(&fix.position_manager, &0);
    let stale = fix
        .vault_client
        .try_withdraw(&(10 * ONE_USDC), &lp, &lp, &lp);
    assert!(
        stale.is_err(),
        "partial PnL update must not refresh LP-exit freshness"
    );

    fix.vault_client
        .update_net_pnl_full_sync(&fix.position_manager, &0);
    fix.vault_client.withdraw(&(10 * ONE_USDC), &lp, &lp, &lp);
}

#[test]
fn test_withdraw_with_stale_sync_but_nothing_reserved_succeeds() {
    let fix = setup();
    let lp = Address::generate(&fix.env);

    set_ts(&fix, 1_000);
    deposit_usdc(&fix, &lp, 1_000 * ONE_USDC);

    // No reserved liquidity → no open positions → freshness is irrelevant.
    set_ts(&fix, 1_000_000);
    fix.vault_client.withdraw(&(500 * ONE_USDC), &lp, &lp, &lp);
}

// ===========================================================================
// accrue_fees clamps to headroom instead of reverting
// ===========================================================================

#[test]
fn test_accrue_fees_clamps_to_headroom() {
    let fix = setup();
    let lp = Address::generate(&fix.env);

    deposit_usdc(&fix, &lp, 100 * ONE_USDC);
    fix.vault_client
        .reserve_liquidity(&fix.position_manager, &(90 * ONE_USDC));

    // Headroom is 10; booking 50 must clamp, not revert (this call sits
    // inside PM's liquidation path).
    fix.vault_client
        .accrue_fees(&fix.position_manager, &(50 * ONE_USDC));
    assert_eq!(
        fix.vault_client.unclaimed_fees(),
        10 * ONE_USDC,
        "accrual must clamp to total - reserved - unclaimed"
    );

    // Already at the cap: a further accrual books zero.
    fix.vault_client
        .accrue_fees(&fix.position_manager, &ONE_USDC);
    assert_eq!(fix.vault_client.unclaimed_fees(), 10 * ONE_USDC);
}

// ===========================================================================
// reserve_liquidity upholds reserved + unclaimed <= total
// ===========================================================================

#[test]
fn test_reserve_cannot_overlap_fee_buffer() {
    let fix = setup();
    let lp = Address::generate(&fix.env);

    deposit_usdc(&fix, &lp, 100 * ONE_USDC);
    push_fees(&fix, 20 * ONE_USDC);

    // total=120, unclaimed=20: reserving 110 would overlap the fee buffer.
    let too_much = fix
        .vault_client
        .try_reserve_liquidity(&fix.position_manager, &(110 * ONE_USDC));
    assert!(too_much.is_err(), "reserve must not overlap unclaimed fees");

    // Exactly the LP capital (100) is fine.
    fix.vault_client
        .reserve_liquidity(&fix.position_manager, &(100 * ONE_USDC));
}
