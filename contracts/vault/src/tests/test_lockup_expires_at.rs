#![cfg(test)]

//! Tests for the LP lockup migration: `LastDepositTime` -> `LockupExpiresAt`.
//!
//! Migration spec (step 3 of the multi-step lockup feature):
//!   * On deposit/mint: write `now + cooldown_duration` (read from ConfigManager)
//!     to per-user persistent storage.
//!   * On withdraw/redeem: panic `CooldownNotElapsed` (#8) if `now < stored_expiry`.
//!   * Frozen-at-deposit: subsequent admin changes to `cooldown_duration` MUST
//!     NOT affect already-stored lockups.
//!   * Multiple deposits reset (overwrite) the lockup to the new
//!     `now + cooldown_duration`.
//!   * New view `lockup_expires_at(user) -> u64`: 0 if no deposit was ever made,
//!     otherwise the stored expiry timestamp (may be in the past — view is
//!     informational, not a guard).
//!   * New event `Lockup { user, expires_at }` with topic `"lockup"`, emitted
//!     on every deposit/mint that sets a lockup.
//!
//! NOTE: Existing tests in `test_withdraw.rs`, `test_deposit.rs`, etc. set
//! `cooldown_duration: 0`, which under both the old and new semantics means
//! "no lockup ever". They must continue passing post-migration. The
//! `test_zero_cooldown_disables_lockup` case below is a regression anchor for
//! that property.

use soroban_sdk::{
    testutils::{Address as _, Events as _, Ledger, LedgerInfo},
    Address, Env, IntoVal, String, Symbol, TryIntoVal, Val,
};

// ---------------------------------------------------------------------------
// Constants -- 7-decimal USDC (Stellar standard, matches test_withdraw.rs)
// ---------------------------------------------------------------------------

const DECIMALS: u32 = 7;
const ONE_USDC: i128 = 10_000_000; // 1.0000000

// ---------------------------------------------------------------------------
// Test fixture (mirrors test_withdraw.rs `setup()` to reuse its conventions).
// ---------------------------------------------------------------------------

struct TestFixture {
    env: Env,
    admin: Address,
    token_client: mock_token::MockTokenClient<'static>,
    config_client: config_manager::ConfigManagerClient<'static>,
    vault_id: Address,
    vault_client: crate::VaultContractClient<'static>,
}

/// Deploy mock-token, config-manager, and vault. Initialize all three with
/// `cooldown_duration: 0` (matching the existing fixture). Tests override the
/// cooldown to a non-zero value before depositing.
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
        token_client,
        config_client,
        vault_id,
        vault_client,
    }
}

/// Set the protocol-wide `cooldown_duration` (resets all other limits to the
/// fixture defaults).
fn set_cooldown(fix: &TestFixture, cooldown_duration: u64) {
    fix.config_client.update_protocol_limits(
        &fix.admin,
        &config_manager::ProtocolLimits {
            min_collateral: 1,
            cooldown_duration,
            min_position_lifetime: 0,
            max_utilization_ratio: 10_000,
            adl_pnl_bps: 9_000,
            adl_utilization_bps: 9_500,
            liquidation_threshold_bps: 200,
        },
    );
}

/// Set the ledger timestamp (other LedgerInfo fields fixed to safe defaults).
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

/// Mint USDC to `addr` and deposit into the vault. Returns the shares minted.
fn deposit_usdc(fix: &TestFixture, addr: &Address, amount: i128) -> i128 {
    fix.token_client.mint(addr, &amount);
    fix.vault_client.deposit(&amount, addr, addr, addr)
}

// ===========================================================================
// 1. View returns 0 for an address that has never deposited.
// ===========================================================================

#[test]
fn test_lockup_expires_at_zero_for_user_who_never_deposited() {
    let fix = setup();
    let stranger = Address::generate(&fix.env);

    // No deposit has ever happened for `stranger`.
    let expiry = fix.vault_client.lockup_expires_at(&stranger);
    assert_eq!(
        expiry, 0,
        "lockup_expires_at must return 0 for a user that never deposited"
    );
}

