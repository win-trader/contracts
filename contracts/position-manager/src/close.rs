// Position-close lifecycle. The four kind-specific entry points
// (decrease / liquidate / deleverage / execute-order) handle their own
// pre-validation (lifetime / health / ADL trigger / TP-SL trigger) and emit
// their own events; the shared `execute_close` owns the rest — settlement
// money flows, realized-PnL update, fee distribution with the underwater
// cap, OI/avg/reservation updates, position cleanup, and post-mutation
// PnL refresh.

use soroban_sdk::{panic_with_error, token::TokenClient, Address, Env, Symbol};

use interfaces::VaultClient;
use stellar_contract_utils::math::{mul_div_i128, Rounding};

use crate::config_loaders;
use crate::errors::PositionManagerError;
use crate::events;
use crate::math;
use crate::pnl_refresh::refresh_market_unrealized_pnl;
use crate::revenue;
use crate::storage;
use crate::tick::{MarketTick, PositionEvaluation};
use crate::tp_sl_escrow;
use crate::types::{CloseType, Position};
use crate::vault_view::VaultView;

struct PositionSlice {
    size: i128,
    collateral: i128,
    base_exposure: i128,
    borrow_fee_debt: i128,
    skew_fee_debt: i128,
}

fn position_slice(env: &Env, pos: &Position, size: i128) -> PositionSlice {
    if size == pos.size {
        return PositionSlice {
            size,
            collateral: pos.collateral,
            base_exposure: pos.base_exposure,
            borrow_fee_debt: pos.borrow_fee_debt,
            skew_fee_debt: pos.skew_fee_debt,
        };
    }
    PositionSlice {
        size,
        collateral: mul_div_i128(env, pos.collateral, size, pos.size, Rounding::Floor),
        base_exposure: mul_div_i128(env, pos.base_exposure, size, pos.size, Rounding::Floor),
        borrow_fee_debt: mul_div_i128(env, pos.borrow_fee_debt, size, pos.size, Rounding::Floor),
        skew_fee_debt: mul_div_i128(env, pos.skew_fee_debt, size, pos.size, Rounding::Floor),
    }
}

// ---------------------------------------------------------------------------
// Decrease (user-initiated, partial or full close)
// ---------------------------------------------------------------------------

pub fn do_decrease_position(
    env: &Env,
    trader: &Address,
    symbol: &Symbol,
    size_delta: i128,
    acceptable_price: i128,
) {
    // Load position + check lifetime FIRST so a revert-bound call doesn't
    // burn gas on the vault snapshot, oracle fetch, and index update done by
    // VaultView / MarketTick refresh.
    let pos = storage::get_position(env, trader, symbol)
        .unwrap_or_else(|| panic_with_error!(env, PositionManagerError::PositionNotFound));
    let limits = config_loaders::limits(env);
    let now = env.ledger().timestamp();
    if now < pos.last_increased_time + limits.min_position_lifetime {
        panic_with_error!(env, PositionManagerError::PositionNotOldEnough);
    }

    let vault_addr = storage::get_vault_address(env);
    let view = VaultView::refresh(env, &vault_addr);
    let market_tick = MarketTick::refresh(env, symbol, &view);
    let mark_price = market_tick.mark_price;

    // Close-side slippage. Direction is inverted from `increase`: a long is
    // closing INTO the bid (wants high mark), a short is closing INTO the ask
    // (wants low mark). Passing 0 opts out.
    if acceptable_price > 0 {
        if pos.is_long && mark_price < acceptable_price {
            panic_with_error!(env, PositionManagerError::SlippageExceeded);
        }
        if !pos.is_long && mark_price > acceptable_price {
            panic_with_error!(env, PositionManagerError::SlippageExceeded);
        }
    }

    // Reject over-close rather than silently clamping. Callers explicitly
    // request a size; an over-large delta indicates a bug or stale client
    // state and must surface as an error.
    if size_delta > pos.size {
        panic_with_error!(env, PositionManagerError::SizeDeltaExceedsPosition);
    }
    let actual_delta = size_delta;
    let is_full_close = actual_delta == pos.size;

    // Proportional collateral. mul_div widens through I256, so the
    // `collateral * actual_delta` intermediate cannot trap on a large
    // position before the divide brings it back into range.
    let slice = position_slice(env, &pos, actual_delta);
    let collateral_delta = slice.collateral;
    let eval = market_tick.evaluate(
        env,
        &pos,
        slice.size,
        slice.collateral,
        slice.base_exposure,
        slice.borrow_fee_debt,
        slice.skew_fee_debt,
    );

    execute_close(
        env,
        trader,
        symbol,
        &pos,
        market_tick,
        &slice,
        &eval,
        &CloseType::User,
        None,
    );

    let new_total_size = pos.size - actual_delta;
    let new_total_collateral = if is_full_close {
        0
    } else {
        pos.collateral - collateral_delta
    };
    events::DecreasePosition {
        trader: trader.clone(),
        symbol: symbol.clone(),
        size_delta: actual_delta,
        pnl: eval.pnl,
        borrow_fee: eval.borrow_fee,
        skew_fee: eval.skew_fee,
        mark_price,
        is_full_close,
        new_total_size,
        new_total_collateral,
    }
    .publish(env);
}

