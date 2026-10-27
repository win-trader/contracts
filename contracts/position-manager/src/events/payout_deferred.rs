use super::EventHeader;
use soroban_sdk::{contractevent, Address, Env};

#[contractevent(topics = ["paydefer"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PayoutDeferred {
    pub header: EventHeader,
    pub owner: Address,
    pub amount: i128,
}

#[contractevent(topics = ["payclaim"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PayoutClaimed {
    pub header: EventHeader,
    pub owner: Address,
    pub amount: i128,
}

pub fn emit_payout_deferred(env: &Env, owner: &Address, amount: i128) {
    PayoutDeferred {
        header: super::vault_header(env, &env.current_contract_address()),
        owner: owner.clone(),
        amount,
    }
    .publish(env);
}

pub fn emit_payout_claimed(env: &Env, owner: &Address, amount: i128) {
    PayoutClaimed {
        header: super::vault_header(env, owner),
        owner: owner.clone(),
        amount,
    }
    .publish(env);
}
