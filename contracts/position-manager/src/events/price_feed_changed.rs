use super::EventHeader;
use soroban_sdk::{contractevent, Address, Env};

#[contractevent(topics = ["feedset"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PriceFeedChanged {
    pub header: EventHeader,
    pub price_feed: Address,
}

pub fn emit_price_feed_changed(env: &Env, actor: &Address, price_feed: &Address) {
    PriceFeedChanged {
        header: super::vault_header(env, actor),
        price_feed: price_feed.clone(),
    }
    .publish(env);
}
