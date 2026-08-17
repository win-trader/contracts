use soroban_sdk::{contractevent, Address, Env};

/// Cash added during a shortfall without minting shares (§15.2).
#[contractevent(topics = ["recap"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Recapitalized {
    pub contributor: Address,
    pub amount: i128,
}

pub fn emit_recapitalized(env: &Env, contributor: &Address, amount: i128) {
    Recapitalized {
        contributor: contributor.clone(),
        amount,
    }
    .publish(env);
}
