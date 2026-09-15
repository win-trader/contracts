use super::EventHeader;
use soroban_sdk::{contractevent, Address, Env};

/// A referral reward was accrued to a referrer out of a position's closing
/// fee (attribution for the indexer; the money move is in `RevenueSplit`).
#[contractevent(topics = ["refaccr"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferralAccrued {
    pub header: EventHeader,
    pub referrer: Address,
    pub position_id: u64,
    pub amount: i128,
}

pub fn emit_referral_accrued(env: &Env, referrer: &Address, position_id: u64, amount: i128) {
    ReferralAccrued {
        header: super::vault_header(env, referrer),
        referrer: referrer.clone(),
        position_id,
        amount,
    }
    .publish(env);
}
