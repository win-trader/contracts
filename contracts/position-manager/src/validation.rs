use soroban_sdk::{panic_with_error, Env};

use shared::{GlobalConfig, MarketConfig};

use crate::errors::PositionManagerError;

pub fn validate_global(env: &Env, c: &GlobalConfig) {
    if !shared::validation::global_is_valid(c) {
        panic_with_error!(env, PositionManagerError::InvalidConfig);
    }
}

pub fn validate_market(env: &Env, c: &MarketConfig) {
    if !shared::validation::market_is_valid(c) {
        panic_with_error!(env, PositionManagerError::InvalidConfig);
    }
}

pub fn validate_orders(env: &Env, is_long: bool, take_profit: i128, stop_loss: i128, price: i128) {
    if take_profit < 0 || stop_loss < 0 {
        panic_with_error!(env, PositionManagerError::InvalidOrder);
    }
    let invalid = if is_long {
        (take_profit > 0 && take_profit <= price) || (stop_loss > 0 && stop_loss >= price)
    } else {
        (take_profit > 0 && take_profit >= price) || (stop_loss > 0 && stop_loss <= price)
    };
    if invalid {
        panic_with_error!(env, PositionManagerError::InvalidOrder);
    }
}
