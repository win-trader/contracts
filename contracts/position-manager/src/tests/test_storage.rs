use soroban_sdk::{contract, testutils::Address as _, Address, Env, symbol_short};

use crate::storage;
use crate::types::{MarketInfo, Position};

/// Constructor-free host contract used to exercise the `storage` helpers in
/// isolation. `PositionManagerContract`'s constructor seeds `Initialized`,
/// `ConfigManager`, and `OracleRouter`, which would defeat the "defaults /
/// panics-when-unset" assertions below. A bare contract gives a clean storage
/// context keyed by the same `StorageKey` enum, so every helper round-trips
/// exactly as it would inside the real contract.
#[contract]
struct StorageHost;

/// Helper: register the host contract and run a closure inside its storage context.
fn with_contract<F: FnOnce(&Env, &Address)>(f: F) {
    let env = Env::default();
    let contract_id = env.register(StorageHost, ());
    env.as_contract(&contract_id, || f(&env, &contract_id));
}

// ===========================================================================
// Instance storage: Initialized
// ===========================================================================

#[test]
fn test_is_initialized_defaults_to_false() {
    with_contract(|env, _| {
        assert_eq!(storage::is_initialized(env), false);
    });
}

#[test]
fn test_set_initialized_then_read() {
    with_contract(|env, _| {
        storage::set_initialized(env);
        assert_eq!(storage::is_initialized(env), true);
    });
}

// ===========================================================================
// Instance storage: VaultAddress
// ===========================================================================

#[test]
#[should_panic]
fn test_get_vault_address_panics_when_not_set() {
    with_contract(|env, _| {
        let _ = storage::get_vault_address(env);
    });
}

#[test]
fn test_set_and_get_vault_address() {
    with_contract(|env, _| {
        let addr = Address::generate(env);
        storage::set_vault_address(env, &addr);
        assert_eq!(storage::get_vault_address(env), addr);
    });
}

// ===========================================================================
// Instance storage: ConfigManager
// ===========================================================================

#[test]
#[should_panic]
fn test_get_config_manager_panics_when_not_set() {
    with_contract(|env, _| {
        let _ = storage::get_config_manager(env);
    });
}

#[test]
fn test_set_and_get_config_manager() {
    with_contract(|env, _| {
        let addr = Address::generate(env);
        storage::set_config_manager(env, &addr);
        assert_eq!(storage::get_config_manager(env), addr);
    });
}

// ===========================================================================
// Instance storage: OracleRouter
// ===========================================================================

#[test]
#[should_panic]
fn test_get_oracle_router_panics_when_not_set() {
    with_contract(|env, _| {
        let _ = storage::get_oracle_router(env);
    });
}

#[test]
fn test_set_and_get_oracle_router() {
    with_contract(|env, _| {
        let addr = Address::generate(env);
        storage::set_oracle_router(env, &addr);
        assert_eq!(storage::get_oracle_router(env), addr);
    });
}

// ===========================================================================
// Instance storage: IsPaused
// ===========================================================================

#[test]
fn test_get_paused_defaults_to_false() {
    with_contract(|env, _| {
        assert_eq!(storage::get_paused(env), false);
    });
}

#[test]
fn test_set_paused_true_then_false() {
    with_contract(|env, _| {
        storage::set_paused(env, true);
        assert_eq!(storage::get_paused(env), true);
        storage::set_paused(env, false);
        assert_eq!(storage::get_paused(env), false);
    });
}

// ===========================================================================
// Instance storage: Address overwrite semantics
// ===========================================================================

#[test]
fn test_vault_address_overwrite() {
    with_contract(|env, _| {
        let addr1 = Address::generate(env);
        let addr2 = Address::generate(env);
        storage::set_vault_address(env, &addr1);
        storage::set_vault_address(env, &addr2);
        assert_eq!(storage::get_vault_address(env), addr2);
    });
}

// ===========================================================================
// Persistent storage: Position (happy path)
// ===========================================================================

