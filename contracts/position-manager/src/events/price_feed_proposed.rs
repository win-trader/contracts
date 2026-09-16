use super::EventHeader;
use soroban_sdk::{contractevent, Address, Env};

#[contractevent(topics = ["feedprop"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PriceFeedProposed {
    pub header: EventHeader,
    pub price_feed: Address,
    pub effective_at: u64,
}

pub fn emit_price_feed_proposed(env: &Env, actor: &Address, price_feed: &Address, effective_at: u64) {
    PriceFeedProposed {
        header: super::vault_header(env, actor),
        price_feed: price_feed.clone(),
        effective_at,
    }
    .publish(env);
}
