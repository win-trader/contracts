#![cfg(test)]

//! Comprehensive deposit tests for the Vault contract.
//!
//! Covers:
//!   - Happy-path deposits (multiple users)
//!   - ERC-4626 share math invariants (preview == actual, proportional shares)
//!   - Share price evolution after profit accrual
//!   - Pause enforcement on deposit and max_deposit
//!   - Edge cases: zero amount, minimum amount (1 stroop), huge amounts
//!   - Adversarial: overflow attempts, unauthorized operations

use soroban_sdk::{testutils::Address as _, Address, Env, String, Symbol};

// ---------------------------------------------------------------------------
// Test fixture
// ---------------------------------------------------------------------------

struct DepositFixture {
    env: Env,
    admin: Address,
    token_client: mock_token::MockTokenClient<'static>,
    config_client: config_manager::ConfigManagerClient<'static>,
    vault_id: Address,
    vault_client: crate::VaultContractClient<'static>,
    position_manager: Address,
}

/// Deploy mock-token, config-manager, and vault. Initialize all three.
fn setup() -> DepositFixture {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let position_manager = Address::generate(&env);

    // Deploy mock USDC (7 decimals)
    let token_id = env.register(mock_token::MockToken, ());
    let token_client = mock_token::MockTokenClient::new(&env, &token_id);
    token_client.initialize(
        &admin,
        &7u32,
        &String::from_str(&env, "USD Coin"),
        &String::from_str(&env, "USDC"),
    );

    // Deploy config manager
    let config_id = env.register(config_manager::ConfigManagerContract, (admin.clone(),));
    let config_client = config_manager::ConfigManagerClient::new(&env, &config_id);

    // Deploy vault (constructor binds asset, config_manager, position_manager)
    let vault_id = env.register(
        crate::VaultContract,
        (token_id.clone(), config_id.clone(), position_manager.clone()),
    );
    let vault_client = crate::VaultContractClient::new(&env, &vault_id);

    // SAFETY: env lives in the fixture, clients borrow from it.
    let token_client = unsafe { core::mem::transmute(token_client) };
    let config_client = unsafe { core::mem::transmute(config_client) };
    let vault_client = unsafe { core::mem::transmute(vault_client) };

    DepositFixture {
        env,
        admin,
        token_client,
        config_client,
        vault_id,
        vault_client,
        position_manager,
    }
}

/// Helper: mint USDC to an address.
fn mint_usdc(fix: &DepositFixture, to: &Address, amount: i128) {
    fix.token_client.mint(to, &amount);
}

/// 7-decimal USDC helper: 1 USDC = 10_000_000 stroops.
const USDC: i128 = 10_000_000;

/// Share multiplier due to decimals_offset = 6.
/// Vault shares have asset_decimals + 6 = 13 decimals.
/// On first deposit, 1 stroop of assets = 10^6 shares.
const SHARE_MUL: i128 = 1_000_000;

// ===========================================================================
// 1. Basic deposit
// ===========================================================================

#[test]
fn test_deposit_success() {
    let fix = setup();
    let depositor = Address::generate(&fix.env);
    let deposit_amount = 100 * USDC;

    mint_usdc(&fix, &depositor, deposit_amount);

    // Deposit 100 USDC
    let shares = fix.vault_client.deposit(
        &deposit_amount,
        &depositor,  // receiver
        &depositor,  // from
        &depositor,  // operator
    );

    // With empty vault, shares should equal assets (1:1 initial ratio)
    assert!(
        shares > 0,
        "Deposit must return a positive number of shares"
    );

    // Depositor's USDC balance should be zero (all deposited)
    assert_eq!(
        fix.token_client.balance(&depositor),
        0,
        "Depositor USDC balance must be zero after depositing full balance"
    );

    // Vault's total_assets should equal the deposit amount
    assert_eq!(
        fix.vault_client.total_assets(),
        deposit_amount,
        "total_assets must equal the deposited amount"
    );

    // Vault's total_supply should equal shares minted
    assert_eq!(
        fix.vault_client.total_supply(),
        shares,
        "total_supply must equal the shares returned from deposit"
    );

    // Depositor's share balance should match
    assert_eq!(
        fix.vault_client.balance(&depositor),
        shares,
        "Depositor share balance must equal shares returned"
    );

    // Vault USDC balance (the token held by the vault contract) should equal deposit_amount
    assert_eq!(
        fix.token_client.balance(&fix.vault_id),
        deposit_amount,
        "Vault token balance must equal deposited amount"
    );

    // Free liquidity should equal total_assets (no reservations)
    assert_eq!(
        fix.vault_client.free_liquidity(),
        deposit_amount,
        "Free liquidity must equal total_assets when no liquidity is reserved"
    );
}

