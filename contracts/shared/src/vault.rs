//! Shared vault contract interface.
//!
//! The vault is deliberately thin: it holds the one collateral-token
//! balance and the LP share token, and it moves cash only on instruction
//! from the PositionManager (claim transfers) or the RequestRouter (LP
//! settlement). All accounting state lives in the PositionManager.
//!
//! The two claim-transfer entry points differ in exactly one invariant:
//! `transfer_claim` re-checks cash-ownership conservation
//! (`physical - amount >= claims_after`) and therefore fails during a cash
//! shortfall — use it for ordinary outgoing claims (§15.2 stops those).
//! `transfer_safety_claim` skips that check — it is reserved for position
//! settlement, liquidation rewards, and budget payouts, which must not be
//! blockable by a shortfall (a safety action must never wait, §15.4).

use soroban_sdk::{contractclient, Address, BytesN, Env};

use crate::types::{AccountingSnapshot, LpConfig, OracleRound, SettlementResult};

#[contractclient(name = "VaultClient")]
pub trait VaultInterface {
    /// One-time wiring of the RequestRouter address (ADMIN).
    fn set_request_router(env: Env, caller: Address, request_router: Address);

    /// Pull `amount` collateral from `from` into the vault
    /// (PositionManager only). The caller is responsible for recording the
    /// matching claim.
    fn receive_collateral(env: Env, caller: Address, from: Address, amount: i128);

    /// Conservation-checked outgoing claim (PositionManager only):
    /// `claims_after` is the caller's post-transfer non-LP claim total, and
    /// the transfer must leave at least that much cash behind. Fails during
    /// a shortfall.
    fn transfer_claim(
        env: Env,
        caller: Address,
        recipient: Address,
        amount: i128,
        claims_after: i128,
    );

    /// Unchecked outgoing settlement transfer (PositionManager only) — for
    /// trader payouts, liquidation/keeper rewards, and execution budgets.
    /// See the module docs for why this bypasses the conservation check.
    fn transfer_safety_claim(env: Env, caller: Address, recipient: Address, amount: i128);

    /// Settle a matured deposit request against `round` (RequestRouter
    /// only, §13.5). Returns `Failed` (escrow refunded by the router)
    /// instead of panicking for business rejections.
    fn settle_deposit(
        env: Env,
        caller: Address,
        owner: Address,
        assets: i128,
        round: OracleRound,
    ) -> SettlementResult;

    /// Settle a matured withdrawal request against `round` (RequestRouter
    /// only, §13.6). Full-or-nothing; never leaves a cash claim behind.
    fn settle_withdrawal(
        env: Env,
        caller: Address,
        owner: Address,
        shares: i128,
        round: OracleRound,
    ) -> SettlementResult;

    fn set_lp_config(env: Env, caller: Address, config: LpConfig);
    fn get_lp_config(env: Env) -> LpConfig;
    /// Whether the protocol currently accepts new LP requests (§14).
    fn can_create_lp_request(env: Env) -> bool;
    /// Read-only accounting snapshot at `round` — delegates to the
    /// PositionManager with the current physical cash.
    fn accounting_snapshot(env: Env, round: OracleRound) -> AccountingSnapshot;
    /// `collateral_token.balanceOf(vault)` — the only authoritative cash
    /// balance (§4.1).
    fn physical_cash(env: Env) -> i128;
    fn query_asset(env: Env) -> Address;
    fn total_share_supply(env: Env) -> i128;

    /// Operational pause: LP settlements resolve as `Failed` (escrow
    /// refunded) instead of executing.
    fn pause(env: Env, caller: Address);
    fn unpause(env: Env, caller: Address);
    fn propose_upgrade(env: Env, caller: Address, wasm_hash: BytesN<32>);
    fn cancel_upgrade(env: Env, caller: Address);
    /// Re-extend instance storage TTL. Open to anyone.
    fn bump_vault_state(env: Env);
}
