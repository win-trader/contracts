use super::EventHeader;
use shared::RiskState;
use soroban_sdk::{contractevent, Address, Env, Symbol};

#[contractevent(topics = ["riskstate"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiskStateChanged {
    pub header: EventHeader,
    pub is_long: bool,
    pub previous_state: RiskState,
    pub next_state: RiskState,
    pub pnl_factor_bps: i128,
}

pub fn emit_risk_state_changed(
    env: &Env,
    market: &Symbol,
    actor: &Address,
    is_long: bool,
    previous_state: RiskState,
    next_state: RiskState,
    pnl_factor_bps: i128,
) {
    RiskStateChanged {
        header: super::header(env, market, actor),
        is_long,
        previous_state,
        next_state,
        pnl_factor_bps,
    }
    .publish(env);
}
