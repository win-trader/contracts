//! `set_tp_sl` flow: change a Position's TP and SL prices, charging /
//! refunding the TP/SL execution-fee escrow as the orders are added or
//! cleared. Direction validation also lives here so `increase.rs` can call
//! `validate_tp_sl` on the post-update Position without depending on the
//! whole module.

use soroban_sdk::{panic_with_error, Address, Env, Symbol};

use crate::config_loaders;
use crate::errors::PositionManagerError;
use crate::events;
use crate::storage;
use crate::tp_sl_escrow;
use crate::types::Position;

pub fn do_set_tp_sl(
    env: &Env,
    trader: &Address,
    symbol: &Symbol,
    take_profit: i128,
    stop_loss: i128,
) {
    let mut pos = storage::get_position(env, trader, symbol)
        .unwrap_or_else(|| panic_with_error!(env, PositionManagerError::PositionNotFound));

    validate_tp_sl(env, &pos, take_profit, stop_loss);

    let tp_sl_execution_fee = config_loaders::fee_config(env).tp_sl_execution_fee;
    let delta = tp_sl_escrow::escrow_delta(
        pos.execution_fee_escrow,
        take_profit,
        stop_loss,
        tp_sl_execution_fee,
    );
    tp_sl_escrow::apply_delta(env, trader, delta);
    pos.execution_fee_escrow += delta;

    pos.take_profit = take_profit;
    pos.stop_loss = stop_loss;
    storage::set_position(env, trader, symbol, &pos);

    events::SetTpSl {
        trader: trader.clone(),
        symbol: symbol.clone(),
        take_profit,
        stop_loss,
    }
    .publish(env);
}

/// Validate TP/SL prices. Only `>= 0` is enforced — a trader is free to set
/// values on either side of their entry price (required for trailing-stop
/// and profit-locking workflows, which need SL above entry on a winning
/// long, etc.). Frontends are responsible for warning on values that would
/// trigger immediately at the current mark.
pub(crate) fn validate_tp_sl(env: &Env, _pos: &Position, take_profit: i128, stop_loss: i128) {
    if take_profit < 0 || stop_loss < 0 {
        panic_with_error!(env, PositionManagerError::InvalidTpSl);
    }
}
