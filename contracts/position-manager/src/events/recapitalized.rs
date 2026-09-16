use super::EventHeader;
use soroban_sdk::{contractevent, Address, Env};

#[contractevent(topics = ["recap"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Recapitalized {
    pub header: EventHeader,
    pub contributor: Address,
    pub amount: i128,
}

pub fn emit_recapitalized(env: &Env, contributor: &Address, amount: i128) {
    Recapitalized {
        header: super::vault_header(env, contributor),
        contributor: contributor.clone(),
        amount,
    }
    .publish(env);
}
