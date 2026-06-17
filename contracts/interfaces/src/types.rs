use soroban_sdk::{contracttype, BytesN};

/// Global safety thresholds for price validation.
#[contracttype]
#[derive(Clone, Debug)]
pub struct OracleConfig {
    /// Maximum allowed spread between oracle sources in basis points
    /// (e.g., 100 = 1%). Bounded at `shared::constants::MAX_DEVIATION_BPS_CEILING`.
    pub max_deviation_bps: i128,
    /// Maximum age of an external SEP-40 price feed before it is rejected
    /// as stale (in seconds).
    pub staleness_threshold: u64,
    /// How long a cached aggregated price remains valid (in seconds). A
    /// `get_price` call within this window of the last fetch returns the
    /// cached value without re-querying sources. Must be > 0 and
    /// <= `staleness_threshold` (otherwise the cache could outlive a fresh
    /// source price and serve stale data).
    pub cache_duration: u64,
    /// Minimum number of source responses that must agree within
    /// `max_deviation_bps` for OracleRouter to return a price. Floored at
    /// `shared::constants::MIN_REQUIRED_SOURCES_FLOOR`, ceilinged at
    /// `shared::constants::MAX_ORACLE_SOURCES`.
    pub min_required_sources: u32,
}

/// Represents a single trader's open leveraged position.
#[contracttype]
#[derive(Clone, Debug)]
pub struct Position {
    /// USDC collateral deposited by the trader.
    pub collateral: i128,
    /// Notional size of the position in USDC.
    pub size: i128,
    /// Oracle price at the time the position was opened (scaled by 1e7).
    pub entry_price: i128,
    /// Global borrow accumulator index at position open (for lazy fee calc).
    pub entry_borrow_index: i128,
    /// Global funding accumulator index at position open (for lazy fee calc).
    pub entry_funding_index: i128,
    /// True for a long position, false for a short.
    pub is_long: bool,
    /// Block timestamp when the position was last increased (anti-front-running lock).
    pub last_increased_time: u64,
    /// Take-profit price (scaled by 1e7). 0 = not set.
    pub take_profit: i128,
    /// Stop-loss price (scaled by 1e7). 0 = not set.
    pub stop_loss: i128,
    /// Flat USDC fee escrowed when TP or SL is set. Paid to executor on trigger, refunded on user close / ADL, forfeited to revenue on liquidation.
    pub execution_fee_escrow: i128,
}

/// Global market state for a single tradeable asset symbol.
#[contracttype]
#[derive(Clone, Debug)]
pub struct MarketInfo {
    /// Volume-weighted average entry price of all active long positions.
    pub global_long_avg_price: i128,
    /// Volume-weighted average entry price of all active short positions.
    pub global_short_avg_price: i128,
    /// Total notional size of all open long positions.
    pub long_open_interest: i128,
    /// Total notional size of all open short positions.
    pub short_open_interest: i128,
    /// Cumulative borrow fee index (grows monotonically with time).
    pub acc_borrow_index: i128,
    /// Cumulative funding rate index (signed; positive = longs pay shorts).
    pub acc_funding_index: i128,
    /// Timestamp of the last keeper index update.
    pub last_index_update: u64,
}

/// Data required during a WASM migration. Single definition for all contracts.
#[contracttype]
pub struct MigrationData {
    pub version: u32,
}

/// Pending WASM upgrade — set by `propose_upgrade`, consumed by `upgrade`
/// (cleared atomically on a successful install), or cleared by `cancel_upgrade`.
/// Single shape across every protocol contract; all four contracts store it at
/// the shared `pending_upgrade` Symbol key in their own instance storage (see
/// `interfaces::upgrade::pending_upgrade_key`). `upgrade` refuses to install
/// unless `pending.wasm_hash` matches the supplied hash and `now >= eta`.
#[contracttype]
#[derive(Clone, Debug)]
pub struct PendingUpgrade {
    pub wasm_hash: BytesN<32>,
    pub eta: u64,
}
