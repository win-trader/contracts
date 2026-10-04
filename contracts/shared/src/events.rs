use soroban_sdk::{contractevent, contracttype, Address, BytesN, Env, Symbol};

#[contractevent(topics = ["upgprp"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpgradeProposed {
    pub wasm_hash: BytesN<32>,
    pub eta: u64,
}

#[contractevent(topics = ["upgcan"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpgradeCancelled {
    pub caller: Address,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventHeader {
    pub event_version: u32,
    pub ledger_timestamp: u64,
    pub market: Symbol,
    pub actor: Address,
}

pub const EVENT_VERSION: u32 = 1;

pub fn header(env: &Env, market: &Symbol, actor: &Address) -> EventHeader {
    EventHeader {
        event_version: EVENT_VERSION,
        ledger_timestamp: env.ledger().timestamp(),
        market: market.clone(),
        actor: actor.clone(),
    }
}

pub fn vault_header(env: &Env, actor: &Address) -> EventHeader {
    EventHeader {
        event_version: EVENT_VERSION,
        ledger_timestamp: env.ledger().timestamp(),
        market: Symbol::new(env, "vault"),
        actor: actor.clone(),
    }
}
