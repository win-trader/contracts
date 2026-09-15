use super::EventHeader;
use shared::RiskState;
use soroban_sdk::{contractevent, Address, Env, Symbol};

/// A side entered or left a restricted risk state (§14). Emitted only on
/// actual transitions — the keeper's push signal for ADL/hard-cap duty.
#[contractevent(topics = ["riskstate"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiskStateChanged {
    pub header: EventHeader,
    pub is_long: bool,
    /// §12.6 requires both ends of the transition: a consumer watching for
    /// "a side became deleveraging-eligible" cannot infer it from the new
    /// state alone.
    pub previous_state: RiskState,
    pub next_state: RiskState,
    /// The side's positive PnL as a share of cash LP equity, in bps — the
    /// number the thresholds are compared against.
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
