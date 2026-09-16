use crate::types::{GlobalConfig, KeeperRewards, MarketConfig};

pub const MIN_COLLATERAL: i128 = 10_000_000;
pub const MIN_POSITION_LIFETIME: u64 = 60;
pub const MAX_ORDER_LIFETIME_SECONDS: u64 = 604_800;
pub const MAX_MARKET_ORDER_LIFETIME: u64 = 300;
pub const MIN_BORROW_FEE_SECONDS: u64 = 900;
pub const FUNDING_HALF_LIFE_SECONDS: u64 = 43_200;
pub const MAX_PRICE_AGE_SECONDS: u64 = 60;
pub const RISK_CAPACITY_LIMIT_BPS: u32 = 8_500;
pub const BASE_BORROW_RATE_BPS_DAY: i128 = 25;
pub const MAX_VARIABLE_BORROW_BPS_DAY: i128 = 250;
pub const FEE_LP_REVENUE_SHARE_BPS: u32 = 9_000;
pub const BORROW_LP_REVENUE_SHARE_BPS: u32 = 9_000;
pub const REFERRAL_FEE_SHARE_BPS: u32 = 250;
pub const CONFIG_TIMELOCK_SECONDS: u64 = 172_800;
pub const MAX_ACTIVE_MARKETS: u32 = 8;
pub const GLOBAL_HARD_CAP_FACTOR_LIMIT_BPS: u32 = 10_000;
pub const HARD_CAP_RELATCH_BAND_BPS: u32 = 2_500;
pub const MAX_WITHDRAW_UTILIZATION_BPS: u32 = 8_000;
pub const MIN_DEPOSIT_NAV_FACTOR_BPS: u32 = 1_000;

pub const LP_REQUEST_DELAY_LOCAL: u64 = 60;
pub const LP_REQUEST_DELAY_TEST: u64 = 3_600;
pub const LP_REQUEST_DELAY_PRODUCTION: u64 = 86_400;

pub const KEEPER_REWARD: i128 = 2_500_000;

pub const OPEN_FEE_BPS: u32 = 0;
pub const CLOSE_SIZE_FEE_BPS: u32 = 5;
pub const CLOSE_PNL_FEE_BPS: u32 = 1_000;
pub const MAX_FUNDING_RATE_BPS_DAY: i128 = 80;
pub const INSTANT_WEIGHT_BPS: u32 = 3_000;
pub const MARKET_RISK_FACTOR_BPS: u32 = 1_000;
pub const INITIAL_MARGIN_BPS: u32 = 500;
pub const MAINTENANCE_MARGIN_BPS: u32 = 250;
pub const RECOVERY_PNL_FACTOR_BPS: u32 = 250;
pub const WARNING_PNL_FACTOR_BPS: u32 = 400;
pub const ADL_PNL_FACTOR_BPS: u32 = 500;
pub const HARD_CAP_PNL_FACTOR_BPS: u32 = 600;
pub const MAX_LONG_SIZE_OPEN_INTEREST: i128 = 1_000_000_000_000_000;
pub const MAX_SHORT_SIZE_OPEN_INTEREST: i128 = 1_000_000_000_000_000;
pub const MAX_LONG_BASE_EXPOSURE: i128 = 1_000_000_000_000_000_000;
pub const MAX_SHORT_BASE_EXPOSURE: i128 = 1_000_000_000_000_000_000;
pub const ORDER_EXECUTION_DELAY_SECONDS: u64 = 5;

pub fn keeper_rewards() -> KeeperRewards {
    KeeperRewards {
        open: KEEPER_REWARD,
        limit_order: KEEPER_REWARD,
        increase: KEEPER_REWARD,
        decrease: KEEPER_REWARD,
        close: KEEPER_REWARD,
        tp: KEEPER_REWARD,
        sl: KEEPER_REWARD,
        expiry: KEEPER_REWARD,
        liquidation: KEEPER_REWARD,
        adl: KEEPER_REWARD,
        lp_resolve: KEEPER_REWARD,
    }
}

pub fn global_config() -> GlobalConfig {
    GlobalConfig {
        min_collateral: MIN_COLLATERAL,
        min_position_lifetime: MIN_POSITION_LIFETIME,
        max_order_lifetime_seconds: MAX_ORDER_LIFETIME_SECONDS,
        max_market_order_lifetime: MAX_MARKET_ORDER_LIFETIME,
        min_borrow_fee_seconds: MIN_BORROW_FEE_SECONDS,
        funding_half_life_seconds: FUNDING_HALF_LIFE_SECONDS,
        max_price_age_seconds: MAX_PRICE_AGE_SECONDS,
        risk_capacity_limit_bps: RISK_CAPACITY_LIMIT_BPS,
        base_borrow_rate_bps_day: BASE_BORROW_RATE_BPS_DAY,
        max_variable_borrow_bps_day: MAX_VARIABLE_BORROW_BPS_DAY,
        fee_lp_revenue_share_bps: FEE_LP_REVENUE_SHARE_BPS,
        borrow_lp_revenue_share_bps: BORROW_LP_REVENUE_SHARE_BPS,
        referral_fee_share_bps: REFERRAL_FEE_SHARE_BPS,
        config_timelock_seconds: CONFIG_TIMELOCK_SECONDS,
        max_active_markets: MAX_ACTIVE_MARKETS,
        global_hard_cap_limit_bps: GLOBAL_HARD_CAP_FACTOR_LIMIT_BPS,
        hard_cap_relatch_band_bps: HARD_CAP_RELATCH_BAND_BPS,
        keeper_rewards: keeper_rewards(),
    }
}

pub fn market_config() -> MarketConfig {
    MarketConfig {
        open_fee_bps: OPEN_FEE_BPS,
        close_size_fee_bps: CLOSE_SIZE_FEE_BPS,
        close_pnl_fee_bps: CLOSE_PNL_FEE_BPS,
        max_funding_rate_bps_day: MAX_FUNDING_RATE_BPS_DAY,
        instant_weight_bps: INSTANT_WEIGHT_BPS,
        market_risk_factor_bps: MARKET_RISK_FACTOR_BPS,
        initial_margin_bps: INITIAL_MARGIN_BPS,
        maintenance_margin_bps: MAINTENANCE_MARGIN_BPS,
        recovery_pnl_factor_bps: RECOVERY_PNL_FACTOR_BPS,
        warning_pnl_factor_bps: WARNING_PNL_FACTOR_BPS,
        adl_pnl_factor_bps: ADL_PNL_FACTOR_BPS,
        hard_cap_pnl_factor_bps: HARD_CAP_PNL_FACTOR_BPS,
        max_long_size_open_interest: MAX_LONG_SIZE_OPEN_INTEREST,
        max_short_size_open_interest: MAX_SHORT_SIZE_OPEN_INTEREST,
        max_long_base_exposure: MAX_LONG_BASE_EXPOSURE,
        max_short_base_exposure: MAX_SHORT_BASE_EXPOSURE,
        order_execution_delay_seconds: ORDER_EXECUTION_DELAY_SECONDS,
    }
}