// ---------------------------------------------------------------------------
// Liquidate (keeper-initiated, gated on health < 0)
// ---------------------------------------------------------------------------

pub fn do_liquidate_position(env: &Env, caller: &Address, trader: &Address, symbol: &Symbol) {
    let vault_addr = storage::get_vault_address(env);
    let view = VaultView::refresh(env, &vault_addr);
    let market_tick = MarketTick::refresh(env, symbol, &view);
    let mark_price = market_tick.mark_price;

    let pos = storage::get_position(env, trader, symbol)
        .unwrap_or_else(|| panic_with_error!(env, PositionManagerError::PositionNotFound));

    let limits = config_loaders::limits(env);
    let slice = position_slice(env, &pos, pos.size);
    let eval = market_tick.evaluate(
        env,
        &pos,
        slice.size,
        slice.collateral,
        slice.base_exposure,
        slice.borrow_fee_debt,
        slice.skew_fee_debt,
    );

    // Both carrying fees reduce the same health value used by settlement.
    let threshold_amount = mul_div_i128(
        env,
        pos.collateral,
        limits.liquidation_threshold_bps as i128,
        shared::constants::BPS,
        Rounding::Floor,
    );
    if eval.effective_health >= threshold_amount {
        panic_with_error!(env, PositionManagerError::HealthFactorOk);
    }

    let pos_size = pos.size;
    let pos_collateral = pos.collateral;

    execute_close(
        env,
        trader,
        symbol,
        &pos,
        market_tick,
        &slice,
        &eval,
        &CloseType::Liquidation,
        Some(caller),
    );

    events::Liquidate {
        trader: trader.clone(),
        symbol: symbol.clone(),
        size: pos_size,
        collateral: pos_collateral,
        pnl: eval.pnl,
        borrow_fee: eval.borrow_fee,
        skew_fee: eval.skew_fee,
        mark_price,
        executor: caller.clone(),
    }
    .publish(env);
}

// ---------------------------------------------------------------------------
// Deleverage / ADL (keeper-initiated, gated on global PnL or utilization)
// ---------------------------------------------------------------------------

pub fn do_deleverage_position(env: &Env, trader: &Address, symbol: &Symbol) {
    // Both ADL trigger ratios use the PnL-excluded basis so an oracle wick
    // cannot shrink the denominator and spuriously fire ADL.
    //
    // The PnL trigger's numerator is `TotalUnrealizedPnl` ONLY — the vault's
    // live liability to open traders, the same value PM syncs to the vault
    // for `free_liquidity`. `RealizedPnl` is a lifetime cumulative total
    // that has already physically settled into `total_assets` (profits paid
    // via `pay_profit`, losses absorbed via `record_absorbed_collateral`);
    // folding it in would double-count history: a losing past would mask a
    // dangerous open book, and a winning past would let keepers ADL
    // positions carrying no current risk.
    //
    // Sensitivity note: `safe_basis = total_assets - unclaimed_fees` is
    // strictly ≤ `total_assets`, so both ADL ratios are *more sensitive*
    // — ADL fires slightly more aggressively for a given unrealized PnL or
    // reserved level. The magnitude depends on the unclaimed_fees buildup,
    // which is bounded by Vault's `accrue_fees` clamp
    // (`unclaimed_fees + reserved <= total_assets`) and reset by admin
    // `claim_fees` calls. Operators should treat the ADL thresholds in
    // `ProtocolLimits` as upper bounds on the safe-basis ratio, not on the
    // raw `total_assets` ratio.
    let vault_addr = storage::get_vault_address(env);
    let view = VaultView::refresh(env, &vault_addr);
    let market_tick = MarketTick::refresh(env, symbol, &view);
    let mark_price = market_tick.mark_price;

    let limits = config_loaders::limits(env);
    let unrealized_pnl = storage::get_total_unrealized_pnl(env);
    let pnl_ratio = view.adl_pnl_ratio_bps(env, unrealized_pnl);
    let utilization = view.utilization_bps();

    if pnl_ratio <= limits.adl_pnl_bps as i128 && utilization <= limits.adl_utilization_bps as i128
    {
        panic_with_error!(env, PositionManagerError::AdlNotTriggered);
    }

    let pos = storage::get_position(env, trader, symbol)
        .unwrap_or_else(|| panic_with_error!(env, PositionManagerError::PositionNotFound));

    let slice = position_slice(env, &pos, pos.size);
    let eval = market_tick.evaluate(
        env,
        &pos,
        slice.size,
        slice.collateral,
        slice.base_exposure,
        slice.borrow_fee_debt,
        slice.skew_fee_debt,
    );

    // Only profitable positions can be ADL'd.
    if eval.pnl <= 0 {
        panic_with_error!(env, PositionManagerError::AdlTargetNotProfitable);
    }

    let pos_size = pos.size;

    execute_close(
        env,
        trader,
        symbol,
        &pos,
        market_tick,
        &slice,
        &eval,
        &CloseType::Deleverage,
        None,
    );

    events::Adl {
        trader: trader.clone(),
        symbol: symbol.clone(),
        size: pos_size,
        pnl: eval.pnl,
        mark_price,
    }
    .publish(env);
}

