use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum OracleRouterError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    /// Every oracle source returned data older than `staleness_threshold`,
    /// or returned invalid (zero/negative) prices, or a future timestamp.
    StalePrice = 4,
    /// Spread between source prices exceeds `max_deviation_bps`.
    PriceDeviationTooHigh = 5,
    /// No SEP-40 oracle sources are configured for the requested symbol.
    NoPriceSources = 6,
    /// Cross-contract call to an oracle source failed.
    PriceFetchFailed = 7,
    /// Oracle configuration field is invalid (e.g., zero threshold, out-of-range bps).
    InvalidConfig = 8,
    /// Fewer than `min_required_sources` valid prices were returned.
    InsufficientSources = 9,
    /// `set_oracle_sources` called with more than `MAX_ORACLE_SOURCES` entries.
    TooManySources = 10,
    /// Deviation math would overflow on the supplied prices.
    DeviationOverflow = 11,
    /// `upgrade` rejected — no `propose_upgrade` was made before commit.
    NoPendingUpgrade = 12,
    /// `upgrade` rejected — timelock has not elapsed yet.
    UpgradeTimelockNotElapsed = 13,
    /// `upgrade` rejected — `new_wasm_hash` does not match the proposed
    /// `PendingUpgrade.wasm_hash`.
    UpgradeHashMismatch = 14,
    /// A source's `decimals()` differs from `shared::constants::PRICE_DECIMALS`,
    /// or the source could not be queried for its scale.
    InvalidSourceDecimals = 15,
    /// Even-count median averaging would overflow on the supplied prices.
    MedianOverflow = 16,
    PositionManagerAlreadySet = 17,
    PositionManagerNotSet = 18,
    RoundNotFound = 19,
}
