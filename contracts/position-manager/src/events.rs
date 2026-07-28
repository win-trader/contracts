//! Position-lifecycle and governance events.
//!
//! Every state-changing entry point emits one event carrying the amounts it
//! moved — the settlement events double as the on-chain audit trail for the
//! cash-transition table (doc §6), and the offchain indexer keys on the
//! topic literals below. Wide settlement events use `data_format = "map"`
//! so fields are self-describing and can grow.

use soroban_sdk::{contractevent, contracttype, Address, Symbol};

use shared::{GlobalConfig, MarketConfig};

/// Why a position left the book.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CloseReason {
    /// The owner decreased to zero.
    Trader,
    /// A keeper or third party liquidated an unhealthy position.
    Liquidation,
    /// Funded auto-deleveraging on an ADL/hard-cap side.
    Deleverage,
    /// A take-profit or stop-loss trigger executed.
    Order,
}

#[contractevent(topics = ["posopen"], data_format = "map")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PositionOpened {
    #[topic]
    pub position_id: u64,
    pub owner: Address,
    pub market: Symbol,
    pub is_long: bool,
    pub size: i128,
    pub base_exposure: i128,
    /// Collateral stored after the opening fee.
    pub stored_collateral: i128,
    pub execution_budget: i128,
    pub price: i128,
    pub opening_fee: i128,
}

#[contractevent(topics = ["posinc"], data_format = "map")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PositionIncreased {
    #[topic]
    pub position_id: u64,
    pub size_added: i128,
    pub base_added: i128,
    pub collateral_added: i128,
    pub price: i128,
    pub opening_fee: i128,
    /// Stored collateral after fees and capitalization.
    pub stored_collateral: i128,
}

/// A partial close (§12.2). Fee fields are the amounts actually collected in
/// this settlement; `funding_received` is the credit capitalized from the
/// guaranteed receiver claim.
#[contractevent(topics = ["posdec"], data_format = "map")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PositionDecreased {
    #[topic]
    pub position_id: u64,
    pub size_removed: i128,
    pub price: i128,
    pub raw_pnl: i128,
    pub payable_pnl: i128,
    pub realized_payout: i128,
    pub collateral_withdrawn: i128,
    pub receiver_funding_paid: i128,
    pub lp_funding_paid: i128,
    pub borrow_paid: i128,
    pub funding_received: i128,
    pub loss_collected: i128,
}

/// A full close via any path — `reason` distinguishes trader close,
/// liquidation, ADL, and triggered orders.
#[contractevent(topics = ["posclose"], data_format = "map")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PositionClosed {
    #[topic]
    pub position_id: u64,
    pub owner: Address,
    pub market: Symbol,
    pub reason: CloseReason,
    pub size: i128,
    pub price: i128,
    pub raw_pnl: i128,
    pub payable_pnl: i128,
    /// Residual collateral paid to the owner after the waterfall.
    pub collateral_payout: i128,
    pub bad_debt: i128,
    pub liquidation_reward: i128,
    pub execution_budget_refunded: i128,
    pub receiver_funding_paid: i128,
    pub lp_funding_paid: i128,
    pub borrow_paid: i128,
    pub funding_received: i128,
}

#[contractevent(topics = ["baddebt"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BadDebt {
    pub position_id: u64,
    pub amount: i128,
}

#[contractevent(topics = ["ordexec"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrderExecuted {
    pub position_id: u64,
    pub executor: Address,
    pub budget_paid: i128,
}

/// ADL reward paid from the risk-keeper reserve (§14).
#[contractevent(topics = ["adlreward"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdlRewardPaid {
    pub position_id: u64,
    pub keeper: Address,
    pub amount: i128,
}

/// Reward for revealing an insolvent position, paid from the risk-keeper
/// reserve (§12.3).
#[contractevent(topics = ["insreward"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InsolvencyRewardPaid {
    pub position_id: u64,
    pub keeper: Address,
    pub amount: i128,
}

#[contractevent(topics = ["budgetin"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionBudgetFunded {
    pub position_id: u64,
    pub amount: i128,
}

#[contractevent(topics = ["budgetout"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionBudgetWithdrawn {
    pub position_id: u64,
    pub amount: i128,
}

#[contractevent(topics = ["protclaim"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtocolClaimed {
    pub recipient: Address,
    pub amount: i128,
}

/// Cash added during a shortfall without minting shares (§15.2).
#[contractevent(topics = ["recap"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Recapitalized {
    pub contributor: Address,
    pub amount: i128,
}

#[contractevent(topics = ["cfgglobal"], data_format = "map")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GlobalConfigUpdated {
    pub config: GlobalConfig,
}

#[contractevent(topics = ["cfgmarket"], data_format = "map")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarketConfigUpdated {
    #[topic]
    pub market: Symbol,
    pub config: MarketConfig,
}

#[contractevent(topics = ["mktstatus"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarketStatusChanged {
    pub market: Symbol,
    pub disabled: bool,
}

#[contractevent(topics = ["pause"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PauseChanged {
    pub paused: bool,
}