// ===========================================================================
// 2. Deposit at T=1000 with cooldown=300 sets expiry to exactly 1300.
//
// Timeline:
//   T=1000   user deposits 100 USDC (cooldown_duration = 300)
//            => stored expiry = 1000 + 300 = 1300
// ===========================================================================

#[test]
fn test_deposit_sets_lockup_expires_at_now_plus_cooldown() {
    let fix = setup();
    let user = Address::generate(&fix.env);

    set_ts(&fix, 1000);
    set_cooldown(&fix, 300);

    deposit_usdc(&fix, &user, 100 * ONE_USDC);

    let expiry = fix.vault_client.lockup_expires_at(&user);
    assert_eq!(
        expiry, 1300,
        "lockup_expires_at must equal now + cooldown_duration (1000 + 300 = 1300)"
    );
}

// ===========================================================================
// 3. Withdraw before lockup expiry reverts with CooldownNotElapsed (#8).
//
// Timeline:
//   T=1000   deposit (cooldown=300) => expiry=1300
//   T=1299   withdraw -> 1299 < 1300 -> reverts #8
// ===========================================================================

#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn test_withdraw_reverts_before_lockup_expiry() {
    let fix = setup();
    let user = Address::generate(&fix.env);

    set_ts(&fix, 1000);
    set_cooldown(&fix, 300);
    deposit_usdc(&fix, &user, 100 * ONE_USDC);

    // Advance to one second before expiry
    set_ts(&fix, 1299);

    // Must panic with VaultError::CooldownNotElapsed = 8
    fix.vault_client
        .withdraw(&(50 * ONE_USDC), &user, &user, &user);
}

// ===========================================================================
// 4. Withdraw at lockup expiry succeeds (boundary: now == expiry, the check
//    is strict `now < expiry` so 1300 < 1300 is false => allowed).
//
// Timeline:
//   T=1000   deposit (cooldown=300) => expiry=1300
//   T=1300   withdraw 50 USDC -> succeeds
// ===========================================================================

#[test]
fn test_withdraw_succeeds_at_lockup_expiry() {
    let fix = setup();
    let user = Address::generate(&fix.env);

    set_ts(&fix, 1000);
    set_cooldown(&fix, 300);
    deposit_usdc(&fix, &user, 100 * ONE_USDC);

    set_ts(&fix, 1300);

    // 1300 < 1300 == false => allowed.
    let user_token_before = fix.token_client.balance(&user);
    fix.vault_client
        .withdraw(&(50 * ONE_USDC), &user, &user, &user);
    let user_token_after = fix.token_client.balance(&user);

    assert_eq!(
        user_token_after - user_token_before,
        50 * ONE_USDC,
        "withdraw must succeed at the exact lockup expiry boundary (now == expiry)"
    );
}

// ===========================================================================
// 5. Withdraw long after expiry succeeds.
//
// Timeline:
//   T=1000   deposit (cooldown=300) => expiry=1300
//   T=9999   withdraw -> succeeds (1300 in the deep past)
// ===========================================================================

#[test]
fn test_withdraw_succeeds_after_lockup_expiry() {
    let fix = setup();
    let user = Address::generate(&fix.env);

    set_ts(&fix, 1000);
    set_cooldown(&fix, 300);
    deposit_usdc(&fix, &user, 100 * ONE_USDC);

    set_ts(&fix, 9999);

    let user_token_before = fix.token_client.balance(&user);
    fix.vault_client
        .withdraw(&(50 * ONE_USDC), &user, &user, &user);
    let user_token_after = fix.token_client.balance(&user);

    assert_eq!(
        user_token_after - user_token_before,
        50 * ONE_USDC,
        "withdraw must succeed long after the lockup expiry"
    );
}

