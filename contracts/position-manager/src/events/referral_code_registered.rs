use super::EventHeader;
use soroban_sdk::{contractevent, Address, Env, Symbol};

/// A referrer claimed ownership of a referral code (owner immutable after).
#[contractevent(topics = ["refreg"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferralCodeRegistered {
    pub header: EventHeader,
    pub owner: Address,
    pub code: Symbol,
}

pub fn emit_referral_code_registered(env: &Env, owner: &Address, code: &Symbol) {
    ReferralCodeRegistered {
        header: super::vault_header(env, owner),
        owner: owner.clone(),
        code: code.clone(),
    }
    .publish(env);
}
