//! Bounds validation for ConfigManager-tunable structs. Each rule fires a
//! distinct per-rule error code (22–48) so off-chain monitors can identify
//! which constraint was violated.
//!
//! Pattern: `value.validate(&env)` panics on the first violation. Used at
//! every entrypoint that accepts one of these wire structs so a future
//! mutator cannot bypass the bounds.

use shared::{BorrowRateConfig, FeeConfig, FeeSplits, ProtocolLimits};
use shared::constants::{
    BPS, MAX_BASE_BORROW_RATE_BPS, MAX_BASE_FUNDING_RATE_BPS, MAX_COOLDOWN_DURATION,
    MAX_FUNDING_CUT_BPS, MAX_LIQUIDATION_BOUNTY_BPS, MAX_MIN_POSITION_LIFETIME_SECS,
    MAX_OPEN_FEE_BPS, MAX_SLOPE2_BPS, MAX_TP_SL_EXECUTION_FEE, MIN_ADL_PNL_BPS,
    MIN_LIQUIDATION_THRESHOLD_BPS,
};
use soroban_sdk::{panic_with_error, Env};

use crate::errors::ConfigManagerError;

/// Concentrates the bounds invariants for the three tunable wire structs.
/// Implemented locally — orphan rule prevents adding `impl` blocks on
/// `shared` types directly.
pub trait Validate {
    /// Panics with the rule-specific `ConfigManagerError` variant on
    /// failure; returns normally otherwise.
    fn validate(&self, env: &Env);
}

impl Validate for FeeSplits {
    fn validate(&self, env: &Env) {
        // Promote to u64 so adversarial u32 components cannot wrap before
        // the sum comparison reaches BPS.
        let sum = (self.lp_bps as u64) + (self.dev_bps as u64) + (self.staker_bps as u64);
        if sum != BPS as u64 {
            panic_with_error!(env, ConfigManagerError::InvalidFeeSplitSum);
        }
    }
}

impl Validate for FeeConfig {
    fn validate(&self, env: &Env) {
        if self.open_fee_bps > MAX_OPEN_FEE_BPS {
            panic_with_error!(env, ConfigManagerError::InvalidOpenFee);
        }
        if self.liquidation_bounty_bps > MAX_LIQUIDATION_BOUNTY_BPS {
            panic_with_error!(env, ConfigManagerError::InvalidLiquidationBounty);
        }
        if self.tp_sl_execution_fee < 0 || self.tp_sl_execution_fee > MAX_TP_SL_EXECUTION_FEE {
            panic_with_error!(env, ConfigManagerError::InvalidTpSlExecutionFee);
        }
    }
}

impl Validate for ProtocolLimits {
    fn validate(&self, env: &Env) {
        if self.min_collateral < 1 {
            panic_with_error!(env, ConfigManagerError::InvalidMinCollateral);
        }
        if self.max_utilization_ratio < 1 || self.max_utilization_ratio > BPS {
            panic_with_error!(env, ConfigManagerError::InvalidMaxUtilization);
        }
        if self.funding_cut_bps > MAX_FUNDING_CUT_BPS {
            panic_with_error!(env, ConfigManagerError::InvalidFundingCut);
        }
        if self.adl_pnl_bps < MIN_ADL_PNL_BPS || self.adl_pnl_bps > (BPS as u32) {
            panic_with_error!(env, ConfigManagerError::InvalidAdlPnl);
        }
        if self.adl_utilization_bps < 1 || self.adl_utilization_bps > (BPS as u32) {
            panic_with_error!(env, ConfigManagerError::InvalidAdlUtilization);
        }
        if self.liquidation_threshold_bps < MIN_LIQUIDATION_THRESHOLD_BPS
            || self.liquidation_threshold_bps > (BPS as u32) / 10
        {
            panic_with_error!(env, ConfigManagerError::InvalidLiquidationThreshold);
        }
        if self.cooldown_duration > MAX_COOLDOWN_DURATION {
            panic_with_error!(env, ConfigManagerError::InvalidCooldownDuration);
        }
        if self.min_position_lifetime > MAX_MIN_POSITION_LIFETIME_SECS {
            panic_with_error!(env, ConfigManagerError::InvalidMinPositionLifetime);
        }
    }
}

impl Validate for BorrowRateConfig {
    fn validate(&self, env: &Env) {
        if self.base_borrow_rate_bps < 0
            || self.slope1_bps < 0
            || self.slope2_bps < 0
            || self.base_funding_rate_bps < 0
        {
            panic_with_error!(env, ConfigManagerError::InvalidBorrowRateNegative);
        }
        if self.optimal_utilization_bps < 1 || self.optimal_utilization_bps > BPS {
            panic_with_error!(env, ConfigManagerError::InvalidOptimalUtilization);
        }
        if self.slope2_bps < self.slope1_bps {
            panic_with_error!(env, ConfigManagerError::InvalidSlopeOrdering);
        }
        if self.slope2_bps > MAX_SLOPE2_BPS {
            panic_with_error!(env, ConfigManagerError::InvalidSlopeTooSteep);
        }
        if self.base_borrow_rate_bps > MAX_BASE_BORROW_RATE_BPS {
            panic_with_error!(env, ConfigManagerError::InvalidBaseBorrowRate);
        }
        if self.base_funding_rate_bps > MAX_BASE_FUNDING_RATE_BPS {
            panic_with_error!(env, ConfigManagerError::InvalidBaseFundingRate);
        }
    }
}
