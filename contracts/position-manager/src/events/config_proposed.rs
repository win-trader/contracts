use super::EventHeader;
use soroban_sdk::{contractevent, Address, Env, Symbol};

#[contractevent(topics = ["cfgprop"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigurationProposed {
    pub header: EventHeader,
    pub market: Option<Symbol>,
    pub effective_at: u64,
}

pub fn emit_config_proposed(
    env: &Env,
    actor: &Address,
    market: Option<Symbol>,
    effective_at: u64,
) {
    ConfigurationProposed {
        header: super::vault_header(env, actor),
        market,
        effective_at,
    }
    .publish(env);
}
