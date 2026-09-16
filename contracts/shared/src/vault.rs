use soroban_sdk::{contractclient, Address, BytesN, Env};

use crate::types::{AccountingSnapshot, LpConfig, SettlementResult};

/// Holds the collateral balance and the LP share token. Moves cash only for the
/// position manager and the request router; all accounting lives in the position manager.
#[contractclient(name = "VaultClient")]
pub trait VaultInterface {
    /// One-time wiring of the request router (ADMIN).
    fn set_request_router(env: Env, caller: Address, request_router: Address);

    /// Pull collateral from `from` into the vault (position manager only).
    fn receive_collateral(env: Env, caller: Address, from: Address, amount: i128);

    /// Pay out a claim, leaving at least `claims_after` in cash (position manager only).
    fn transfer_claim(
        env: Env,
        caller: Address,
        recipient: Address,
        amount: i128,
        claims_after: i128,
    );

    /// Pay out a settlement, refund, or keeper reward without the conservation check
    /// (position manager only).
    fn transfer_safety_claim(env: Env, caller: Address, recipient: Address, amount: i128);

    /// Settle a matured deposit (request router only). Business rejections return `Failed`.
    fn settle_deposit(
        env: Env,
        caller: Address,
        owner: Address,
        assets: i128,
    ) -> SettlementResult;

    /// Settle a matured withdrawal (request router only). Pays the resolve reward to
    /// `executor` and the owner's assets to the router.
    fn settle_withdrawal(
        env: Env,
        caller: Address,
        owner: Address,
        shares: i128,
        executor: Address,
    ) -> SettlementResult;

    /// The live `keeper_lp_resolve_reward`.
    fn lp_resolve_reward(env: Env) -> i128;

    /// The position manager's `config_timelock_seconds`.
    fn config_timelock_seconds(env: Env) -> u64;

    /// Set the LP request policy (ADMIN).
    fn set_lp_config(env: Env, caller: Address, config: LpConfig);

    /// The LP request policy.
    fn get_lp_config(env: Env) -> LpConfig;

    /// Whether LP requests may be created (not paused).
    fn can_create_lp_request(env: Env) -> bool;

    /// Whether the vault or the protocol is paused.
    fn lp_paused(env: Env) -> bool;

    /// Read-only accounting snapshot at current prices.
    fn accounting_snapshot(env: Env) -> AccountingSnapshot;

    /// The vault's collateral token balance.
    fn physical_cash(env: Env) -> i128;

    /// The collateral token.
    fn query_asset(env: Env) -> Address;

    /// Total LP share supply.
    fn total_share_supply(env: Env) -> i128;

    /// Hold LP request creation and resolution (PAUSER).
    fn pause(env: Env, caller: Address);

    /// Clear the vault pause (UNPAUSER).
    fn unpause(env: Env, caller: Address);

    /// Propose a WASM upgrade (UPGRADER).
    fn propose_upgrade(env: Env, caller: Address, wasm_hash: BytesN<32>);

    /// Cancel a pending upgrade (PAUSER).
    fn cancel_upgrade(env: Env, caller: Address);

    /// Extend instance storage TTL. Open to anyone.
    fn bump_vault_state(env: Env);
}
