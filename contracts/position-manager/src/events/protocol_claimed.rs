use soroban_sdk::{contractevent, Address, Env};

#[contractevent(topics = ["protclaim"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtocolClaimed {
    pub recipient: Address,
    pub amount: i128,
}

pub fn emit_protocol_claimed(env: &Env, recipient: &Address, amount: i128) {
    ProtocolClaimed {
        recipient: recipient.clone(),
        amount,
    }
    .publish(env);
}
