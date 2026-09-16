use super::EventHeader;
use soroban_sdk::{contractevent, Address, Env, Symbol};

#[contractevent(topics = ["cfgcancel"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProposalCancelled {
    pub header: EventHeader,
    pub market: Option<Symbol>,
    pub price_feed: bool,
}

pub fn emit_proposal_cancelled(env: &Env, actor: &Address, market: Option<Symbol>, price_feed: bool) {
    ProposalCancelled {
        header: super::vault_header(env, actor),
        market,
        price_feed,
    }
    .publish(env);
}