// ===========================================================================
// 6. KEY: admin shortening cooldown does NOT release an existing lockup.
//
// Frozen-at-deposit semantics: the stored expiry is fixed at deposit time and
// is unaffected by later cooldown_duration changes.
//
// Timeline:
//   T=1000   cooldown=600, deposit => expiry=1600 (frozen)
//            admin updates cooldown to 60 (NEW deposits would only lock 60s)
//   T=1100   withdraw -> would be unlocked under the NEW cooldown (1100>=1060),
//            but the stored expiry is still 1600 -> reverts #8
// ===========================================================================

#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn test_admin_shortening_cooldown_does_not_release_existing_lockup() {
    let fix = setup();
    let user = Address::generate(&fix.env);

    set_ts(&fix, 1000);
    set_cooldown(&fix, 600);
    deposit_usdc(&fix, &user, 100 * ONE_USDC);
    // Stored expiry is 1600 (frozen).

    // Admin shortens the cooldown after the deposit. This MUST NOT affect the
    // already-stored lockup.
    set_cooldown(&fix, 60);

    // 100 seconds after deposit -- only 60s would be required under the NEW
    // cooldown, but the lockup is frozen at 1600.
    set_ts(&fix, 1100);

    // Must panic with #8 because 1100 < 1600.
    fix.vault_client
        .withdraw(&(50 * ONE_USDC), &user, &user, &user);
}

// ===========================================================================
// 7. Symmetric: admin lengthening cooldown does NOT extend an existing lockup.
//
// Timeline:
//   T=1000   cooldown=60, deposit => expiry=1060 (frozen)
//            admin updates cooldown to 86_400 (NEW deposits would lock 24h)
//   T=1060   withdraw -> 1060 < 1060 == false -> succeeds (frozen expiry)
// ===========================================================================

#[test]
fn test_admin_lengthening_cooldown_does_not_extend_existing_lockup() {
    let fix = setup();
    let user = Address::generate(&fix.env);

    set_ts(&fix, 1000);
    set_cooldown(&fix, 60);
    deposit_usdc(&fix, &user, 100 * ONE_USDC);
    // Stored expiry is 1060 (frozen).

    // Admin extends cooldown.
    set_cooldown(&fix, 86_400);

    // Advance to the original expiry (frozen).
    set_ts(&fix, 1060);

    // Must succeed: stored expiry was frozen at 1060, even though new deposits
    // would now lock for 86_400 seconds.
    let user_token_before = fix.token_client.balance(&user);
    fix.vault_client
        .withdraw(&(50 * ONE_USDC), &user, &user, &user);
    let user_token_after = fix.token_client.balance(&user);

    assert_eq!(
        user_token_after - user_token_before,
        50 * ONE_USDC,
        "lengthening cooldown post-deposit must NOT extend an existing lockup"
    );
}

// ===========================================================================
// 8. Second deposit resets the lockup to a new expiry (overwrite, not min/max).
//
// Timeline:
//   T=1000   cooldown=300, deposit#1 => expiry=1300
//   T=1100   deposit#2 => expiry=1400 (overwrites 1300)
//   T=1300   withdraw -> would be allowed under deposit#1 (1300 == 1300), but
//            the stored expiry is now 1400 -> reverts #8
//   T=1400   withdraw -> succeeds
// ===========================================================================

