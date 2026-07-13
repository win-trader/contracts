#![cfg(test)]

//! Comprehensive tests for the vault contract's **withdraw** and **redeem**
//! functionality, including free-liquidity constraints, pause guards, and
//! adversarial edge cases.
//!
//! These tests are written in TDD style -- they define the *expected* behavior
//! and will fail against incomplete or buggy implementations.

use soroban_sdk::{testutils::Address as _, Address, Env, String, Symbol};

// ---------------------------------------------------------------------------
// Constants -- 7-decimal USDC (Stellar standard)
// ---------------------------------------------------------------------------

const DECIMALS: u32 = 7;
const ONE_USDC: i128 = 10_000_000; // 1.0000000

// ---------------------------------------------------------------------------
// Test fixture
// ---------------------------------------------------------------------------

struct TestFixture {
    env: Env,
    admin: Address,
    // token_id: Address,
    token_client: mock_token::MockTokenClient<'static>,
    // config_id: Address,
    config_client: config_manager::ConfigManagerClient<'static>,
    // vault_id: Address,
    vault_client: crate::VaultContractClient<'static>,
    position_manager: Address,
}

/// Deploy mock-token, config-manager, and vault. Initialize all three.
fn setup() -> TestFixture {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let position_manager = Address::generate(&env);

    // --- mock USDC token ---
    let token_id = env.register(mock_token::MockToken, ());
    let token_client = mock_token::MockTokenClient::new(&env, &token_id);
    token_client.initialize(
        &admin,
        &DECIMALS,
        &String::from_str(&env, "USD Coin"),
        &String::from_str(&env, "USDC"),
    );

    // --- config manager ---
    let config_id = env.register(config_manager::ConfigManagerContract, (admin.clone(),));
    let config_client = config_manager::ConfigManagerClient::new(&env, &config_id);

    // Set protocol limits with zero cooldown so existing tests pass
    config_client.update_protocol_limits(
        &admin,
        &config_manager::ProtocolLimits {
            min_collateral: 1,
            cooldown_duration: 0,
            min_position_lifetime: 0,
            max_utilization_ratio: 10_000,
            adl_pnl_bps: 9_000,
            adl_utilization_bps: 9_500,
            liquidation_threshold_bps: 200,
        },
    );

    config_client.update_carrying_fee_config(
        &admin,
        &config_manager::CarryingFeeConfig {
            base_borrow_rate_bps: 100,
            slope1_bps: 500,
            slope2_bps: 5_000,
            optimal_utilization_bps: 8_000,
            max_skew_rate_bps: 100,
        },
    );

    // --- vault (constructor binds asset, config_manager, position_manager) ---
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
        // token_id,
        token_client,
        // config_id,
        config_client,
        // vault_id,
        vault_client,
        position_manager,
    }
}

/// Mint USDC to `addr` and deposit into the vault. Returns the shares minted.
fn deposit_usdc(fix: &TestFixture, addr: &Address, amount: i128) -> i128 {
    fix.token_client.mint(addr, &amount);
    fix.vault_client.deposit(&amount, addr, addr, addr)
}

/// Grant PAUSER role to `pauser` via the config manager.
fn grant_pauser(fix: &TestFixture, pauser: &Address) {
    let role = Symbol::new(&fix.env, shared::constants::ROLE_PAUSER);
    fix.config_client.grant_role(&fix.admin, &role, pauser);
}

// ===========================================================================
// 1. test_withdraw_success
//    Deposit 100 USDC, withdraw 50 USDC. Verify shares burned, USDC returned,
//    total_assets decreases.
// ===========================================================================

#[test]
fn test_withdraw_success() {
    let fix = setup();
    let user = Address::generate(&fix.env);

    let deposit_amount = 100 * ONE_USDC;
    let shares = deposit_usdc(&fix, &user, deposit_amount);
    assert!(shares > 0, "deposit must return positive shares");

    // Snapshot before withdraw
    let total_assets_before = fix.vault_client.total_assets();
    let user_token_before = fix.token_client.balance(&user);

    let withdraw_amount = 50 * ONE_USDC;
    let shares_burned = fix
        .vault_client
        .withdraw(&withdraw_amount, &user, &user, &user);

    assert!(
        shares_burned > 0,
        "withdraw must burn a positive number of shares"
    );

    // User USDC balance must increase by withdraw_amount
    let user_token_after = fix.token_client.balance(&user);
    assert_eq!(
        user_token_after - user_token_before,
        withdraw_amount,
        "user must receive exactly the requested USDC amount"
    );

    // total_assets must decrease by withdraw_amount
    let total_assets_after = fix.vault_client.total_assets();
    assert_eq!(
        total_assets_before - total_assets_after,
        withdraw_amount,
        "total_assets must decrease by the withdrawn amount"
    );

    // Vault share balance must decrease
    let remaining_shares = fix.vault_client.balance(&user);
    assert_eq!(
        remaining_shares,
        shares - shares_burned,
        "user share balance must decrease by shares_burned"
    );
}

