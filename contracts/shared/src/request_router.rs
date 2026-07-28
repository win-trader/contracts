//! Shared RequestRouter contract interface.
//!
//! The RequestRouter is the FIFO queue for delayed LP deposits and
//! withdrawals (§13.2/§13.3). It escrows the assets or shares, assigns each
//! request the first canonical oracle round at or after
//! `request_time + lp_request_delay`, and settles strictly in request-id
//! order — full settlement or full refund, never a partial fill.

use soroban_sdk::{contractclient, Address, BytesN, Env};

use crate::types::{LpRequest, SettlementResult};

#[contractclient(name = "RequestRouterClient")]
pub trait RequestRouter {
    /// Escrow `assets` collateral and queue a deposit (owner auth).
    /// Rejected while any market side is risk-restricted or the vault has a
    /// shortfall. Returns the request id.
    fn request_deposit(env: Env, owner: Address, assets: i128) -> u64;

    /// Escrow `shares` LP tokens and queue a withdrawal (owner auth).
    /// Escrowed shares stay in the total supply until settlement (§5.4).
    fn request_withdrawal(env: Env, owner: Address, shares: i128) -> u64;

    /// Resolve the FIFO head against the latest oracle round. Open to any
    /// authenticated executor — the head can always be cleared, so one
    /// request cannot block the queue (§13.3). Settles in full, or refunds
    /// the escrow when the request expired (its assigned round passed) or
    /// the vault rejected it.
    fn resolve_next(env: Env, executor: Address) -> SettlementResult;

    fn get_request(env: Env, request_id: u64) -> LpRequest;
    fn next_request_to_resolve(env: Env) -> u64;

    fn propose_upgrade(env: Env, caller: Address, wasm_hash: BytesN<32>);
    fn cancel_upgrade(env: Env, caller: Address);
}
