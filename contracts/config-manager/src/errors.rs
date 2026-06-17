use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ConfigManagerError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    /// `set_upgrade_timelock` called with seconds below `MIN_UPGRADE_TIMELOCK`.
    UpgradeTimelockTooShort = 6,
    /// `propose_admin(caller, new_admin)` rejected because `caller == new_admin`.
    InvalidAdminProposal = 7,
    /// `accept_admin` rejected — caller is not the currently pending admin.
    NotPendingAdmin = 8,
    /// `accept_admin` rejected — there is no pending admin proposal.
    NoPendingAdmin = 9,
    /// `upgrade` rejected — no `propose_upgrade` was made before commit.
    /// The two-step upgrade flow requires a prior proposal.
    NoPendingUpgrade = 10,
    /// `upgrade` rejected — timelock has not elapsed yet.
    UpgradeTimelockNotElapsed = 11,
    /// `upgrade` rejected — `new_wasm_hash` does not match the proposed
    /// `PendingUpgrade.wasm_hash`.
    UpgradeHashMismatch = 12,
    /// `accept_admin` rejected — the proposal is older than
    /// `ADMIN_PROPOSAL_TTL_SECS`.
    AdminProposalExpired = 13,
    /// `set_upgrade_timelock` called with seconds above
    /// `MAX_UPGRADE_TIMELOCK_SECS`.
    UpgradeTimelockTooLong = 14,

    // ---- Per-rule FeeSplits codes ----
    /// FeeSplits components (lp/dev/staker) do not sum to exactly BPS.
    InvalidFeeSplitSum = 22,

    // ---- Per-rule ProtocolLimits codes (30–37) ----
    /// `min_collateral` is not strictly positive.
    InvalidMinCollateral = 30,
    /// `max_utilization_ratio` is out of (0, BPS] range.
    InvalidMaxUtilization = 31,
    /// `funding_cut_bps` exceeds `MAX_FUNDING_CUT_BPS`.
    InvalidFundingCut = 32,
    /// `adl_pnl_bps` is below `MIN_ADL_PNL_BPS` or above BPS.
    InvalidAdlPnl = 33,
    /// `adl_utilization_bps` is out of (0, BPS] range.
    InvalidAdlUtilization = 34,
    /// `liquidation_threshold_bps` is below `MIN_LIQUIDATION_THRESHOLD_BPS`
    /// or exceeds 10% of collateral.
    InvalidLiquidationThreshold = 35,
    /// `cooldown_duration` exceeds `MAX_COOLDOWN_DURATION`.
    InvalidCooldownDuration = 36,
    /// `min_position_lifetime` exceeds `MAX_MIN_POSITION_LIFETIME_SECS`.
    InvalidMinPositionLifetime = 37,

    // ---- Per-rule BorrowRateConfig codes (40–43) ----
    /// A BorrowRateConfig rate is negative.
    InvalidBorrowRateNegative = 40,
    /// `optimal_utilization_bps` is out of (0, BPS] range.
    InvalidOptimalUtilization = 41,
    /// `slope2_bps < slope1_bps` — kink curve must be non-decreasing.
    InvalidSlopeOrdering = 42,
    /// `slope2_bps` exceeds `MAX_SLOPE2_BPS`.
    InvalidSlopeTooSteep = 43,

    // ---- Per-rule FeeConfig codes (44–46) ----
    /// `open_fee_bps` exceeds `MAX_OPEN_FEE_BPS`.
    InvalidOpenFee = 44,
    /// `liquidation_bounty_bps` exceeds `MAX_LIQUIDATION_BOUNTY_BPS`.
    InvalidLiquidationBounty = 45,
    /// `tp_sl_execution_fee` is negative or exceeds `MAX_TP_SL_EXECUTION_FEE`.
    InvalidTpSlExecutionFee = 46,
    /// `base_borrow_rate_bps` exceeds `MAX_BASE_BORROW_RATE_BPS`.
    InvalidBaseBorrowRate = 47,
    /// `base_funding_rate_bps` exceeds `MAX_BASE_FUNDING_RATE_BPS`.
    InvalidBaseFundingRate = 48,
}
