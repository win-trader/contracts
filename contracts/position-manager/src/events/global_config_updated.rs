use shared::GlobalConfig;
use soroban_sdk::{contractevent, Env};

#[contractevent(topics = ["cfgglobal"], data_format = "map")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GlobalConfigUpdated {
    pub config: GlobalConfig,
}

pub fn emit_global_config_updated(env: &Env, config: &GlobalConfig) {
    GlobalConfigUpdated {
        config: config.clone(),
    }
    .publish(env);
}
