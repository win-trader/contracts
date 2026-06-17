#![cfg(test)]

//! Tests for `claim_fees_to` -- a PM-only endpoint that allows the position
//! manager to claim a *specific amount* of accrued fees and route them to an
//! arbitrary recipient.
//!
//! Expected signature:
//!   `claim_fees_to(env, caller, recipient, amount)`
//!
//! Rules:
//!   - caller MUST be the stored position_manager address (NotPositionManager = 7)
//!   - amount MUST be > 0 (ZeroAmount = 6)
//!   - amount MUST be <= unclaimed_fees (InsufficientFees = 10)
//!   - on success: transfers `amount` of the underlying asset from vault to
//!     `recipient` and decrements `unclaimed_fees` by `amount`
//!
//! These tests are written TDD-style and WILL FAIL until the implementation
//! is added to `VaultContract`.

use soroban_sdk::{testutils::Address as _, Address, Env, String, Symbol};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const ONE_USDC: i128 = 10_000_000; // 1.0000000 (7 decimals)

// ---------------------------------------------------------------------------
// Test Fixture (mirrors test_operations.rs)
// ---------------------------------------------------------------------------

struct TestFixture {
    env: Env,
    admin: Address,
    #[allow(dead_code)]
    token_id: Address,
    token_client: mock_token::MockTokenClient<'static>,
    #[allow(dead_code)]
    config_id: Address,
    #[allow(dead_code)]
    config_client: config_manager::ConfigManagerClient<'static>,
    vault_id: Address,
    vault_client: crate::VaultContractClient<'static>,
    position_manager: Address,
}

/// Deploy mock-token, config-manager, and vault. Initialize all three.
fn setup() -> TestFixture {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let position_manager = Address::generate(&env);

    // Deploy mock USDC token (7 decimals)
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

    TestFixture {
        env,
        admin,
        token_id,
        token_client,
        config_id,
        config_client,
        vault_id,
        vault_client,
        position_manager,
    }
}

/// Seed the vault with USDC by minting to a depositor and depositing via the
/// vault. Returns (depositor_address, shares_minted).
fn seed_vault(fix: &TestFixture, amount: i128) -> (Address, i128) {
    let depositor = Address::generate(&fix.env);
    fix.token_client.mint(&depositor, &amount);
    let shares = fix.vault_client.preview_deposit(&amount);
    let _assets_needed = fix.vault_client.mint(&shares, &depositor, &depositor, &depositor);
    (depositor, shares)
}

// ===========================================================================
// claim_fees_to -- happy-path tests
// ===========================================================================

mod claim_fees_to_success {
    use super::*;

    /// PM calls claim_fees_to with valid amount. Verify:
    ///   - recipient receives exactly `amount` of the underlying token
    ///   - unclaimed_fees decremented (free_liquidity rises by the claimed amount)
    #[test]
    fn test_claim_fees_to_success() {
        let fix = setup();
        seed_vault(&fix, 100 * ONE_USDC);

        // Accrue 10 USDC in fees
        fix.vault_client
            .accrue_fees(&fix.position_manager, &(10 * ONE_USDC));

        let recipient = Address::generate(&fix.env);
        let recipient_before = fix.token_client.balance(&recipient);

        // free_liquidity before claim = 100 - 0 - 10 - 0 = 90
        let free_before = fix.vault_client.free_liquidity();
        assert_eq!(free_before, 90 * ONE_USDC);

        // Claim the full 10 USDC of fees
        fix.vault_client.claim_fees_to(
            &fix.position_manager,
            &recipient,
            &(10 * ONE_USDC),
        );

        // Recipient must have received exactly 10 USDC
        let recipient_after = fix.token_client.balance(&recipient);
        assert_eq!(
            recipient_after - recipient_before,
            10 * ONE_USDC,
            "Recipient should receive exactly the claimed fee amount"
        );

        // free_liquidity after claim: total_assets dropped by 10 (tokens sent out),
        // unclaimed_fees dropped by 10 => free_liq = 90 - 0 - 0 - 0 = 90
        let free_after = fix.vault_client.free_liquidity();
        assert_eq!(
            free_after,
            90 * ONE_USDC,
            "Free liquidity stays the same (total_assets and unclaimed_fees both decrease)"
        );
    }

