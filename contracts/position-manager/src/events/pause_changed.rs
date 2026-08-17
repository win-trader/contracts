use soroban_sdk::{contractevent, Env};

#[contractevent(topics = ["pause"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PauseChanged {
    pub paused: bool,
}

pub fn emit_pause_changed(env: &Env, paused: bool) {
    PauseChanged { paused }.publish(env);
}
