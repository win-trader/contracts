use soroban_sdk::{contracttype, Address, BytesN, Symbol};

#[contracttype]
#[derive(Clone, Debug)]
pub struct Position {
    pub id: u64,
    pub owner: Address,
    pub market: Symbol,
    pub is_long: bool,
    pub size: i128,
    pub base_exposure: i128,
    pub stored_collateral: i128,
    pub risk_units: i128,
    pub borrow_index_snapshot: i128,
    pub stored_minimum_borrow_fee: i128,
    pub receiver_payer_index_snapshot: i128,
    pub lp_payer_index_snapshot: i128,
    pub receiver_index_snapshot: i128,
    pub opened_at: u64,
    pub pending_mutation_action_id: Option<u64>,
    pub take_profit: Trigger,
    pub stop_loss: Trigger,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Trigger {
    None,
    Attached(TriggerInstruction),
}

impl Trigger {
    pub fn instruction(&self) -> Option<&TriggerInstruction> {
        match self {
            Trigger::Attached(i) => Some(i),
            Trigger::None => None,
        }
    }

    pub fn is_attached(&self) -> bool {
        matches!(self, Trigger::Attached(_))
    }

    pub fn price(&self) -> i128 {
        match self {
            Trigger::Attached(i) => i.trigger_price,
            Trigger::None => 0,
        }
    }
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TriggerInstruction {
    pub trigger_price: i128,
    pub acceptable_price: i128,
    pub committed_at: u64,
    pub execute_after: u64,
    pub commit_observed_at: u64,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActionKind {
    MarketOpen,
    LimitOpen,
    Decrease,
    Close,
}

#[contracttype]
#[derive(Clone, Debug)]
pub enum ActionPayload {
    MarketOpen(OpenPayload),
    LimitOpen(OpenPayload, TriggerCondition),
    Decrease(DecreasePayload),
    Close(ClosePayload),
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct OpenPayload {
    pub is_long: bool,
    pub size: i128,
    pub submitted_collateral: i128,
    pub acceptable_price: i128,
    pub expires_at: u64,
    pub take_profit: i128,
    pub stop_loss: i128,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TriggerCondition {
    pub trigger_price: i128,
    pub trigger_above: bool,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct DecreasePayload {
    pub position_id: u64,
    pub size_removed: i128,
    pub acceptable_price: i128,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct ClosePayload {
    pub position_id: u64,
    pub acceptable_price: i128,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct PendingAction {
    pub action_id: u64,
    pub owner: Address,
    pub market_id: Symbol,
    pub kind: ActionKind,
    pub created_at: u64,
    pub execute_after: u64,
    pub commit_observed_at: u64,
    pub escrowed_collateral: i128,
    pub payload: ActionPayload,
}

impl ActionPayload {
    pub fn open(&self) -> Option<&OpenPayload> {
        match self {
            ActionPayload::MarketOpen(open) | ActionPayload::LimitOpen(open, _) => Some(open),
            _ => None,
        }
    }

    pub fn position_id(&self) -> Option<u64> {
        match self {
            ActionPayload::Decrease(p) => Some(p.position_id),
            ActionPayload::Close(p) => Some(p.position_id),
            _ => None,
        }
    }

    pub fn acceptable_price(&self) -> i128 {
        match self {
            ActionPayload::MarketOpen(o) | ActionPayload::LimitOpen(o, _) => o.acceptable_price,
            ActionPayload::Decrease(p) => p.acceptable_price,
            ActionPayload::Close(p) => p.acceptable_price,
        }
    }
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActionOutcome {
    Executed,
    Failed,
    Cancelled,
    Expired,
    Superseded,
    NotReady,
    Pending,
    RequiresLiquidation,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureReason {
    PriceBoundExceeded,
    CapacityExceeded,
    ExposureCapExceeded,
    SideRestricted,
    InsufficientCollateral,
    UnpayableProfit,
    PositionGone,
    MarketPaused,
    SizeTooSmall,
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
    pub hard_cap_payout_factor: i128,
    pub hard_cap_reference_pnl: i128,
}

impl MarketSide {
    pub fn new() -> Self {
        MarketSide {
            size_open_interest: 0,
            base_exposure: 0,
            stored_collateral_total: 0,
            risk_units: 0,
            risk_state: RiskState::Normal,
            hard_cap_payout_factor: crate::constants::INDEX_PRECISION,
            hard_cap_reference_pnl: 0,
        }
    }
}

impl Default for MarketSide {
    fn default() -> Self {
        Self::new()
    }
}

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
    pub open_fee_bps: u32,
    pub close_size_fee_bps: u32,
    pub close_pnl_fee_bps: u32,
    pub max_funding_rate_bps_day: i128,
    pub instant_weight_bps: u32,
    pub market_risk_factor_bps: u32,
    pub initial_margin_bps: u32,
    pub maintenance_margin_bps: u32,
    pub recovery_pnl_factor_bps: u32,
    pub warning_pnl_factor_bps: u32,
    pub adl_pnl_factor_bps: u32,
    pub hard_cap_pnl_factor_bps: u32,
    pub max_long_size_open_interest: i128,
    pub max_short_size_open_interest: i128,
    pub max_long_base_exposure: i128,
    pub max_short_base_exposure: i128,
    pub order_execution_delay_seconds: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeeperRewards {
    pub open: i128,
    pub limit_order: i128,
    pub decrease: i128,
    pub close: i128,
    pub tp: i128,
    pub sl: i128,
    pub expiry: i128,
    pub liquidation: i128,
    pub adl: i128,
    pub lp_resolve: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GlobalConfig {
    pub min_collateral: i128,
    pub min_position_lifetime: u64,
    pub max_order_lifetime_seconds: u64,
    pub max_market_order_lifetime: u64,
    pub min_borrow_fee_seconds: u64,
    pub funding_half_life_seconds: u64,
    pub max_price_age_seconds: u64,
    pub risk_capacity_limit_bps: u32,
    pub base_borrow_rate_bps_day: i128,
    pub max_variable_borrow_bps_day: i128,
    pub fee_lp_revenue_share_bps: u32,
    pub borrow_lp_revenue_share_bps: u32,
    pub config_timelock_seconds: u64,
    pub max_active_markets: u32,
    pub global_hard_cap_limit_bps: u32,
    pub hard_cap_relatch_band_bps: u32,
    pub keeper_rewards: KeeperRewards,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LpConfig {
    pub max_withdraw_utilization_bps: u32,
    pub min_deposit_nav_factor_bps: u32,
    pub lp_request_delay_seconds: u64,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct Market {
    pub long: MarketSide,
    pub short: MarketSide,
    pub receiver_backed_index_long: i128,
    pub receiver_backed_index_short: i128,
    pub lp_backed_index_long: i128,
    pub lp_backed_index_short: i128,
    pub receiver_index_long: i128,
    pub receiver_index_short: i128,
    pub current_payer_side: PayerSide,
    pub current_payer_rate: i128,
    pub skew_ema: i128,
    pub last_funding_checkpoint: u64,
    pub long_payer_remainders: RemainderGroup,
    pub short_payer_remainders: RemainderGroup,
    pub pending_receiver_funding: i128,
    pub config: MarketConfig,
}

#[contracttype]
#[derive(Clone, Debug, Default)]
pub struct RemainderGroup {
    pub receiver_payer_remainder: i128,
    pub lp_payer_remainder: i128,
    pub receiver_liability_remainder: i128,
    pub distribution_remainder: i128,
}

#[derive(Clone, Copy, Debug)]
pub struct FundingIndices {
    pub receiver_backed_payer: i128,
    pub lp_backed_payer: i128,
    pub receiver: i128,
}

impl Market {
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
            skew_ema: 0,
            last_funding_checkpoint: now,
            long_payer_remainders: RemainderGroup::default(),
            short_payer_remainders: RemainderGroup::default(),
            pending_receiver_funding: 0,
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

    pub fn payer_remainders(&self, long_pays: bool) -> &RemainderGroup {
        if long_pays {
            &self.long_payer_remainders
        } else {
            &self.short_payer_remainders
        }
    }

    pub fn payer_remainders_mut(&mut self, long_pays: bool) -> &mut RemainderGroup {
        if long_pays {
            &mut self.long_payer_remainders
        } else {
            &mut self.short_payer_remainders
        }
    }

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
    pub restricted_side_count: u32,
    pub deleveraging_side_count: u32,
    pub min_equity_clear_of_adl: i128,
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
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct LpRequest {
    pub id: u64,
    pub owner: Address,
    pub kind: LpRequestKind,
    pub amount: i128,
    /// The resolve reward fixed when the request was made.
    pub reward: i128,
    pub request_time: u64,
    pub execute_after: u64,
    pub status: LpRequestStatus,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettlementStatus {
    Settled,
    Failed,
    NotReady,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct SettlementResult {
    pub status: SettlementStatus,
    pub amount: i128,
    pub reward: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingFeesView {
    pub funding_paid_to_receivers: i128,
    pub funding_paid_to_lps: i128,
    pub funding_received: i128,
    pub borrow: i128,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct PendingGlobalConfig {
    pub config: GlobalConfig,
    pub effective_at: u64,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct PendingPriceFeed {
    pub price_feed: Address,
    pub effective_at: u64,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct PendingMarketConfig {
    pub config: MarketConfig,
    pub effective_at: u64,
}

#[contracttype]
pub struct MigrationData {
    pub version: u32,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct PendingUpgrade {
    pub wasm_hash: BytesN<32>,
    pub eta: u64,
}