    /// Claim a partial amount of fees. The remainder must stay as unclaimed.
    #[test]
    fn test_claim_fees_to_partial() {
        let fix = setup();
        seed_vault(&fix, 100 * ONE_USDC);

        // Accrue 10 USDC in fees
        fix.vault_client
            .accrue_fees(&fix.position_manager, &(10 * ONE_USDC));

        let recipient = Address::generate(&fix.env);

        // Claim only 3 USDC out of 10
        fix.vault_client.claim_fees_to(
            &fix.position_manager,
            &recipient,
            &(3 * ONE_USDC),
        );

        // Recipient gets 3 USDC
        assert_eq!(
            fix.token_client.balance(&recipient),
            3 * ONE_USDC,
            "Recipient should receive exactly the partial claim amount"
        );

        // total_assets = 97 (3 sent out), unclaimed = 7 => free_liq = 97 - 0 - 7 - 0 = 90
        assert_eq!(
            fix.vault_client.free_liquidity(),
            90 * ONE_USDC,
            "Free liquidity unchanged (total_assets and unclaimed_fees both decrease proportionally)"
        );
    }

    /// Multiple sequential partial claims should each decrement correctly.
    #[test]
    fn test_claim_fees_to_multiple_partial_claims() {
        let fix = setup();
        seed_vault(&fix, 100 * ONE_USDC);

        // Accrue 20 USDC in fees
        fix.vault_client
            .accrue_fees(&fix.position_manager, &(20 * ONE_USDC));

        let recipient_a = Address::generate(&fix.env);
        let recipient_b = Address::generate(&fix.env);

        // First claim: 8 USDC to recipient_a
        fix.vault_client.claim_fees_to(
            &fix.position_manager,
            &recipient_a,
            &(8 * ONE_USDC),
        );

        // Second claim: 5 USDC to recipient_b
        fix.vault_client.claim_fees_to(
            &fix.position_manager,
            &recipient_b,
            &(5 * ONE_USDC),
        );

        assert_eq!(
            fix.token_client.balance(&recipient_a),
            8 * ONE_USDC,
            "First recipient should have 8 USDC"
        );
        assert_eq!(
            fix.token_client.balance(&recipient_b),
            5 * ONE_USDC,
            "Second recipient should have 5 USDC"
        );

        // total_assets = 87 (13 sent out), unclaimed = 7 => free_liq = 87 - 0 - 7 - 0 = 80
        assert_eq!(
            fix.vault_client.free_liquidity(),
            80 * ONE_USDC,
            "Free liquidity reflects both reduced total_assets and remaining unclaimed fees"
        );
    }

    /// Claim exactly 1 unit (minimum non-zero). Verify precision at the
    /// smallest granularity.
    #[test]
    fn test_claim_fees_to_minimum_amount() {
        let fix = setup();
        seed_vault(&fix, 100 * ONE_USDC);

        // Accrue 1 stroop (smallest unit)
        fix.vault_client
            .accrue_fees(&fix.position_manager, &1i128);

        let recipient = Address::generate(&fix.env);
        fix.vault_client
            .claim_fees_to(&fix.position_manager, &recipient, &1i128);

        assert_eq!(
            fix.token_client.balance(&recipient),
            1i128,
            "Recipient should receive exactly 1 stroop"
        );
    }
}

// ===========================================================================
// claim_fees_to -- access control / authorization tests
// ===========================================================================

mod claim_fees_to_auth {
    use super::*;