// ===========================================================================
// 2. test_withdraw_full_amount
//    Deposit then withdraw everything. Balance and total_supply reach 0.
// ===========================================================================

#[test]
fn test_withdraw_full_amount() {
    let fix = setup();
    let user = Address::generate(&fix.env);

    let deposit_amount = 100 * ONE_USDC;
    let _shares = deposit_usdc(&fix, &user, deposit_amount);

    // Withdraw the full deposited amount
    fix.vault_client
        .withdraw(&deposit_amount, &user, &user, &user);

    assert_eq!(
        fix.vault_client.balance(&user),
        0,
        "user share balance must be 0 after full withdrawal"
    );
    assert_eq!(
        fix.vault_client.total_assets(),
        0,
        "total_assets must be 0 after full withdrawal"
    );
    assert_eq!(
        fix.vault_client.total_supply(),
        0,
        "total_supply must be 0 when all shares are burned"
    );
    assert_eq!(
        fix.token_client.balance(&user),
        deposit_amount,
        "user must hold all their USDC back after full withdrawal"
    );
}

// ===========================================================================
// 3. test_redeem_success
//    Deposit, then redeem half the shares. Verify assets returned correctly.
// ===========================================================================

#[test]
fn test_redeem_success() {
    let fix = setup();
    let user = Address::generate(&fix.env);

    let deposit_amount = 100 * ONE_USDC;
    let shares = deposit_usdc(&fix, &user, deposit_amount);

    let shares_to_redeem = shares / 2;
    let user_token_before = fix.token_client.balance(&user);

    let assets_returned = fix
        .vault_client
        .redeem(&shares_to_redeem, &user, &user, &user);

    assert!(
        assets_returned > 0,
        "redeem must return a positive asset amount"
    );

    // At 1:1 ratio (no fee, no yield), half the shares should return half the assets
    assert_eq!(
        assets_returned,
        deposit_amount / 2,
        "redeeming half the shares must return half the deposited assets (1:1 ratio)"
    );

    let user_token_after = fix.token_client.balance(&user);
    assert_eq!(
        user_token_after - user_token_before,
        assets_returned,
        "user USDC balance must increase by assets_returned"
    );

    // Share balance must decrease
    assert_eq!(
        fix.vault_client.balance(&user),
        shares - shares_to_redeem,
        "user share balance must decrease by shares_to_redeem"
    );
}

// ===========================================================================
// 4. test_withdraw_exceeds_free_liquidity_reverts
//    Deposit 100 USDC, reserve 80, try withdraw 50 (only 20 free).
//    Must panic with InsufficientFreeLiquidity (= 4).
// ===========================================================================

#[test]
#[should_panic(expected = "Error(Contract, #4)")]
fn test_withdraw_exceeds_free_liquidity_reverts() {
    let fix = setup();
    let user = Address::generate(&fix.env);

    deposit_usdc(&fix, &user, 100 * ONE_USDC);

    // Reserve 80 USDC -- only 20 remain free
    fix.vault_client
        .reserve_liquidity(&fix.position_manager, &(80 * ONE_USDC));

    // Attempt to withdraw 50 USDC -- exceeds the 20 USDC free liquidity
    fix.vault_client
        .withdraw(&(50 * ONE_USDC), &user, &user, &user);
}

// ===========================================================================
// 5. test_max_withdraw_respects_free_liquidity
//    Deposit 100, reserve 60. max_withdraw should return 40 (not 100).
// ===========================================================================

