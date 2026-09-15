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

/// 1e7 — price precision (§2.1). Prices, collateral cash amounts, USD
/// notionals, and base exposure use this scale: with seven decimal places,
/// one dollar or one whole unit is `10_000_000`.
///
/// PnL numerators (§2.8) carry one extra `PRICE_PRECISION` factor and are
/// converted to cash exactly once at the final step.
pub const PRICE_PRECISION: i128 = 10_000_000;
/// Protocol-wide price scale, expressed as a decimal exponent:
/// `PRICE_PRECISION == 10^PRICE_DECIMALS`. The external price feed must
/// report this scale; it is checked when the feed is wired, because a feed
/// reporting different decimals would misprice every position silently.
pub const PRICE_DECIMALS: u32 = 7;
/// 1e14 — the scale for borrow and funding rates, cumulative indices, skew
/// fractions, and decay factors (§2.1): everywhere fractional precision
/// beyond basis points is required. bps/day rates are stored multiplied by
/// this so fractional per-second accrual never rounds to zero before the
/// remainder carry.
pub const INDEX_PRECISION: i128 = 100_000_000_000_000;
/// 10_000 — basis-point denominator, one whole as basis points (§2.1).
/// Single source of truth.
pub const BPS: i128 = 10_000;
/// 1e6 — LP shares carry six more decimal places than the collateral asset
/// (§2.1): the share supply per collateral unit at the initial conversion
/// rate. The vault's decimals offset produces the same scale; §7.17's
/// conversion offsets are stated in terms of this constant.
pub const SHARE_SCALE: i128 = 1_000_000;

// ---------------------------------------------------------------------------
// Role constants — mirrored in ConfigManager's role names.
// ---------------------------------------------------------------------------

/// Ultimate authority — typically a multi-sig or DAO. Can manage all roles.
pub const ROLE_ADMIN: &str = "ADMIN";
/// Authorized to push WASM upgrades to protocol contracts.
pub const ROLE_UPGRADER: &str = "UPGRADER";
/// §12.3 `pause_authority` — may **set** `paused`, not clear it. Pausing is
/// a safety action whose worst case is lost volume, so it sits behind a fast
/// key that can act without ceremony.
pub const ROLE_PAUSER: &str = "PAUSER";
/// §12.3 `unpause_authority` — may clear `paused`. Unpausing re-admits risk,
/// so it belongs with the slower authority alongside configuration.
pub const ROLE_UNPAUSER: &str = "UNPAUSER";
// There is deliberately no keeper role. §7.0 defines keeper authorization as
// the caller authenticating the address that will receive the reward, and
// nothing more: ADL is bounded by its state gate and a checkpoint pays no
// reward and moves no value, so neither has anything for an allowlist to
// protect. The last genuinely permissioned keeper operation was round
// publication, and rounds went with the oracle.

/// §12.3 `oracle_authority` — names the contract that supplies
/// authenticated prices. Its own role because pointing the protocol at a
/// different feed is neither a parameter change nor a safety action.
pub const ROLE_ORACLE: &str = "ORACLE";
/// §12.3 `protocol_recipient` — may claim accumulated protocol revenue.
/// Separate from the configuration authority: claiming moves money out,
/// configuring does not.
pub const ROLE_PROTOCOL: &str = "PROTOCOL";
/// Default upgrade timelock: 24h. ConfigManager admin can raise but not lower
/// below `MIN_UPGRADE_TIMELOCK`.
pub const DEFAULT_UPGRADE_TIMELOCK: u64 = 86_400;

// ---------------------------------------------------------------------------
// Oracle and governance limits.
// ---------------------------------------------------------------------------

/// Minimum permissible `upgrade_timelock_seconds` — 24h. The admin cannot
/// shorten the timelock below this floor.
pub const MIN_UPGRADE_TIMELOCK: u64 = 86_400;

/// Maximum permissible upgrade timelock — 30 days. Bounds admin error: an
/// oversized timelock would push every upgrade eta past the horizon (or
/// overflow the eta addition) and block all upgrade proposals.
pub const MAX_UPGRADE_TIMELOCK_SECS: u64 = 2_592_000;
/// Lifetime of a pending admin proposal — 7 days. `accept_admin` rejects
/// older proposals so a forgotten proposal is not a standing capability
/// held by the proposed key.
pub const ADMIN_PROPOSAL_TTL_SECS: u64 = 604_800;
