//! Single source of truth for every named numeric constant in the protocol.
//!
//! Three categories of value live here:
//!   1. TTL constants (storage extend window in ledgers).
//!   2. Default values seeded by `ConfigManager::__constructor`.
//!   3. Compile-time floors/ceilings that bound the admin-tunable values
//!      ConfigManager stores. Changing a floor or ceiling requires a contract
//!      upgrade (and waits the configured `upgrade_timelock_seconds`).
//!
//! All numerical literals in the protocol must reference one of these names
//! rather than inlining a magic number.

// ---------------------------------------------------------------------------
// Time
// ---------------------------------------------------------------------------

/// Stellar mainnet target ledger close time.
pub const SECONDS_PER_LEDGER: u64 = 5;
/// 17_280 ledgers per day at 5s ledger close time.
pub const LEDGERS_PER_DAY: u32 = 17_280;
/// 31_536_000 — 365 days. Used in annualised rate math.
pub const SECONDS_PER_YEAR: u64 = 31_536_000;

// ---------------------------------------------------------------------------
// TTL constants (instance + shared persistent storage extend window)
// ---------------------------------------------------------------------------

/// 30 days in ledgers — threshold before extending instance storage.
pub const INSTANCE_THRESHOLD: u32 = 30 * LEDGERS_PER_DAY;
/// 31 days in ledgers — target lifetime after extending instance storage.
pub const INSTANCE_BUMP: u32 = 31 * LEDGERS_PER_DAY;

/// 45 days in ledgers — threshold before extending shared persistent storage.
pub const SHARED_THRESHOLD: u32 = 45 * LEDGERS_PER_DAY;
/// 46 days in ledgers — target lifetime after extending shared persistent storage.
pub const SHARED_BUMP: u32 = 46 * LEDGERS_PER_DAY;
/// 46 days in seconds — ceiling for `cooldown_duration` so a write can never
/// outlive the TTL of the slot it touches.
pub const SHARED_BUMP_SECONDS: u64 = (SHARED_BUMP as u64) * SECONDS_PER_LEDGER;

// ---------------------------------------------------------------------------
// Math precision (used by PositionManager + tests)
// ---------------------------------------------------------------------------

/// 1e7 — price precision. All on-chain prices are scaled by this.
pub const PRECISION: i128 = 10_000_000;
/// Protocol-wide price scale, expressed as a decimal exponent: `PRECISION ==
/// 10^PRICE_DECIMALS`. Every SEP-40 source the OracleRouter aggregates must
/// report this scale, or its prices would skew the median.
pub const PRICE_DECIMALS: u32 = 7;
/// 1e14 — carrying-fee index accumulator precision.
pub const INDEX_PRECISION: i128 = 100_000_000_000_000;
/// 1e18 — stored base-exposure precision used by additive PnL math.
pub const EXPOSURE_PRECISION: i128 = 1_000_000_000_000_000_000;
/// 10_000 — basis-point denominator. Single source of truth.
pub const BPS: i128 = 10_000;

// ---------------------------------------------------------------------------
// Role constants — mirrored in ConfigManager's role names.
// ---------------------------------------------------------------------------

/// Ultimate authority — typically a multi-sig or DAO. Can manage all roles.
pub const ROLE_ADMIN: &str = "ADMIN";
/// Authorized to push WASM upgrades to protocol contracts.
pub const ROLE_UPGRADER: &str = "UPGRADER";
/// Authorized to pause/unpause Vault and PositionManager.
pub const ROLE_PAUSER: &str = "PAUSER";
/// Whitelisted keeper bot network for liquidations, ADL, index updates.
pub const ROLE_KEEPER: &str = "KEEPER";
/// Whitelisted oracle publishers — push CEX/aggregator prices into oracle
/// contracts. Distinct from KEEPER so the price-publishing surface can be
/// rotated/revoked independently of the liquidation keeper network.
pub const ROLE_ORACLE: &str = "ORACLE";

// ---------------------------------------------------------------------------
// Defaults seeded by ConfigManager::__constructor
// ---------------------------------------------------------------------------

/// Default dev/treasury fee share: 10% (1000 bps).
pub const DEFAULT_DEV_BPS: u32 = 1_000;
/// Default LP fee share: 90% (9000 bps).
pub const DEFAULT_LP_BPS: u32 = 9_000;
/// Default staker fee share: 0% — stakers may not be onboarded yet.
pub const DEFAULT_STAKER_BPS: u32 = 0;

/// Default open-fee bps applied to notional on position open: 0.1% (10 bps).
pub const DEFAULT_OPEN_FEE_BPS: u32 = 10;
/// Default liquidation bounty paid to liquidator from collateral: 1% (100 bps).
pub const DEFAULT_LIQUIDATION_BOUNTY_BPS: u32 = 100;
/// Default flat TP/SL execution fee: 0.5 USDC at PRECISION (1e7) scale.
pub const DEFAULT_TP_SL_EXECUTION_FEE: i128 = 5_000_000;

/// Maximum open fee: 1% (100 bps).
pub const MAX_OPEN_FEE_BPS: u32 = 100;
/// Maximum liquidation bounty: 10% (1000 bps).
pub const MAX_LIQUIDATION_BOUNTY_BPS: u32 = 1_000;
/// Maximum flat TP/SL execution fee at PRECISION scale.
pub const MAX_TP_SL_EXECUTION_FEE: i128 = 100_000_000_000;

