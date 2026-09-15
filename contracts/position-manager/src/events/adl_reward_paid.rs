use soroban_sdk::{contractevent, Address, Env};

/// ADL reward paid from the risk-keeper reserve (§14).
#[contractevent(topics = ["adlreward"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(dead_code)]
pub struct AdlRewardPaid {
    pub position_id: u64,
    pub keeper: Address,
    pub amount: i128,
}

#[allow(dead_code)]
pub fn emit_adl_reward(env: &Env, position_id: u64, keeper: &Address, amount: i128) {
    AdlRewardPaid {
        position_id,
        keeper: keeper.clone(),
        amount,
    }
    .publish(env);
}
