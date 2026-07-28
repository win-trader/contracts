use soroban_sdk::{contractclient, Address, BytesN, Env};

use crate::types::{LpRequest, SettlementResult};

#[contractclient(name = "RequestRouterClient")]
pub trait RequestRouter {
    fn request_deposit(env: Env, owner: Address, assets: i128) -> u64;
    fn request_withdrawal(env: Env, owner: Address, shares: i128) -> u64;
    fn resolve_next(env: Env, executor: Address) -> SettlementResult;
    fn get_request(env: Env, request_id: u64) -> LpRequest;
    fn next_request_to_resolve(env: Env) -> u64;
    fn propose_upgrade(env: Env, caller: Address, wasm_hash: BytesN<32>);
    fn cancel_upgrade(env: Env, caller: Address);
}
