use super::EventHeader;
use soroban_sdk::{contractevent, Address, Env};

#[contractevent(topics = ["protclaim"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtocolClaimed {
    pub header: EventHeader,
    pub recipient: Address,
    pub amount: i128,
}

pub fn emit_protocol_claimed(env: &Env, actor: &Address, recipient: &Address, amount: i128) {
    ProtocolClaimed {
        header: super::vault_header(env, actor),
        recipient: recipient.clone(),
        amount,
    }
    .publish(env);
}
