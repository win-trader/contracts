use super::EventHeader;
use shared::GlobalConfig;
use soroban_sdk::{contractevent, Address, Env};

#[contractevent(topics = ["cfgglobal"], data_format = "map")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GlobalConfigUpdated {
    pub header: EventHeader,
    pub config: GlobalConfig,
}

pub fn emit_global_config_updated(env: &Env, actor: &Address, config: &GlobalConfig) {
    GlobalConfigUpdated {
        header: super::vault_header(env, actor),
        config: config.clone(),
    }
    .publish(env);
}