#[test]
fn test_second_deposit_resets_lockup_to_new_expiry() {
    let fix = setup();
    let user = Address::generate(&fix.env);

    set_ts(&fix, 1000);
    set_cooldown(&fix, 300);
    deposit_usdc(&fix, &user, 50 * ONE_USDC);
    assert_eq!(
        fix.vault_client.lockup_expires_at(&user),
        1300,
        "first deposit at T=1000 must set expiry to 1300"
    );

    // Second deposit 100 seconds later (still locked under deposit#1).
    set_ts(&fix, 1100);
    deposit_usdc(&fix, &user, 50 * ONE_USDC);
    assert_eq!(
        fix.vault_client.lockup_expires_at(&user),
        1400,
        "second deposit must overwrite the lockup to now + cooldown (1100 + 300 = 1400)"
    );

    // At T=1300 (deposit#1's old expiry), withdraw must fail because the new
    // expiry is 1400.
    set_ts(&fix, 1300);
    let res = fix
        .vault_client
        .try_withdraw(&(10 * ONE_USDC), &user, &user, &user);
    match res {
        Ok(_) => panic!(
            "withdraw at T=1300 must revert because the second deposit reset the lockup to 1400"
        ),
        Err(_) => { /* expected */ }
    }

    // At T=1400 (the new expiry, boundary inclusive: 1400 < 1400 == false),
    // withdraw must succeed.
    set_ts(&fix, 1400);
    let user_token_before = fix.token_client.balance(&user);
    fix.vault_client
        .withdraw(&(10 * ONE_USDC), &user, &user, &user);
    let user_token_after = fix.token_client.balance(&user);
    assert_eq!(
        user_token_after - user_token_before,
        10 * ONE_USDC,
        "withdraw must succeed at the second deposit's reset expiry T=1400"
    );
}

// ===========================================================================
// 9. Deposit emits a "lockup" event with `expires_at` and the user address.
//
// The vault `events.rs` uses `data_format = "vec"` for all events, so the
// `Lockup` event payload should be a Vec<Val> matching the struct fields in
// declaration order: [user, expires_at]. (If `user` is a `#[topic]`, it is in
// the topics tuple instead and not in the data — adjust the test if the event
// keeps `user` as data; the current OZ pattern in this repo allows both. Here
// we assert via topic == "lockup" + data containing the expires_at value, and
// also verify the user appears either in topics or in data.)
// ===========================================================================

#[test]
fn test_deposit_emits_lockup_event_with_expiry() {
    let fix = setup();
    let user = Address::generate(&fix.env);

    set_ts(&fix, 1000);
    set_cooldown(&fix, 300);
    deposit_usdc(&fix, &user, 100 * ONE_USDC);

    let lockup_topic: Symbol = Symbol::new(&fix.env, "lockup");
    let mut found: Option<(soroban_sdk::Vec<Val>, Val)> = None;

    let all = fix.env.events().all();
    for entry in all.iter() {
        let (contract, topics, data) = entry;
        if contract != fix.vault_id {
            continue;
        }
        if topics.len() == 0 {
            continue;
        }
        let first_topic_val = topics.get(0).unwrap();
        let first_topic: Result<Symbol, _> = first_topic_val.try_into_val(&fix.env);
        if let Ok(s) = first_topic {
            if s == lockup_topic {
                found = Some((topics, data));
                break;
            }
        }
    }

    let (topics, data) =
        found.expect("deposit must emit a `lockup`-topic event from the vault contract");

    // The event must reference the user address: either in topics (if `user`
    // is `#[topic]`) or in the data Vec (if `user` is plain). Iterate both
    // and assert the user appears at least once.
    let user_val: Val = (&user).into_val(&fix.env);
    let mut user_present = false;
    for i in 0..topics.len() {
        let t = topics.get(i).unwrap();
        let parsed: Result<Address, _> = t.try_into_val(&fix.env);
        if let Ok(a) = parsed {
            if a == user {
                user_present = true;
                break;
            }
        }
        // String/Val identity fallback (covers raw Val comparisons).
        let _ = user_val;
    }

    // The vault Lockup event is emitted as `data_format = "vec"` with both
    // `user` and `expires_at` as plain (non-topic) data fields. Match exactly
    // that shape — attempting alternative shapes via try_into_val panics
    // unrecoverably inside the SDK on size mismatch.
    let (event_user, expires_at): (Address, u64) = data.try_into_val(&fix.env).expect(
        "lockup event data must unpack as (Address, u64) -- vec payload of (user, expires_at)",
    );

    assert_eq!(
        event_user, user,
        "lockup event must reference the depositing user in its data payload"
    );
    assert_eq!(
        expires_at, 1300,
        "lockup event must carry expires_at = now + cooldown_duration (1000 + 300 = 1300)"
    );
    let _ = user_present;
}

