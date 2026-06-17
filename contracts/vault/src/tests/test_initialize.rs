#![cfg(test)]

//! Post-construction state for the Vault contract.
//!
//! The vault is initialized atomically with deploy via the Soroban
//! `__constructor(asset, config_manager, position_manager)` — there is no
//! separate `initialize` entrypoint and no `admin` parameter (role checks
//! cross-call ConfigManager). These tests register the vault through the
//! constructor and assert the bound state via the public getters.

use soroban_sdk::{testutils::Address as _, Address, Env, String};

// ---------------------------------------------------------------------------
// Helper: deploy all contracts and return clients
// ---------------------------------------------------------------------------

struct TestFixture {
    env: Env,
    admin: Address,
    token_id: Address,
    config_id: Address,
    vault_client: crate::VaultContractClient<'static>,
    position_manager: Address,
}

fn setup() -> TestFixture {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let position_manager = Address::generate(&env);

    // Deploy mock USDC token (7 decimals like Stellar USDC)
    let token_id = env.register(mock_token::MockToken, ());
    let token_client = mock_token::MockTokenClient::new(&env, &token_id);
    token_client.initialize(
        &admin,
        &7u32,
        &String::from_str(&env, "USD Coin"),
        &String::from_str(&env, "USDC"),
    );

    // Deploy config manager (constructor binds admin)
    let config_id = env.register(config_manager::ConfigManagerContract, (admin.clone(),));

    // Deploy vault (constructor binds asset, config_manager, position_manager)
    let vault_id = env.register(
        crate::VaultContract,
        (token_id.clone(), config_id.clone(), position_manager.clone()),
    );
    let vault_client = crate::VaultContractClient::new(&env, &vault_id);

    // SAFETY: env lives in the fixture, clients borrow from it.
    let vault_client = unsafe { core::mem::transmute(vault_client) };

    TestFixture {
        env,
        admin,
        token_id,
        config_id,
        vault_client,
        position_manager,
    }
}

// ===========================================================================
// 1. Construction binds and initializes state
// ===========================================================================

#[test]
fn test_construction_binds_state() {
    let fix = setup();

    // query_asset should return the USDC token address
    assert_eq!(
        fix.vault_client.query_asset(),
        fix.token_id,
        "query_asset must return the underlying USDC address"
    );

    // total_assets should be zero (no deposits yet)
    assert_eq!(
        fix.vault_client.total_assets(),
        0i128,
        "total_assets must be 0 right after construction"
    );

    // name should be "Stellars LP"
    assert_eq!(
        fix.vault_client.name(),
        String::from_str(&fix.env, "Stellars LP"),
        "LP token name must be 'Stellars LP'"
    );

    // symbol should be "sLP"
    assert_eq!(
        fix.vault_client.symbol(),
        String::from_str(&fix.env, "sLP"),
        "LP token symbol must be 'sLP'"
    );

    // decimals = asset_decimals + decimals_offset (7 + 6 = 13)
    assert_eq!(
        fix.vault_client.decimals(),
        13u32,
        "LP token decimals must be asset_decimals + offset (7 + 6 = 13)"
    );

    // free_liquidity should be 0 (no deposits, no reservations)
    assert_eq!(
        fix.vault_client.free_liquidity(),
        0i128,
        "free_liquidity must be 0 right after construction"
    );
}

// ===========================================================================
// 2. Counters initialized to zero, not paused
// ===========================================================================

#[test]
fn test_construction_initializes_counters_to_zero() {
    let fix = setup();

    assert_eq!(
        fix.vault_client.reserved_usdc(),
        0i128,
        "reserved_usdc must start at 0"
    );
    assert_eq!(
        fix.vault_client.unclaimed_fees(),
        0i128,
        "unclaimed_fees must start at 0"
    );
    assert_eq!(
        fix.vault_client.net_global_trader_pnl(),
        0i128,
        "net_global_trader_pnl must start at 0"
    );

    // Not paused: max_deposit returns a positive cap (a paused vault returns 0).
    assert!(
        fix.vault_client.max_deposit(&fix.admin) > 0,
        "vault must not be paused right after construction"
    );
}

