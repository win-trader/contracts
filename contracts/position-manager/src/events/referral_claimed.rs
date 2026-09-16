use super::EventHeader;
use soroban_sdk::{contractevent, Address, Env};

#[contractevent(topics = ["refclaim"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferralClaimed {
    pub header: EventHeader,
    pub referrer: Address,
    pub amount: i128,
}

pub fn emit_referral_claimed(env: &Env, referrer: &Address, amount: i128) {
    ReferralClaimed {
        header: super::vault_header(env, referrer),
        referrer: referrer.clone(),
        amount,
    }
    .publish(env);
}
