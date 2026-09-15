use super::EventHeader;
use soroban_sdk::{contractevent, Address, Env};

#[contractevent(topics = ["pause"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PauseChanged {
    pub header: EventHeader,
    pub paused: bool,
}

pub fn emit_pause_changed(env: &Env, actor: &Address, paused: bool) {
    PauseChanged {
        header: super::vault_header(env, actor),
        paused,
    }
    .publish(env);
}
