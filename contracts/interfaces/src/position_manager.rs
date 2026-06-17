use soroban_sdk::{contractclient, Address, BytesN, Env, Symbol};

use crate::types::{MarketInfo, Position};

/// PositionManager contract interface.
/// Trading engine for the perpetual DEX (positions, ADL, liquidations).
#[contractclient(name = "PositionManagerClient")]
pub trait PositionManager {
    // Initialization is the contract `__constructor(config_manager,
    // oracle_router)` — atomic with deploy, closing the first-caller
    // front-running window. Not a trait method (Soroban constructors are
    // inherent). The Vault link is wired separately via `set_vault` because
    // Vault and PositionManager reference each other: Vault binds its trusted
    // PositionManager atomically in its own constructor (the custodial side),
    // so PositionManager is deployed first and cannot know the Vault address
    // until after the Vault exists.

    /// Wire the linked Vault. ADMIN-only and one-shot — reverts once the Vault
    /// is already set, so the trusted Vault link is immutable after deploy.
    /// Called once by the deployer immediately after the Vault is deployed.
    fn set_vault(env: Env, caller: Address, vault_address: Address);

    /// Open or add to a leveraged position. `acceptable_price` bounds the
    /// mark price the open is willing to execute at — pass `0` to skip the
    /// slippage check. For longs, revert if `mark_price > acceptable_price`;
    /// for shorts, revert if `mark_price < acceptable_price`.
    ///
    /// **TP/SL semantics on increase**: `take_profit = 0` and `stop_loss = 0`
    /// mean "leave the prior value unchanged" — `0` does NOT clear an
    /// existing order. To clear TP/SL, call [`set_tp_sl`] with the explicit
    /// `0` value (which clears, refunds escrow, and emits `SetTpSl`).
    fn increase_position(
        env: Env,
        trader: Address,
        symbol: Symbol,
        size: i128,
        collateral: i128,
        is_long: bool,
        take_profit: i128,
        stop_loss: i128,
        acceptable_price: i128,
    );

    /// Close or reduce a position and realize PnL. `acceptable_price` bounds
    /// the mark price the close is willing to execute at — pass `0` to skip
    /// the slippage check. For longs (closing on the bid), revert if
    /// `mark_price < acceptable_price`; for shorts (closing on the ask),
    /// revert if `mark_price > acceptable_price`.
    fn decrease_position(
        env: Env,
        trader: Address,
        symbol: Symbol,
        size_delta: i128,
        acceptable_price: i128,
    );

    /// Force-close an undercollateralized position. Permissionless: any
    /// authenticated caller may invoke it; the health gate is the only
    /// barrier, and the caller earns the liquidation bounty.
    fn liquidate_position(env: Env, caller: Address, trader: Address, symbol: Symbol);

    /// Sync global borrow and funding accumulators. KEEPER only. Works
    /// during pause (accrual clamps at the pause boundary).
    fn update_indices(env: Env, caller: Address, symbol: Symbol);

    /// Reprice every market's unrealized PnL against current marks and push
    /// the global total to the Vault. Permissionless and no-arg: it only
    /// makes the Vault's synced PnL more accurate, so a withdrawing LP can
    /// call it to satisfy the Vault's PnL-freshness gate without depending on
    /// the keeper. Reverts if a market carrying open interest cannot be priced.
    fn sync_unrealized_pnl(env: Env);

    /// Execute a TP/SL order. Permissionless: any authenticated caller may
    /// invoke it; the price-trigger gate is the only barrier, and the caller
    /// earns the execution-fee escrow.
    fn execute_order(env: Env, caller: Address, trader: Address, symbol: Symbol);

    /// Set take-profit and stop-loss prices on an existing position. Passing
    /// `0` for either field CLEARS that side; this is the opposite of the
    /// `0`-means-leave-unchanged semantics on [`increase_position`]. Calling
    /// `set_tp_sl(trader, symbol, 0, 0)` clears both and refunds the
    /// execution-fee escrow.
    fn set_tp_sl(env: Env, trader: Address, symbol: Symbol, take_profit: i128, stop_loss: i128);

    /// Auto-Deleveraging: force-close highest-RoE position. KEEPER only.
    fn deleverage_position(env: Env, caller: Address, trader: Address, symbol: Symbol);

    /// Extend Soroban TTL for an active position.
    fn bump_position(env: Env, user_address: Address, symbol: Symbol);

    /// Emergency pause — PAUSER role only.
    fn pause(env: Env, caller: Address);

    /// Unpause — PAUSER role only.
    fn unpause(env: Env, caller: Address);

    /// Set the maximum leverage for a market. ADMIN only.
    /// Floor enforced at `shared::constants::MIN_LEVERAGE` — use
    /// `disable_market` to take a market offline.
    fn set_max_leverage(env: Env, caller: Address, symbol: Symbol, max_leverage: i128);

    /// Get the maximum leverage for a market.
    fn get_max_leverage(env: Env, symbol: Symbol) -> i128;

    /// Disable trading for `symbol` — opens are rejected, closes still work.
    /// PAUSER only. Distinct from a global pause; emits MarketDisabled.
    fn disable_market(env: Env, caller: Address, symbol: Symbol);

    /// Re-enable a previously disabled market. PAUSER only.
    fn enable_market(env: Env, caller: Address, symbol: Symbol);

    /// Returns true if `symbol` is currently disabled for opens.
    fn is_market_disabled(env: Env, symbol: Symbol) -> bool;

    /// Propose a WASM upgrade. UPGRADER role only.
    fn propose_upgrade(env: Env, caller: Address, wasm_hash: BytesN<32>);

    /// PAUSER veto of a pending upgrade.
    fn cancel_upgrade(env: Env, caller: Address);

    /// Read-only: get a trader's position for a symbol.
    fn get_position(env: Env, trader: Address, symbol: Symbol) -> Position;

    /// Read-only: get global market state for a symbol.
    fn get_market(env: Env, symbol: Symbol) -> MarketInfo;

    /// Cumulative realized PnL across all closed positions (net of fees).
    /// Read-only, reporting only. Tracked separately from
    /// `total_unrealized_pnl` because realized winnings/losses have already
    /// moved USDC through `pay_profit` / `record_absorbed_collateral`, and
    /// are therefore reflected directly in `vault.total_assets`. Not used by
    /// any risk gate — the ADL trigger reads `total_unrealized_pnl` alone.
    fn realized_pnl(env: Env) -> i128;

    /// Net unrealized PnL across all open positions across all markets. This
    /// is the value PM syncs to `vault.update_net_pnl` so `free_liquidity` can
    /// clamp LP-claimable funds against open trader winnings. Realized PnL is
    /// intentionally excluded.
    fn total_unrealized_pnl(env: Env) -> i128;

    /// Per-market unrealized PnL — the contribution of `symbol` to
    /// `total_unrealized_pnl`. Sum across all active markets must equal
    /// `total_unrealized_pnl` (an invariant the test suite asserts).
    fn market_unrealized_pnl(env: Env, symbol: Symbol) -> i128;
}