fn sample_position() -> Position {
    Position {
        collateral: 1_000_0000000,
        size: 10_000_0000000,
        entry_price: 50_000_0000000,
        entry_borrow_index: 1_0000000,
        entry_funding_index: 0,
        is_long: true,
        last_increased_time: 1_700_000_000,
        take_profit: 0,
        stop_loss: 0,
        execution_fee_escrow: 0,
    }
}

#[test]
fn test_get_position_returns_none_when_missing() {
    with_contract(|env, _| {
        let trader = Address::generate(env);
        let symbol = symbol_short!("BTC");
        assert!(storage::get_position(env, &trader, &symbol).is_none());
    });
}

#[test]
fn test_set_and_get_position() {
    with_contract(|env, _| {
        let trader = Address::generate(env);
        let symbol = symbol_short!("BTC");
        let mut pos = sample_position();
        // Non-zero execution_fee_escrow to verify the new field round-trips
        // through persistent storage. Field holds the flat USDC fee paid when
        // TP or SL is first activated on a position.
        pos.execution_fee_escrow = 5_000_000; // $0.5 at PRECISION

        storage::set_position(env, &trader, &symbol, &pos);
        let loaded = storage::get_position(env, &trader, &symbol).unwrap();

        assert_eq!(loaded.collateral, pos.collateral);
        assert_eq!(loaded.size, pos.size);
        assert_eq!(loaded.entry_price, pos.entry_price);
        assert_eq!(loaded.entry_borrow_index, pos.entry_borrow_index);
        assert_eq!(loaded.entry_funding_index, pos.entry_funding_index);
        assert_eq!(loaded.is_long, pos.is_long);
        assert_eq!(loaded.last_increased_time, pos.last_increased_time);
        assert_eq!(loaded.execution_fee_escrow, pos.execution_fee_escrow);
    });
}

#[test]
fn test_position_execution_fee_escrow_defaults_to_zero() {
    // Verifies that a Position with no TP/SL active stores a zero escrow.
    with_contract(|env, _| {
        let trader = Address::generate(env);
        let symbol = symbol_short!("BTC");
        let pos = sample_position(); // execution_fee_escrow defaults to 0

        storage::set_position(env, &trader, &symbol, &pos);
        let loaded = storage::get_position(env, &trader, &symbol).unwrap();

        assert_eq!(loaded.execution_fee_escrow, 0);
    });
}

#[test]
fn test_position_execution_fee_escrow_round_trips_max_value() {
    // Adversarial: maximum allowed execution fee at PRECISION scale should
    // survive the SDK XDR encode/decode without truncation.
    with_contract(|env, _| {
        let trader = Address::generate(env);
        let symbol = symbol_short!("BTC");
        let mut pos = sample_position();
        pos.execution_fee_escrow = shared::constants::MAX_TP_SL_EXECUTION_FEE;

        storage::set_position(env, &trader, &symbol, &pos);
        let loaded = storage::get_position(env, &trader, &symbol).unwrap();

        assert_eq!(
            loaded.execution_fee_escrow,
            shared::constants::MAX_TP_SL_EXECUTION_FEE
        );
    });
}

// ===========================================================================
// Persistent storage: Position (delete)
// ===========================================================================

#[test]
fn test_delete_position_removes_entry() {
    with_contract(|env, _| {
        let trader = Address::generate(env);
        let symbol = symbol_short!("BTC");
        storage::set_position(env, &trader, &symbol, &sample_position());
        storage::delete_position(env, &trader, &symbol);
        assert!(storage::get_position(env, &trader, &symbol).is_none());
    });
}

#[test]
fn test_delete_position_nonexistent_does_not_panic() {
    with_contract(|env, _| {
        let trader = Address::generate(env);
        let symbol = symbol_short!("BTC");
        storage::delete_position(env, &trader, &symbol);
    });
}

// ===========================================================================
// Persistent storage: Position isolation
// ===========================================================================

