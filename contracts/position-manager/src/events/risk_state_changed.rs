use shared::RiskState;
use soroban_sdk::{contractevent, Env, Symbol};

/// A side entered or left a restricted risk state (§14). Emitted only on
/// actual transitions — the keeper's push signal for ADL/hard-cap duty.
#[contractevent(topics = ["riskstate"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiskStateChanged {
    pub market: Symbol,
    pub is_long: bool,
    pub state: RiskState,
}

pub fn emit_risk_state_changed(env: &Env, market: &Symbol, is_long: bool, state: RiskState) {
    RiskStateChanged {
        market: market.clone(),
        is_long,
        state,
    }
    .publish(env);
}
