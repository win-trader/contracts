use soroban_sdk::{contractevent, Address, Env};

/// Reward for revealing an insolvent position, paid from the risk-keeper
/// reserve (§12.3).
#[contractevent(topics = ["insreward"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InsolvencyRewardPaid {
    pub position_id: u64,
    pub keeper: Address,
    pub amount: i128,
}

pub fn emit_insolvency_reward(env: &Env, position_id: u64, keeper: &Address, amount: i128) {
    InsolvencyRewardPaid {
        position_id,
        keeper: keeper.clone(),
        amount,
    }
    .publish(env);
}