#[test]
fn test_max_withdraw_respects_free_liquidity() {
    let fix = setup();
    let user = Address::generate(&fix.env);

    deposit_usdc(&fix, &user, 100 * ONE_USDC);
    fix.vault_client
        .reserve_liquidity(&fix.position_manager, &(60 * ONE_USDC));

    let max_w = fix.vault_client.max_withdraw(&user);
    assert_eq!(
        max_w,
        40 * ONE_USDC,
        "max_withdraw must return free_liquidity (40 USDC) when it is less than user assets"
    );
}

// ===========================================================================
// 6. test_max_withdraw_when_paused_returns_zero
// ===========================================================================

#[test]
fn test_max_withdraw_when_paused_returns_zero() {
    let fix = setup();
    let user = Address::generate(&fix.env);
    let pauser = Address::generate(&fix.env);
    grant_pauser(&fix, &pauser);

    deposit_usdc(&fix, &user, 100 * ONE_USDC);

    fix.vault_client.pause(&pauser);

    assert_eq!(
        fix.vault_client.max_withdraw(&user),
        0,
        "max_withdraw must return 0 when the vault is paused"
    );
}

// ===========================================================================
// 7. test_max_redeem_when_paused_returns_zero
// ===========================================================================

#[test]
fn test_max_redeem_when_paused_returns_zero() {
    let fix = setup();
    let user = Address::generate(&fix.env);
    let pauser = Address::generate(&fix.env);
    grant_pauser(&fix, &pauser);

    deposit_usdc(&fix, &user, 100 * ONE_USDC);

    fix.vault_client.pause(&pauser);

    assert_eq!(
        fix.vault_client.max_redeem(&user),
        0,
        "max_redeem must return 0 when the vault is paused"
    );
}

// ===========================================================================
// 8. test_withdraw_when_paused_reverts
//    Pause vault, try to withdraw. Must panic with Paused (= 3).
// ===========================================================================

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn test_withdraw_when_paused_reverts() {
    let fix = setup();
    let user = Address::generate(&fix.env);
    let pauser = Address::generate(&fix.env);
    grant_pauser(&fix, &pauser);

    deposit_usdc(&fix, &user, 100 * ONE_USDC);

    fix.vault_client.pause(&pauser);

    // Withdraw while paused -- must panic with VaultError::Paused = 3
    fix.vault_client
        .withdraw(&(50 * ONE_USDC), &user, &user, &user);
}

// ===========================================================================
// 9. test_redeem_when_paused_reverts
//    Pause vault, try to redeem. Must panic with Paused (= 3).
// ===========================================================================

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn test_redeem_when_paused_reverts() {
    let fix = setup();
    let user = Address::generate(&fix.env);
    let pauser = Address::generate(&fix.env);
    grant_pauser(&fix, &pauser);

    let shares = deposit_usdc(&fix, &user, 100 * ONE_USDC);

    fix.vault_client.pause(&pauser);

    // Redeem while paused -- must panic with VaultError::Paused = 3
    fix.vault_client.redeem(&shares, &user, &user, &user);
}

// ===========================================================================
// 10. test_withdraw_after_release_liquidity
//     Deposit 100, reserve 80, release 30 (50 reserved). Now can withdraw
//     up to 50 USDC.
// ===========================================================================

#[test]
fn test_withdraw_after_release_liquidity() {
    let fix = setup();
    let user = Address::generate(&fix.env);

    deposit_usdc(&fix, &user, 100 * ONE_USDC);

    fix.vault_client
        .reserve_liquidity(&fix.position_manager, &(80 * ONE_USDC));
    // Free = 20

    fix.vault_client
        .release_liquidity(&fix.position_manager, &(30 * ONE_USDC));
    // Reserved now 50, free = 50

    let free = fix.vault_client.free_liquidity();
    assert_eq!(
        free,
        50 * ONE_USDC,
        "free_liquidity must be 50 USDC after reserving 80 and releasing 30"
    );

    // Withdraw exactly the free amount -- must succeed
    let user_token_before = fix.token_client.balance(&user);
    fix.vault_client
        .withdraw(&(50 * ONE_USDC), &user, &user, &user);
    let user_token_after = fix.token_client.balance(&user);

    assert_eq!(
        user_token_after - user_token_before,
        50 * ONE_USDC,
        "user must receive exactly 50 USDC after release_liquidity"
    );
}

// ===========================================================================
// ADVERSARIAL: Additional edge-case and security tests
// ===========================================================================

// ---------------------------------------------------------------------------
// 11. Withdraw zero amount -- should revert or be a no-op
// ---------------------------------------------------------------------------

