use soroban_sdk::{contractevent, Address, Env};

/// The protocol was pointed at a different external price feed (§12.3).
///
/// Worth its own event: every price the protocol acts on comes from this
/// address, so a change to it is the single highest-consequence wiring
/// change governance can make.
#[contractevent(topics = ["feedset"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PriceFeedChanged {
    pub price_feed: Address,
}

pub fn emit_price_feed_changed(env: &Env, price_feed: &Address) {
    PriceFeedChanged {
        price_feed: price_feed.clone(),
    }
    .publish(env);
}
