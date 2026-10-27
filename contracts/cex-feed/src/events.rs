use shared::events::{vault_header, EventHeader};
use soroban_sdk::{contractevent, Address, Env, Vec};

use crate::storage::FeedConfig;

#[contractevent(topics = ["srcprop"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourcesProposed {
    pub header: EventHeader,
    pub publishers: Vec<Address>,
    pub config: FeedConfig,
    pub effective_at: u64,
}

#[contractevent(topics = ["srcset"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourcesChanged {
    pub header: EventHeader,
    pub publishers: Vec<Address>,
    pub config: FeedConfig,
}

#[contractevent(topics = ["srccancel"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourcesProposalCancelled {
    pub header: EventHeader,
}

pub fn emit_sources_proposed(
    env: &Env,
    actor: &Address,
    publishers: &Vec<Address>,
    config: &FeedConfig,
    effective_at: u64,
) {
    SourcesProposed {
        header: vault_header(env, actor),
        publishers: publishers.clone(),
        config: config.clone(),
        effective_at,
    }
    .publish(env);
}

pub fn emit_sources_changed(
    env: &Env,
    actor: &Address,
    publishers: &Vec<Address>,
    config: &FeedConfig,
) {
    SourcesChanged {
        header: vault_header(env, actor),
        publishers: publishers.clone(),
        config: config.clone(),
    }
    .publish(env);
}

pub fn emit_sources_proposal_cancelled(env: &Env, actor: &Address) {
    SourcesProposalCancelled {
        header: vault_header(env, actor),
    }
    .publish(env);
}