// ===========================================================================
// 2. Share price evolution after profit accrual
// ===========================================================================

#[test]
fn test_deposit_updates_share_price_after_profit() {
    let fix = setup();
    let user1 = Address::generate(&fix.env);
    let user2 = Address::generate(&fix.env);

    mint_usdc(&fix, &user1, 100 * USDC);
    mint_usdc(&fix, &user2, 100 * USDC);

    // User 1 deposits 100 USDC (first depositor, shares = assets * SHARE_MUL)
    let shares1 = fix.vault_client.deposit(&(100 * USDC), &user1, &user1, &user1);
    assert_eq!(shares1, 100 * USDC * SHARE_MUL, "First deposit shares = assets * 10^6");

    // Simulate profit: directly send 100 USDC to the vault (doubles total_assets)
    mint_usdc(&fix, &fix.vault_id, 100 * USDC);

    // Now total_assets = 200 USDC, total_supply = 100 shares
    // Price per share = 200/100 = 2 USDC per share
    assert_eq!(
        fix.vault_client.total_assets(),
        200 * USDC,
        "total_assets should include the profit"
    );

    // User 2 deposits 100 USDC. At ~2 USDC/share, they should get ~50 * USDC * SHARE_MUL shares.
    // Due to virtual shares/assets in the OZ vault (inflation protection), there is minor rounding.
    let shares2 = fix.vault_client.deposit(&(100 * USDC), &user2, &user2, &user2);

    assert!(
        shares2 < shares1,
        "Second depositor must receive fewer shares because price per share increased"
    );

    // With virtual offset, the exact value has minor rounding vs the ideal 50*USDC*SHARE_MUL.
    // Verify it is within a tiny margin of the expected value.
    let expected_approx = 50 * USDC * SHARE_MUL;
    let diff = if shares2 > expected_approx { shares2 - expected_approx } else { expected_approx - shares2 };
    assert!(
        diff < SHARE_MUL, // less than 1 share-stroop of rounding error
        "Second depositor should get approximately half the shares (100 USDC at ~2 USDC/share). \
         shares2={}, expected_approx={}",
        shares2,
        expected_approx
    );

    // Verify total state
    assert_eq!(
        fix.vault_client.total_assets(),
        300 * USDC,
        "total_assets = 200 (before) + 100 (new deposit)"
    );
    assert_eq!(
        fix.vault_client.total_supply(),
        shares1 + shares2,
        "total_supply = shares from both deposits"
    );
}

// ===========================================================================
// 6. Multiple users deposit different amounts (proportional shares)
// ===========================================================================

#[test]
fn test_deposit_multiple_users_proportional() {
    let fix = setup();
    let user_a = Address::generate(&fix.env);
    let user_b = Address::generate(&fix.env);

    mint_usdc(&fix, &user_a, 300 * USDC);
    mint_usdc(&fix, &user_b, 100 * USDC);

    // User A deposits 300 USDC
    let shares_a = fix.vault_client.deposit(&(300 * USDC), &user_a, &user_a, &user_a);

    // User B deposits 100 USDC
    let shares_b = fix.vault_client.deposit(&(100 * USDC), &user_b, &user_b, &user_b);

    // Both deposited at 1:1, so shares should be proportional
    assert_eq!(
        shares_a,
        3 * shares_b,
        "User A deposited 3x more, so must have 3x the shares"
    );

    // Verify individual balances
    assert_eq!(
        fix.vault_client.balance(&user_a),
        shares_a,
        "User A share balance must match"
    );
    assert_eq!(
        fix.vault_client.balance(&user_b),
        shares_b,
        "User B share balance must match"
    );

    // total_supply = shares_a + shares_b
    assert_eq!(
        fix.vault_client.total_supply(),
        shares_a + shares_b,
        "total_supply must be the sum of all shares"
    );

    // total_assets = 400 USDC
    assert_eq!(
        fix.vault_client.total_assets(),
        400 * USDC,
        "total_assets must be the sum of all deposits"
    );
}

