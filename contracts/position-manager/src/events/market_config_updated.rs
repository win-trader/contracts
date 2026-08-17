use shared::MarketConfig;
use soroban_sdk::{contractevent, Env, Symbol};

#[contractevent(topics = ["cfgmarket"], data_format = "map")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarketConfigUpdated {
    #[topic]
    pub market: Symbol,
    pub config: MarketConfig,
}

pub fn emit_market_config_updated(env: &Env, market: &Symbol, config: &MarketConfig) {
    MarketConfigUpdated {
        market: market.clone(),
        config: config.clone(),
    }
    .publish(env);
}