#[test]
fn test_withdraw_zero_amount() {
    let fix = setup();
    let user = Address::generate(&fix.env);

    deposit_usdc(&fix, &user, 100 * ONE_USDC);

    // Withdrawing zero assets should either revert or return 0 shares burned.
    // The OZ Vault implementation may handle this differently, but zero withdrawal
    // should not alter any state.
    let result = fix.vault_client.try_withdraw(&0, &user, &user, &user);
    if let Ok(Ok(shares_burned)) = result {
        assert_eq!(
            shares_burned, 0,
            "withdrawing zero assets must burn zero shares"
        );
        assert_eq!(
            fix.vault_client.total_assets(),
            100 * ONE_USDC,
            "total_assets must not change on zero withdraw"
        );
    }
    // If it reverts, that is also acceptable behavior for a zero-amount guard.
}

// ---------------------------------------------------------------------------
// 12. Redeem zero shares -- should revert or be a no-op
// ---------------------------------------------------------------------------

#[test]
fn test_redeem_zero_shares() {
    let fix = setup();
    let user = Address::generate(&fix.env);

    deposit_usdc(&fix, &user, 100 * ONE_USDC);

    let result = fix.vault_client.try_redeem(&0, &user, &user, &user);
    if let Ok(Ok(assets_returned)) = result {
        assert_eq!(
            assets_returned, 0,
            "redeeming zero shares must return zero assets"
        );
    }
}

// ---------------------------------------------------------------------------
// 13. Withdraw more than deposited -- must revert (insufficient shares)
// ---------------------------------------------------------------------------

#[test]
#[should_panic]
fn test_withdraw_more_than_deposited_reverts() {
    let fix = setup();
    let user = Address::generate(&fix.env);

    deposit_usdc(&fix, &user, 100 * ONE_USDC);

    // Attempt to withdraw 200 USDC when only 100 is deposited
    fix.vault_client
        .withdraw(&(200 * ONE_USDC), &user, &user, &user);
}

// ---------------------------------------------------------------------------
// 14. Redeem more shares than owned -- must revert
// ---------------------------------------------------------------------------

#[test]
#[should_panic]
fn test_redeem_more_shares_than_owned_reverts() {
    let fix = setup();
    let user = Address::generate(&fix.env);

    let shares = deposit_usdc(&fix, &user, 100 * ONE_USDC);

    // Attempt to redeem 2x the shares the user holds
    fix.vault_client.redeem(&(shares * 2), &user, &user, &user);
}

// ---------------------------------------------------------------------------
// 15. Redeem exceeds free liquidity -- must revert with
//     InsufficientFreeLiquidity (= 4)
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "Error(Contract, #4)")]
fn test_redeem_exceeds_free_liquidity_reverts() {
    let fix = setup();
    let user = Address::generate(&fix.env);

    let shares = deposit_usdc(&fix, &user, 100 * ONE_USDC);

    // Reserve 90 -- only 10 USDC free
    fix.vault_client
        .reserve_liquidity(&fix.position_manager, &(90 * ONE_USDC));

    // Redeem all shares -- needs 100 USDC but only 10 is free
    fix.vault_client.redeem(&shares, &user, &user, &user);
}

// ---------------------------------------------------------------------------
// 16. max_withdraw for user with zero balance returns 0
// ---------------------------------------------------------------------------

#[test]
fn test_max_withdraw_zero_balance_user() {
    let fix = setup();
    let stranger = Address::generate(&fix.env);

    // Stranger has never deposited
    assert_eq!(
        fix.vault_client.max_withdraw(&stranger),
        0,
        "max_withdraw for a user with no shares must be 0"
    );
}

// ---------------------------------------------------------------------------
// 17. max_redeem for user with zero balance returns 0
// ---------------------------------------------------------------------------

#[test]
fn test_max_redeem_zero_balance_user() {
    let fix = setup();
    let stranger = Address::generate(&fix.env);

    assert_eq!(
        fix.vault_client.max_redeem(&stranger),
        0,
        "max_redeem for a user with no shares must be 0"
    );
}

// ---------------------------------------------------------------------------
// 18. max_withdraw capped by user assets even if free_liquidity is higher
// ---------------------------------------------------------------------------