// ===========================================================================
// 10. mint() path also sets the lockup (parity with deposit()).
//
// Timeline:
//   T=1000   cooldown=300, mint shares -> expiry=1300
// ===========================================================================

#[test]
fn test_mint_path_also_sets_lockup() {
    let fix = setup();
    let user = Address::generate(&fix.env);

    set_ts(&fix, 1000);
    set_cooldown(&fix, 300);

    // Pre-mint USDC so the vault can pull assets to back the shares.
    let assets_needed = 100 * ONE_USDC;
    fix.token_client.mint(&user, &assets_needed);

    // Compute shares for that asset amount, then mint.
    let shares = fix.vault_client.preview_deposit(&assets_needed);
    fix.vault_client.mint(&shares, &user, &user, &user);

    let expiry = fix.vault_client.lockup_expires_at(&user);
    assert_eq!(
        expiry, 1300,
        "mint() path must set lockup_expires_at to now + cooldown_duration (1000 + 300 = 1300)"
    );
}

// ===========================================================================
// 11. redeem() path also enforces the lockup (parity with withdraw()).
//
// Timeline:
//   T=1000   cooldown=300, deposit => expiry=1300
//   T=1299   redeem -> 1299 < 1300 -> reverts #8
// ===========================================================================

#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn test_redeem_path_also_enforces_lockup() {
    let fix = setup();
    let user = Address::generate(&fix.env);

    set_ts(&fix, 1000);
    set_cooldown(&fix, 300);
    let shares = deposit_usdc(&fix, &user, 100 * ONE_USDC);

    set_ts(&fix, 1299);

    // Must panic with VaultError::CooldownNotElapsed = 8
    fix.vault_client.redeem(&(shares / 2), &user, &user, &user);
}

// ===========================================================================
// 12. Zero cooldown disables the lockup (regression anchor).
//
// Existing tests in test_withdraw.rs and test_deposit.rs run with
// `cooldown_duration: 0`. Under both the old (`now < last_deposit + 0`) and
// the new (`now < now + 0`) semantics this evaluates to false, so withdraw is
// always allowed. This test pins that property.
//
// Timeline:
//   T=1000   cooldown=0, deposit => expiry=1000
//            immediate withdraw -> 1000 < 1000 == false -> succeeds
// ===========================================================================

#[test]
fn test_zero_cooldown_disables_lockup() {
    let fix = setup();
    let user = Address::generate(&fix.env);

    set_ts(&fix, 1000);
    set_cooldown(&fix, 0);
    deposit_usdc(&fix, &user, 100 * ONE_USDC);

    assert_eq!(
        fix.vault_client.lockup_expires_at(&user),
        1000,
        "zero cooldown must store expiry == now (1000)"
    );

    // Immediate withdraw at the same timestamp must succeed.
    let user_token_before = fix.token_client.balance(&user);
    fix.vault_client
        .withdraw(&(50 * ONE_USDC), &user, &user, &user);
    let user_token_after = fix.token_client.balance(&user);

    assert_eq!(
        user_token_after - user_token_before,
        50 * ONE_USDC,
        "zero cooldown must allow immediate withdraw (regression anchor for existing tests)"
    );
}

// ===========================================================================
// ADVERSARIAL EDGE CASES (additional)
// ===========================================================================

// ---------------------------------------------------------------------------
// 13. lockup_expires_at view does NOT panic when the stored expiry is in the
//     past. The view is informational, not a guard.
// ---------------------------------------------------------------------------

