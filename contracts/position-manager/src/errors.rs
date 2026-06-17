use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum PositionManagerError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Paused = 3,
    /// New trade would push vault utilization past MaxUtilizationRatio (85%).
    UtilizationCapBreached = 4,
    /// decrease_position called before MinPositionLifetime has elapsed.
    PositionNotOldEnough = 5,
    PositionNotFound = 6,
    Unauthorized = 7,
    ZeroAmount = 8,
    /// liquidate_position called but the position is still healthy.
    HealthFactorOk = 9,
    /// deleverage_position called but ADL trigger conditions are not met.
    AdlNotTriggered = 10,
    /// Position leverage exceeds the per-market max leverage.
    ExcessiveLeverage = 11,
    /// No max leverage configured for this market symbol.
    MarketNotConfigured = 12,
    /// execute_order called but neither TP nor SL trigger condition is met.
    OrderNotTriggered = 13,
    /// Invalid take-profit or stop-loss price for the position direction.
    InvalidTpSl = 14,
    /// increase_position called with `is_long` opposite to the existing position's direction.
    DirectionMismatch = 15,
    /// Collateral below the protocol's min_collateral limit.
    BelowMinCollateral = 16,
    /// ADL target position is not profitable (PnL <= 0).
    AdlTargetNotProfitable = 17,
    /// Max leverage exceeds the absolute safety cap (200x).
    LeverageCapExceeded = 18,
    /// Mark price at execution time exceeded the trader's `acceptable_price`.
    SlippageExceeded = 19,
    /// `set_max_leverage` called with a value below `MIN_LEVERAGE`. Use
    /// `disable_market` to take a market offline instead.
    LeverageBelowFloor = 20,
    /// Trading is disabled for this market — `enable_market` re-opens it.
    MarketDisabled = 21,
    /// `decrease_position` called with `size_delta > pos.size`. Use
    /// `pos.size` (or simply close fully) instead of over-closing.
    SizeDeltaExceedsPosition = 22,
    /// `upgrade` rejected — no `propose_upgrade` was made before commit.
    NoPendingUpgrade = 23,
    /// `upgrade` rejected — timelock has not elapsed yet.
    UpgradeTimelockNotElapsed = 24,
    /// `upgrade` rejected — `new_wasm_hash` does not match the proposed
    /// `PendingUpgrade.wasm_hash`.
    UpgradeHashMismatch = 25,
}