/// Default minimum collateral: $1 USDC at 1e7 precision.
pub const DEFAULT_MIN_COLLATERAL: i128 = 10_000_000;
/// Default cooldown between vault deposit and withdrawal: 5 minutes.
pub const DEFAULT_COOLDOWN_DURATION: u64 = 300;
/// Default minimum position lifetime: 60 seconds.
pub const DEFAULT_MIN_POSITION_LIFETIME: u64 = 60;
/// Default max vault utilization: 85% (8500 bps).
pub const DEFAULT_MAX_UTILIZATION_RATIO: i128 = 8_500;
/// Default ADL trigger: net PnL / total assets threshold: 90% (9000 bps).
pub const DEFAULT_ADL_PNL_BPS: u32 = 9_000;
/// Default ADL trigger: utilization threshold: 95% (9500 bps).
pub const DEFAULT_ADL_UTILIZATION_BPS: u32 = 9_500;
/// Default liquidation health threshold: 2% (200 bps). Liquidations trigger
/// when health < collateral × threshold_bps / 10_000.
pub const DEFAULT_LIQUIDATION_THRESHOLD_BPS: u32 = 200;

/// Default base borrow rate: 1% annualized (100 bps).
pub const DEFAULT_BASE_BORROW_RATE_BPS: i128 = 100;
/// Default borrow rate slope below optimal utilization: 5% (500 bps).
pub const DEFAULT_SLOPE1_BPS: i128 = 500;
/// Default borrow rate slope above optimal utilization: 50% (5000 bps).
pub const DEFAULT_SLOPE2_BPS: i128 = 5_000;
/// Default optimal utilization breakpoint: 80% (8000 bps).
pub const DEFAULT_OPTIMAL_UTILIZATION_BPS: i128 = 8_000;
/// Default maximum skew carrying rate: 50% annualized at maximum risk.
pub const DEFAULT_MAX_SKEW_RATE_BPS: i128 = 5_000;

/// Default oracle quorum. Two sources preserves a real quorum and deviation
/// check when multiple feeds are configured.
pub const DEFAULT_MIN_REQUIRED_SOURCES: u32 = 2;
/// Default upgrade timelock: 24h. ConfigManager admin can raise but not lower
/// below `MIN_UPGRADE_TIMELOCK`.
pub const DEFAULT_UPGRADE_TIMELOCK: u64 = 86_400;

// ---------------------------------------------------------------------------
// Floors / ceilings — bound the admin-tunable values above. Changing these
// requires a contract upgrade (subject to the 24h timelock).
// ---------------------------------------------------------------------------

/// Minimum permissible `upgrade_timelock_seconds` — 24h. The admin cannot
/// shorten the timelock below this floor.
pub const MIN_UPGRADE_TIMELOCK: u64 = 86_400;

/// Maximum per-market `max_leverage` (i128 to match math contexts).
pub const MAX_LEVERAGE_CAP: i128 = 200;
/// Minimum permissible `max_leverage`. `set_max_leverage(symbol, 1)` is
/// rejected — market disablement is a distinct, event-emitting entrypoint.
pub const MIN_LEVERAGE: u32 = 2;

/// Maximum permissible `max_deviation_bps` in OracleConfig — 100%. Stops the
/// admin from disabling the deviation gate by setting it to `i128::MAX`.
pub const MAX_DEVIATION_BPS_CEILING: i128 = 10_000;

/// Maximum number of oracle sources per symbol (`primary + secondary` for
/// the legacy API, or the flat source pool post-refactor). Bounds the O(n²)
/// dedup cost.
pub const MAX_ORACLE_SOURCES: u32 = 16;
/// Minimum permissible `min_required_sources`. A single-source median has no
/// quorum and a structurally-zero deviation check.
pub const MIN_REQUIRED_SOURCES_FLOOR: u32 = 2;

/// Minimum permissible `adl_pnl_bps` — 50%. Stops the admin from configuring
/// continuous ADL.
pub const MIN_ADL_PNL_BPS: u32 = 5_000;
/// Maximum permissible `slope2_bps` — 200% per-year slope. Stops the admin
/// from configuring a slope that overflows in PM borrow-fee math.
pub const MAX_SLOPE2_BPS: i128 = 20_000;
/// Maximum permissible `cooldown_duration` — must not exceed the TTL of the
/// `LockupExpiresAt` slot it bumps.
pub const MAX_COOLDOWN_DURATION: u64 = SHARED_BUMP_SECONDS;

/// Maximum permissible `base_borrow_rate_bps` — 100% per year. Stops the
/// admin from configuring a base rate that overflows or runs away in PM
/// borrow-fee index math (same rationale as `MAX_SLOPE2_BPS`).
pub const MAX_BASE_BORROW_RATE_BPS: i128 = 10_000;
/// Maximum permissible dominant-side skew carrying rate: 200% per year.
pub const MAX_SKEW_RATE_BPS: i128 = 20_000;
/// Minimum permissible `liquidation_threshold_bps` — 0.5%. A zero threshold
/// makes positions liquidatable only once health is already negative, so
/// every liquidation would start inside a bad-debt window.
pub const MIN_LIQUIDATION_THRESHOLD_BPS: u32 = 50;
/// Maximum permissible `min_position_lifetime` — 1 day.
pub const MAX_MIN_POSITION_LIFETIME_SECS: u64 = 86_400;
/// Maximum permissible upgrade timelock — 30 days. Bounds admin error: an
/// oversized timelock would push every upgrade eta past the horizon (or
/// overflow the eta addition) and block all upgrade proposals.
pub const MAX_UPGRADE_TIMELOCK_SECS: u64 = 2_592_000;
/// Lifetime of a pending admin proposal — 7 days. `accept_admin` rejects
/// older proposals so a forgotten proposal is not a standing capability
/// held by the proposed key.
pub const ADMIN_PROPOSAL_TTL_SECS: u64 = 604_800;
