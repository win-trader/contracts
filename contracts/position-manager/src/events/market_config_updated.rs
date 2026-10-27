use super::EventHeader;
use shared::MarketConfig;
use soroban_sdk::{contractevent, Address, Env, Symbol};

#[contractevent(topics = ["cfgmarket"], data_format = "map")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarketConfigUpdated {
    #[topic]
    pub market: Symbol,
    pub header: EventHeader,
    pub config: MarketConfig,
}

pub fn emit_market_config_updated(
    env: &Env,
    market: &Symbol,
    actor: &Address,
    config: &MarketConfig,
) {
    MarketConfigUpdated {
        market: market.clone(),
        header: super::header(env, market, actor),
        config: config.clone(),
    }
    .publish(env);
}
