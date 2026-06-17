//! `increase_position` flow: open a new Position or add to an existing one.
//! Settlement money flows for revenue go through [`crate::revenue`]; TP/SL
//! escrow lifecycle is owned by [`crate::tp_sl_escrow`]; the surviving
//! Position math (weighted-avg entry, weighted-avg indices) is local.

use soroban_sdk::{panic_with_error, token::TokenClient, Address, Env, Symbol};

use interfaces::VaultClient;

use crate::config_loaders;
use crate::errors::PositionManagerError;
use crate::events;
use crate::math;
use crate::pnl_refresh::refresh_market_unrealized_pnl;
use crate::revenue;
use crate::storage;
use crate::tick::MarketTick;
use crate::tp_sl::validate_tp_sl;
use crate::tp_sl_escrow;
use crate::types::Position;

pub fn do_increase_position(
    env: &Env,
    trader: &Address,
    symbol: &Symbol,
    size: i128,
    collateral: i128,
    is_long: bool,
    take_profit: i128,
    stop_loss: i128,
    acceptable_price: i128,
) {
    // Snapshot vault state once before the mark price is fetched, then
    // refresh indices (the tick uses `view.utilization_bps()` internally for
    // the borrow-rate update, so both refreshes use the same basis).
    let vault_addr = storage::get_vault_address(env);
    let view = crate::vault_view::VaultView::refresh(env, &vault_addr);
    let market_tick = MarketTick::refresh(env, symbol, &view);
    let mark_price = market_tick.mark_price;
    let mut market = market_tick.market;

    // acceptable_price slippage. Passing 0 opts out. For longs the mark must
    // be at-or-below acceptable; for shorts at-or-above.
    if acceptable_price > 0 {
        if is_long && mark_price > acceptable_price {
            panic_with_error!(env, PositionManagerError::SlippageExceeded);
        }
        if !is_long && mark_price < acceptable_price {
            panic_with_error!(env, PositionManagerError::SlippageExceeded);
        }
    }

    let vault = VaultClient::new(env, &vault_addr);

    let existing = storage::get_position(env, trader, symbol);

    // Enforce minimum collateral. Fee is a sidecar — the check is on the
    // param value, not on `collateral - fee`.
    let limits = config_loaders::limits(env);
    if collateral < limits.min_collateral {
        panic_with_error!(env, PositionManagerError::BelowMinCollateral);
    }

    // Open fee + TP/SL escrow.
    let fee_config = config_loaders::fee_config(env);
    let open_fee = math::calc_open_fee(size, fee_config.open_fee_bps);

    let prior_escrow = existing.as_ref().map(|p| p.execution_fee_escrow).unwrap_or(0);
    let prior_tp = existing.as_ref().map(|p| p.take_profit).unwrap_or(0);
    let prior_sl = existing.as_ref().map(|p| p.stop_loss).unwrap_or(0);
    let (resulting_tp, resulting_sl) =
        tp_sl_escrow::resulting_tp_sl(prior_tp, prior_sl, take_profit, stop_loss);
    // Increase never refunds — TP/SL clearing happens via `set_tp_sl`.
    let escrow_owed = core::cmp::max(
        0,
        tp_sl_escrow::escrow_delta(
            prior_escrow,
            resulting_tp,
            resulting_sl,
            fee_config.tp_sl_execution_fee,
        ),
    );

    let position = match existing {
        Some(mut pos) => {
            if is_long != pos.is_long {
                panic_with_error!(env, PositionManagerError::DirectionMismatch);
            }
            pos.entry_price =
                math::update_global_avg_price(pos.entry_price, pos.size, mark_price, size);
            // Weighted-average entry indices so accrued fees reset proportionally
            pos.entry_borrow_index = math::update_global_avg_price(
                pos.entry_borrow_index,
                pos.size,
                market.acc_borrow_index,
                size,
            );
            pos.entry_funding_index = math::update_global_avg_price(
                pos.entry_funding_index,
                pos.size,
                market.acc_funding_index,
                size,
            );
            pos.size += size;
            pos.collateral += collateral;
            pos.last_increased_time = env.ledger().timestamp();
            if take_profit > 0 {
                pos.take_profit = take_profit;
            }
            if stop_loss > 0 {
                pos.stop_loss = stop_loss;
            }
            pos.execution_fee_escrow += escrow_owed;
            pos
        }
        None => Position {
            collateral,
            size,
            entry_price: mark_price,
            entry_borrow_index: market.acc_borrow_index,
            entry_funding_index: market.acc_funding_index,
            is_long,
            last_increased_time: env.ledger().timestamp(),
            take_profit,
            stop_loss,
            execution_fee_escrow: escrow_owed,
        },
    };

    let max_leverage = storage::get_max_leverage(env, symbol)
        .unwrap_or_else(|| panic_with_error!(env, PositionManagerError::MarketNotConfigured));
    // `checked_mul` guards against an i128 overflow trap when an adversarial
    // caller passes a collateral value near `i128::MAX / max_leverage`.
    // Overflow implies the leverage limit is exceeded by definition (the
    // bound product itself exceeds the i128 range), so route to the typed
    // `ExcessiveLeverage` rather than a host panic.
    let max_size = position
        .collateral
        .checked_mul(max_leverage)
        .unwrap_or_else(|| panic_with_error!(env, PositionManagerError::ExcessiveLeverage));
    if position.size > max_size {
        panic_with_error!(env, PositionManagerError::ExcessiveLeverage);
    }

    validate_tp_sl(env, &position, position.take_profit, position.stop_loss);

    if is_long {
        market.global_long_avg_price = math::update_global_avg_price(
            market.global_long_avg_price,
            market.long_open_interest,
            mark_price,
            size,
        );
        market.long_open_interest += size;
    } else {
        market.global_short_avg_price = math::update_global_avg_price(
            market.global_short_avg_price,
            market.short_open_interest,
            mark_price,
            size,
        );
        market.short_open_interest += size;
    }

    // Use the snapshot's safe basis — the mark-price-insensitive denominator
    // — so a wicking oracle cannot bias whether opens pass the cap. Computed
    // against the POST-reserve value so we never overshoot.
    let new_reserved = view.reserved + size;
    let util_bps = math::calc_utilization_bps(new_reserved, view.safe_basis);
    if util_bps > limits.max_utilization_ratio {
        panic_with_error!(env, PositionManagerError::UtilizationCapBreached);
    }

    // Move collateral + open_fee + escrow_owed from trader to PM in a single
    // bundled transfer — AFTER validation so a reverted open never strands
    // the trader's funds in PM.
    let trader_owed = collateral + open_fee + escrow_owed;
    transfer_collateral_in(env, trader, trader_owed);

    // Forward the open fee to vault + slice (PM already holds the dollars).
    revenue::recv_revenue(env, &vault_addr, open_fee);

    let contract_addr = env.current_contract_address();
    vault.reserve_liquidity(&contract_addr, &size);

    storage::set_position(env, trader, symbol, &position);
    storage::set_market(env, symbol, &market);

    events::IncreasePosition {
        trader: trader.clone(),
        symbol: symbol.clone(),
        size_delta: size,
        collateral,
        entry_price: position.entry_price,
        is_long,
        tp: position.take_profit,
        sl: position.stop_loss,
        new_total_size: position.size,
        new_total_collateral: position.collateral,
        entry_borrow_index: position.entry_borrow_index,
        entry_funding_index: position.entry_funding_index,
        last_increased_time: position.last_increased_time,
    }
    .publish(env);

    refresh_market_unrealized_pnl(env, symbol, mark_price);
}

fn transfer_collateral_in(env: &Env, trader: &Address, amount: i128) {
    let asset = config_loaders::vault_asset(env);
    let token = TokenClient::new(env, &asset);
    let contract_addr = env.current_contract_address();
    token.transfer(trader, &contract_addr, &amount);
}
