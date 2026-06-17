// ---------------------------------------------------------------------------
// Tests for: update_indices (KEEPER-only index accumulator advancement)
//
// These tests are written BEFORE the implementation (TDD). They MUST compile
// but are expected to FAIL until the update_indices logic in contract.rs and
// any supporting functions in logic.rs are implemented.
//
// Test categories:
//   1. Guard tests (NotInitialized, Paused, Unauthorized)
//   2. Happy-path tests (basic accumulation, zero-dt no-op, multiple calls, zero OI)
//   3. Edge-case / adversarial tests (fresh market, balanced OI, large time delta)
// ---------------------------------------------------------------------------

use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Ledger, LedgerInfo},
    Address, Env, String, Symbol,
};

use crate::contract::PositionManagerContract;
use crate::math::{
    accumulate_borrow_index, accumulate_funding_index, calc_borrow_rate, calc_funding_rate,
    calc_utilization_bps,
};
use shared::constants::INDEX_PRECISION;
use crate::storage;
use crate::types::MarketInfo;
use crate::PositionManagerClient;

use super::helpers::{self, DualOracle};
use oracle_router::{OracleConfig, OracleRouterClient, OracleRouterContract};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const ONE_USDC: i128 = 10_000_000; // 1e7 (7 decimals)

// Default borrow/funding rate constants (match ConfigManager defaults in setup).
const BASE_BORROW_RATE: i128 = 100;
const SLOPE1: i128 = 500;
const SLOPE2: i128 = 5_000;
const OPTIMAL_UTIL: i128 = 8_000;
const BASE_FUNDING_RATE: i128 = 100;

/// Initial ledger timestamp used across all tests.
const T0: u64 = 1_000_000;

/// Default time advance for happy-path tests (1 hour = 3600 seconds).
const ONE_HOUR: u64 = 3_600;

// ---------------------------------------------------------------------------
// Test Fixture
// ---------------------------------------------------------------------------

struct UpdateIndicesFixture {
    env: Env,
    pm_client: PositionManagerClient<'static>,
    // vault_id: Address,
    vault_client: vault::VaultContractClient<'static>,
    // config_id: Address,
    config_client: config_manager::ConfigManagerClient<'static>,
    // token_id: Address,
    token_client: mock_token::MockTokenClient<'static>,
    oracle_client: DualOracle<'static>,
    admin: Address,
    keeper: Address,
    non_keeper: Address,
}

