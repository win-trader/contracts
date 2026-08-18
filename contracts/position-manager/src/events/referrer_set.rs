use soroban_sdk::{contractevent, Address, Env, Symbol};

/// A trader (re-)pointed themselves at a referrer via a code.
#[contractevent(topics = ["refset"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferrerSet {
    pub trader: Address,
    pub referrer: Address,
    pub code: Symbol,
}

pub fn emit_referrer_set(env: &Env, trader: &Address, referrer: &Address, code: &Symbol) {
    ReferrerSet {
        trader: trader.clone(),
        referrer: referrer.clone(),
        code: code.clone(),
    }
    .publish(env);
}