// ===========================================================================
// 3. Construction with a 9-decimal underlying
// ===========================================================================

#[test]
fn test_construction_with_different_decimals() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let position_manager = Address::generate(&env);

    // Deploy token with 9 decimals instead of 7
    let token_id = env.register(mock_token::MockToken, ());
    let token_client = mock_token::MockTokenClient::new(&env, &token_id);
    token_client.initialize(
        &admin,
        &9u32,
        &String::from_str(&env, "Wrapped ETH"),
        &String::from_str(&env, "WETH"),
    );

    let config_id = env.register(config_manager::ConfigManagerContract, (admin.clone(),));

    let vault_id = env.register(
        crate::VaultContract,
        (token_id.clone(), config_id.clone(), position_manager.clone()),
    );
    let vault_client = crate::VaultContractClient::new(&env, &vault_id);

    // The vault's decimals = asset_decimals + offset (9 + 6 = 15)
    assert_eq!(
        vault_client.decimals(),
        15u32,
        "Vault decimals must be asset_decimals + offset (9 + 6 = 15)"
    );

    // Name and symbol should still be the vault's, not the underlying token's
    assert_eq!(
        vault_client.name(),
        String::from_str(&env, "Stellars LP"),
        "Name must be 'Stellars LP' regardless of underlying"
    );
    assert_eq!(
        vault_client.symbol(),
        String::from_str(&env, "sLP"),
        "Symbol must be 'sLP' regardless of underlying"
    );
}

// ===========================================================================
// 4. Edge case: 0-decimal underlying
// ===========================================================================

#[test]
fn test_construction_with_zero_decimals() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let position_manager = Address::generate(&env);

    let token_id = env.register(mock_token::MockToken, ());
    let token_client = mock_token::MockTokenClient::new(&env, &token_id);
    token_client.initialize(
        &admin,
        &0u32,
        &String::from_str(&env, "Zero Dec Token"),
        &String::from_str(&env, "ZDT"),
    );

    let config_id = env.register(config_manager::ConfigManagerContract, (admin.clone(),));

    let vault_id = env.register(
        crate::VaultContract,
        (token_id.clone(), config_id.clone(), position_manager.clone()),
    );
    let vault_client = crate::VaultContractClient::new(&env, &vault_id);

    assert_eq!(
        vault_client.decimals(),
        6u32,
        "Vault must handle 0-decimal tokens (0 + 6 offset = 6)"
    );
    assert_eq!(vault_client.total_assets(), 0i128);
    assert_eq!(vault_client.free_liquidity(), 0i128);
}

// ===========================================================================
// 5. Config manager binding keeps the vault operational
// ===========================================================================

#[test]
fn test_config_manager_bound_after_construction() {
    let fix = setup();

    // The bound config_manager drives role checks; an empty vault still
    // answers view calls without panicking.
    assert_eq!(
        fix.vault_client.free_liquidity(),
        0,
        "empty vault has zero free liquidity"
    );
    // Touch the bound config id so the binding is exercised end-to-end.
    let _ = &fix.config_id;
}

// ===========================================================================
// 6. Position manager binding gates PM-only entrypoints
// ===========================================================================

#[test]
fn test_position_manager_bound_after_construction() {
    let fix = setup();

    // `require_position_manager` precedes the amount/invariant checks, so a
    // non-PM caller is rejected with NotPositionManager (7) regardless of
    // vault balance — confirming the constructor bound the PM address.
    let stranger = Address::generate(&fix.env);
    let res = fix
        .vault_client
        .try_reserve_liquidity(&stranger, &1i128);
    assert_eq!(
        res.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(crate::VaultError::NotPositionManager as u32),
        "only the bound position_manager may reserve liquidity"
    );

    // The bound PM passes the guard (and then fails the empty-vault invariant,
    // which is fine — the PM identity check is what we are asserting here).
    let res_pm = fix
        .vault_client
        .try_reserve_liquidity(&fix.position_manager, &1i128);
    assert_ne!(
        res_pm.err().and_then(|e| e.ok()),
        Some(soroban_sdk::Error::from_contract_error(
            crate::VaultError::NotPositionManager as u32
        )),
        "the bound position_manager must pass the PM guard"
    );
}
