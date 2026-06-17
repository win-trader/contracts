use soroban_sdk::{contractevent, Address, Symbol};

#[contractevent(topics = ["increase"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IncreasePosition {
    #[topic]
    pub trader: Address,
    pub symbol: Symbol,
    pub size_delta: i128,
    pub collateral: i128,
    pub entry_price: i128,
    pub is_long: bool,
    pub tp: i128,
    pub sl: i128,
    pub new_total_size: i128,
    pub new_total_collateral: i128,
    pub entry_borrow_index: i128,
    pub entry_funding_index: i128,
    pub last_increased_time: u64,
}

#[contractevent(topics = ["decrease"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecreasePosition {
    #[topic]
    pub trader: Address,
    pub symbol: Symbol,
    pub size_delta: i128,
    pub pnl: i128,
    pub borrow_fee: i128,
    pub funding_fee: i128,
    pub mark_price: i128,
    pub is_full_close: bool,
    /// Absolute position size after this decrease.
    pub new_total_size: i128,
    /// Absolute position collateral after this decrease.
    pub new_total_collateral: i128,
}

#[contractevent(topics = ["liq"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Liquidate {
    #[topic]
    pub trader: Address,
    pub symbol: Symbol,
    pub size: i128,
    pub collateral: i128,
    pub pnl: i128,
    pub borrow_fee: i128,
    pub funding_fee: i128,
    pub mark_price: i128,
    pub executor: Address,
}

#[contractevent(topics = ["exec_ord"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecuteOrder {
    #[topic]
    pub trader: Address,
    pub symbol: Symbol,
    pub size: i128,
    pub pnl: i128,
    pub mark_price: i128,
    pub is_tp: bool,
    pub executor: Address,
}

#[contractevent(topics = ["adl"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Adl {
    #[topic]
    pub trader: Address,
    pub symbol: Symbol,
    pub size: i128,
    pub pnl: i128,
    pub mark_price: i128,
}

#[contractevent(topics = ["indices"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateIndices {
    #[topic]
    pub symbol: Symbol,
    pub acc_borrow_index: i128,
    pub acc_funding_index: i128,
    pub timestamp: u64,
}

#[contractevent(topics = ["tp_sl"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SetTpSl {
    #[topic]
    pub trader: Address,
    pub symbol: Symbol,
    pub take_profit: i128,
    pub stop_loss: i128,
}

#[contractevent(topics = ["max_lev"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SetMaxLeverage {
    #[topic]
    pub symbol: Symbol,
    pub max_leverage: i128,
}

#[contractevent(topics = ["mkt_pnl"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarketPnlUpdate {
    #[topic]
    pub symbol: Symbol,
    pub unrealized_pnl: i128,
}

#[contractevent(topics = ["pause"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pause {
    pub is_paused: bool,
    pub caller: Address,
}

#[contractevent(topics = ["mkt_dis"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarketDisabled {
    #[topic]
    pub symbol: Symbol,
    pub caller: Address,
}

#[contractevent(topics = ["mkt_ena"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarketEnabled {
    #[topic]
    pub symbol: Symbol,
    pub caller: Address,
}

// Upgrade events live in `interfaces::events` — the
// `TimelockedUpgradeable` trait's default methods emit them, so no
// per-contract definition is needed here.