#[test]
fn test_max_withdraw_capped_by_user_assets() {
    let fix = setup();
    let user1 = Address::generate(&fix.env);
    let user2 = Address::generate(&fix.env);

    // user1 deposits 30, user2 deposits 70 => total 100 free
    deposit_usdc(&fix, &user1, 30 * ONE_USDC);
    deposit_usdc(&fix, &user2, 70 * ONE_USDC);

    // Free liquidity is 100, but user1 only has 30 worth of assets
    let max_w = fix.vault_client.max_withdraw(&user1);
    assert_eq!(
        max_w,
        30 * ONE_USDC,
        "max_withdraw must be capped by the user's own deposited assets, not total free liquidity"
    );
}

// ---------------------------------------------------------------------------
// 19. Withdraw and redeem after unpause should succeed
// ---------------------------------------------------------------------------

#[test]
fn test_withdraw_succeeds_after_unpause() {
    let fix = setup();
    let user = Address::generate(&fix.env);
    let pauser = Address::generate(&fix.env);
    grant_pauser(&fix, &pauser);

    deposit_usdc(&fix, &user, 100 * ONE_USDC);

    // Pause then unpause
    fix.vault_client.pause(&pauser);
    fix.vault_client.unpause(&pauser);

    // Withdraw should work again
    let user_token_before = fix.token_client.balance(&user);
    fix.vault_client
        .withdraw(&(50 * ONE_USDC), &user, &user, &user);
    let user_token_after = fix.token_client.balance(&user);

    assert_eq!(
        user_token_after - user_token_before,
        50 * ONE_USDC,
        "withdraw must succeed after unpausing the vault"
    );
}

#[test]
fn test_redeem_succeeds_after_unpause() {
    let fix = setup();
    let user = Address::generate(&fix.env);
    let pauser = Address::generate(&fix.env);
    grant_pauser(&fix, &pauser);

    let shares = deposit_usdc(&fix, &user, 100 * ONE_USDC);

    fix.vault_client.pause(&pauser);
    fix.vault_client.unpause(&pauser);

    // Redeem should work again
    let assets_returned = fix.vault_client.redeem(&(shares / 2), &user, &user, &user);
    assert!(
        assets_returned > 0,
        "redeem must succeed after unpausing the vault"
    );
}

// ---------------------------------------------------------------------------
// 20. Withdraw exactly free_liquidity boundary
// ---------------------------------------------------------------------------

#[test]
fn test_withdraw_exactly_at_free_liquidity_boundary() {
    let fix = setup();
    let user = Address::generate(&fix.env);

    deposit_usdc(&fix, &user, 100 * ONE_USDC);

    // Reserve 60 -- exactly 40 free
    fix.vault_client
        .reserve_liquidity(&fix.position_manager, &(60 * ONE_USDC));

    // Withdraw exactly 40 -- right at the boundary, must succeed
    let user_token_before = fix.token_client.balance(&user);
    fix.vault_client
        .withdraw(&(40 * ONE_USDC), &user, &user, &user);
    let user_token_after = fix.token_client.balance(&user);

    assert_eq!(
        user_token_after - user_token_before,
        40 * ONE_USDC,
        "withdrawing exactly free_liquidity must succeed"
    );
}

// ---------------------------------------------------------------------------
// 21. Withdraw 1 unit over free_liquidity must revert
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "Error(Contract, #4)")]
fn test_withdraw_one_over_free_liquidity_reverts() {
    let fix = setup();
    let user = Address::generate(&fix.env);

    deposit_usdc(&fix, &user, 100 * ONE_USDC);

    fix.vault_client
        .reserve_liquidity(&fix.position_manager, &(60 * ONE_USDC));

    // 40 USDC free, try 40 + 1 unit
    fix.vault_client
        .withdraw(&(40 * ONE_USDC + 1), &user, &user, &user);
}

// ---------------------------------------------------------------------------
// 22. preview_withdraw and preview_redeem consistency
// ---------------------------------------------------------------------------

#[test]
fn test_preview_withdraw_matches_actual_withdraw() {
    let fix = setup();
    let user = Address::generate(&fix.env);

    deposit_usdc(&fix, &user, 100 * ONE_USDC);

    let withdraw_amount = 50 * ONE_USDC;
    let preview_shares = fix.vault_client.preview_withdraw(&withdraw_amount);
    let actual_shares = fix
        .vault_client
        .withdraw(&withdraw_amount, &user, &user, &user);

    assert_eq!(
        preview_shares, actual_shares,
        "preview_withdraw must match the actual shares burned by withdraw"
    );
}

