use soroban_sdk::{contracttype, Address, BytesN, Symbol, Vec};

/// Global safety thresholds for price validation.
#[contracttype]
#[derive(Clone, Debug)]
pub struct OracleConfig {
    /// Maximum allowed spread between oracle sources in basis points
    /// (e.g., 100 = 1%). Bounded at `crate::constants::MAX_DEVIATION_BPS_CEILING`.
    pub max_deviation_bps: i128,
    /// Maximum age of an external SEP-40 price feed before it is rejected
    /// as stale (in seconds).
    pub staleness_threshold: u64,
    /// How long a cached aggregated price remains valid after the router
    /// fetch (in seconds). A cache hit also requires every source timestamp
    /// used for the cached median to remain within `staleness_threshold`.
    /// Must be > 0 and <= `staleness_threshold`.
    pub cache_duration: u64,
    /// Minimum number of source responses that must agree within
    /// `max_deviation_bps` for OracleRouter to return a price. Floored at
    /// `crate::constants::MIN_REQUIRED_SOURCES_FLOOR`, ceilinged at
    /// `crate::constants::MAX_ORACLE_SOURCES`.
    pub min_required_sources: u32,
}

/// Represents a single trader's open leveraged position.
#[contracttype]
#[derive(Clone, Debug)]
pub struct Position {
    pub id: u64,
    pub owner: Address,
    pub market: Symbol,
    pub is_long: bool,
    /// USD notional at `PRICE_PRECISION`.
    pub size: i128,
    /// Asset units at `PRICE_PRECISION`.
    pub base_exposure: i128,
    /// Trader-owned collateral recorded in contract state (the doc's
    /// "stored collateral"). Effective collateral — stored collateral after
    /// pending fees and funding credits — is always derived, never stored.
    pub stored_collateral: i128,
    /// Fixed gross capacity assigned when risk opens.
    pub risk_units: i128,
    pub borrow_debt: i128,
    pub funding_paid_to_receivers_debt: i128,
    pub funding_paid_to_lps_debt: i128,
    pub funding_received_debt: i128,
    /// Cash owned by an optional-order executor.
    pub execution_budget: i128,
    pub last_increased_time: u64,
    /// Trigger price for the optional take-profit order; `0` = none.
    pub take_profit: i128,
    /// Trigger price for the optional stop-loss order; `0` = none.
    pub stop_loss: i128,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiskState {
    Normal,
    Warning,
    Adl,
    HardCap,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct MarketSide {
    pub size_open_interest: i128,
    pub base_exposure: i128,
    pub stored_collateral_total: i128,
    pub risk_units: i128,
    pub risk_state: RiskState,
}

impl MarketSide {
    pub fn new() -> Self {
        MarketSide {
            size_open_interest: 0,
            base_exposure: 0,
            stored_collateral_total: 0,
            risk_units: 0,
            risk_state: RiskState::Normal,
        }
    }
}

impl Default for MarketSide {
    fn default() -> Self {
        Self::new()
    }
}

/// Which side currently pays funding (§8.1: the side with more base
/// exposure). `None` when the market is balanced or empty.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PayerSide {
    None,
    Long,
    Short,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarketConfig {
    pub open_fee_low_bps: u32,
    pub open_fee_high_bps: u32,
    pub max_funding_rate_bps_day: i128,
    pub market_risk_factor_bps: u32,
    pub max_long_size_open_interest: i128,
    pub max_short_size_open_interest: i128,
    pub max_long_base_exposure: i128,
    pub max_short_base_exposure: i128,
    pub recovery_pnl_factor_bps: u32,
    pub warning_pnl_factor_bps: u32,
    pub adl_pnl_factor_bps: u32,
    pub hard_cap_pnl_factor_bps: u32,
    pub maintenance_margin_bps: u32,
    pub liquidation_reward_bps: u32,
    pub adl_reward_bps: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GlobalConfig {
    pub min_collateral: i128,
    pub min_position_lifetime: u64,
    pub risk_capacity_limit_bps: u32,
    pub base_borrow_rate_bps_day: i128,
    pub max_variable_borrow_bps_day: i128,
    pub lp_revenue_share_bps: u32,
    pub risk_keeper_revenue_share_bps: u32,
    pub hard_cap_factor_limit_bps: u32,
    pub max_adl_reward: i128,
    pub max_insolvent_touch_reward: i128,
    pub max_active_markets: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LpConfig {
    pub max_withdraw_utilization_bps: u32,
    pub min_deposit_nav_factor_bps: u32,
    pub lp_request_delay: u64,
}

/// Authoritative per-market state: the side aggregates, the funding indices,
/// the current funding flows, and the market configuration.
///
/// Soroban limits UDT field names to 30 characters, so where a doc glossary
/// term is longer the field drops the redundant qualifier and its doc
/// comment carries the full term (e.g. `receiver_backed_index_long` is the
/// doc's `receiver_backed_payer_index` for the long side).
#[contracttype]
#[derive(Clone, Debug)]
pub struct Market {
    pub long: MarketSide,
    pub short: MarketSide,
    /// Cumulative payer fee per unit of dominant-side size whose collection
    /// restores cash backing an already-accrued receiver claim (§8.2).
    pub receiver_backed_index_long: i128,
    pub receiver_backed_index_short: i128,
    /// Cumulative payer fee per unit of dominant-side size that is LP
    /// revenue on collection (§8.2).
    pub lp_backed_index_long: i128,
    pub lp_backed_index_short: i128,
    /// Cumulative funding credit per unit of light-side size (§8.2).
    pub receiver_index_long: i128,
    pub receiver_index_short: i128,
    pub current_payer_side: PayerSide,
    /// `INDEX_PRECISION`-scaled bps/day payer rate for the current interval.
    pub current_payer_rate: i128,
    /// Receiver-allocated share of the payer flow, cash/second at
    /// `INDEX_PRECISION` (§8.1).
    pub receiver_flow_per_second: i128,
    /// LP-allocated remainder of the payer flow, cash/second at
    /// `INDEX_PRECISION` (§8.1).
    pub lp_flow_per_second: i128,
    pub last_funding_checkpoint: u64,
    pub receiver_payer_remainder: i128,
    pub lp_payer_remainder: i128,
    pub receiver_index_remainder: i128,
    pub receiver_flow_remainder: i128,
    pub config: MarketConfig,
}

/// The three funding indices that apply to one position direction (§8.2).
#[derive(Clone, Copy, Debug)]
pub struct FundingIndices {
    pub receiver_backed_payer: i128,
    pub lp_backed_payer: i128,
    pub receiver: i128,
}

impl Market {
    /// A market with empty sides, zeroed indices, and its funding clock
    /// started at `now`.
    pub fn new(config: MarketConfig, now: u64) -> Self {
        Market {
            long: MarketSide::new(),
            short: MarketSide::new(),
            receiver_backed_index_long: 0,
            receiver_backed_index_short: 0,
            lp_backed_index_long: 0,
            lp_backed_index_short: 0,
            receiver_index_long: 0,
            receiver_index_short: 0,
            current_payer_side: PayerSide::None,
            current_payer_rate: 0,
            receiver_flow_per_second: 0,
            lp_flow_per_second: 0,
            last_funding_checkpoint: now,
            receiver_payer_remainder: 0,
            lp_payer_remainder: 0,
            receiver_index_remainder: 0,
            receiver_flow_remainder: 0,
            config,
        }
    }

    pub fn side(&self, is_long: bool) -> &MarketSide {
        if is_long {
            &self.long
        } else {
            &self.short
        }
    }

    pub fn side_mut(&mut self, is_long: bool) -> &mut MarketSide {
        if is_long {
            &mut self.long
        } else {
            &mut self.short
        }
    }

    /// The indices that apply to a position on the given direction.
    pub fn funding_indices(&self, is_long: bool) -> FundingIndices {
        if is_long {
            FundingIndices {
                receiver_backed_payer: self.receiver_backed_index_long,
                lp_backed_payer: self.lp_backed_index_long,
                receiver: self.receiver_index_long,
            }
        } else {
            FundingIndices {
                receiver_backed_payer: self.receiver_backed_index_short,
                lp_backed_payer: self.lp_backed_index_short,
                receiver: self.receiver_index_short,
            }
        }
    }
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct AccountingSnapshot {
    pub physical_cash: i128,
    pub non_lp_claims: i128,
    pub cash_lp_equity: i128,
    pub cash_shortfall: i128,
    pub required_risk_backing: i128,
    pub free_lp_capital: i128,
    pub vault_nav: i128,
    pub total_risk_units: i128,
    pub open_position_count: u64,
    pub lp_blocked_side_count: u32,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct RoundPrice {
    pub symbol: Symbol,
    pub price: i128,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct OracleRound {
    pub id: u64,
    pub timestamp: u64,
    pub previous_id: u64,
    pub previous_timestamp: u64,
    pub prices: Vec<RoundPrice>,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LpRequestKind {
    Deposit,
    Withdrawal,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LpRequestStatus {
    Pending,
    Settled,
    Failed,
    Expired,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct LpRequest {
    pub id: u64,
    pub owner: Address,
    pub kind: LpRequestKind,
    pub amount: i128,
    pub request_time: u64,
    pub execute_after: u64,
    pub status: LpRequestStatus,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettlementStatus {
    Settled,
    Failed,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct SettlementResult {
    pub status: SettlementStatus,
    /// Shares minted for a deposit or assets paid for a withdrawal.
    pub amount: i128,
}

/// Data required during a WASM migration. Single definition for all contracts.
#[contracttype]
pub struct MigrationData {
    pub version: u32,
}

/// Pending WASM upgrade — set by `propose_upgrade`, consumed by `upgrade`
/// (cleared atomically on a successful install), or cleared by `cancel_upgrade`.
/// Single shape across every protocol contract. Contracts store it at
/// the shared `pending_upgrade` Symbol key in their own instance storage (see
/// `crate::upgrade::pending_upgrade_key`). `upgrade` refuses to install
/// unless `pending.wasm_hash` matches the supplied hash and `now >= eta`.
#[contracttype]
#[derive(Clone, Debug)]
pub struct PendingUpgrade {
    pub wasm_hash: BytesN<32>,
    pub eta: u64,
}
