use super::EventHeader;
use soroban_sdk::{contractevent, Address, Env, Symbol};

#[contractevent(topics = ["refset"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferrerSet {
    pub header: EventHeader,
    pub trader: Address,
    pub referrer: Address,
    pub code: Symbol,
}

pub fn emit_referrer_set(env: &Env, trader: &Address, referrer: &Address, code: &Symbol) {
    ReferrerSet {
        header: super::vault_header(env, trader),
        trader: trader.clone(),
        referrer: referrer.clone(),
        code: code.clone(),
    }
    .publish(env);
}