#[test]
fn test_lockup_expires_at_view_returns_past_value() {
    let fix = setup();
    let user = Address::generate(&fix.env);

    set_ts(&fix, 1000);
    set_cooldown(&fix, 300);
    deposit_usdc(&fix, &user, 100 * ONE_USDC);

    // Advance well past the expiry.
    set_ts(&fix, 999_999);

    let expiry = fix.vault_client.lockup_expires_at(&user);
    assert_eq!(
        expiry, 1300,
        "lockup_expires_at must return the stored value even when it is far in the past (informational view)"
    );
}

// ---------------------------------------------------------------------------
// 14. Each user has their own independent lockup (no cross-contamination).
//
// Timeline:
//   T=1000   cooldown=300, alice deposits => alice.expiry=1300
//   T=1500   bob deposits => bob.expiry=1800
//   T=1700   alice can withdraw (1700 >= 1300); bob cannot (1700 < 1800)
// ---------------------------------------------------------------------------

#[test]
fn test_lockup_per_user_independent() {
    let fix = setup();
    let alice = Address::generate(&fix.env);
    let bob = Address::generate(&fix.env);

    set_ts(&fix, 1000);
    set_cooldown(&fix, 300);
    deposit_usdc(&fix, &alice, 100 * ONE_USDC);

    set_ts(&fix, 1500);
    deposit_usdc(&fix, &bob, 100 * ONE_USDC);

    assert_eq!(fix.vault_client.lockup_expires_at(&alice), 1300);
    assert_eq!(fix.vault_client.lockup_expires_at(&bob), 1800);

    // Advance to T=1700: alice unlocked, bob still locked.
    set_ts(&fix, 1700);

    // Alice succeeds.
    let alice_before = fix.token_client.balance(&alice);
    fix.vault_client
        .withdraw(&(10 * ONE_USDC), &alice, &alice, &alice);
    assert_eq!(
        fix.token_client.balance(&alice) - alice_before,
        10 * ONE_USDC,
        "alice must be able to withdraw at T=1700 (her expiry was 1300)"
    );

    // Bob fails.
    let bob_res = fix
        .vault_client
        .try_withdraw(&(10 * ONE_USDC), &bob, &bob, &bob);
    match bob_res {
        Ok(_) => panic!("bob must NOT be able to withdraw at T=1700 (his expiry is 1800)"),
        Err(_) => { /* expected */ }
    }
}

// ---------------------------------------------------------------------------
// 15. mint() with non-zero cooldown also enforces the lockup on subsequent
//     redeem.
//
// Timeline:
//   T=1000   cooldown=300, mint => expiry=1300
//   T=1100   redeem -> reverts #8
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn test_mint_then_redeem_within_lockup_reverts() {
    let fix = setup();
    let user = Address::generate(&fix.env);

    set_ts(&fix, 1000);
    set_cooldown(&fix, 300);

    let assets_needed = 100 * ONE_USDC;
    fix.token_client.mint(&user, &assets_needed);
    let shares = fix.vault_client.preview_deposit(&assets_needed);
    fix.vault_client.mint(&shares, &user, &user, &user);

    set_ts(&fix, 1100);

    // Must panic with VaultError::CooldownNotElapsed = 8
    fix.vault_client.redeem(&(shares / 2), &user, &user, &user);
}

// ---------------------------------------------------------------------------
// 16. After a successful withdraw past the lockup, a new deposit re-arms the
//     lockup — the old expiry must NOT linger as the floor.
//
// Timeline:
//   T=1000   cooldown=300, deposit#1 => expiry=1300
//   T=2000   withdraw 50 (allowed), deposit#2 (90 USDC) => expiry=2300
//   T=2100   withdraw -> 2100 < 2300 -> reverts #8 (NOT allowed by old expiry)
// ---------------------------------------------------------------------------

