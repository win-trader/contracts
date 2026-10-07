use shared::events::{vault_header, EventHeader};
use soroban_sdk::{contractevent, Address, Env, Symbol};

#[contractevent(topics = ["cfgprop"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigurationProposed {
    pub header: EventHeader,
    pub market: Option<Symbol>,
    pub effective_at: u64,
}

pub fn emit_config_proposed(env: &Env, actor: &Address, market: Option<Symbol>, effective_at: u64) {
    ConfigurationProposed {
        header: vault_header(env, actor),
        market,
        effective_at,
    }
    .publish(env);
}

#[contractevent(topics = ["feedprop"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PriceFeedProposed {
    pub header: EventHeader,
    pub price_feed: Address,
    pub effective_at: u64,
}

pub fn emit_price_feed_proposed(env: &Env, actor: &Address, price_feed: &Address, effective_at: u64) {
    PriceFeedProposed {
        header: vault_header(env, actor),
        price_feed: price_feed.clone(),
        effective_at,
    }
    .publish(env);
}

#[contractevent(topics = ["cfgcancel"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProposalCancelled {
    pub header: EventHeader,
    pub market: Option<Symbol>,
    pub price_feed: bool,
}

pub fn emit_proposal_cancelled(env: &Env, actor: &Address, market: Option<Symbol>, price_feed: bool) {
    ProposalCancelled {
        header: vault_header(env, actor),
        market,
        price_feed,
    }
    .publish(env);
}