#[test]
fn test_preview_redeem_matches_actual_redeem() {
    let fix = setup();
    let user = Address::generate(&fix.env);

    let shares = deposit_usdc(&fix, &user, 100 * ONE_USDC);

    let redeem_shares = shares / 2;
    let preview_assets = fix.vault_client.preview_redeem(&redeem_shares);
    let actual_assets = fix.vault_client.redeem(&redeem_shares, &user, &user, &user);

    assert_eq!(
        preview_assets, actual_assets,
        "preview_redeem must match the actual assets returned by redeem"
    );
}

// ---------------------------------------------------------------------------
// 23. Negative amount withdraw -- must revert
// ---------------------------------------------------------------------------

#[test]
#[should_panic]
fn test_withdraw_negative_amount_reverts() {
    let fix = setup();
    let user = Address::generate(&fix.env);

    deposit_usdc(&fix, &user, 100 * ONE_USDC);

    // Negative withdrawal is nonsensical and must panic
    fix.vault_client.withdraw(&(-1), &user, &user, &user);
}

// ---------------------------------------------------------------------------
// 24. Negative amount redeem -- must revert
// ---------------------------------------------------------------------------

#[test]
#[should_panic]
fn test_redeem_negative_amount_reverts() {
    let fix = setup();
    let user = Address::generate(&fix.env);

    deposit_usdc(&fix, &user, 100 * ONE_USDC);

    fix.vault_client.redeem(&(-1), &user, &user, &user);
}

// ---------------------------------------------------------------------------
// 25. Withdraw from empty vault (no deposits) -- must revert
// ---------------------------------------------------------------------------

#[test]
#[should_panic]
fn test_withdraw_from_empty_vault_reverts() {
    let fix = setup();
    let user = Address::generate(&fix.env);

    // No deposits have been made, vault is empty
    fix.vault_client
        .withdraw(&(1 * ONE_USDC), &user, &user, &user);
}

// ---------------------------------------------------------------------------
// 26. Redeem from empty vault (no deposits) -- must revert
// ---------------------------------------------------------------------------

#[test]
#[should_panic]
fn test_redeem_from_empty_vault_reverts() {
    let fix = setup();
    let user = Address::generate(&fix.env);

    // No deposits, user has no shares
    fix.vault_client
        .redeem(&(1 * ONE_USDC), &user, &user, &user);
}

// ---------------------------------------------------------------------------
// 27. Multiple users withdraw independently
// ---------------------------------------------------------------------------

#[test]
fn test_multiple_users_withdraw_independently() {
    let fix = setup();
    let alice = Address::generate(&fix.env);
    let bob = Address::generate(&fix.env);

    deposit_usdc(&fix, &alice, 100 * ONE_USDC);
    deposit_usdc(&fix, &bob, 200 * ONE_USDC);

    // Alice withdraws 50
    fix.vault_client
        .withdraw(&(50 * ONE_USDC), &alice, &alice, &alice);

    // Bob withdraws 150
    fix.vault_client
        .withdraw(&(150 * ONE_USDC), &bob, &bob, &bob);

    // Alice still has 50 USDC equivalent in shares
    let alice_max = fix.vault_client.max_withdraw(&alice);
    assert_eq!(
        alice_max,
        50 * ONE_USDC,
        "Alice must have 50 USDC left to withdraw after withdrawing 50 of 100"
    );

    // Bob still has 50 USDC equivalent in shares
    let bob_max = fix.vault_client.max_withdraw(&bob);
    assert_eq!(
        bob_max,
        50 * ONE_USDC,
        "Bob must have 50 USDC left to withdraw after withdrawing 150 of 200"
    );

    // total_assets = 50 + 50 = 100
    assert_eq!(
        fix.vault_client.total_assets(),
        100 * ONE_USDC,
        "total_assets must equal sum of remaining user assets"
    );
}

// ---------------------------------------------------------------------------
// 28. max_redeem consistency with max_withdraw
// ---------------------------------------------------------------------------

#[test]
fn test_max_redeem_consistent_with_max_withdraw() {
    let fix = setup();
    let user = Address::generate(&fix.env);

    deposit_usdc(&fix, &user, 100 * ONE_USDC);
    fix.vault_client
        .reserve_liquidity(&fix.position_manager, &(60 * ONE_USDC));

    let max_w = fix.vault_client.max_withdraw(&user);
    let max_r = fix.vault_client.max_redeem(&user);

    // max_redeem shares, when converted to assets, should equal max_withdraw
    let assets_from_redeem = fix.vault_client.preview_redeem(&max_r);

    assert_eq!(
        assets_from_redeem, max_w,
        "max_redeem converted to assets must equal max_withdraw"
    );
}

