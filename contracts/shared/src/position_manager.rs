//! Shared PositionManager contract interface.
//!
//! The PositionManager is the protocol's accounting ledger: it owns the
//! market and position state, the non-LP claim totals, and every fee index.
//! The vault holds the cash; this contract decides who owns it.
//!
//! Conventions used across the trait:
//! - Prices, USD notionals, and base exposures are scaled by
//!   `constants::PRICE_PRECISION`; cash amounts use the collateral token's
//!   native decimals (identical scale on this deployment).
//! - `0` is the "none" sentinel for `take_profit`, `stop_loss`, and
//!   `acceptable_price` — a zero bound disables that check.
//! - Functions taking a `caller` verify both `require_auth` and a
//!   ConfigManager role (or a specific contract address); functions taking
//!   `owner`/`position_id` require the position owner's auth.

use soroban_sdk::{contractclient, Address, BytesN, Env, Symbol, Vec};

use crate::types::{AccountingSnapshot, GlobalConfig, Market, MarketConfig, OracleRound, Position};

#[contractclient(name = "PositionManagerClient")]
pub trait PositionManager {
    /// One-time wiring of the vault address (ADMIN). Panics with
    /// `AlreadyInitialized` on a second call.
    fn set_vault(env: Env, caller: Address, vault: Address);

    /// Open a leveraged position (§12.1). Transfers
    /// `collateral + execution_budget` from `owner` — nothing is charged at
    /// open (§11.1) — and enforces the initial margin, capacity, and
    /// market-side limits. `acceptable_price` bounds the execution price
    /// (max for longs, min for shorts; `0` = no bound). Returns the new
    /// position id.
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

    /// Add size and/or collateral to an open position (§12.1). Capitalizes
    /// all accrued fees first; added size is held to the initial margin and
    /// must pass the same risk gates as an open.
    fn increase_position(
        env: Env,
        position_id: u64,
        size_added: i128,
        collateral_added: i128,
        acceptable_price: i128,
    );

    /// Remove size and/or withdraw collateral (§12.2). `size_removed` equal
    /// to the position size is a full close and settles through the close
    /// waterfall; a partial close capitalizes accrued fees and must leave
    /// the position at or above maintenance margin. Rejected before
    /// `min_position_lifetime` has elapsed since the last increase.
    fn decrease_position(
        env: Env,
        position_id: u64,
        size_removed: i128,
        collateral_withdrawn: i128,
        acceptable_price: i128,
    );

    /// Close a position whose effective collateral (including pending fees
    /// and payable PnL) is below maintenance margin (§12.3). Open to any
    /// authenticated caller; pays the liquidation reward from the position
    /// and, for an insolvent position, a capped touch reward from the
    /// risk-keeper reserve.
    fn liquidate_position(env: Env, caller: Address, position_id: u64);

    /// Close a profitable position on a side in the ADL or hard-cap state
    /// (KEEPER, §14). Pays a capped reward from the risk-keeper reserve.
    fn deleverage_position(env: Env, caller: Address, position_id: u64);

    /// Execute a triggered take-profit/stop-loss close (§12.4). Open to any
    /// authenticated caller; pays the position's full execution budget to
    /// the executor. Panics `InvalidOrder` if no trigger price is crossed.
    fn execute_order(env: Env, caller: Address, position_id: u64);

    /// Set the conditional-order trigger prices (owner). `0` clears a
    /// trigger; a nonzero trigger must be on the correct side of the
    /// current price.
    fn set_tp_sl(env: Env, position_id: u64, take_profit: i128, stop_loss: i128);

    /// Add executor cash to a position's execution budget (owner, §12.4).
    fn fund_execution_budget(env: Env, position_id: u64, amount: i128);

    /// Withdraw unused execution budget (owner). Blocked during a cash
    /// shortfall via the vault's conservation-checked transfer.
    fn withdraw_execution_budget(env: Env, position_id: u64, amount: i128);

    /// Checkpoint the global indices and one market's funding indices to
    /// now (KEEPER, §10). Fee accrual is lazy; this bounds staleness.
    fn update_indices(env: Env, caller: Address, market: Symbol);

    /// Replace the global configuration (ADMIN). Checkpoints first so the
    /// old parameters price all past time (§10.3).
    fn set_global_config(env: Env, caller: Address, config: GlobalConfig);

    /// Create a market or replace an existing market's configuration
    /// (ADMIN). Bounded by `max_active_markets` and the global hard-cap
    /// factor limit.
    fn set_market_config(env: Env, caller: Address, market: Symbol, config: MarketConfig);

    /// Block new opens/increases on one market (PAUSER). Existing positions
    /// keep accruing and can always decrease, close, or be liquidated.
    fn disable_market(env: Env, caller: Address, market: Symbol);
    fn enable_market(env: Env, caller: Address, market: Symbol);
    fn is_market_disabled(env: Env, market: Symbol) -> bool;

    /// LP-settlement snapshot (vault only, §13.5/§13.6): checkpoints global
    /// accrual, evaluates and persists every side's risk state at the
    /// round's prices, and returns the accounting snapshot the settlement
    /// decides against.
    fn prepare_lp_snapshot(
        env: Env,
        caller: Address,
        round: OracleRound,
        physical_cash: i128,
    ) -> AccountingSnapshot;

    /// Recompute the borrow rate from current utilization (vault only,
    /// called after vault cash moved).
    fn refresh_borrow_rate(env: Env, caller: Address, physical_cash: i128);

    /// Whether new LP requests may be created: no cash shortfall and no
    /// side in a restricted risk state (vault only, §14).
    fn can_create_lp_request(env: Env, caller: Address, physical_cash: i128) -> bool;

    /// Read-only accounting snapshot for `round` — no risk-state
    /// transitions are persisted and no accrual checkpoint runs.
    fn accounting_snapshot(env: Env, round: OracleRound, physical_cash: i128)
        -> AccountingSnapshot;

    fn get_position(env: Env, position_id: u64) -> Position;
    fn get_market(env: Env, market: Symbol) -> Market;
    fn active_markets(env: Env) -> Vec<Symbol>;
    fn global_config(env: Env) -> GlobalConfig;
    /// The guaranteed receiver-funding liability (§8.3).
    fn pending_receiver_funding_total(env: Env) -> i128;
    fn protocol_claimable_total(env: Env) -> i128;
    fn risk_keeper_reserve_total(env: Env) -> i128;
    /// Complete non-LP claims on the vault's physical cash (§4.2).
    fn non_lp_claims(env: Env) -> i128;

    /// Pay out protocol revenue (ADMIN). Conservation-checked against the
    /// remaining claims, so it is blocked during a cash shortfall.
    fn claim_protocol(env: Env, caller: Address, recipient: Address, amount: i128);

    /// Transfer cash into the vault without minting shares (§15.2). Open to
    /// anyone; the cure for a cash shortfall.
    fn recapitalize(env: Env, contributor: Address, amount: i128);

    /// Operational pause: blocks opens and increases. Accrual clocks keep
    /// running (§10.3) and closes/liquidations stay available.
    fn pause(env: Env, caller: Address);
    fn unpause(env: Env, caller: Address);

    fn propose_upgrade(env: Env, caller: Address, wasm_hash: BytesN<32>);
    fn cancel_upgrade(env: Env, caller: Address);

    /// Re-extend a position entry's storage TTL. Open to anyone.
    fn bump_position(env: Env, position_id: u64);
}