#[test]
fn test_post_withdraw_redeposit_rearms_lockup() {
    let fix = setup();
    let user = Address::generate(&fix.env);

    set_ts(&fix, 1000);
    set_cooldown(&fix, 300);
    deposit_usdc(&fix, &user, 100 * ONE_USDC);

    set_ts(&fix, 2000);

    // Withdraw a tiny portion (allowed: 2000 >= 1300).
    fix.vault_client
        .withdraw(&(10 * ONE_USDC), &user, &user, &user);

    // Re-deposit at T=2000 — must overwrite the lockup to 2300.
    deposit_usdc(&fix, &user, 90 * ONE_USDC);
    assert_eq!(
        fix.vault_client.lockup_expires_at(&user),
        2300,
        "re-deposit after a successful withdraw must reset the expiry to now + cooldown"
    );

    set_ts(&fix, 2100);

    // Must revert: even though the user previously had an expired lockup, the
    // new deposit re-armed it.
    let res = fix
        .vault_client
        .try_withdraw(&(5 * ONE_USDC), &user, &user, &user);
    match res {
        Ok(_) => panic!(
            "withdraw at T=2100 must revert (#8) because the redeposit at T=2000 reset the expiry to 2300"
        ),
        Err(_) => { /* expected */ }
    }
}

// ---------------------------------------------------------------------------
// 17. Third-party deposits (receiver != from) are rejected.
//
// Only the depositor themselves can credit shares to their own address. This
// prevents an adversary from repeatedly resetting another user's lockup by
// making tiny deposits in their name.
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "Error(Contract, #13)")]
fn test_third_party_deposit_is_rejected() {
    let fix = setup();
    let sender = Address::generate(&fix.env);
    let receiver = Address::generate(&fix.env);

    set_ts(&fix, 1000);
    set_cooldown(&fix, 300);

    let amount = 100 * ONE_USDC;
    fix.token_client.mint(&sender, &amount);
    fix.vault_client
        .deposit(&amount, &receiver, &sender, &sender);
}

// ===========================================================================
// 18. Mint zero shares — must revert with ZeroAmount (#6).
//
// Symmetric guard with `test_deposit_zero_amount_reverts`. Without this,
// anyone could extend a victim's lockup for free by minting 0 shares to them.
// ===========================================================================

#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn test_mint_zero_shares_reverts() {
    let fix = setup();
    let depositor = Address::generate(&fix.env);
    fix.vault_client
        .mint(&0i128, &depositor, &depositor, &depositor);
}

// ===========================================================================
// 19. Share transfer propagates lockup to recipient (max-of semantics).
//
// Without propagation, an LP could deposit, transfer shares to a fresh address,
// and that address could withdraw immediately, bypassing the cooldown.
//
// Timeline:
//   T=1000  alice deposits 100 USDC (cooldown=300) -> alice.expiry=1300
//   T=1100  alice transfers shares to bob          -> bob.expiry=1300 (inherited)
//   T=1100  bob attempts withdraw -> reverts CooldownNotElapsed (#8)
//   T=1300  bob withdraws -> succeeds
// ===========================================================================