// ---------------------------------------------------------------------------
// 29. Sequential reserve + withdraw + reserve cycle
// ---------------------------------------------------------------------------

#[test]
fn test_reserve_withdraw_reserve_cycle() {
    let fix = setup();
    let user = Address::generate(&fix.env);

    deposit_usdc(&fix, &user, 100 * ONE_USDC);

    // Round 1: reserve 30, withdraw 20
    fix.vault_client
        .reserve_liquidity(&fix.position_manager, &(30 * ONE_USDC));
    assert_eq!(fix.vault_client.free_liquidity(), 70 * ONE_USDC);
    fix.vault_client
        .withdraw(&(20 * ONE_USDC), &user, &user, &user);
    // total_assets = 80, reserved = 30, free = 50
    assert_eq!(fix.vault_client.free_liquidity(), 50 * ONE_USDC);

    // Round 2: reserve another 40 (total reserved 70)
    fix.vault_client
        .reserve_liquidity(&fix.position_manager, &(40 * ONE_USDC));
    // total_assets = 80, reserved = 70, free = 10
    assert_eq!(fix.vault_client.free_liquidity(), 10 * ONE_USDC);

    // Can only withdraw up to 10
    let max_w = fix.vault_client.max_withdraw(&user);
    assert_eq!(
        max_w,
        10 * ONE_USDC,
        "max_withdraw must reflect updated free_liquidity after reserve cycles"
    );
}

// ---------------------------------------------------------------------------
// 30. Withdraw with different receiver and owner
// ---------------------------------------------------------------------------

#[test]
fn test_withdraw_different_receiver() {
    let fix = setup();
    let owner = Address::generate(&fix.env);
    let receiver = Address::generate(&fix.env);

    deposit_usdc(&fix, &owner, 100 * ONE_USDC);

    let receiver_before = fix.token_client.balance(&receiver);

    // Owner withdraws but sends USDC to a different receiver
    fix.vault_client
        .withdraw(&(50 * ONE_USDC), &receiver, &owner, &owner);

    let receiver_after = fix.token_client.balance(&receiver);
    assert_eq!(
        receiver_after - receiver_before,
        50 * ONE_USDC,
        "receiver (not owner) must receive the withdrawn USDC"
    );

    // Owner shares must decrease
    assert_eq!(
        fix.vault_client.max_withdraw(&owner),
        50 * ONE_USDC,
        "owner must have 50 USDC worth of shares remaining"
    );
}

// ---------------------------------------------------------------------------
// 31. Redeem with different receiver
// ---------------------------------------------------------------------------

#[test]
fn test_redeem_different_receiver() {
    let fix = setup();
    let owner = Address::generate(&fix.env);
    let receiver = Address::generate(&fix.env);

    let shares = deposit_usdc(&fix, &owner, 100 * ONE_USDC);

    let receiver_before = fix.token_client.balance(&receiver);

    fix.vault_client
        .redeem(&(shares / 2), &receiver, &owner, &owner);

    let receiver_after = fix.token_client.balance(&receiver);
    assert!(
        receiver_after - receiver_before > 0,
        "receiver must receive assets when owner redeems shares"
    );
}

// ---------------------------------------------------------------------------
// 32. free_liquidity floor at zero (reserved exceeds total_assets scenario)
// ---------------------------------------------------------------------------

#[test]
fn test_free_liquidity_floor_at_zero() {
    let fix = setup();
    let user = Address::generate(&fix.env);

    deposit_usdc(&fix, &user, 100 * ONE_USDC);

    // Reserve the entire amount
    fix.vault_client
        .reserve_liquidity(&fix.position_manager, &(100 * ONE_USDC));

    let free = fix.vault_client.free_liquidity();
    assert_eq!(
        free, 0,
        "free_liquidity must be 0 when reserved equals total_assets"
    );

    // max_withdraw must be 0
    assert_eq!(
        fix.vault_client.max_withdraw(&user),
        0,
        "max_withdraw must be 0 when all liquidity is reserved"
    );
}