// ===========================================================================
// 7. Deposit when paused should revert
// ===========================================================================

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn test_deposit_when_paused_reverts() {
    let fix = setup();
    let depositor = Address::generate(&fix.env);
    mint_usdc(&fix, &depositor, 100 * USDC);

    // Grant PAUSER role and pause the vault
    let pauser = Address::generate(&fix.env);
    let pauser_role = Symbol::new(&fix.env, shared::constants::ROLE_PAUSER);
    fix.config_client.grant_role(&fix.admin, &pauser_role, &pauser);
    fix.vault_client.pause(&pauser);

    // Attempt deposit while paused -- must panic with VaultError::Paused = 3
    fix.vault_client.deposit(&(100 * USDC), &depositor, &depositor, &depositor);
}

// ===========================================================================
// 8. max_deposit returns 0 when paused
// ===========================================================================

#[test]
fn test_max_deposit_when_paused_returns_zero() {
    let fix = setup();
    let depositor = Address::generate(&fix.env);

    // Grant PAUSER role and pause
    let pauser = Address::generate(&fix.env);
    let pauser_role = Symbol::new(&fix.env, shared::constants::ROLE_PAUSER);
    fix.config_client.grant_role(&fix.admin, &pauser_role, &pauser);
    fix.vault_client.pause(&pauser);

    assert_eq!(
        fix.vault_client.max_deposit(&depositor),
        0,
        "max_deposit must return 0 when the vault is paused"
    );
}

// ===========================================================================
// 9. max_deposit returns i128::MAX when not paused
// ===========================================================================

#[test]
fn test_max_deposit_when_not_paused_returns_max() {
    let fix = setup();
    let depositor = Address::generate(&fix.env);

    assert_eq!(
        fix.vault_client.max_deposit(&depositor),
        i128::MAX,
        "max_deposit must return i128::MAX when vault is not paused"
    );
}

// ===========================================================================
// 10. Deposit zero amount — must revert with ZeroAmount (#6).
//
// Without the guard, anyone can extend a victim's lockup for free by minting
// 0 shares to them. The wrapper rejects zero-asset deposits before recording
// the lockup or delegating to OZ Vault::deposit.
// ===========================================================================

#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn test_deposit_zero_amount_reverts() {
    let fix = setup();
    let depositor = Address::generate(&fix.env);
    fix.vault_client.deposit(&0i128, &depositor, &depositor, &depositor);
}

// ===========================================================================
// 11. preview_deposit matches actual deposit
// ===========================================================================

#[test]
fn test_preview_deposit_matches_actual() {
    let fix = setup();
    let depositor = Address::generate(&fix.env);
    let deposit_amount = 100 * USDC;

    mint_usdc(&fix, &depositor, deposit_amount);

    // Preview should predict the exact shares returned by deposit.
    let preview = fix.vault_client.preview_deposit(&deposit_amount);
    let actual = fix.vault_client.deposit(
        &deposit_amount,
        &depositor,
        &depositor,
        &depositor,
    );

    assert_eq!(
        preview, actual,
        "preview_deposit must match actual shares returned by deposit"
    );
}

// ===========================================================================
// 12. convert_to_shares and convert_to_assets are inverse
// ===========================================================================

#[test]
fn test_convert_to_shares_and_assets_are_inverse() {
    let fix = setup();
    let depositor = Address::generate(&fix.env);

    // Seed the vault with an initial deposit so exchange rate is established
    mint_usdc(&fix, &depositor, 100 * USDC);
    fix.vault_client.deposit(&(100 * USDC), &depositor, &depositor, &depositor);

    let test_amount = 50 * USDC;
    let shares = fix.vault_client.convert_to_shares(&test_amount);
    let assets_back = fix.vault_client.convert_to_assets(&shares);

    assert_eq!(
        assets_back, test_amount,
        "convert_to_assets(convert_to_shares(x)) must return x (at 1:1 ratio)"
    );
}

// ===========================================================================
// 14. Deposit where receiver != from (third-party deposit) is rejected.
// ===========================================================================

#[test]
#[should_panic(expected = "Error(Contract, #13)")]
fn test_deposit_receiver_different_from_sender_reverts() {
    let fix = setup();
    let sender = Address::generate(&fix.env);
    let receiver = Address::generate(&fix.env);
    let deposit_amount = 50 * USDC;

    mint_usdc(&fix, &sender, deposit_amount);

    fix.vault_client.deposit(
        &deposit_amount,
        &receiver,
        &sender,
        &sender,
    );
}

// ===========================================================================
// 15. Deposit more than the depositor's balance should revert
// ===========================================================================

