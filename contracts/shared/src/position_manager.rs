use soroban_sdk::{contractclient, Address, BytesN, Env, Symbol, Vec};

use crate::types::{
    AccountingSnapshot, ActionOutcome, GlobalConfig, Market, MarketConfig, OpenPayload,
    PendingAction, Position,
};

/// Accounting ledger of the protocol: markets, positions, non-LP claims, and fee indices.
/// Prices, notionals, and cash use `PRICE_PRECISION`; a `0` price bound disables that check.
#[contractclient(name = "PositionManagerClient")]
pub trait PositionManager {
    /// One-time wiring of the vault address (ADMIN).
    fn set_vault(env: Env, caller: Address, vault: Address);

    /// The price feed currently in use.
    fn price_feed(env: Env) -> Address;

    /// Commit a market open and escrow its collateral. Binding; returns the action id.
    fn create_market_open(env: Env, owner: Address, market: Symbol, request: OpenPayload) -> u64;

    /// Commit a limit open and escrow its collateral. Cancellable until expiry.
    fn create_limit_open(
        env: Env,
        owner: Address,
        market: Symbol,
        request: OpenPayload,
        trigger_price: i128,
    ) -> u64;

    /// Settle a market open. The first eligible attempt executes or fails terminally.
    fn settle_market_open(env: Env, keeper: Address, action_id: u64) -> ActionOutcome;

    /// Settle a limit open. Returns `Pending` until the trigger crosses.
    fn settle_limit_open(env: Env, keeper: Address, action_id: u64) -> ActionOutcome;

    /// Owner cancels a limit open before expiry. Returns the refund.
    fn cancel_limit_open(env: Env, action_id: u64) -> i128;

    /// Clean up an expired entry: pays the expiry reward and refunds the rest.
    fn clean_expired_entry(env: Env, keeper: Address, action_id: u64);

    /// Add collateral to a position immediately. No fee, reward, or window reset.
    fn add_collateral(env: Env, position_id: u64, amount: i128);

    /// Commit a partial decrease of `size_removed`.
    fn create_decrease(env: Env, position_id: u64, size_removed: i128, acceptable_price: i128)
        -> u64;

    /// Commit a full close of whatever size remains at settlement.
    fn create_close(env: Env, position_id: u64, acceptable_price: i128) -> u64;

    /// Settle a committed decrease.
    fn settle_decrease(env: Env, keeper: Address, action_id: u64) -> ActionOutcome;

    /// Settle a committed close.
    fn settle_close(env: Env, keeper: Address, action_id: u64) -> ActionOutcome;

    /// Attach or replace a take-profit instruction.
    fn set_take_profit(env: Env, position_id: u64, trigger_price: i128, acceptable_price: i128);

    /// Remove the take-profit instruction.
    fn clear_take_profit(env: Env, position_id: u64);

    /// Attach or replace a stop-loss instruction.
    fn set_stop_loss(env: Env, position_id: u64, trigger_price: i128, acceptable_price: i128);

    /// Remove the stop-loss instruction.
    fn clear_stop_loss(env: Env, position_id: u64);

    /// Close a position whose take-profit has crossed. Returns `Pending` outside its bound.
    fn execute_take_profit(env: Env, keeper: Address, position_id: u64) -> ActionOutcome;

    /// Close a position whose stop-loss has crossed. Returns `Pending` outside its bound.
    fn execute_stop_loss(env: Env, keeper: Address, position_id: u64) -> ActionOutcome;

    /// Liquidate a position at or below its liquidation threshold. Permissionless.
    fn liquidate_position(env: Env, keeper: Address, position_id: u64);

    /// Deleverage a profitable position on a side in `ADL` or `HardCap`. Permissionless.
    fn execute_adl(env: Env, keeper: Address, position_id: u64) -> ActionOutcome;

    /// A pending action. Panics `ActionNotFound` once consumed.
    fn get_pending_action(env: Env, action_id: u64) -> PendingAction;

    /// Checkpoint the borrow index and one market's funding to now. Permissionless.
    fn update_indices(env: Env, caller: Address, market: Symbol);

    /// The MarketGovernor, the only caller allowed to change configuration.
    fn governor(env: Env) -> Address;

    /// Install a global config (governor only). `actor` is who triggered it.
    fn install_global_config(env: Env, caller: Address, actor: Address, config: GlobalConfig);

    /// Register a market or install its config (governor only).
    fn install_market_config(
        env: Env,
        caller: Address,
        actor: Address,
        market_symbol: Symbol,
        config: MarketConfig,
    );

    /// Switch the price feed (governor only).
    fn install_price_feed(env: Env, caller: Address, actor: Address, price_feed: Address);

    /// Remove an empty market from the registry (governor only). Its indices are kept.
    fn deregister_market(env: Env, caller: Address, actor: Address, market_symbol: Symbol);

    /// Block new exposure on one market (PAUSER).
    fn disable_market(env: Env, caller: Address, market: Symbol);

    /// Re-admit exposure on one market (UNPAUSER).
    fn enable_market(env: Env, caller: Address, market: Symbol);

    /// Whether a market is disabled.
    fn is_market_disabled(env: Env, market: Symbol) -> bool;

    /// Checkpoint, price every active market, persist risk states, and snapshot (vault only).
    fn prepare_lp_snapshot(env: Env, caller: Address, physical_cash: i128) -> AccountingSnapshot;

    /// Recompute the borrow rate after vault cash moved (vault only).
    fn refresh_borrow_rate(env: Env, caller: Address, physical_cash: i128);

    /// Whether the protocol is paused.
    fn is_paused(env: Env) -> bool;

    /// Read-only accounting snapshot at current prices.
    fn accounting_snapshot(env: Env, physical_cash: i128) -> AccountingSnapshot;

    /// A position.
    fn get_position(env: Env, position_id: u64) -> Position;

    /// A market's configuration and accounting.
    fn get_market(env: Env, market: Symbol) -> Market;

    /// The active market registry.
    fn active_markets(env: Env) -> Vec<Symbol>;

    /// The global configuration.
    fn global_config(env: Env) -> GlobalConfig;

    /// Guaranteed receiver funding not yet credited to positions.
    fn pending_receiver_funding_total(env: Env) -> i128;

    /// Collected protocol revenue not yet claimed.
    fn protocol_claimable_total(env: Env) -> i128;

    /// All non-LP claims on vault cash.
    fn non_lp_claims(env: Env) -> i128;

    /// Withdraw protocol revenue (PROTOCOL). Blocked while paused or short.
    fn claim_protocol(env: Env, caller: Address, recipient: Address, amount: i128);

    /// Add cash to the vault without minting shares. Open to anyone.
    fn recapitalize(env: Env, contributor: Address, amount: i128);

    /// Withdraw payouts and refunds that could not be delivered at settlement.
    fn claim_payout(env: Env, owner: Address) -> i128;

    /// An owner's undelivered payouts.
    fn unclaimed_payout(env: Env, owner: Address) -> i128;

    /// All undelivered payouts.
    fn unclaimed_payout_total(env: Env) -> i128;

    /// Pause new exposure; exits stay open (PAUSER).
    fn pause(env: Env, caller: Address);

    /// Clear the pause (UNPAUSER).
    fn unpause(env: Env, caller: Address);

    /// Propose a WASM upgrade (UPGRADER).
    fn propose_upgrade(env: Env, caller: Address, wasm_hash: BytesN<32>);

    /// Cancel a pending upgrade (PAUSER).
    fn cancel_upgrade(env: Env, caller: Address);
}
