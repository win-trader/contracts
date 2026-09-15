use super::EventHeader;
use soroban_sdk::{contractevent, Address, Env};

/// The protocol was pointed at a different external price feed (§12.3).
///
/// Worth its own event: every price the protocol acts on comes from this
/// address, so a change to it is the single highest-consequence wiring
/// change governance can make.
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