#[test]
fn test_positions_isolated_per_trader() {
    with_contract(|env, _| {
        let trader_a = Address::generate(env);
        let trader_b = Address::generate(env);
        let symbol = symbol_short!("BTC");

        let mut pos_a = sample_position();
        pos_a.collateral = 100;
        let mut pos_b = sample_position();
        pos_b.collateral = 200;

        storage::set_position(env, &trader_a, &symbol, &pos_a);
        storage::set_position(env, &trader_b, &symbol, &pos_b);

        assert_eq!(storage::get_position(env, &trader_a, &symbol).unwrap().collateral, 100);
        assert_eq!(storage::get_position(env, &trader_b, &symbol).unwrap().collateral, 200);
    });
}

#[test]
fn test_positions_isolated_per_symbol() {
    with_contract(|env, _| {
        let trader = Address::generate(env);
        let btc = symbol_short!("BTC");
        let eth = symbol_short!("ETH");

        let mut pos_btc = sample_position();
        pos_btc.size = 111;
        let mut pos_eth = sample_position();
        pos_eth.size = 222;

        storage::set_position(env, &trader, &btc, &pos_btc);
        storage::set_position(env, &trader, &eth, &pos_eth);

        assert_eq!(storage::get_position(env, &trader, &btc).unwrap().size, 111);
        assert_eq!(storage::get_position(env, &trader, &eth).unwrap().size, 222);
    });
}

#[test]
fn test_delete_position_does_not_affect_other_positions() {
    with_contract(|env, _| {
        let trader = Address::generate(env);
        let btc = symbol_short!("BTC");
        let eth = symbol_short!("ETH");

        storage::set_position(env, &trader, &btc, &sample_position());
        storage::set_position(env, &trader, &eth, &sample_position());
        storage::delete_position(env, &trader, &btc);

        assert!(storage::get_position(env, &trader, &btc).is_none());
        assert!(storage::get_position(env, &trader, &eth).is_some());
    });
}

// ===========================================================================
// Persistent storage: MarketInfo
// ===========================================================================

#[test]
fn test_get_market_returns_default_when_missing() {
    with_contract(|env, _| {
        let symbol = symbol_short!("BTC");
        let market = storage::get_market(env, &symbol);
        assert_eq!(market.global_long_avg_price, 0);
        assert_eq!(market.global_short_avg_price, 0);
        assert_eq!(market.long_open_interest, 0);
        assert_eq!(market.short_open_interest, 0);
        assert_eq!(market.acc_borrow_index, shared::constants::INDEX_PRECISION);
        assert_eq!(market.acc_funding_index, shared::constants::INDEX_PRECISION);
        assert_eq!(market.last_index_update, env.ledger().timestamp());
    });
}

#[test]
fn test_set_and_get_market() {
    with_contract(|env, _| {
        let symbol = symbol_short!("BTC");
        let info = MarketInfo {
            global_long_avg_price: 50_000_0000000,
            global_short_avg_price: 49_500_0000000,
            long_open_interest: 1_000_000_0000000,
            short_open_interest: 800_000_0000000,
            acc_borrow_index: 1_0100000,
            acc_funding_index: -50000,
            last_index_update: 1_700_000_000,
        };

        storage::set_market(env, &symbol, &info);
        let loaded = storage::get_market(env, &symbol);

        assert_eq!(loaded.global_long_avg_price, info.global_long_avg_price);
        assert_eq!(loaded.global_short_avg_price, info.global_short_avg_price);
        assert_eq!(loaded.long_open_interest, info.long_open_interest);
        assert_eq!(loaded.short_open_interest, info.short_open_interest);
        assert_eq!(loaded.acc_borrow_index, info.acc_borrow_index);
        assert_eq!(loaded.acc_funding_index, info.acc_funding_index);
        assert_eq!(loaded.last_index_update, info.last_index_update);
    });
}