    /// A random address (not PM) must be rejected with NotPositionManager (#7).
    #[test]
    #[should_panic(expected = "Error(Contract, #7)")]
    fn test_claim_fees_to_not_pm() {
        let fix = setup();
        seed_vault(&fix, 100 * ONE_USDC);

        fix.vault_client
            .accrue_fees(&fix.position_manager, &(10 * ONE_USDC));

        let attacker = Address::generate(&fix.env);
        let recipient = Address::generate(&fix.env);

        // Non-PM caller -- must panic with NotPositionManager
        fix.vault_client
            .claim_fees_to(&attacker, &recipient, &(5 * ONE_USDC));
    }

    /// The admin address is NOT the position manager and must also be rejected.
    #[test]
    #[should_panic(expected = "Error(Contract, #7)")]
    fn test_claim_fees_to_admin_is_not_pm() {
        let fix = setup();
        seed_vault(&fix, 100 * ONE_USDC);

        fix.vault_client
            .accrue_fees(&fix.position_manager, &(10 * ONE_USDC));

        let recipient = Address::generate(&fix.env);

        // Admin is not PM -- must fail
        fix.vault_client
            .claim_fees_to(&fix.admin, &recipient, &(5 * ONE_USDC));
    }

    /// An address with the KEEPER role is still not PM and must be rejected.
    #[test]
    #[should_panic(expected = "Error(Contract, #7)")]
    fn test_claim_fees_to_keeper_role_not_pm() {
        let fix = setup();
        seed_vault(&fix, 100 * ONE_USDC);

        fix.vault_client
            .accrue_fees(&fix.position_manager, &(10 * ONE_USDC));

        // Grant KEEPER role to some address -- still not PM
        let keeper = Address::generate(&fix.env);
        let keeper_role = Symbol::new(&fix.env, shared::constants::ROLE_KEEPER);
        fix.config_client
            .grant_role(&fix.admin, &keeper_role, &keeper);

        let recipient = Address::generate(&fix.env);
        fix.vault_client
            .claim_fees_to(&keeper, &recipient, &(5 * ONE_USDC));
    }
}

// ===========================================================================
// claim_fees_to -- input validation tests
// ===========================================================================

mod claim_fees_to_validation {
    use super::*;

    /// amount > unclaimed_fees must panic with InsufficientFees (#10).
    #[test]
    #[should_panic(expected = "Error(Contract, #10)")]
    fn test_claim_fees_to_exceeds_unclaimed() {
        let fix = setup();
        seed_vault(&fix, 100 * ONE_USDC);

        // Accrue only 5 USDC
        fix.vault_client
            .accrue_fees(&fix.position_manager, &(5 * ONE_USDC));

        let recipient = Address::generate(&fix.env);

        // Try to claim 10 USDC -- exceeds the 5 USDC available
        fix.vault_client.claim_fees_to(
            &fix.position_manager,
            &recipient,
            &(10 * ONE_USDC),
        );
    }

    /// amount == 0 must panic with ZeroAmount (#6).
    #[test]
    #[should_panic(expected = "Error(Contract, #6)")]
    fn test_claim_fees_to_zero_amount() {
        let fix = setup();
        seed_vault(&fix, 100 * ONE_USDC);

        fix.vault_client
            .accrue_fees(&fix.position_manager, &(10 * ONE_USDC));

        let recipient = Address::generate(&fix.env);

        // Zero amount -- must panic
        fix.vault_client
            .claim_fees_to(&fix.position_manager, &recipient, &0i128);
    }

    /// Negative amount must also panic with ZeroAmount (#6), same as other
    /// vault endpoints that reject amount <= 0.
    #[test]
    #[should_panic(expected = "Error(Contract, #6)")]
    fn test_claim_fees_to_negative_amount() {
        let fix = setup();
        seed_vault(&fix, 100 * ONE_USDC);

        fix.vault_client
            .accrue_fees(&fix.position_manager, &(10 * ONE_USDC));

        let recipient = Address::generate(&fix.env);

        fix.vault_client
            .claim_fees_to(&fix.position_manager, &recipient, &(-1i128));
    }

