//! Negative-auth tests: every role-gated entrypoint must panic with
//! `Unauthorized` when called by an address that does not hold the required
//! role.
//!
//! The standard Fixture grants ADMIN+PAUSER+KEEPER only to `f.admin` and
//! KEEPER also to `f.keeper`; every other address (including `f.trader` and
//! freshly-generated test addresses) is roleless. We use `f.trader` as a
//! stand-in for "any unauthorised caller" — its lack of ADMIN/PAUSER/UPGRADER
//! role makes the cross-contract `has_role` check return false and the guard
//! panic with the contract-specific Unauthorized code. `env.mock_all_auths()`
//! bypasses `caller.require_auth()` but not the role check; pre-granted roles
//! are real state, so the gates fire correctly when called by a roleless
//! address.
//!
//! Each test fires `should_panic(expected = "Error(Contract, #N)")` with the
//! correct per-crate error code:
//! - PositionManager::Unauthorized   = #7
//! - Vault::Unauthorized             = #5
//! - OracleRouter::Unauthorized      = #3
//! - ConfigManager::Unauthorized     = #3

use soroban_sdk::{symbol_short, testutils::Address as _, Address, BytesN, Env, IntoVal};
use test_suites::testutils::{Fixture, USDC_UNIT};

// ===========================================================================
// PositionManager — Unauthorized = #7
// ===========================================================================

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn pm_set_max_leverage_rejects_non_admin() {
    let env = Env::default();
    let f = Fixture::deploy(&env);
    // f.trader has no roles. ADMIN is needed for set_max_leverage.
    f.set_max_leverage(&f.trader, &symbol_short!("BTC"), &50_i128);
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn pm_disable_market_rejects_non_pauser() {
    let env = Env::default();
    let f = Fixture::deploy(&env);
    f.disable_market(&f.trader, &symbol_short!("BTC"));
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn pm_enable_market_rejects_non_pauser() {
    let env = Env::default();
    let f = Fixture::deploy(&env);
    f.enable_market(&f.trader, &symbol_short!("BTC"));
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn pm_pause_rejects_non_pauser() {
    let env = Env::default();
    let f = Fixture::deploy(&env);
    f.pause_pm(&f.trader);
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn pm_unpause_rejects_non_pauser() {
    let env = Env::default();
    let f = Fixture::deploy(&env);
    // pause first (using admin who DOES have PAUSER) so unpause has work to do
    f.pause_pm(&f.admin);
    // then a non-pauser tries to unpause
    f.unpause_pm(&f.trader);
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn pm_update_indices_rejects_non_keeper() {
    let env = Env::default();
    let f = Fixture::deploy(&env);
    // generate a fresh address — neither admin (has KEEPER) nor keeper.
    let outsider = Address::generate(&env);
    f.update_indices(&outsider, &symbol_short!("BTC"));
}

// NOTE: `liquidate_position` and `execute_order` are INTENTIONALLY
// permissionless by design (see `contracts/position-manager/src/contract.rs`
// — "liquidations must always work to prevent bad debt"). The protocol
// invites third parties to call them. There is no negative-auth test for
// these two; their permissionless nature is a deliberate trade-off and the
// authorization surface is "caller authorizes themselves to spend gas".

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn pm_deleverage_rejects_non_keeper() {
    let env = Env::default();
    let f = Fixture::deploy(&env);
    let outsider = Address::generate(&env);
    f.deleverage_position(&outsider, &f.trader, &symbol_short!("BTC"));
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn pm_propose_upgrade_rejects_non_upgrader() {
    let env = Env::default();
    let f = Fixture::deploy(&env);
    // No address holds UPGRADER in the default fixture — the trader is
    // particularly unprivileged.
    let dummy_hash: BytesN<32> = BytesN::from_array(&env, &[0u8; 32]);
    f.position_manager.propose_upgrade(&f.trader, &dummy_hash);
}

// ===========================================================================
// Vault — Unauthorized = #5
// ===========================================================================

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn vault_pause_rejects_non_pauser() {
    let env = Env::default();
    let f = Fixture::deploy(&env);
    f.pause_vault(&f.trader);
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn vault_unpause_rejects_non_pauser() {
    let env = Env::default();
    let f = Fixture::deploy(&env);
    f.pause_vault(&f.admin);
    f.unpause_vault(&f.trader);
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn vault_claim_fees_rejects_non_admin() {
    let env = Env::default();
    let f = Fixture::deploy(&env);
    // Seed some fees so claim_fees has work and reaches the auth check first.
    f.vault.accrue_fees(&f.pm_addr, &(10 * USDC_UNIT));
    let recipient = Address::generate(&env);
    f.claim_fees(&f.trader, &recipient);
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn vault_propose_upgrade_rejects_non_upgrader() {
    let env = Env::default();
    let f = Fixture::deploy(&env);
    let dummy_hash: BytesN<32> = BytesN::from_array(&env, &[0u8; 32]);
    f.vault.propose_upgrade(&f.trader, &dummy_hash);
}

// PM-only vault functions: pay_profit / reserve_liquidity / accrue_fees /
// record_absorbed_collateral all use `require_position_manager` which is
// address-equality (not role-based). Caller != pm_addr triggers
// `NotPositionManager = #7` in the vault. Worth pinning the same way.

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn vault_pay_profit_rejects_non_pm() {
    let env = Env::default();
    let f = Fixture::deploy(&env);
    f.vault.pay_profit(&f.trader, &f.trader, &(100 * USDC_UNIT));
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn vault_reserve_liquidity_rejects_non_pm() {
    let env = Env::default();
    let f = Fixture::deploy(&env);
    f.vault.reserve_liquidity(&f.trader, &(100 * USDC_UNIT));
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn vault_accrue_fees_rejects_non_pm() {
    let env = Env::default();
    let f = Fixture::deploy(&env);
    f.vault.accrue_fees(&f.trader, &(10 * USDC_UNIT));
}

// Vault `mint()` mirrors `deposit()`'s DepositMustBeSelf guard: receiver,
// from, and operator must all be the same address. These tests pin the
// rejection path for both kinds of asymmetric calls (receiver differs;
// operator differs).

#[test]
#[should_panic(expected = "Error(Contract, #13)")]
fn vault_mint_rejects_receiver_different_from_sender() {
    let env = Env::default();
    let f = Fixture::deploy(&env);
    let sender = Address::generate(&env);
    let receiver = Address::generate(&env);
    f.usdc.mint(&sender, &(50 * USDC_UNIT));
    // Request the protocol to mint shares for `receiver` paid by `sender`.
    // DepositMustBeSelf (= #13) must fire — receiver != from.
    f.mint_vault(&(10 * USDC_UNIT), &receiver, &sender, &sender);
}

#[test]
#[should_panic(expected = "Error(Contract, #13)")]
fn vault_mint_rejects_operator_different_from_sender() {
    let env = Env::default();
    let f = Fixture::deploy(&env);
    let sender = Address::generate(&env);
    let operator = Address::generate(&env);
    f.usdc.mint(&sender, &(50 * USDC_UNIT));
    // receiver == from, but operator differs — DepositMustBeSelf must fire.
    f.mint_vault(&(10 * USDC_UNIT), &sender, &sender, &operator);
}

// ===========================================================================
// OracleRouter — Unauthorized = #3
// ===========================================================================

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn oracle_set_sources_rejects_non_admin() {
    let env = Env::default();
    let f = Fixture::deploy(&env);
    let sources = soroban_sdk::vec![&env];
    f.oracle_router
        .set_oracle_sources(&f.trader, &symbol_short!("BTC"), &sources);
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn oracle_propose_upgrade_rejects_non_upgrader() {
    let env = Env::default();
    let f = Fixture::deploy(&env);
    let dummy_hash: BytesN<32> = BytesN::from_array(&env, &[0u8; 32]);
    f.oracle_router.propose_upgrade(&f.trader, &dummy_hash);
}

// ===========================================================================
// ConfigManager — Unauthorized = #3
// ===========================================================================

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn config_grant_role_rejects_non_admin() {
    let env = Env::default();
    let f = Fixture::deploy(&env);
    let role = soroban_sdk::Symbol::new(&env, "PAUSER");
    let target: Address = Address::generate(&env);
    f.config_manager.grant_role(&f.trader, &role, &target);
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn config_update_fee_splits_rejects_non_admin() {
    let env = Env::default();
    let f = Fixture::deploy(&env);
    let new_splits = config_manager::FeeSplits {
        lp_bps: 8000,
        dev_bps: 2000,
        staker_bps: 0,
    };
    f.config_manager.update_fee_splits(&f.trader, &new_splits);
}

// IntoVal is imported above only to keep the keepalive symbol in scope;
// some tests pass typed structs by reference and rely on the soroban-sdk
// macro expansion within the contractclient.
#[allow(dead_code)]
fn _keepalive() {
    let _ = std::any::type_name::<dyn IntoVal<Env, soroban_sdk::Val>>;
}
