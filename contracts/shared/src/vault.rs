//! Shared vault contract interface.

use soroban_sdk::{contractclient, Address, BytesN, Env};

use crate::types::{AccountingSnapshot, LpConfig, OracleRound, SettlementResult};

#[contractclient(name = "VaultClient")]
pub trait VaultInterface {
    fn set_request_router(env: Env, caller: Address, request_router: Address);

    fn receive_collateral(env: Env, caller: Address, from: Address, amount: i128);
    fn transfer_claim(
        env: Env,
        caller: Address,
        recipient: Address,
        amount: i128,
        claims_after: i128,
    );
    fn transfer_safety_claim(env: Env, caller: Address, recipient: Address, amount: i128);

    fn settle_deposit(
        env: Env,
        caller: Address,
        owner: Address,
        assets: i128,
        round: OracleRound,
    ) -> SettlementResult;
    fn settle_withdrawal(
        env: Env,
        caller: Address,
        owner: Address,
        shares: i128,
        round: OracleRound,
    ) -> SettlementResult;

    fn set_lp_config(env: Env, caller: Address, config: LpConfig);
    fn get_lp_config(env: Env) -> LpConfig;
    fn can_create_lp_request(env: Env) -> bool;
    fn accounting_snapshot(env: Env, round: OracleRound) -> AccountingSnapshot;
    fn physical_cash(env: Env) -> i128;
    fn query_asset(env: Env) -> Address;
    fn total_share_supply(env: Env) -> i128;

    fn pause(env: Env, caller: Address);
    fn unpause(env: Env, caller: Address);
    fn propose_upgrade(env: Env, caller: Address, wasm_hash: BytesN<32>);
    fn cancel_upgrade(env: Env, caller: Address);
    fn bump_vault_state(env: Env);
}