#[test]
#[should_panic]
fn test_deposit_exceeding_balance_reverts() {
    let fix = setup();
    let depositor = Address::generate(&fix.env);

    // Only mint 10 USDC but try to deposit 100 USDC
    mint_usdc(&fix, &depositor, 10 * USDC);

    fix.vault_client.deposit(&(100 * USDC), &depositor, &depositor, &depositor);
}

// ===========================================================================
// 16. Adversarial: deposit with maximum i128 value (overflow attempt)
// ===========================================================================

#[test]
#[should_panic]
fn test_deposit_i128_max_overflows() {
    let fix = setup();
    let depositor = Address::generate(&fix.env);

    // Even with minted balance, this should overflow in share math
    mint_usdc(&fix, &depositor, i128::MAX);

    fix.vault_client.deposit(&i128::MAX, &depositor, &depositor, &depositor);
}

// ===========================================================================
// 17. Adversarial: deposit negative amount should revert
// ===========================================================================

#[test]
#[should_panic]
fn test_deposit_negative_amount_reverts() {
    let fix = setup();
    let depositor = Address::generate(&fix.env);

    mint_usdc(&fix, &depositor, 100 * USDC);

    // Negative deposit must not be allowed
    fix.vault_client.deposit(&(-1i128), &depositor, &depositor, &depositor);
}

// ===========================================================================
// 18. Deposit 1 stroop (minimum possible amount)
// ===========================================================================

#[test]
fn test_deposit_one_stroop() {
    let fix = setup();
    let depositor = Address::generate(&fix.env);

    mint_usdc(&fix, &depositor, 1);

    let shares = fix.vault_client.deposit(&1i128, &depositor, &depositor, &depositor);

    assert!(
        shares >= 1,
        "Depositing 1 stroop must produce at least 1 share (initial 1:1 ratio)"
    );

    assert_eq!(
        fix.vault_client.total_assets(),
        1,
        "total_assets must be 1 stroop"
    );
}

// ===========================================================================
// 19. Pause -> unpause -> deposit should succeed
// ===========================================================================

#[test]
fn test_deposit_after_unpause_succeeds() {
    let fix = setup();
    let depositor = Address::generate(&fix.env);
    mint_usdc(&fix, &depositor, 100 * USDC);

    // Pause
    let pauser = Address::generate(&fix.env);
    let pauser_role = Symbol::new(&fix.env, shared::constants::ROLE_PAUSER);
    fix.config_client.grant_role(&fix.admin, &pauser_role, &pauser);
    fix.vault_client.pause(&pauser);

    // Unpause
    fix.vault_client.unpause(&pauser);

    // Deposit should work after unpause
    let shares = fix.vault_client.deposit(&(100 * USDC), &depositor, &depositor, &depositor);
    assert!(shares > 0, "Deposit must succeed after unpause");
}

// ===========================================================================
// 22. max_deposit returns correct value after pause/unpause cycle
// ===========================================================================

#[test]
fn test_max_deposit_after_unpause_returns_max() {
    let fix = setup();
    let depositor = Address::generate(&fix.env);

    let pauser = Address::generate(&fix.env);
    let pauser_role = Symbol::new(&fix.env, shared::constants::ROLE_PAUSER);
    fix.config_client.grant_role(&fix.admin, &pauser_role, &pauser);

    // Pause
    fix.vault_client.pause(&pauser);
    assert_eq!(fix.vault_client.max_deposit(&depositor), 0);

    // Unpause
    fix.vault_client.unpause(&pauser);
    assert_eq!(
        fix.vault_client.max_deposit(&depositor),
        i128::MAX,
        "max_deposit must return i128::MAX after unpause"
    );
}

// ===========================================================================
// 23. Deposit does not affect reserved_usdc or free_liquidity incorrectly
// ===========================================================================

#[test]
fn test_deposit_does_not_affect_reserved_usdc() {
    let fix = setup();
    let depositor = Address::generate(&fix.env);
    mint_usdc(&fix, &depositor, 100 * USDC);

    fix.vault_client.deposit(&(100 * USDC), &depositor, &depositor, &depositor);

    // free_liquidity = total_assets - reserved_usdc - unclaimed_fees - max(0, net_pnl) = 100 - 0 - 0 - 0 = 100
    assert_eq!(
        fix.vault_client.free_liquidity(),
        100 * USDC,
        "Deposit should not change reserved_usdc; free_liquidity must equal total_assets"
    );
}