// ---------------------------------------------------------------------------
// 33. Withdraw smallest possible amount (1 unit)
// ---------------------------------------------------------------------------

#[test]
fn test_withdraw_smallest_unit() {
    let fix = setup();
    let user = Address::generate(&fix.env);

    deposit_usdc(&fix, &user, 100 * ONE_USDC);

    let user_token_before = fix.token_client.balance(&user);

    // Withdraw the smallest possible unit: 1 (0.0000001 USDC)
    fix.vault_client.withdraw(&1, &user, &user, &user);

    let user_token_after = fix.token_client.balance(&user);
    assert_eq!(
        user_token_after - user_token_before,
        1,
        "user must receive exactly 1 unit of USDC"
    );
}

// ---------------------------------------------------------------------------
// 34. Free liquidity accounts for unclaimed fees
// ---------------------------------------------------------------------------

#[test]
fn test_free_liquidity_accounts_for_unclaimed_fees() {
    let fix = setup();
    let user = Address::generate(&fix.env);

    deposit_usdc(&fix, &user, 100 * ONE_USDC);

    // Accrue 10 USDC in fees via position manager
    fix.vault_client
        .accrue_fees(&fix.position_manager, &(10 * ONE_USDC));

    // free_liquidity = 100 - 0 - 10 - 0 = 90
    assert_eq!(
        fix.vault_client.free_liquidity(),
        90 * ONE_USDC,
        "free_liquidity must subtract unclaimed_fees"
    );

    // max_withdraw must respect the reduced free liquidity
    let max_w = fix.vault_client.max_withdraw(&user);
    assert_eq!(
        max_w,
        90 * ONE_USDC,
        "max_withdraw must be capped by free_liquidity accounting for unclaimed fees"
    );
}

// ---------------------------------------------------------------------------
// 35. Free liquidity accounts for positive net global trader PnL
// ---------------------------------------------------------------------------

#[test]
fn test_free_liquidity_accounts_for_positive_net_pnl() {
    let fix = setup();
    let user = Address::generate(&fix.env);

    deposit_usdc(&fix, &user, 100 * ONE_USDC);

    // Set net PnL to +20 USDC (traders are in profit overall)
    fix.vault_client
        .update_net_pnl(&fix.position_manager, &(20 * ONE_USDC));

    // free_liquidity = 100 - 0 - 0 - 20 = 80
    assert_eq!(
        fix.vault_client.free_liquidity(),
        80 * ONE_USDC,
        "free_liquidity must subtract positive net_global_trader_pnl"
    );
}

// ---------------------------------------------------------------------------
// 36. Free liquidity ignores negative net global trader PnL
// ---------------------------------------------------------------------------

#[test]
fn test_free_liquidity_ignores_negative_net_pnl() {
    let fix = setup();
    let user = Address::generate(&fix.env);

    deposit_usdc(&fix, &user, 100 * ONE_USDC);

    // Set net PnL to -30 USDC (traders are in loss overall)
    fix.vault_client
        .update_net_pnl(&fix.position_manager, &(-30 * ONE_USDC));

    // free_liquidity = 100 - 0 - 0 - max(0, -30) = 100 - 0 = 100
    assert_eq!(
        fix.vault_client.free_liquidity(),
        100 * ONE_USDC,
        "free_liquidity must ignore negative net_global_trader_pnl (max(0, pnl) = 0)"
    );
}

// ---------------------------------------------------------------------------
// 37. Full free liquidity formula with all components
// ---------------------------------------------------------------------------

#[test]
fn test_free_liquidity_full_formula() {
    let fix = setup();
    let user = Address::generate(&fix.env);

    deposit_usdc(&fix, &user, 200 * ONE_USDC);

    // Reserve 50
    fix.vault_client
        .reserve_liquidity(&fix.position_manager, &(50 * ONE_USDC));

    // Accrue 10 in fees
    fix.vault_client
        .accrue_fees(&fix.position_manager, &(10 * ONE_USDC));

    // Set net PnL to +30
    fix.vault_client
        .update_net_pnl(&fix.position_manager, &(30 * ONE_USDC));

    // free_liquidity = max(0, 200 - 50 - 10 - 30) = 110
    assert_eq!(
        fix.vault_client.free_liquidity(),
        110 * ONE_USDC,
        "free_liquidity = total_assets - reserved - unclaimed_fees - max(0, net_pnl)"
    );
}
