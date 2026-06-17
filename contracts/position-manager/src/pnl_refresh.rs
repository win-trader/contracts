//! Unrealized PnL accounting. Single writer of `MarketUnrealizedPnl(symbol)`
//! and `TotalUnrealizedPnl`, and the only path that syncs the global
//! unrealized PnL to the Vault — which `Vault.free_liquidity` uses to bound
//! LP-claimable funds against open trader winnings.
//!
//! Every Close path and `update_indices` calls [`refresh_market_unrealized_pnl`]
//! after the market's OI / avg / mark price has settled. `RealizedPnl` is
//! tracked separately for ADL / off-chain reporting; it is NOT sent to the
//! Vault, because realized winnings have already moved USDC at close time
//! (via `vault.pay_profit` / `vault.record_absorbed_collateral`) and are
//! reflected directly in `total_assets`.

use soroban_sdk::{Env, Symbol};

use interfaces::{MarketInfo, OracleRouterClient, VaultClient};

use crate::events;
use crate::math;
use crate::storage;

pub fn refresh_market_unrealized_pnl(env: &Env, symbol: &Symbol, mark_price: i128) {
    let market = storage::get_market(env, symbol);
    let new_total = apply_market_pnl(env, symbol, &market, mark_price);
    push_net_pnl(env, new_total);
}

/// Reprice one market's unrealized PnL and fold the delta into the global
/// total. Does NOT push to the Vault — the caller batches that single
/// cross-call. Takes the already-loaded `market` so callers iterating the
/// registry don't re-read it. Returns the new global total.
fn apply_market_pnl(env: &Env, symbol: &Symbol, market: &MarketInfo, mark_price: i128) -> i128 {
    let new_market_pnl = math::calc_market_unrealized_pnl(
        market.long_open_interest,
        market.global_long_avg_price,
        market.short_open_interest,
        market.global_short_avg_price,
        mark_price,
    );

    let old_market_pnl = storage::get_market_unrealized_pnl(env, symbol);
    let delta = new_market_pnl - old_market_pnl;

    storage::set_market_unrealized_pnl(env, symbol, new_market_pnl);
    events::MarketPnlUpdate {
        symbol: symbol.clone(),
        unrealized_pnl: new_market_pnl,
    }
    .publish(env);
    let new_total = storage::get_total_unrealized_pnl(env) + delta;
    storage::set_total_unrealized_pnl(env, new_total);
    new_total
}

/// Push the global unrealized total to the Vault's `NetGlobalTraderPnl`.
fn push_net_pnl(env: &Env, total: i128) {
    let vault_addr = storage::get_vault_address(env);
    let vault = VaultClient::new(env, &vault_addr);
    let contract_addr = env.current_contract_address();
    vault.update_net_pnl(&contract_addr, &total);
}

/// Reprice every registered market's unrealized PnL against current oracle
/// marks and push the refreshed global total to the Vault. Permissionless by
/// design: it can only make the Vault's `NetGlobalTraderPnl` *more* accurate,
/// so any caller — the keeper, or an LP about to withdraw — may invoke it.
///
/// This is what makes the Vault's single `LastPnlSyncTime` honest across
/// multiple markets. A per-market tick only reprices the one market it
/// touches while bumping the global timestamp, so without a whole-book
/// repricing the freshness gate could pass on a `net_pnl` that is stale for
/// every market except the last one a keeper happened to checkpoint. After
/// this call, every market carrying open interest has been repriced.
///
/// Markets with zero open interest are skipped: they contribute zero and
/// their cached PnL is already zero from the close that flattened them, so
/// they need no oracle price. A market that *does* carry OI but whose oracle
/// cannot price it makes this call revert — the correct failure, since the
/// vault must not let an LP exit against an unknowable liability.
pub fn sync_all_unrealized_pnl(env: &Env) {
    let oracle_addr = storage::get_oracle_router(env);
    let oracle = OracleRouterClient::new(env, &oracle_addr);
    // Reprice every OI-carrying market, then push the final total to the Vault
    // once. Per-market cross-calls would all be superseded by the last anyway
    // (the call is atomic — a mid-loop revert rolls back every write).
    let mut latest_total: Option<i128> = None;
    for symbol in storage::get_market_registry(env).iter() {
        let market = storage::get_market(env, &symbol);
        if market.long_open_interest == 0 && market.short_open_interest == 0 {
            continue;
        }
        let mark = oracle.get_price(&symbol);
        latest_total = Some(apply_market_pnl(env, &symbol, &market, mark));
    }
    if let Some(total) = latest_total {
        push_net_pnl(env, total);
    }
}