/// Deploy all contracts needed for update_indices tests:
///   - MockToken (USDC)
///   - ConfigManager (with KEEPER role granted to `keeper`)
///   - Vault (initialized with mock-token as asset)
///   - PositionManager (initialized with vault, config, and a dummy oracle)
///
/// The ledger timestamp is pinned to T0 so tests can control time precisely.
fn setup() -> UpdateIndicesFixture {
    let env = Env::default();
    env.mock_all_auths();

    // Pin the ledger timestamp to T0
    env.ledger().set(LedgerInfo {
        timestamp: T0,
        protocol_version: 23,
        sequence_number: 100,
        network_id: [0u8; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 100,
        min_persistent_entry_ttl: 100,
        max_entry_ttl: 10_000_000,
    });

    let admin = Address::generate(&env);
    let keeper = Address::generate(&env);
    let non_keeper = Address::generate(&env);

    // -- Deploy mock USDC token (7 decimals) --
    let token_id = env.register(mock_token::MockToken, ());
    let token_client = mock_token::MockTokenClient::new(&env, &token_id);
    token_client.initialize(
        &admin,
        &7u32,
        &String::from_str(&env, "USD Coin"),
        &String::from_str(&env, "USDC"),
    );

    // -- Deploy ConfigManager and grant KEEPER role --
    let config_id = env.register(config_manager::ConfigManagerContract, (admin.clone(),));
    let config_client = config_manager::ConfigManagerClient::new(&env, &config_id);
    config_client.grant_role(&admin, &Symbol::new(&env, "KEEPER"), &keeper);

    config_client.update_borrow_rate_config(
        &admin,
        &config_manager::BorrowRateConfig {
            base_borrow_rate_bps: 100,
            slope1_bps: 500,
            slope2_bps: 5_000,
            optimal_utilization_bps: 8_000,
            base_funding_rate_bps: 100,
        },
    );

    // -- Deploy MockOracle + OracleRouter --
    // The PM constructor binds the OracleRouter, so the router is deployed
    // before the PM.
    let (oracle_client, oracle_sources) = helpers::register_dual_oracle(&env);
    // Set default prices for common symbols
    oracle_client.set_price(&symbol_short!("BTC"), &(50_000 * 10_000_000_i128));
    oracle_client.set_price(&symbol_short!("ETH"), &(3_000 * 10_000_000_i128));
    oracle_client.set_price(&Symbol::new(&env, "SOL"), &(100 * 10_000_000_i128));

    let oracle_router_id = env.register(OracleRouterContract, (config_id.clone(),));
    let oracle_router_client = OracleRouterClient::new(&env, &oracle_router_id);
    oracle_router_client.set_oracle_config(
        &admin,
        &OracleConfig {
            max_deviation_bps: 500,
            staleness_threshold: 86400,
            cache_duration: 10,
            min_required_sources: 2,},
    );
    oracle_router_client.set_oracle_sources(
        &admin,
        &symbol_short!("BTC"),
        &oracle_sources);
    oracle_router_client.set_oracle_sources(
        &admin,
        &symbol_short!("ETH"),
        &oracle_sources);
    oracle_router_client.set_oracle_sources(
        &admin,
        &Symbol::new(&env, "SOL"),
        &oracle_sources);

    // -- Deploy PositionManager (binds config + oracle in its constructor),
    //    then the Vault (binds the trusted PM in its own constructor), then
    //    wire the Vault back into the PM exactly once. --
    let pm_id = env.register(PositionManagerContract, (config_id.clone(), oracle_router_id.clone()));
    let pm_client = PositionManagerClient::new(&env, &pm_id);

    let vault_id = env.register(vault::VaultContract, (token_id.clone(), config_id.clone(), pm_id.clone()));
    let vault_client = vault::VaultContractClient::new(&env, &vault_id);

    pm_client.set_vault(&admin, &vault_id);

    // SAFETY: env lives in the fixture, clients borrow from it.
    let pm_client = unsafe { core::mem::transmute(pm_client) };
    let vault_client = unsafe { core::mem::transmute(vault_client) };
    let config_client = unsafe { core::mem::transmute(config_client) };
    let token_client = unsafe { core::mem::transmute(token_client) };
    let oracle_client = unsafe { core::mem::transmute(oracle_client) };

    UpdateIndicesFixture {
        env,
        pm_client,
        // vault_id,
        vault_client,
        // config_id,
        config_client,
        // token_id,
        token_client,
        oracle_client,
        admin,
        keeper,
        non_keeper,
    }
}

/// Seed liquidity into the vault so free_liquidity() returns a non-zero value.
/// Mints `amount` USDC to `depositor`, then deposits into the vault.
fn seed_vault_liquidity(f: &UpdateIndicesFixture, amount: i128) {
    let depositor = Address::generate(&f.env);
    f.token_client.mint(&depositor, &amount);
    f.vault_client
        .deposit(&amount, &depositor, &depositor, &depositor);
}

/// Write a MarketInfo directly into storage for a given symbol.
fn seed_market(f: &UpdateIndicesFixture, symbol: &Symbol, market: &MarketInfo) {
    f.env.as_contract(&f.pm_client.address, || {
        storage::set_market(&f.env, symbol, market);
    });
}

/// Read MarketInfo from storage for a given symbol.
fn _read_market(f: &UpdateIndicesFixture, symbol: &Symbol) -> MarketInfo {
    let pm_addr = f.pm_client.address.clone();
    let sym = symbol.clone();
    f.env
        .as_contract(&pm_addr, || storage::get_market(&f.env, &sym))
}

/// Advance the ledger timestamp by `delta` seconds from T0.
fn advance_time(f: &UpdateIndicesFixture, new_timestamp: u64) {
    f.env.ledger().set(LedgerInfo {
        timestamp: new_timestamp,
        protocol_version: 23,
        sequence_number: 100,
        network_id: [0u8; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 100,
        min_persistent_entry_ttl: 100,
        max_entry_ttl: 10_000_000,
    });
}

/// Force Vault.reserved_usdc to the given target via reserve/release.
/// Vault is now the single source of truth for reserved liquidity (#8).
fn set_total_reserved(f: &UpdateIndicesFixture, amount: i128) {
    let cur = f.vault_client.reserved_usdc();
    if amount > cur {
        f.vault_client
            .reserve_liquidity(&f.pm_client.address, &(amount - cur));
    } else if cur > amount {
        f.vault_client
            .release_liquidity(&f.pm_client.address, &(cur - amount));
    }
}

// ===========================================================================
// 1. Guard tests
// ===========================================================================

#[test]
fn test_update_indices_during_pause_checkpoints_at_pause_boundary() {
    // The keeper may checkpoint during a pause: the refresh clamps the
    // billable window at the pause boundary, so a paused call bills exactly
    // the pre-pause segment and nothing inside the pause window. A second
    // call deeper into the pause accrues nothing further.
    let f = setup();
    let symbol = Symbol::new(&f.env, "BTC");

    seed_vault_liquidity(&f, 1_000_000 * ONE_USDC);
    set_total_reserved(&f, 200_000 * ONE_USDC);

    let initial_market = MarketInfo {
        global_long_avg_price: 50_000 * ONE_USDC,
        global_short_avg_price: 50_000 * ONE_USDC,
        long_open_interest: 500_000 * ONE_USDC,
        short_open_interest: 300_000 * ONE_USDC,
        acc_borrow_index: INDEX_PRECISION,
        acc_funding_index: INDEX_PRECISION,
        last_index_update: T0,
    };
    seed_market(&f, &symbol, &initial_market);

    f.config_client
        .grant_role(&f.admin, &Symbol::new(&f.env, "PAUSER"), &f.admin);

    // Run live for one hour, then pause.
    advance_time(&f, T0 + ONE_HOUR);
    f.pm_client.pause(&f.admin);

    // Checkpoint during the pause: bills [T0, pause_start] only.
    advance_time(&f, T0 + ONE_HOUR + 50);
    f.pm_client.update_indices(&f.keeper, &symbol);
    let market_at_checkpoint = f.pm_client.get_market(&symbol);
    assert_eq!(
        market_at_checkpoint.last_index_update,
        T0 + ONE_HOUR,
        "billable window must clamp at the pause boundary"
    );
    assert!(
        market_at_checkpoint.acc_borrow_index > INDEX_PRECISION,
        "the pre-pause hour must accrue borrow interest"
    );

    // Deeper into the pause, a second checkpoint accrues nothing.
    advance_time(&f, T0 + ONE_HOUR + 500);
    f.pm_client.update_indices(&f.keeper, &symbol);
    let market_later = f.pm_client.get_market(&symbol);
    assert_eq!(
        market_later.acc_borrow_index, market_at_checkpoint.acc_borrow_index,
        "no borrow accrual may happen inside the pause window"
    );
    assert_eq!(
        market_later.acc_funding_index, market_at_checkpoint.acc_funding_index,
        "no funding accrual may happen inside the pause window"
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn test_update_indices_reverts_unauthorized_caller() {
    // Scenario: A caller without the KEEPER role attempts to call
    // update_indices. Must revert with PositionManagerError::Unauthorized (7) —
    // the panic now comes from PM's wrapper (not shared::require_role) so the
    // code identifies the source contract.
    let f = setup();
    let symbol = Symbol::new(&f.env, "BTC");

    // non_keeper does NOT have KEEPER role
    f.pm_client.update_indices(&f.non_keeper, &symbol);
}

// ===========================================================================
// 2. Happy-path tests
// ===========================================================================

#[test]
fn test_update_indices_advances_borrow_and_funding_indices() {
    // Scenario: A market has some long OI and short OI. After advancing
    // the ledger timestamp by ONE_HOUR, calling update_indices should:
    //   - Calculate utilization from TotalReserved / (free_liquidity + TotalReserved)
    //   - Advance acc_borrow_index based on borrow rate and time delta
    //   - Advance acc_funding_index based on OI imbalance and time delta
    //   - Update last_index_update to the current timestamp
    let f = setup();
    let symbol = Symbol::new(&f.env, "BTC");

    // Seed vault with 1,000,000 USDC so free_liquidity is non-zero
    seed_vault_liquidity(&f, 1_000_000 * ONE_USDC);

    // Set TotalReserved to 200,000 USDC (20% utilization)
    let total_reserved = 200_000 * ONE_USDC;
    set_total_reserved(&f, total_reserved);

    // Seed a market with some OI imbalance (long > short => positive funding rate)
    let initial_market = MarketInfo {
        global_long_avg_price: 50_000 * ONE_USDC,
        global_short_avg_price: 50_000 * ONE_USDC,
        long_open_interest: 500_000 * ONE_USDC,
        short_open_interest: 300_000 * ONE_USDC,
        acc_borrow_index: INDEX_PRECISION,  // start at 1.0
        acc_funding_index: INDEX_PRECISION, // start at 1.0
        last_index_update: T0,
    };
    seed_market(&f, &symbol, &initial_market);

    // Advance time by ONE_HOUR
    let t1 = T0 + ONE_HOUR;
    advance_time(&f, t1);

    // Call update_indices
    f.pm_client.update_indices(&f.keeper, &symbol);

    // Compute expected values
    let free_liq = f.vault_client.free_liquidity();
    let total_assets = free_liq + total_reserved;
    let util_bps = calc_utilization_bps(total_reserved, total_assets);
    let borrow_rate = calc_borrow_rate(util_bps, BASE_BORROW_RATE, SLOPE1, SLOPE2, OPTIMAL_UTIL);
    let expected_borrow_index = accumulate_borrow_index(INDEX_PRECISION, borrow_rate, ONE_HOUR);

    let funding_rate = calc_funding_rate(
        initial_market.long_open_interest,
        initial_market.short_open_interest,
        BASE_FUNDING_RATE,
    );
    let expected_funding_index = accumulate_funding_index(INDEX_PRECISION, funding_rate, ONE_HOUR);

    // Read updated market
    let updated = f.pm_client.get_market(&symbol);

    assert_eq!(
        updated.last_index_update, t1,
        "last_index_update must be set to the current ledger timestamp"
    );
    assert_eq!(
        updated.acc_borrow_index, expected_borrow_index,
        "Borrow index must advance by the correct amount for the given utilization and time delta"
    );
    assert_eq!(
        updated.acc_funding_index, expected_funding_index,
        "Funding index must advance by the correct amount for the given OI imbalance and time delta"
    );
    // OI values must NOT change
    assert_eq!(
        updated.long_open_interest, initial_market.long_open_interest,
        "Long OI must remain unchanged by update_indices"
    );
    assert_eq!(
        updated.short_open_interest, initial_market.short_open_interest,
        "Short OI must remain unchanged by update_indices"
    );
}

#[test]
fn test_update_indices_zero_time_delta_is_noop() {
    // Scenario: If the ledger timestamp equals last_index_update (time_delta = 0),
    // update_indices should be a no-op and indices must not change.
    let f = setup();
    let symbol = Symbol::new(&f.env, "BTC");

    seed_vault_liquidity(&f, 1_000_000 * ONE_USDC);

    let initial_market = MarketInfo {
        global_long_avg_price: 50_000 * ONE_USDC,
        global_short_avg_price: 50_000 * ONE_USDC,
        long_open_interest: 500_000 * ONE_USDC,
        short_open_interest: 300_000 * ONE_USDC,
        acc_borrow_index: INDEX_PRECISION,
        acc_funding_index: INDEX_PRECISION,
        last_index_update: T0, // same as current ledger timestamp
    };
    seed_market(&f, &symbol, &initial_market);

    // Do NOT advance time -- timestamp is still T0
    f.pm_client.update_indices(&f.keeper, &symbol);

    let updated = f.pm_client.get_market(&symbol);
    assert_eq!(
        updated.acc_borrow_index, INDEX_PRECISION,
        "Borrow index must not change when time_delta is zero"
    );
    assert_eq!(
        updated.acc_funding_index, INDEX_PRECISION,
        "Funding index must not change when time_delta is zero"
    );
    assert_eq!(
        updated.last_index_update, T0,
        "last_index_update must remain unchanged on zero time delta"
    );
}

#[test]
fn test_update_indices_multiple_calls_accumulate_correctly() {
    // Scenario: Multiple sequential calls to update_indices should
    // accumulate indices incrementally. The result after two 1-hour calls
    // should differ from a single 2-hour call only by rounding.
    let f = setup();
    let symbol = Symbol::new(&f.env, "BTC");

    seed_vault_liquidity(&f, 1_000_000 * ONE_USDC);

    let initial_market = MarketInfo {
        global_long_avg_price: 50_000 * ONE_USDC,
        global_short_avg_price: 50_000 * ONE_USDC,
        long_open_interest: 400_000 * ONE_USDC,
        short_open_interest: 200_000 * ONE_USDC,
        acc_borrow_index: INDEX_PRECISION,
        acc_funding_index: INDEX_PRECISION,
        last_index_update: T0,
    };
    seed_market(&f, &symbol, &initial_market);

    // First call: advance by 1 hour
    let t1 = T0 + ONE_HOUR;
    advance_time(&f, t1);
    f.pm_client.update_indices(&f.keeper, &symbol);

    let after_first = f.pm_client.get_market(&symbol);
    assert_eq!(after_first.last_index_update, t1);
    assert!(
        after_first.acc_borrow_index > INDEX_PRECISION,
        "Borrow index must increase after first call"
    );

    // Second call: advance by another hour
    let t2 = t1 + ONE_HOUR;
    advance_time(&f, t2);
    f.pm_client.update_indices(&f.keeper, &symbol);

    let after_second = f.pm_client.get_market(&symbol);
    assert_eq!(after_second.last_index_update, t2);
    assert!(
        after_second.acc_borrow_index > after_first.acc_borrow_index,
        "Borrow index must increase further after second call"
    );
    assert!(
        after_second.acc_funding_index != after_first.acc_funding_index
            || initial_market.long_open_interest == initial_market.short_open_interest,
        "Funding index should change when OI is imbalanced"
    );
}

#[test]
fn test_update_indices_zero_oi_funding_rate_is_zero() {
    // Scenario: When both long_open_interest and short_open_interest are zero,
    // calc_funding_rate returns 0. Only the borrow index should advance.
    let f = setup();
    let symbol = Symbol::new(&f.env, "BTC");

    seed_vault_liquidity(&f, 1_000_000 * ONE_USDC);

    let initial_market = MarketInfo {
        global_long_avg_price: 0,
        global_short_avg_price: 0,
        long_open_interest: 0,
        short_open_interest: 0,
        acc_borrow_index: INDEX_PRECISION,
        acc_funding_index: INDEX_PRECISION,
        last_index_update: T0,
    };
    seed_market(&f, &symbol, &initial_market);

    let t1 = T0 + ONE_HOUR;
    advance_time(&f, t1);
    f.pm_client.update_indices(&f.keeper, &symbol);

    let updated = f.pm_client.get_market(&symbol);

    // Funding index should not change (funding rate = 0 when OI = 0)
    assert_eq!(
        updated.acc_funding_index, INDEX_PRECISION,
        "Funding index must not change when total OI is zero (funding rate = 0)"
    );

    // Borrow index SHOULD still advance (base borrow rate is non-zero)
    // With zero reserved and non-zero vault liquidity, utilization = 0,
    // so borrow rate = BASE_BORROW_RATE = 100 bps
    let expected_borrow = accumulate_borrow_index(INDEX_PRECISION, 100, ONE_HOUR);
    assert_eq!(
        updated.acc_borrow_index, expected_borrow,
        "Borrow index must advance at base rate even when OI is zero"
    );

    assert_eq!(updated.last_index_update, t1);
}

// ===========================================================================
// 3. Edge-case / adversarial tests
// ===========================================================================

#[test]
fn test_update_indices_fresh_market_first_call() {
    // Scenario: A brand-new market with last_index_update = 0.
    // The time_delta will be current_timestamp - 0 = T0 (a large number).
    // The implementation must handle this gracefully without overflow.
    // This tests that the very first index update on a fresh market works.
    let f = setup();
    let symbol = Symbol::new(&f.env, "ETH");

    seed_vault_liquidity(&f, 1_000_000 * ONE_USDC);

    // Fresh market: defaults from storage::get_market (all zeros)
    // last_index_update = 0, acc_borrow_index = 0, acc_funding_index = 0
    // We explicitly seed with acc_*_index = INDEX_PRECISION for a clean start
    let initial_market = MarketInfo {
        global_long_avg_price: 0,
        global_short_avg_price: 0,
        long_open_interest: 100_000 * ONE_USDC,
        short_open_interest: 50_000 * ONE_USDC,
        acc_borrow_index: INDEX_PRECISION,
        acc_funding_index: INDEX_PRECISION,
        last_index_update: 0, // never updated before
    };
    seed_market(&f, &symbol, &initial_market);

    // Current time is T0 = 1_000_000 seconds
    f.pm_client.update_indices(&f.keeper, &symbol);

    let updated = f.pm_client.get_market(&symbol);
    assert_eq!(
        updated.last_index_update, T0,
        "After first update, last_index_update must be set to current timestamp"
    );
    assert!(
        updated.acc_borrow_index > INDEX_PRECISION,
        "Borrow index must advance on first call even with large time delta"
    );
}

#[test]
fn test_update_indices_balanced_oi_only_borrow_changes() {
    // Scenario: When long_open_interest == short_open_interest, the funding
    // rate is zero. Only the borrow index should change.
    let f = setup();
    let symbol = Symbol::new(&f.env, "BTC");

    seed_vault_liquidity(&f, 1_000_000 * ONE_USDC);

    let balanced_oi = 300_000 * ONE_USDC;
    let initial_market = MarketInfo {
        global_long_avg_price: 50_000 * ONE_USDC,
        global_short_avg_price: 50_000 * ONE_USDC,
        long_open_interest: balanced_oi,
        short_open_interest: balanced_oi,
        acc_borrow_index: INDEX_PRECISION,
        acc_funding_index: INDEX_PRECISION,
        last_index_update: T0,
    };
    seed_market(&f, &symbol, &initial_market);

    let t1 = T0 + ONE_HOUR;
    advance_time(&f, t1);
    f.pm_client.update_indices(&f.keeper, &symbol);

    let updated = f.pm_client.get_market(&symbol);

    // Funding rate = BASE_FUNDING_RATE * (long - short) / total = 100 * 0 / total = 0
    assert_eq!(
        updated.acc_funding_index, INDEX_PRECISION,
        "Funding index must not change when OI is perfectly balanced"
    );
    assert!(
        updated.acc_borrow_index > INDEX_PRECISION,
        "Borrow index must still advance when OI is balanced"
    );
}

#[test]
fn test_update_indices_large_time_delta() {
    // Scenario: A very large time delta (e.g., 1 year = 31_536_000 seconds).
    // This tests that the index accumulation math does not overflow.
    // With INDEX_PRECISION = 1e14 and rate_bps up to ~5500 (at 100% util),
    // the product rate_bps * INDEX_PRECISION * time_delta must not overflow i128.
    let f = setup();
    let symbol = Symbol::new(&f.env, "BTC");

    seed_vault_liquidity(&f, 1_000_000 * ONE_USDC);

    let initial_market = MarketInfo {
        global_long_avg_price: 50_000 * ONE_USDC,
        global_short_avg_price: 50_000 * ONE_USDC,
        long_open_interest: 100_000 * ONE_USDC,
        short_open_interest: 50_000 * ONE_USDC,
        acc_borrow_index: INDEX_PRECISION,
        acc_funding_index: INDEX_PRECISION,
        last_index_update: T0,
    };
    seed_market(&f, &symbol, &initial_market);

    // Advance by 1 year
    let one_year: u64 = 31_536_000;
    let t1 = T0 + one_year;
    advance_time(&f, t1);

    // Re-set oracle price so it's not stale after the large time advance
    f.oracle_client
        .set_price(&symbol, &(50_000 * 10_000_000_i128));

    // This must not panic from overflow
    f.pm_client.update_indices(&f.keeper, &symbol);

    let updated = f.pm_client.get_market(&symbol);
    assert_eq!(updated.last_index_update, t1);
    assert!(
        updated.acc_borrow_index > INDEX_PRECISION,
        "Borrow index must advance after 1 year"
    );
}

#[test]
fn test_update_indices_high_utilization_steep_borrow_rate() {
    // Scenario: When utilization is above optimal (80%), the borrow rate
    // jumps steeply. Verify the indices reflect the higher rate.
    let f = setup();
    let symbol = Symbol::new(&f.env, "BTC");

    // Seed vault with just enough for 95% utilization
    // total_assets = free_liq + reserved. We want reserved/total = 0.95
    // If we deposit 1_000_000 and reserve 950_000, util = 950k / 1M = 95%
    let deposit_amount = 1_000_000 * ONE_USDC;
    seed_vault_liquidity(&f, deposit_amount);

    let total_reserved = 950_000 * ONE_USDC;
    set_total_reserved(&f, total_reserved);

    let initial_market = MarketInfo {
        global_long_avg_price: 50_000 * ONE_USDC,
        global_short_avg_price: 50_000 * ONE_USDC,
        long_open_interest: 500_000 * ONE_USDC,
        short_open_interest: 500_000 * ONE_USDC, // balanced OI
        acc_borrow_index: INDEX_PRECISION,
        acc_funding_index: INDEX_PRECISION,
        last_index_update: T0,
    };
    seed_market(&f, &symbol, &initial_market);

    let t1 = T0 + ONE_HOUR;
    advance_time(&f, t1);
    f.pm_client.update_indices(&f.keeper, &symbol);

    // Compute expected: util ~= 9500 bps (95%)
    // Note: free_liquidity from the vault accounts for reserved_usdc being
    // set on the vault side. We set TotalReserved on PM side, but vault's
    // free_liquidity() uses its own reserved_usdc (which is 0 in vault storage).
    // The actual utilization depends on how update_indices computes it.
    // For this test, we just verify the borrow index advanced significantly.
    let updated = f.pm_client.get_market(&symbol);

    // At 95% util: rate = 100 + (8000*500/10000) + ((9500-8000)*5000/10000) = 100+400+750 = 1250 bps
    // This is a high rate -- index should advance meaningfully
    assert!(
        updated.acc_borrow_index > INDEX_PRECISION,
        "Borrow index must advance at high utilization"
    );
    assert_eq!(updated.last_index_update, t1);
}

#[test]
fn test_update_indices_does_not_modify_oi_or_avg_prices() {
    // Adversarial: Verify that update_indices ONLY modifies the index
    // accumulators and last_index_update. It must NOT touch OI values
    // or global average prices.
    let f = setup();
    let symbol = Symbol::new(&f.env, "BTC");

    seed_vault_liquidity(&f, 1_000_000 * ONE_USDC);

    let initial_market = MarketInfo {
        global_long_avg_price: 48_000 * ONE_USDC,
        global_short_avg_price: 52_000 * ONE_USDC,
        long_open_interest: 750_000 * ONE_USDC,
        short_open_interest: 250_000 * ONE_USDC,
        acc_borrow_index: 2 * INDEX_PRECISION, // previously accumulated
        acc_funding_index: INDEX_PRECISION + 42, // small offset
        last_index_update: T0,
    };
    seed_market(&f, &symbol, &initial_market);

    let t1 = T0 + ONE_HOUR;
    advance_time(&f, t1);
    f.pm_client.update_indices(&f.keeper, &symbol);

    let updated = f.pm_client.get_market(&symbol);

    assert_eq!(
        updated.global_long_avg_price, initial_market.global_long_avg_price,
        "global_long_avg_price must not be modified by update_indices"
    );
    assert_eq!(
        updated.global_short_avg_price, initial_market.global_short_avg_price,
        "global_short_avg_price must not be modified by update_indices"
    );
    assert_eq!(
        updated.long_open_interest, initial_market.long_open_interest,
        "long_open_interest must not be modified by update_indices"
    );
    assert_eq!(
        updated.short_open_interest, initial_market.short_open_interest,
        "short_open_interest must not be modified by update_indices"
    );
}

#[test]
fn test_update_indices_nonexistent_market_creates_defaults() {
    // Scenario: Calling update_indices on a symbol that has never been written
    // to storage. get_market returns defaults with acc_borrow_index and
    // acc_funding_index at INDEX_PRECISION and last_index_update = now.
    // Since time_delta is 0, update_indices is a no-op. Verify that the
    // default market is correctly initialized when subsequently queried.
    let f = setup();
    let symbol = Symbol::new(&f.env, "SOL"); // never seeded

    seed_vault_liquidity(&f, 1_000_000 * ONE_USDC);

    // get_market returns defaults even without any prior write
    let market = f.pm_client.get_market(&symbol);
    assert_eq!(market.long_open_interest, 0);
    assert_eq!(market.short_open_interest, 0);
    assert_eq!(
        market.acc_borrow_index, INDEX_PRECISION,
        "Default borrow index must be INDEX_PRECISION"
    );
    assert_eq!(
        market.acc_funding_index, INDEX_PRECISION,
        "Default funding index must be INDEX_PRECISION"
    );
}

#[test]
fn test_update_indices_short_heavier_oi_negative_funding() {
    // Scenario: When short OI > long OI, the funding rate is negative.
    // This means shorts pay longs. The acc_funding_index should decrease
    // (or increase in the negative direction, depending on sign convention).
    let f = setup();
    let symbol = Symbol::new(&f.env, "BTC");

    seed_vault_liquidity(&f, 1_000_000 * ONE_USDC);

    let initial_market = MarketInfo {
        global_long_avg_price: 50_000 * ONE_USDC,
        global_short_avg_price: 50_000 * ONE_USDC,
        long_open_interest: 200_000 * ONE_USDC, // longs smaller
        short_open_interest: 800_000 * ONE_USDC, // shorts larger
        acc_borrow_index: INDEX_PRECISION,
        acc_funding_index: INDEX_PRECISION,
        last_index_update: T0,
    };
    seed_market(&f, &symbol, &initial_market);

    let t1 = T0 + ONE_HOUR;
    advance_time(&f, t1);
    f.pm_client.update_indices(&f.keeper, &symbol);

    let updated = f.pm_client.get_market(&symbol);

    // funding_rate = BASE_FUNDING_RATE * (long - short) / total = 100 * (200k - 800k) / 1M < 0
    let funding_rate = calc_funding_rate(
        initial_market.long_open_interest,
        initial_market.short_open_interest,
        BASE_FUNDING_RATE,
    );
    assert!(
        funding_rate < 0,
        "Funding rate must be negative when shorts dominate"
    );

    let expected_funding = accumulate_funding_index(INDEX_PRECISION, funding_rate, ONE_HOUR);
    assert_eq!(
        updated.acc_funding_index, expected_funding,
        "Funding index must decrease when short OI exceeds long OI"
    );
    assert!(
        updated.acc_funding_index < INDEX_PRECISION,
        "Funding index must go below starting value when shorts dominate"
    );
}

#[test]
fn test_update_indices_idempotent_double_call_same_timestamp() {
    // Adversarial: Calling update_indices twice in the same block (same
    // timestamp). The second call should be a no-op because time_delta = 0.
    let f = setup();
    let symbol = Symbol::new(&f.env, "BTC");

    seed_vault_liquidity(&f, 1_000_000 * ONE_USDC);

    let initial_market = MarketInfo {
        global_long_avg_price: 50_000 * ONE_USDC,
        global_short_avg_price: 50_000 * ONE_USDC,
        long_open_interest: 500_000 * ONE_USDC,
        short_open_interest: 300_000 * ONE_USDC,
        acc_borrow_index: INDEX_PRECISION,
        acc_funding_index: INDEX_PRECISION,
        last_index_update: T0,
    };
    seed_market(&f, &symbol, &initial_market);

    let t1 = T0 + ONE_HOUR;
    advance_time(&f, t1);

    // First call advances indices
    f.pm_client.update_indices(&f.keeper, &symbol);
    let after_first = f.pm_client.get_market(&symbol);

    // Second call at same timestamp -- must be no-op
    f.pm_client.update_indices(&f.keeper, &symbol);
    let after_second = f.pm_client.get_market(&symbol);

    assert_eq!(
        after_first.acc_borrow_index, after_second.acc_borrow_index,
        "Second call at same timestamp must not change borrow index"
    );
    assert_eq!(
        after_first.acc_funding_index, after_second.acc_funding_index,
        "Second call at same timestamp must not change funding index"
    );
}
