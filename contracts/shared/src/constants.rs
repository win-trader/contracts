//! Shared time, precision, role, oracle, and governance limits.

// ---------------------------------------------------------------------------
// Time
// ---------------------------------------------------------------------------

/// Stellar mainnet target ledger close time.
pub const SECONDS_PER_LEDGER: u64 = 5;
/// 17_280 ledgers per day at 5s ledger close time.
pub const LEDGERS_PER_DAY: u32 = 17_280;
/// 86_400 — denominator for all bps-per-day rate math.
pub const SECONDS_PER_DAY: u64 = 86_400;

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
/// 46 days in seconds. An LP request delay cannot outlive its storage entry.
pub const SHARED_BUMP_SECONDS: u64 = (SHARED_BUMP as u64) * SECONDS_PER_LEDGER;

// ---------------------------------------------------------------------------
// Math precision (used by PositionManager + tests)
// ---------------------------------------------------------------------------

/// 1e7 — price precision. All on-chain prices, USD notionals, and base
/// exposures are scaled by this.
///
/// Scale mapping against the design doc's numerical model
/// (`docs/design/2026-07-fee-vault-contract-mechanics-ste100.md` §3). The doc
/// specifies idealized 10^30 precisions; the implementation picks smaller
/// scales so every intermediate product fits i128 (a size × index product at
/// 10^30 would overflow at ~1.7 units):
///
/// | doc name          | code name         | scale              |
/// |-------------------|-------------------|--------------------|
/// | `PRICE_PRECISION` | `PRICE_PRECISION` | 1e7                |
/// | `INDEX_PRECISION` | `INDEX_PRECISION` | 1e14               |
/// | `RATE_PRECISION`  | `INDEX_PRECISION` | 1e14 (one scale for rates and indices) |
/// | `FACTOR_PRECISION`| bps (`BPS`)       | 1e4                |
/// | `SHARE_PRECISION` | vault decimals offset | asset decimals + 6 |
/// | `ASSET_PRECISION` | token decimals    | collateral-token native |
///
/// PnL numerators (§7.2) carry one extra `PRICE_PRECISION` factor and are
/// converted to cash exactly once at the final step.
pub const PRICE_PRECISION: i128 = 10_000_000;
/// Protocol-wide price scale, expressed as a decimal exponent:
/// `PRICE_PRECISION == 10^PRICE_DECIMALS`. Every SEP-40 source the
/// OracleRouter aggregates must report this scale, or its prices would skew
/// the median.
pub const PRICE_DECIMALS: u32 = 7;
/// 1e14 — borrow/funding index accumulator precision. Also the scale for
/// stored rates (the doc's `RATE_PRECISION`): bps/day rates are stored
/// multiplied by this so fractional per-second accrual never rounds to zero
/// before the remainder carry.
pub const INDEX_PRECISION: i128 = 100_000_000_000_000;
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
/// Default upgrade timelock: 24h. ConfigManager admin can raise but not lower
/// below `MIN_UPGRADE_TIMELOCK`.
pub const DEFAULT_UPGRADE_TIMELOCK: u64 = 86_400;

// ---------------------------------------------------------------------------
// Oracle and governance limits.
// ---------------------------------------------------------------------------

/// Minimum permissible `upgrade_timelock_seconds` — 24h. The admin cannot
/// shorten the timelock below this floor.
pub const MIN_UPGRADE_TIMELOCK: u64 = 86_400;

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

/// Maximum permissible upgrade timelock — 30 days. Bounds admin error: an
/// oversized timelock would push every upgrade eta past the horizon (or
/// overflow the eta addition) and block all upgrade proposals.
pub const MAX_UPGRADE_TIMELOCK_SECS: u64 = 2_592_000;
/// Lifetime of a pending admin proposal — 7 days. `accept_admin` rejects
/// older proposals so a forgotten proposal is not a standing capability
/// held by the proposed key.
pub const ADMIN_PROPOSAL_TTL_SECS: u64 = 604_800;