    /// Claiming exactly unclaimed_fees (boundary) must succeed -- not off-by-one.
    #[test]
    fn test_claim_fees_to_exact_boundary() {
        let fix = setup();
        seed_vault(&fix, 100 * ONE_USDC);

        fix.vault_client
            .accrue_fees(&fix.position_manager, &(7 * ONE_USDC));

        let recipient = Address::generate(&fix.env);

        // Claim exactly 7 USDC -- must succeed
        fix.vault_client.claim_fees_to(
            &fix.position_manager,
            &recipient,
            &(7 * ONE_USDC),
        );

        assert_eq!(
            fix.token_client.balance(&recipient),
            7 * ONE_USDC,
            "Claiming exactly the unclaimed amount must succeed"
        );

        // total_assets = 93 (7 sent out), unclaimed = 0 => free_liq = 93 - 0 - 0 - 0 = 93
        assert_eq!(
            fix.vault_client.free_liquidity(),
            93 * ONE_USDC,
            "Free liquidity = total_assets after all fees claimed and sent out"
        );
    }

    /// Claiming unclaimed_fees + 1 stroop must fail with InsufficientFees.
    #[test]
    #[should_panic(expected = "Error(Contract, #10)")]
    fn test_claim_fees_to_one_over_boundary() {
        let fix = setup();
        seed_vault(&fix, 100 * ONE_USDC);

        fix.vault_client
            .accrue_fees(&fix.position_manager, &(7 * ONE_USDC));

        let recipient = Address::generate(&fix.env);

        // One stroop over the unclaimed amount
        fix.vault_client.claim_fees_to(
            &fix.position_manager,
            &recipient,
            &(7 * ONE_USDC + 1),
        );
    }

    /// When no fees have been accrued at all, any positive claim must fail
    /// with InsufficientFees (#10).
    #[test]
    #[should_panic(expected = "Error(Contract, #10)")]
    fn test_claim_fees_to_no_fees_accrued() {
        let fix = setup();
        seed_vault(&fix, 100 * ONE_USDC);

        let recipient = Address::generate(&fix.env);

        // No accrue_fees called -- unclaimed is 0, claiming 1 must fail
        fix.vault_client
            .claim_fees_to(&fix.position_manager, &recipient, &1i128);
    }
}

// ===========================================================================
// claim_fees_to -- state consistency / edge cases
// ===========================================================================

mod claim_fees_to_state {
    use super::*;

    /// After a full claim via claim_fees_to, the old claim_fees (admin)
    /// should have nothing left to claim.
    #[test]
    #[should_panic(expected = "Error(Contract, #6)")]
    fn test_claim_fees_to_then_claim_fees_empty() {
        let fix = setup();
        seed_vault(&fix, 100 * ONE_USDC);

        fix.vault_client
            .accrue_fees(&fix.position_manager, &(10 * ONE_USDC));

        let recipient = Address::generate(&fix.env);

        // PM claims all fees via claim_fees_to
        fix.vault_client.claim_fees_to(
            &fix.position_manager,
            &recipient,
            &(10 * ONE_USDC),
        );

        // Admin tries to claim via old claim_fees -- should fail (ZeroAmount)
        let admin_recipient = Address::generate(&fix.env);
        fix.vault_client
            .claim_fees(&fix.admin, &admin_recipient);
    }

    /// Vault token balance must decrease by exactly the claimed amount.
    #[test]
    fn test_claim_fees_to_vault_balance_decreases() {
        let fix = setup();
        seed_vault(&fix, 100 * ONE_USDC);

        fix.vault_client
            .accrue_fees(&fix.position_manager, &(10 * ONE_USDC));

        let vault_balance_before = fix.token_client.balance(&fix.vault_id);

        let recipient = Address::generate(&fix.env);
        fix.vault_client.claim_fees_to(
            &fix.position_manager,
            &recipient,
            &(6 * ONE_USDC),
        );

        let vault_balance_after = fix.token_client.balance(&fix.vault_id);
        assert_eq!(
            vault_balance_before - vault_balance_after,
            6 * ONE_USDC,
            "Vault underlying balance must decrease by exactly the claimed amount"
        );
    }
}