// ---------------------------------------------------------------------------
// Execute Order (TP/SL — keeper-initiated, gated on price trigger)
// ---------------------------------------------------------------------------

pub fn do_execute_order(env: &Env, executor: &Address, trader: &Address, symbol: &Symbol) {
    // Load position + check lifetime FIRST (anti-front-running matches
    // user-initiated decrease) so a revert-bound call doesn't burn gas on
    // the vault snapshot, oracle fetch, and index update.
    let pos = storage::get_position(env, trader, symbol)
        .unwrap_or_else(|| panic_with_error!(env, PositionManagerError::PositionNotFound));
    let limits = config_loaders::limits(env);
    let now = env.ledger().timestamp();
    if now < pos.last_increased_time + limits.min_position_lifetime {
        panic_with_error!(env, PositionManagerError::PositionNotOldEnough);
    }

    let vault_addr = storage::get_vault_address(env);
    let view = VaultView::refresh(env, &vault_addr);
    let market_tick = MarketTick::refresh(env, symbol, &view);
    let mark_price = market_tick.mark_price;

    let tp_hit = market_tick.is_tp_triggered(pos.take_profit, pos.is_long);
    let sl_hit = market_tick.is_sl_triggered(pos.stop_loss, pos.is_long);

    if !tp_hit && !sl_hit {
        panic_with_error!(env, PositionManagerError::OrderNotTriggered);
    }

    let slice = position_slice(env, &pos, pos.size);
    let eval = market_tick.evaluate(
        env,
        &pos,
        slice.size,
        slice.collateral,
        slice.base_exposure,
        slice.borrow_fee_debt,
        slice.skew_fee_debt,
    );
    let pos_size = pos.size;

    execute_close(
        env,
        trader,
        symbol,
        &pos,
        market_tick,
        &slice,
        &eval,
        &CloseType::OrderExecution,
        Some(executor),
    );

    events::ExecuteOrder {
        trader: trader.clone(),
        symbol: symbol.clone(),
        size: pos_size,
        pnl: eval.pnl,
        mark_price,
        is_tp: tp_hit,
        executor: executor.clone(),
    }
    .publish(env);
}

// ---------------------------------------------------------------------------
// Shared close orchestration
// ---------------------------------------------------------------------------

