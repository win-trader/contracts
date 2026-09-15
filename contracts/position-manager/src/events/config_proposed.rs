use soroban_sdk::{contractevent, Env, Symbol};

/// §12.3 — a configuration change was proposed and is waiting out its
/// timelock. `market` is `None` for a global proposal.
///
/// The proposal is announced without its contents: the values are read back
/// from storage, and an event that restated them would be a second place for
/// them to disagree. What matters here is that a change is pending and when
/// it becomes applicable — the observability the timelock exists to provide.
#[contractevent(topics = ["cfgprop"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigurationProposed {
    pub market: Option<Symbol>,
    pub effective_at: u64,
}

pub fn emit_config_proposed(env: &Env, market: Option<Symbol>, effective_at: u64) {
    ConfigurationProposed {
        market,
        effective_at,
    }
    .publish(env);
}