// ===========================================================================
// 24. Deposit with reserved liquidity: free_liquidity reflects correctly
// ===========================================================================

#[test]
fn test_deposit_with_existing_reserved_liquidity() {
    let fix = setup();
    let depositor = Address::generate(&fix.env);
    mint_usdc(&fix, &depositor, 200 * USDC);

    fix.vault_client.deposit(&(200 * USDC), &depositor, &depositor, &depositor);

    // Reserve 50 USDC via position manager
    fix.vault_client.reserve_liquidity(&fix.position_manager, &(50 * USDC));

    // free_liquidity = 200 - 50 = 150
    assert_eq!(
        fix.vault_client.free_liquidity(),
        150 * USDC,
        "free_liquidity must subtract reserved_usdc"
    );

    // Now deposit more
    let depositor2 = Address::generate(&fix.env);
    mint_usdc(&fix, &depositor2, 100 * USDC);
    fix.vault_client.deposit(&(100 * USDC), &depositor2, &depositor2, &depositor2);

    // free_liquidity = 300 - 50 = 250
    assert_eq!(
        fix.vault_client.free_liquidity(),
        250 * USDC,
        "free_liquidity must increase by new deposit amount"
    );
}

// ===========================================================================
// 25. Adversarial: share price manipulation via donation before first deposit
//     (inflation attack / vault share manipulation)
// ===========================================================================

#[test]
fn test_deposit_inflation_attack_frontrun() {
    let fix = setup();
    let attacker = Address::generate(&fix.env);
    let victim = Address::generate(&fix.env);

    // Step 1: Attacker deposits 1 stroop to become the first depositor
    mint_usdc(&fix, &attacker, 1);
    let attacker_shares = fix.vault_client.deposit(&1i128, &attacker, &attacker, &attacker);
    assert_eq!(attacker_shares, 1 * SHARE_MUL, "Attacker gets SHARE_MUL shares for 1 stroop (offset=6)");

    // Step 2: Attacker donates a large amount directly to vault to inflate share price
    // This is the classic ERC-4626 inflation attack
    let donation = 1_000 * USDC;
    mint_usdc(&fix, &fix.vault_id, donation);

    // Now: total_assets = 1 + donation, total_supply = 1
    // Price per share = (1 + donation) / 1

    // Step 3: Victim deposits 500 USDC
    let victim_deposit = 500 * USDC;
    mint_usdc(&fix, &victim, victim_deposit);
    let victim_shares = fix.vault_client.deposit(
        &victim_deposit,
        &victim,
        &victim,
        &victim,
    );

    // With inflated price: shares = 500 * USDC * 1 / (1 + 1000 * USDC)
    // = 5_000_000_000 * 1 / 10_000_000_001 = 0 (truncated!)
    // If victim gets 0 shares, they lose their entire deposit.
    // This test documents whether the OZ vault has inflation attack protection.
    //
    // A well-designed vault should either:
    //   a) Use a virtual offset to prevent this, OR
    //   b) Revert on 0-share deposits
    //
    // We assert that the victim should NOT get 0 shares (if they do, it is a vulnerability).
    assert!(
        victim_shares > 0,
        "INFLATION ATTACK: Victim deposited {} USDC but received 0 shares. \
         The vault is vulnerable to share price inflation attacks.",
        victim_deposit
    );
}

// ===========================================================================
// 26. Deposit naming the vault as receiver is rejected (receiver != from).
// ===========================================================================

#[test]
#[should_panic(expected = "Error(Contract, #13)")]
fn test_deposit_receiver_is_vault_itself_reverts() {
    let fix = setup();
    let depositor = Address::generate(&fix.env);
    mint_usdc(&fix, &depositor, 50 * USDC);

    fix.vault_client.deposit(
        &(50 * USDC),
        &fix.vault_id,
        &depositor,
        &depositor,
    );
}

// ===========================================================================
// 29. Sequential deposits maintain correct accounting
// ===========================================================================