#[test]
fn test_markets_isolated_per_symbol() {
    with_contract(|env, _| {
        let btc = symbol_short!("BTC");
        let eth = symbol_short!("ETH");

        let btc_market = MarketInfo {
            global_long_avg_price: 50_000, global_short_avg_price: 0,
            long_open_interest: 100, short_open_interest: 0,
            acc_borrow_index: 1, acc_funding_index: 0, last_index_update: 0,
        };
        let eth_market = MarketInfo {
            global_long_avg_price: 3_000, global_short_avg_price: 0,
            long_open_interest: 200, short_open_interest: 0,
            acc_borrow_index: 2, acc_funding_index: 0, last_index_update: 0,
        };

        storage::set_market(env, &btc, &btc_market);
        storage::set_market(env, &eth, &eth_market);

        assert_eq!(storage::get_market(env, &btc).long_open_interest, 100);
        assert_eq!(storage::get_market(env, &eth).long_open_interest, 200);
    });
}

// ===========================================================================
// Adversarial: extreme / boundary values
// ===========================================================================

#[test]
fn test_position_with_extreme_values() {
    with_contract(|env, _| {
        let trader = Address::generate(env);
        let symbol = symbol_short!("BTC");
        let extreme = Position {
            collateral: i128::MAX, size: i128::MIN, entry_price: 0,
            entry_borrow_index: i128::MAX, entry_funding_index: i128::MIN,
            is_long: false, last_increased_time: u64::MAX,
            take_profit: 0, stop_loss: 0,
            execution_fee_escrow: i128::MAX,
        };

        storage::set_position(env, &trader, &symbol, &extreme);
        let loaded = storage::get_position(env, &trader, &symbol).unwrap();

        assert_eq!(loaded.collateral, i128::MAX);
        assert_eq!(loaded.size, i128::MIN);
        assert_eq!(loaded.entry_price, 0);
        assert_eq!(loaded.entry_borrow_index, i128::MAX);
        assert_eq!(loaded.entry_funding_index, i128::MIN);
        assert_eq!(loaded.is_long, false);
        assert_eq!(loaded.last_increased_time, u64::MAX);
        assert_eq!(loaded.execution_fee_escrow, i128::MAX);
    });
}

#[test]
fn test_market_with_negative_funding_index() {
    with_contract(|env, _| {
        let symbol = symbol_short!("BTC");
        let info = MarketInfo {
            global_long_avg_price: 0, global_short_avg_price: 0,
            long_open_interest: 0, short_open_interest: 0,
            acc_borrow_index: 0, acc_funding_index: i128::MIN,
            last_index_update: 0,
        };

        storage::set_market(env, &symbol, &info);
        assert_eq!(storage::get_market(env, &symbol).acc_funding_index, i128::MIN);
    });
}

// ===========================================================================
// Adversarial: overwrite semantics
// ===========================================================================

#[test]
fn test_position_overwrite() {
    with_contract(|env, _| {
        let trader = Address::generate(env);
        let symbol = symbol_short!("BTC");

        let mut pos = sample_position();
        pos.collateral = 100;
        storage::set_position(env, &trader, &symbol, &pos);
        pos.collateral = 999;
        storage::set_position(env, &trader, &symbol, &pos);

        assert_eq!(storage::get_position(env, &trader, &symbol).unwrap().collateral, 999);
    });
}

#[test]
fn test_market_overwrite() {
    with_contract(|env, _| {
        let symbol = symbol_short!("ETH");
        let mut info = MarketInfo {
            global_long_avg_price: 1, global_short_avg_price: 2,
            long_open_interest: 3, short_open_interest: 4,
            acc_borrow_index: 5, acc_funding_index: 6, last_index_update: 7,
        };
        storage::set_market(env, &symbol, &info);
        info.long_open_interest = 999;
        storage::set_market(env, &symbol, &info);

        assert_eq!(storage::get_market(env, &symbol).long_open_interest, 999);
    });
}
