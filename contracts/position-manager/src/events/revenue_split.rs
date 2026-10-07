use super::{EventHeader, FeeSource};
use soroban_sdk::{contractevent, Address, Env, Symbol};

#[contractevent(topics = ["revsplit"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RevenueSplit {
    pub header: EventHeader,
    pub position_id: u64,
    pub source: FeeSource,
    pub collected: i128,
    pub lp_share: i128,
    pub protocol_share: i128,
}

#[allow(clippy::too_many_arguments)]
pub fn emit_revenue_split(
    env: &Env,
    market: &Symbol,
    actor: &Address,
    position_id: u64,
    source: FeeSource,
    collected: i128,
    lp_share: i128,
    protocol_share: i128,
) {
    RevenueSplit {
        header: super::header(env, market, actor),
        position_id,
        source,
        collected,
        lp_share,
        protocol_share,
    }
    .publish(env);
}