#[test]
fn test_sequential_deposits_same_user() {
    let fix = setup();
    let depositor = Address::generate(&fix.env);

    mint_usdc(&fix, &depositor, 300 * USDC);

    // Three sequential deposits of 100 USDC each
    let shares1 = fix.vault_client.deposit(&(100 * USDC), &depositor, &depositor, &depositor);
    let shares2 = fix.vault_client.deposit(&(100 * USDC), &depositor, &depositor, &depositor);
    let shares3 = fix.vault_client.deposit(&(100 * USDC), &depositor, &depositor, &depositor);

    // All deposits at same exchange rate, so shares should be equal
    assert_eq!(shares1, shares2, "Same-price deposits must yield equal shares");
    assert_eq!(shares2, shares3, "Same-price deposits must yield equal shares");

    // Total shares = 3x
    assert_eq!(
        fix.vault_client.balance(&depositor),
        shares1 + shares2 + shares3,
        "Total shares must be sum of all individual deposits"
    );

    assert_eq!(
        fix.vault_client.total_assets(),
        300 * USDC,
        "total_assets must equal sum of all deposits"
    );
}

// ===========================================================================
// 30. Deposit after loss (share price decreased)
// ===========================================================================

#[test]
fn test_deposit_after_vault_loss() {
    let fix = setup();
    let user1 = Address::generate(&fix.env);
    let user2 = Address::generate(&fix.env);

    mint_usdc(&fix, &user1, 100 * USDC);

    // User 1 deposits 100 USDC
    let shares1 = fix.vault_client.deposit(&(100 * USDC), &user1, &user1, &user1);

    // Simulate loss to LPs: PM pays a profitable trader 50 USDC, draining the vault.
    let trader = Address::generate(&fix.env);
    fix.vault_client
        .pay_profit(&fix.position_manager, &trader, &(50 * USDC));

    // Now total_assets should be 50 USDC, total_supply still = shares1
    // Price per share = 50 / shares1 < 1

    // User 2 deposits 100 USDC -- should get MORE shares than user1 got
    // because price per share is now lower
    mint_usdc(&fix, &user2, 100 * USDC);
    let shares2 = fix.vault_client.deposit(&(100 * USDC), &user2, &user2, &user2);

    assert!(
        shares2 > shares1,
        "After a loss, new depositors should receive more shares per USDC \
         (share price is lower). shares2={}, shares1={}",
        shares2,
        shares1
    );
}

// ===========================================================================
// 31. Adversarial: deposit with from != operator (unauthorized transfer attempt)
// ===========================================================================

#[test]
fn test_deposit_operator_must_authorize() {
    // This test verifies that operator.require_auth() is enforced.
    // When mock_all_auths is NOT used, the deposit should fail if the operator
    // has not authorized the call.
    let env = Env::default();
    // NOTE: Do NOT mock auths -- we want real auth checks

    let admin = Address::generate(&env);
    let position_manager = Address::generate(&env);

    let token_id = env.register(mock_token::MockToken, ());
    let token_client = mock_token::MockTokenClient::new(&env, &token_id);

    // Constructors run at deploy; mock auths first so they (and setup) succeed.
    env.mock_all_auths();
    token_client.initialize(
        &admin,
        &7u32,
        &String::from_str(&env, "USDC"),
        &String::from_str(&env, "USDC"),
    );

    let config_id = env.register(config_manager::ConfigManagerContract, (admin.clone(),));

    let vault_id = env.register(
        crate::VaultContract,
        (token_id.clone(), config_id.clone(), position_manager.clone()),
    );
    let vault_client = crate::VaultContractClient::new(&env, &vault_id);

    let depositor = Address::generate(&env);
    token_client.mint(&depositor, &(100 * USDC));

    // Make the deposit with mocked auths -- should succeed
    let _shares = vault_client.deposit(&(100 * USDC), &depositor, &depositor, &depositor);

    // Verify auth was required from operator
    let auths = env.auths();
    assert!(
        !auths.is_empty(),
        "deposit must require at least one authorization (from operator)"
    );

    // The operator (depositor) should appear in the auth list
    let has_operator_auth = auths.iter().any(|(addr, _)| *addr == depositor);
    assert!(
        has_operator_auth,
        "Operator must appear in the authorization list"
    );
}

// ===========================================================================
// 33. Large deposit stress test
// ===========================================================================

#[test]
fn test_deposit_large_amount() {
    let fix = setup();
    let depositor = Address::generate(&fix.env);

    // 1 billion USDC (7 decimals)
    let large_amount = 1_000_000_000 * USDC;
    mint_usdc(&fix, &depositor, large_amount);

    let shares = fix.vault_client.deposit(&large_amount, &depositor, &depositor, &depositor);

    assert_eq!(
        shares, large_amount * SHARE_MUL,
        "Large deposit at first-deposit ratio must return assets * SHARE_MUL shares"
    );
    assert_eq!(
        fix.vault_client.total_assets(),
        large_amount,
        "total_assets must handle large values"
    );
}
