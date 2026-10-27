use soroban_sdk::{contractclient, Address, BytesN, Env};

use crate::types::{LpRequest, SettlementResult};

/// FIFO queue for delayed LP deposits and withdrawals. Settles in full or refunds in full.
#[contractclient(name = "RequestRouterClient")]
pub trait RequestRouter {
    /// Escrow collateral and queue a deposit. Rejected while paused.
    fn request_deposit(env: Env, owner: Address, assets: i128) -> u64;

    /// Escrow shares and queue a withdrawal. Rejected while paused.
    fn request_withdrawal(env: Env, owner: Address, shares: i128) -> u64;

    /// Resolve the queue head. Returns `NotReady` before its delay or while paused.
    fn resolve_next(env: Env, executor: Address) -> SettlementResult;

    /// An LP request.
    fn get_request(env: Env, request_id: u64) -> LpRequest;

    /// The id of the queue head.
    fn next_request_to_resolve(env: Env) -> u64;

    /// Refund the queue head in full and move past it (PAUSER). Only once it has
    /// been resolvable for a day; the escape hatch for a head that cannot resolve.
    fn skip_head(env: Env, caller: Address);

    /// Withdraw LP payouts and refunds that could not be delivered at resolution.
    fn claim_lp_payout(env: Env, owner: Address) -> i128;

    /// An owner's undelivered LP payouts.
    fn lp_payout_claimable(env: Env, owner: Address) -> i128;

    /// Propose a WASM upgrade (UPGRADER).
    fn propose_upgrade(env: Env, caller: Address, wasm_hash: BytesN<32>);

    /// Cancel a pending upgrade (PAUSER).
    fn cancel_upgrade(env: Env, caller: Address);
}