#[test]
fn test_share_transfer_propagates_lockup() {
    let fix = setup();
    let alice = Address::generate(&fix.env);
    let bob = Address::generate(&fix.env);

    set_ts(&fix, 1000);
    set_cooldown(&fix, 300);

    let amount = 100 * ONE_USDC;
    let shares = deposit_usdc(&fix, &alice, amount);

    // Bob has no lockup yet.
    assert_eq!(fix.vault_client.lockup_expires_at(&bob), 0);

    set_ts(&fix, 1100);
    let bob_muxed: soroban_sdk::MuxedAddress = bob.clone().into();
    fix.vault_client.transfer(&alice, &bob_muxed, &shares);

    // Bob inherited alice's expiry (1300 > bob's 0).
    assert_eq!(
        fix.vault_client.lockup_expires_at(&bob),
        1300,
        "transfer must propagate sender's expiry to recipient (max-of semantics)"
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn test_share_transfer_recipient_cannot_bypass_lockup() {
    let fix = setup();
    let alice = Address::generate(&fix.env);
    let bob = Address::generate(&fix.env);

    set_ts(&fix, 1000);
    set_cooldown(&fix, 300);

    let amount = 100 * ONE_USDC;
    let shares = deposit_usdc(&fix, &alice, amount);

    set_ts(&fix, 1100);
    let bob_muxed: soroban_sdk::MuxedAddress = bob.clone().into();
    fix.vault_client.transfer(&alice, &bob_muxed, &shares);

    // Bob attempts to withdraw before the inherited expiry — must revert.
    fix.vault_client.withdraw(&(amount / 2), &bob, &bob, &bob);
}

#[test]
fn test_share_transfer_keeps_recipients_longer_lockup() {
    let fix = setup();
    let alice = Address::generate(&fix.env);
    let bob = Address::generate(&fix.env);

    set_ts(&fix, 1000);
    set_cooldown(&fix, 300);

    // Bob deposits first with a much longer-cooldown setup -> expiry 5000.
    set_cooldown(&fix, 4000);
    set_ts(&fix, 1000);
    let _ = deposit_usdc(&fix, &bob, 50 * ONE_USDC);
    assert_eq!(fix.vault_client.lockup_expires_at(&bob), 5000);

    // Reduce cooldown back; alice deposits at T=1000 too -> expiry 1300.
    set_cooldown(&fix, 300);
    let alice_shares = deposit_usdc(&fix, &alice, 100 * ONE_USDC);
    assert_eq!(fix.vault_client.lockup_expires_at(&alice), 1300);

    // Alice transfers to bob — bob's longer expiry must win (max-of, not overwrite).
    let bob_muxed: soroban_sdk::MuxedAddress = bob.clone().into();
    fix.vault_client.transfer(&alice, &bob_muxed, &alice_shares);
    assert_eq!(
        fix.vault_client.lockup_expires_at(&bob),
        5000,
        "transfer must NOT shorten recipient's existing longer lockup"
    );
}

#[test]
fn test_share_transfer_when_sender_lockup_expired_no_propagation() {
    let fix = setup();
    let alice = Address::generate(&fix.env);
    let bob = Address::generate(&fix.env);

    set_ts(&fix, 1000);
    set_cooldown(&fix, 300);

    let amount = 100 * ONE_USDC;
    let shares = deposit_usdc(&fix, &alice, amount);
    assert_eq!(fix.vault_client.lockup_expires_at(&alice), 1300);

    // Time passes well beyond alice's expiry.
    set_ts(&fix, 9999);

    let bob_muxed: soroban_sdk::MuxedAddress = bob.clone().into();
    fix.vault_client.transfer(&alice, &bob_muxed, &shares);

    // Bob never gets a lockup — alice's was already expired.
    assert_eq!(
        fix.vault_client.lockup_expires_at(&bob),
        0,
        "expired sender lockup must not be propagated"
    );
}

// ===========================================================================
// 20. transfer_from path also propagates lockup.
// ===========================================================================

#[test]
fn test_transfer_from_propagates_lockup() {
    let fix = setup();
    let alice = Address::generate(&fix.env);
    let bob = Address::generate(&fix.env);
    let spender = Address::generate(&fix.env);

    set_ts(&fix, 1000);
    set_cooldown(&fix, 300);

    let amount = 100 * ONE_USDC;
    let shares = deposit_usdc(&fix, &alice, amount);

    // Alice approves spender for the shares.
    fix.vault_client
        .approve(&alice, &spender, &shares, &10_000_000);

    set_ts(&fix, 1100);
    fix.vault_client
        .transfer_from(&spender, &alice, &bob, &shares);

    assert_eq!(
        fix.vault_client.lockup_expires_at(&bob),
        1300,
        "transfer_from must propagate sender's expiry to recipient"
    );
}
