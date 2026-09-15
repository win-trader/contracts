use crate::ledger::Ledger;
use shared::{Market, PayerSide};
use super::EventHeader;
use soroban_sdk::{contractevent, Address, Env, Symbol};

/// Funding/borrow indices and current rates after a keeper checkpoint
/// (`update_indices`). The off-chain fee projection and staleness monitors
/// key on this event. Values are exact at `timestamp`; position actions
/// between keeper runs change flows and rates without emitting one, so
/// projections carry keeper-cadence staleness.
#[contractevent(topics = ["mktchk"], data_format = "map")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarketCheckpoint {
    #[topic]
    pub market: Symbol,
    pub header: EventHeader,
    pub receiver_backed_index_long: i128,
    pub receiver_backed_index_short: i128,
    pub lp_backed_index_long: i128,
    pub lp_backed_index_short: i128,
    pub receiver_index_long: i128,
    pub receiver_index_short: i128,
    pub current_payer_side: PayerSide,
    pub current_payer_rate: i128,
    pub skew_ema: i128,
    pub borrow_index: i128,
    pub current_borrow_rate: i128,
    pub timestamp: u64,
}

pub fn emit_market_checkpoint(
    env: &Env,
    market_symbol: &Symbol,
    actor: &Address,
    market: &Market,
    ledger: &Ledger,
    timestamp: u64,
) {
    MarketCheckpoint {
        market: market_symbol.clone(),
        header: super::header(env, market_symbol, actor),
        receiver_backed_index_long: market.receiver_backed_index_long,
        receiver_backed_index_short: market.receiver_backed_index_short,
        lp_backed_index_long: market.lp_backed_index_long,
        lp_backed_index_short: market.lp_backed_index_short,
        receiver_index_long: market.receiver_index_long,
        receiver_index_short: market.receiver_index_short,
        current_payer_side: market.current_payer_side,
        current_payer_rate: market.current_payer_rate,
        skew_ema: market.skew_ema,
        borrow_index: ledger.borrow_index,
        current_borrow_rate: ledger.current_borrow_rate,
        timestamp,
    }
    .publish(env);
}
