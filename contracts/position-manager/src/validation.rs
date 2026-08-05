//! Input validation: governance config bounds, slippage guards, and
//! conditional-order sanity checks.

use soroban_sdk::{panic_with_error, Env};

use shared::constants::BPS;
use shared::{GlobalConfig, MarketConfig};

use crate::errors::PositionManagerError;

pub fn validate_global(env: &Env, c: &GlobalConfig) {
    let split = c.lp_revenue_share_bps as u64 + c.risk_keeper_revenue_share_bps as u64;
    if c.min_collateral <= 0
        || c.risk_capacity_limit_bps == 0
        || c.risk_capacity_limit_bps > BPS as u32
        || c.base_borrow_rate_bps_day < 0
        || c.base_borrow_rate_bps_day > BPS
        || c.max_variable_borrow_bps_day < 0
        || c.max_variable_borrow_bps_day > BPS
        || split > BPS as u64
        || c.hard_cap_factor_limit_bps > BPS as u32
        || c.max_adl_reward < 0
        || c.max_insolvent_touch_reward < 0
        || c.max_active_markets == 0
    {
        panic_with_error!(env, PositionManagerError::InvalidConfig);
    }
}

pub fn validate_market(env: &Env, c: &MarketConfig) {
    if c.close_fee_low_bps > c.close_fee_high_bps
        || c.close_fee_high_bps > BPS as u32
        || c.max_funding_rate_bps_day < 0
        || c.max_funding_rate_bps_day > BPS
        || c.market_risk_factor_bps == 0
        || c.market_risk_factor_bps > BPS as u32
        || c.recovery_pnl_factor_bps >= c.warning_pnl_factor_bps
        || c.warning_pnl_factor_bps >= c.adl_pnl_factor_bps
        || c.adl_pnl_factor_bps >= c.hard_cap_pnl_factor_bps
        || c.hard_cap_pnl_factor_bps > BPS as u32
        || c.maintenance_margin_bps == 0
        || c.maintenance_margin_bps > c.initial_margin_bps
        || c.initial_margin_bps > BPS as u32
        || c.liquidation_reward_bps > BPS as u32
        || c.adl_reward_bps > BPS as u32
        || c.max_long_size_open_interest <= 0
        || c.max_short_size_open_interest <= 0
        || c.max_long_base_exposure <= 0
        || c.max_short_base_exposure <= 0
    {
        panic_with_error!(env, PositionManagerError::InvalidConfig);
    }
}

/// Slippage guard. `acceptable == 0` means no bound. Opening a long (or
/// closing a short) tolerates prices at or below the bound; the mirror
/// directions tolerate prices at or above it.
pub fn check_slippage(env: &Env, is_long: bool, opening: bool, price: i128, acceptable: i128) {
    if acceptable == 0 {
        return;
    }
    let bad = if opening {
        if is_long {
            price > acceptable
        } else {
            price < acceptable
        }
    } else if is_long {
        price < acceptable
    } else {
        price > acceptable
    };
    if bad {
        panic_with_error!(env, PositionManagerError::SlippageExceeded);
    }
}

/// Conditional-order sanity: a zero trigger means "none"; a nonzero trigger
/// must sit on the profitable (take-profit) or losing (stop-loss) side of
/// the current price.
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
