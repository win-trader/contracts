use super::EventHeader;
use soroban_sdk::{contractevent, Address, Env, Symbol};

#[contractevent(topics = ["mktstatus"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarketStatusChanged {
    pub header: EventHeader,
    pub disabled: bool,
}

pub fn emit_market_status_changed(env: &Env, market: &Symbol, actor: &Address, disabled: bool) {
    MarketStatusChanged {
        header: super::header(env, market, actor),
        disabled,
    }
    .publish(env);
}
