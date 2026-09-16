use soroban_sdk::{panic_with_error, Env};

use shared::constants::BPS;
use shared::{GlobalConfig, KeeperRewards, MarketConfig};

use crate::errors::PositionManagerError;

const MAX_BOUNDED_DURATION: u64 = 86_400;
const MAX_ORDER_LIFETIME_CEILING: u64 = 2_592_000;
const MIN_MARKET_ORDER_LIFETIME: u64 = 60;
const MAX_FUNDING_HALF_LIFE: u64 = 31_536_000;

fn reward_bounds(r: &KeeperRewards) -> (i128, i128) {
    let all = [
        r.open,
        r.limit_order,
        r.increase,
        r.decrease,
        r.close,
        r.tp,
        r.sl,
        r.expiry,
        r.liquidation,
        r.adl,
        r.lp_resolve,
    ];
    let mut lowest = all[0];
    let mut highest = all[0];
    for value in all {
        if value < lowest {
            lowest = value;
        }
        if value > highest {
            highest = value;
        }
    }
    (lowest, highest)
}

pub fn validate_global(env: &Env, c: &GlobalConfig) {
    let (lowest_reward, highest_reward) = reward_bounds(&c.keeper_rewards);
    let fee_split = c.fee_lp_revenue_share_bps as u64 + c.referral_fee_share_bps as u64;
    if c.min_collateral <= 0
        || lowest_reward < 0
        || highest_reward > c.min_collateral
        || c.min_collateral <= c.keeper_rewards.liquidation
        || c.min_position_lifetime > MAX_BOUNDED_DURATION
        || c.min_borrow_fee_seconds == 0
        || c.min_borrow_fee_seconds > MAX_BOUNDED_DURATION
        || c.max_order_lifetime_seconds == 0
        || c.max_order_lifetime_seconds > MAX_ORDER_LIFETIME_CEILING
        || c.max_market_order_lifetime < MIN_MARKET_ORDER_LIFETIME
        || c.max_market_order_lifetime > c.max_order_lifetime_seconds
        || c.funding_half_life_seconds < 60
        || c.funding_half_life_seconds > MAX_FUNDING_HALF_LIFE
        || c.max_price_age_seconds == 0
        || c.max_price_age_seconds > MAX_BOUNDED_DURATION
        || c.risk_capacity_limit_bps == 0
        || c.risk_capacity_limit_bps > BPS as u32
        || c.base_borrow_rate_bps_day < 0
        || c.base_borrow_rate_bps_day > BPS
        || c.max_variable_borrow_bps_day < 0
        || c.max_variable_borrow_bps_day > BPS
        || fee_split > BPS as u64
        || c.borrow_lp_revenue_share_bps > BPS as u32
        || c.max_active_markets == 0
        || c.global_hard_cap_limit_bps == 0
        || c.global_hard_cap_limit_bps > BPS as u32
        || c.hard_cap_relatch_band_bps == 0
        || c.hard_cap_relatch_band_bps > BPS as u32
        || c.config_timelock_seconds == 0
    {
        panic_with_error!(env, PositionManagerError::InvalidConfig);
    }
}

pub fn validate_market(env: &Env, c: &MarketConfig) {
    if c.open_fee_bps > BPS as u32
        || c.close_size_fee_bps > BPS as u32
        || c.close_pnl_fee_bps > BPS as u32
        || c.max_funding_rate_bps_day < 0
        || c.max_funding_rate_bps_day > BPS
        || c.instant_weight_bps > BPS as u32
        || c.market_risk_factor_bps == 0
        || c.market_risk_factor_bps > BPS as u32
        || c.maintenance_margin_bps == 0
        || c.maintenance_margin_bps > c.initial_margin_bps
        || c.initial_margin_bps > BPS as u32
        || c.recovery_pnl_factor_bps >= c.warning_pnl_factor_bps
        || c.warning_pnl_factor_bps >= c.adl_pnl_factor_bps
        || c.adl_pnl_factor_bps >= c.hard_cap_pnl_factor_bps
        || c.hard_cap_pnl_factor_bps > BPS as u32
        || c.max_long_size_open_interest <= 0
        || c.max_short_size_open_interest <= 0
        || c.max_long_base_exposure <= 0
        || c.max_short_base_exposure <= 0
        || c.max_long_size_open_interest > 10_000_000_000_000_000
        || c.max_short_size_open_interest > 10_000_000_000_000_000
        || c.max_long_base_exposure > 1_000_000_000_000_000_000
        || c.max_short_base_exposure > 1_000_000_000_000_000_000
        || c.order_execution_delay_seconds < 1
        || c.order_execution_delay_seconds > 30
    {
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
