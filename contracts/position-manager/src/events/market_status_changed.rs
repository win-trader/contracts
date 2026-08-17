use soroban_sdk::{contractevent, Env, Symbol};

#[contractevent(topics = ["mktstatus"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarketStatusChanged {
    pub market: Symbol,
    pub disabled: bool,
}

pub fn emit_market_status_changed(env: &Env, market: &Symbol, disabled: bool) {
    MarketStatusChanged {
        market: market.clone(),
        disabled,
    }
    .publish(env);
}
