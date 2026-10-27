use shared::constants::{PRICE_DECIMALS, ROLE_ADMIN, ROLE_ORACLE};
use shared::{Asset, PriceData};
use soroban_sdk::{contract, contractimpl, panic_with_error, Address, Env, Symbol, Vec};

use crate::errors::CexFeedError;
use crate::events;
use crate::logic::{self, MAX_BATCH, MAX_PRICE};
use crate::storage::{self, FeedConfig, PendingSources, SourcePrice};

#[contract]
pub struct CexFeedContract;

fn has_role(env: &Env, caller: &Address, role: &str) -> bool {
    shared::has_role(env, &storage::config_manager(env), role, caller)
}

fn require_oracle_or_admin(env: &Env, caller: &Address) {
    caller.require_auth();
    if !has_role(env, caller, ROLE_ORACLE) && !has_role(env, caller, ROLE_ADMIN) {
        panic_with_error!(env, CexFeedError::Unauthorized);
    }
}

fn require_valid_sources(env: &Env, publishers: &Vec<Address>, config: &FeedConfig) {
    if !logic::sources_are_valid(publishers, config) {
        panic_with_error!(env, CexFeedError::InvalidConfig);
    }
}

#[contractimpl]
impl CexFeedContract {
    pub fn __constructor(
        env: Env,
        config_manager: Address,
        timelock_seconds: u64,
        publishers: Vec<Address>,
        config: FeedConfig,
    ) {
        if !logic::timelock_is_valid(timelock_seconds) {
            panic_with_error!(&env, CexFeedError::InvalidConfig);
        }
        require_valid_sources(&env, &publishers, &config);
        storage::init(
            &env,
            &config_manager,
            timelock_seconds,
            &publishers,
            &config,
        );
        shared::bump_instance_ttl(&env);
    }

    /// Record `publisher`'s prices, observed on its exchange at `observed_at`.
    /// The stamp is `min(observed_at, ledger time)`: a price that lands late
    /// keeps its older observation time, so it can never pass as observed
    /// after a commit it predates. One call carries every symbol.
    pub fn set_prices(env: Env, publisher: Address, observed_at: u64, prices: Vec<SourcePrice>) {
        publisher.require_auth();
        if !storage::publishers(&env).contains(&publisher) {
            panic_with_error!(&env, CexFeedError::NotPublisher);
        }
        if prices.is_empty() || prices.len() > MAX_BATCH {
            panic_with_error!(&env, CexFeedError::InvalidPrice);
        }
        let timestamp = core::cmp::min(observed_at, env.ledger().timestamp());
        for entry in prices.iter() {
            if entry.price <= 0 || entry.price > MAX_PRICE {
                panic_with_error!(&env, CexFeedError::InvalidPrice);
            }
            let data = PriceData {
                price: entry.price,
                timestamp,
            };
            storage::save_source_price(&env, &publisher, &entry.symbol, &data);
        }
        shared::bump_instance_ttl(&env);
    }

    /// SEP-40: the median of the fresh sources for `Other(symbol)`, stamped
    /// with the oldest of them. `None` below `min_sources` fresh sources or
    /// past `max_deviation_bps`.
    pub fn lastprice(env: Env, asset: Asset) -> Option<PriceData> {
        match asset {
            Asset::Other(symbol) => logic::aggregate(&env, &symbol),
            Asset::Stellar(_) => None,
        }
    }

    /// SEP-40: prices carry `PRICE_DECIMALS` places.
    pub fn decimals(_env: Env) -> u32 {
        PRICE_DECIMALS
    }

    /// One publisher's last price for `symbol`, fresh or not.
    pub fn source_price(env: Env, publisher: Address, symbol: Symbol) -> Option<PriceData> {
        storage::source_price(&env, &publisher, &symbol)
    }

    pub fn publishers(env: Env) -> Vec<Address> {
        storage::publishers(&env)
    }

    pub fn config(env: Env) -> FeedConfig {
        storage::config(&env)
    }

    pub fn timelock_seconds(env: Env) -> u64 {
        storage::timelock_seconds(&env)
    }

    pub fn pending_sources(env: Env) -> Option<PendingSources> {
        storage::pending_sources(&env)
    }

    /// Replace the publisher set and config after `timelock_seconds`; a new
    /// proposal overwrites the pending one.
    pub fn propose_sources(
        env: Env,
        caller: Address,
        publishers: Vec<Address>,
        config: FeedConfig,
    ) {
        caller.require_auth();
        if !has_role(&env, &caller, ROLE_ORACLE) {
            panic_with_error!(&env, CexFeedError::Unauthorized);
        }
        require_valid_sources(&env, &publishers, &config);
        let effective_at = env
            .ledger()
            .timestamp()
            .saturating_add(storage::timelock_seconds(&env));
        storage::save_pending_sources(
            &env,
            &PendingSources {
                publishers: publishers.clone(),
                config: config.clone(),
                effective_at,
            },
        );
        events::emit_sources_proposed(&env, &caller, &publishers, &config, effective_at);
        shared::bump_instance_ttl(&env);
    }

    /// Permissionless once due; lapses one timelock after that.
    pub fn apply_sources(env: Env, caller: Address) {
        caller.require_auth();
        let pending = storage::pending_sources(&env)
            .unwrap_or_else(|| panic_with_error!(&env, CexFeedError::NoPendingSources));
        let now = env.ledger().timestamp();
        if now < pending.effective_at {
            panic_with_error!(&env, CexFeedError::SourcesTimelockNotElapsed);
        }
        if now
            > pending
                .effective_at
                .saturating_add(storage::timelock_seconds(&env))
        {
            panic_with_error!(&env, CexFeedError::SourcesProposalExpired);
        }
        storage::clear_pending_sources(&env);
        storage::save_sources(&env, &pending.publishers, &pending.config);
        events::emit_sources_changed(&env, &caller, &pending.publishers, &pending.config);
        shared::bump_instance_ttl(&env);
    }

    // ADMIN can cancel too, so revoking a compromised ORACLE key is enough to
    // stop its proposal.
    pub fn cancel_sources(env: Env, caller: Address) {
        require_oracle_or_admin(&env, &caller);
        if storage::pending_sources(&env).is_none() {
            panic_with_error!(&env, CexFeedError::NoPendingSources);
        }
        storage::clear_pending_sources(&env);
        events::emit_sources_proposal_cancelled(&env, &caller);
        shared::bump_instance_ttl(&env);
    }

    /// Drop a compromised publisher now. Never adds a publisher; with fewer
    /// publishers than `min_sources`, `lastprice` returns `None` and the
    /// protocol stops settling until a new set is applied. A pending proposal
    /// that includes the removed publisher is cancelled with it.
    pub fn remove_publisher(env: Env, caller: Address, publisher: Address) {
        require_oracle_or_admin(&env, &caller);
        let mut publishers = storage::publishers(&env);
        let Some(index) = publishers.first_index_of(&publisher) else {
            panic_with_error!(&env, CexFeedError::NotPublisher);
        };
        publishers.remove(index);
        storage::save_publishers(&env, &publishers);
        events::emit_sources_changed(&env, &caller, &publishers, &storage::config(&env));
        if let Some(pending) = storage::pending_sources(&env) {
            if pending.publishers.contains(&publisher) {
                storage::clear_pending_sources(&env);
                events::emit_sources_proposal_cancelled(&env, &caller);
            }
        }
        shared::bump_instance_ttl(&env);
    }
}
