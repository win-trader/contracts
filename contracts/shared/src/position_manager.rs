//! Shared PositionManager contract interface.

use soroban_sdk::{contractclient, Address, BytesN, Env, Symbol, Vec};

use crate::types::{
    AccountingSnapshot, GlobalConfig, MarketConfig, MarketInfo, OracleRound, Position,
};

#[contractclient(name = "PositionManagerClient")]
pub trait PositionManager {
    fn set_vault(env: Env, caller: Address, vault: Address);

    #[allow(clippy::too_many_arguments)]
    fn open_position(
        env: Env,
        owner: Address,
        market: Symbol,
        is_long: bool,
        size: i128,
        collateral: i128,
        execution_budget: i128,
        take_profit: i128,
        stop_loss: i128,
        acceptable_price: i128,
    ) -> u64;

    fn increase_position(
        env: Env,
        position_id: u64,
        size_added: i128,
        collateral_added: i128,
        acceptable_price: i128,
    );

    fn decrease_position(
        env: Env,
        position_id: u64,
        size_removed: i128,
        collateral_withdrawn: i128,
        acceptable_price: i128,
    );

    fn liquidate_position(env: Env, caller: Address, position_id: u64);
    fn deleverage_position(env: Env, caller: Address, position_id: u64);
    fn execute_order(env: Env, caller: Address, position_id: u64);
    fn set_tp_sl(env: Env, position_id: u64, take_profit: i128, stop_loss: i128);
    fn fund_execution_budget(env: Env, position_id: u64, amount: i128);
    fn withdraw_execution_budget(env: Env, position_id: u64, amount: i128);

    fn update_indices(env: Env, caller: Address, market: Symbol);
    fn set_global_config(env: Env, caller: Address, config: GlobalConfig);
    fn set_market_config(env: Env, caller: Address, market: Symbol, config: MarketConfig);
    fn disable_market(env: Env, caller: Address, market: Symbol);
    fn enable_market(env: Env, caller: Address, market: Symbol);
    fn is_market_disabled(env: Env, market: Symbol) -> bool;

    fn prepare_lp_snapshot(
        env: Env,
        caller: Address,
        round: OracleRound,
        physical_cash: i128,
    ) -> AccountingSnapshot;
    fn refresh_borrow_rate(env: Env, caller: Address, physical_cash: i128);
    fn can_create_lp_request(env: Env, caller: Address, physical_cash: i128) -> bool;
    fn accounting_snapshot(env: Env, round: OracleRound, physical_cash: i128)
        -> AccountingSnapshot;

    fn get_position(env: Env, position_id: u64) -> Position;
    fn get_market(env: Env, market: Symbol) -> MarketInfo;
    fn active_markets(env: Env) -> Vec<Symbol>;
    fn global_config(env: Env) -> GlobalConfig;
    fn pending_receiver_funding_total(env: Env) -> i128;
    fn protocol_claimable_total(env: Env) -> i128;
    fn risk_keeper_reserve_total(env: Env) -> i128;
    fn non_lp_claims(env: Env) -> i128;

    fn claim_protocol(env: Env, caller: Address, recipient: Address, amount: i128);
    fn recapitalize(env: Env, contributor: Address, amount: i128);

    fn pause(env: Env, caller: Address);
    fn unpause(env: Env, caller: Address);
    fn propose_upgrade(env: Env, caller: Address, wasm_hash: BytesN<32>);
    fn cancel_upgrade(env: Env, caller: Address);
    fn bump_position(env: Env, position_id: u64);
}