/// Settle a Close's economic effects and finalize the resulting state.
///
/// Money flows: release Vault reservation, route collateral via
/// `pm_to_trader / vault_to_trader / pm_to_vault`, accrue protocol fees, pay
/// the liquidation bounty (Liquidation only), and settle the TP/SL
/// execution-fee escrow (refund / executor payout / vault forfeit per
/// close kind). State finalization: recompute global avg before decrementing
/// OI, delete-or-update the Position, persist the Market, refresh
/// Unrealized PnL.
///
/// The revenue-fee cap fires only when the close is underwater
/// (`trader_payout == 0`). In that branch, distributable fees are clamped to
/// `vault_absorbed` (collateral_delta minus bounty) so LPs aren't asked to
/// fund the dev+staker slice from their own capital and the bounty has
/// strict priority.
#[allow(clippy::too_many_arguments)]
fn execute_close(
    env: &Env,
    trader: &Address,
    symbol: &Symbol,
    pos: &Position,
    tick: MarketTick,
    slice: &PositionSlice,
    eval: &PositionEvaluation,
    kind: &CloseType,
    executor: Option<&Address>,
) {
    let size_delta = slice.size;
    let collateral_delta = slice.collateral;
    let mut market = tick.market;
    let mark_price = tick.mark_price;

    let vault_addr = storage::get_vault_address(env);
    let vault = VaultClient::new(env, &vault_addr);
    let contract_addr = env.current_contract_address();
    let asset = config_loaders::vault_asset(env);
    let token = TokenClient::new(env, &asset);

    // ----- Cluster 1: settlement -----

    let trader_payout = if eval.effective_health > 0 {
        eval.effective_health
    } else {
        0
    };

    // Token routing.
    let pm_to_trader = if trader_payout <= collateral_delta {
        trader_payout
    } else {
        collateral_delta
    };
    let vault_to_trader = trader_payout.saturating_sub(collateral_delta);
    let pm_to_vault = collateral_delta.saturating_sub(trader_payout);

    // Liquidation bounty is clamped to pm_to_vault: the bounty has priority
    // over revenue fees but cannot exceed the absorbed collateral.
    let bounty = if matches!(kind, CloseType::Liquidation) {
        let fc = config_loaders::fee_config(env);
        let raw_bounty = math::calc_liquidation_bounty(collateral_delta, fc.liquidation_bounty_bps);
        core::cmp::min(raw_bounty, pm_to_vault)
    } else {
        0
    };
    let vault_absorbed = pm_to_vault.saturating_sub(bounty);

    // Release Vault reservation.
    if size_delta > 0 {
        vault.release_liquidity(&contract_addr, &size_delta);
    }

    if vault_to_trader > 0 {
        vault.pay_profit(&contract_addr, trader, &vault_to_trader);
    }
    if vault_absorbed > 0 {
        // Snapshot the vault's balance pre-transfer so
        // `record_absorbed_collateral` can verify `post - pre == amount`.
        let pre_balance = token.balance(&vault_addr);
        token.transfer(&contract_addr, &vault_addr, &vault_absorbed);
        vault.record_absorbed_collateral(&contract_addr, trader, &vault_absorbed, &pre_balance);
    }
    if pm_to_trader > 0 {
        token.transfer(&contract_addr, trader, &pm_to_trader);
    }
    if bounty > 0 {
        if let Some(addr) = executor {
            token.transfer(&contract_addr, addr, &bounty);
        }
    }

    // Track the full economic outcome in realized PnL.
    let net_economic_pnl = eval.pnl - eval.borrow_fee - eval.skew_fee;
    let old_realized = storage::get_realized_pnl(env);
    storage::set_realized_pnl(env, old_realized + net_economic_pnl);

    // Reslice the borrow fee with the underwater cap. The cap binds
    // only when `trader_payout == 0`: in that branch absorbed collateral net
    // of bounty is the only value available for dev+staker attribution. The
    // dollars are already in the vault (via record_absorbed_collateral on the
    // underwater branch, or via the prior open-fee + collateral inflows
    // otherwise), so this is a re-tag, not a new transfer.
    let total_fees = eval.borrow_fee;
    let distributable_fees = if trader_payout > 0 {
        total_fees
    } else {
        core::cmp::min(total_fees, vault_absorbed)
    };
    revenue::reslice_revenue(env, &vault, distributable_fees);

    // Settle the position's TP/SL execution-fee escrow per Close kind. The
    // partial-User-close branch carries the escrow forward on the surviving
    // Position (see the partial-update arm of state finalization below).
    let is_partial = size_delta < pos.size;
    tp_sl_escrow::settle_on_close(
        env,
        trader,
        kind,
        pos.execution_fee_escrow,
        executor,
        is_partial,
    );

    // ----- Cluster 2: state finalization -----

    if pos.is_long {
        market.long_open_interest -= size_delta;
        market.long_base_exposure -= slice.base_exposure;
        market.global_long_avg_price =
            math::derive_entry_price(env, market.long_open_interest, market.long_base_exposure);
    } else {
        market.short_open_interest -= size_delta;
        market.short_base_exposure -= slice.base_exposure;
        market.global_short_avg_price =
            math::derive_entry_price(env, market.short_open_interest, market.short_base_exposure);
    }

    // Vault's ReservedUsdc was already decremented by release_liquidity above.

    // Delete or update the Position.
    let is_full_close = size_delta == pos.size;
    if is_full_close {
        storage::delete_position(env, trader, symbol);
    } else {
        let updated = Position {
            collateral: pos.collateral - collateral_delta,
            size: pos.size - size_delta,
            base_exposure: pos.base_exposure - slice.base_exposure,
            entry_price: math::derive_entry_price(
                env,
                pos.size - size_delta,
                pos.base_exposure - slice.base_exposure,
            ),
            borrow_fee_debt: pos.borrow_fee_debt - slice.borrow_fee_debt,
            skew_fee_debt: pos.skew_fee_debt - slice.skew_fee_debt,
            is_long: pos.is_long,
            last_increased_time: pos.last_increased_time,
            take_profit: pos.take_profit,
            stop_loss: pos.stop_loss,
            execution_fee_escrow: pos.execution_fee_escrow,
        };
        storage::set_position(env, trader, symbol, &updated);
    }

    storage::set_market(env, symbol, &market);

    // Refresh Unrealized PnL after the OI/avg mutations.
    refresh_market_unrealized_pnl(env, symbol, mark_price);
}
