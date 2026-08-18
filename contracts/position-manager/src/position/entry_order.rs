//! Limit / stop entry orders (doc §12.4).
//!
//! Placing an order is only a storage write — no funds move. The owner
//! separately grants a token allowance to the vault. When the oracle price
//! crosses the trigger, a keeper calls `execute_entry_order`, which pulls
//! the collateral via that allowance and opens the position through the
//! exact same core as a market `open_position` (`open::open_from_collateral`),
//! so the filled position — including `last_increased_time = fill time` — is
//! indistinguishable from one opened at market.

use crate::{
    auth::{require_auth, require_initialized, require_market_active, require_not_paused},
    errors::PositionManagerError,
    events::{self, CancelReason},
    ledger, math,
    position::open,
    snapshot, storage, validation,
};
use crate::{borrow, funding};
use shared::{EntryOrder, EntryOrderParams};
use soroban_sdk::{panic_with_error, Address, Env, Symbol};

/// Record a limit/stop entry order. No funds move; the owner must grant the
/// vault a token allowance covering `collateral + execution_budget` for the
/// keeper to pull at fill.
pub fn place_entry_order(
    env: Env,
    owner: Address,
    market_symbol: Symbol,
    params: EntryOrderParams,
) -> u64 {
    require_initialized(&env);
    require_auth(&owner);
    require_market_active(&env, &market_symbol);
    open::require_valid_open_input(
        &env,
        params.size,
        params.collateral,
        params.execution_budget,
    );

    if params.trigger_price <= 0 || params.expires_at <= env.ledger().timestamp() {
        panic_with_error!(&env, PositionManagerError::InvalidOrder);
    }

    let market = storage::get_market(&env, &market_symbol);
    // Collateral requirement is a share of notional (price-independent), so
    // it is validated the same way here as at a market open.
    open::require_sufficient_collateral(&env, params.collateral, &market.config, params.size);

    // The attached TP/SL are validated against the trigger price, since the
    // fill lands at ~the trigger; they are applied as-is at fill.
    validation::validate_orders(
        &env,
        params.is_long,
        params.take_profit,
        params.stop_loss,
        params.trigger_price,
    );

    // Fill when price rises to the trigger (trigger at/above current) or
    // falls to it (trigger below current).
    let current_price = snapshot::authenticated_price(&env, &market_symbol);
    let trigger_above = params.trigger_price >= current_price;

    let id = storage::get_next_entry_order_id(&env);
    storage::update_entry_order_id(&env);
    let order = EntryOrder {
        id,
        owner,
        market: market_symbol,
        is_long: params.is_long,
        size: params.size,
        collateral: params.collateral,
        execution_budget: params.execution_budget,
        take_profit: params.take_profit,
        stop_loss: params.stop_loss,
        acceptable_price: params.acceptable_price,
        trigger_price: params.trigger_price,
        trigger_above,
        expires_at: params.expires_at,
    };
    storage::save_entry_order(&env, &order);
    events::emit_order_placed(&env, &order);
    id
}

/// Fill an entry order whose trigger has crossed. Permissionless (keeper).
/// See the module doc's failure matrix: expired or unfundable orders are
/// removed (commit); a not-yet-triggered, slipped, or open-blocked order
/// reverts and stays pending.
pub fn execute_entry_order(env: Env, caller: Address, order_id: u64) {
    require_initialized(&env);
    require_auth(&caller);
    require_not_paused(&env);

    let order = storage::get_entry_order(&env, order_id);
    let now = env.ledger().timestamp();

    // Expired → sweep (terminal, commit).
    if now > order.expires_at {
        storage::remove_entry_order(&env, order_id);
        events::emit_order_cancelled(&env, order_id, CancelReason::Expired);
        return;
    }

    // A market disabled while the order waited is a transient block: revert
    // and leave the order pending.
    require_market_active(&env, &order.market);

    let mut market = storage::get_market(&env, &order.market);
    let mut ledger = storage::get_ledger(&env);
    borrow::accrue(&env, &mut ledger, now);
    funding::accrue(&env, &mut ledger, &mut market, now);

    let price = snapshot::authenticated_price(&env, &order.market);

    // Trigger gate (non-latching): revert if the price is not across it.
    let triggered = if order.trigger_above {
        price >= order.trigger_price
    } else {
        price <= order.trigger_price
    };
    if !triggered {
        panic_with_error!(&env, PositionManagerError::OrderNotTriggered);
    }

    // Gap protection: revert if the fill would breach the trader's bound.
    validation::check_slippage(&env, order.is_long, true, price, order.acceptable_price);

    // Pull collateral + budget via the allowance. A failure here is
    // terminal — the order is unfundable, so drop it and commit (no funds
    // moved, and the pull is only attempted once the trigger is genuinely
    // crossed).
    let total = math::add(&env, order.collateral, order.execution_budget);
    if !ledger::receive_via_allowance(&env, &order.owner, total) {
        storage::remove_entry_order(&env, order_id);
        events::emit_order_cancelled(&env, order_id, CancelReason::PullFailed);
        return;
    }

    // Open exactly as a market order would. Capacity / risk-state / margin
    // failures panic → full rollback, which returns the pulled cash and
    // leaves the order pending for a later attempt.
    let position_id = open::open_from_collateral(
        env.clone(),
        ledger,
        market,
        order.owner.clone(),
        order.market.clone(),
        order.is_long,
        order.size,
        order.collateral,
        order.execution_budget,
        order.take_profit,
        order.stop_loss,
        price,
        now,
    );

    storage::remove_entry_order(&env, order_id);
    events::emit_order_filled(&env, order_id, position_id, &caller, price);
}

/// Remove a pending entry order. The owner may cancel anytime; once expired,
/// anyone may sweep it (storage cleanup). No funds move.
pub fn cancel_entry_order(env: Env, order_id: u64) {
    require_initialized(&env);
    let order = storage::get_entry_order(&env, order_id);
    let reason = if env.ledger().timestamp() > order.expires_at {
        CancelReason::Expired
    } else {
        require_auth(&order.owner);
        CancelReason::Owner
    };
    storage::remove_entry_order(&env, order_id);
    events::emit_order_cancelled(&env, order_id, reason);
}
