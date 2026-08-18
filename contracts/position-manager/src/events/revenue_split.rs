use super::FeeSource;
use soroban_sdk::{contractevent, Env};

/// A collected fee split into its revenue shares. `lp_share` stays in the
/// vault as LP cash; the others accrue to their claim totals. `referral` is
/// carved from the protocol slice (only nonzero on a referred closing fee);
/// `keeper + lp + protocol + referral == collected`.
#[contractevent(topics = ["revsplit"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RevenueSplit {
    pub position_id: u64,
    pub source: FeeSource,
    pub collected: i128,
    pub keeper_share: i128,
    pub lp_share: i128,
    pub protocol_share: i128,
    pub referral_share: i128,
}

#[allow(clippy::too_many_arguments)]
pub fn emit_revenue_split(
    env: &Env,
    position_id: u64,
    source: FeeSource,
    collected: i128,
    keeper_share: i128,
    lp_share: i128,
    protocol_share: i128,
    referral_share: i128,
) {
    RevenueSplit {
        position_id,
        source,
        collected,
        keeper_share,
        lp_share,
        protocol_share,
        referral_share,
    }
    .publish(env);
}
