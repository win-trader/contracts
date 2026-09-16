use soroban_sdk::{panic_with_error, Address, Env, Symbol};

use shared::{
    GlobalConfig, MarketConfig, PendingGlobalConfig, PendingMarketConfig, PendingPriceFeed,
};

use crate::auth::require_role;
use crate::errors::PositionManagerError;
use crate::{events, storage};

pub fn global_is_conservative(current: &GlobalConfig, proposed: &GlobalConfig) -> bool {
    let mut probe = current.clone();
    probe.risk_capacity_limit_bps = proposed.risk_capacity_limit_bps;
    probe == *proposed && proposed.risk_capacity_limit_bps < current.risk_capacity_limit_bps
}

pub fn market_is_conservative(current: &MarketConfig, proposed: &MarketConfig) -> bool {
    let mut probe = current.clone();
    probe.max_long_size_open_interest = proposed.max_long_size_open_interest;
    probe.max_short_size_open_interest = proposed.max_short_size_open_interest;
    probe.max_long_base_exposure = proposed.max_long_base_exposure;
    probe.max_short_base_exposure = proposed.max_short_base_exposure;
    probe.initial_margin_bps = proposed.initial_margin_bps;
    if probe != *proposed || *proposed == *current {
        return false;
    }
    let ceilings_not_raised = proposed.max_long_size_open_interest
        <= current.max_long_size_open_interest
        && proposed.max_short_size_open_interest <= current.max_short_size_open_interest
        && proposed.max_long_base_exposure <= current.max_long_base_exposure
        && proposed.max_short_base_exposure <= current.max_short_base_exposure;
    ceilings_not_raised && proposed.initial_margin_bps >= current.initial_margin_bps
}

pub fn effective_at(env: &Env, now: u64) -> u64 {
    now.saturating_add(storage::get_global_config(env).config_timelock_seconds)
}

pub fn require_configuration_authority(env: &Env, caller: &Address) {
    require_role(env, caller, shared::constants::ROLE_ADMIN);
}

fn require_applicable(env: &Env, effective_at: u64, now: u64) {
    if now < effective_at {
        panic_with_error!(env, PositionManagerError::ConfigTimelockNotElapsed);
    }
    let window = storage::get_global_config(env).config_timelock_seconds;
    if now > effective_at.saturating_add(window) {
        panic_with_error!(env, PositionManagerError::ConfigProposalExpired);
    }
}

pub fn take_due_global_proposal(env: &Env, now: u64) -> GlobalConfig {
    let pending = storage::try_get_pending_global_config(env)
        .unwrap_or_else(|| panic_with_error!(env, PositionManagerError::NoPendingConfig));
    require_applicable(env, pending.effective_at, now);
    storage::clear_pending_global_config(env);
    pending.config
}

pub fn take_due_market_proposal(env: &Env, market: &Symbol, now: u64) -> MarketConfig {
    let pending = storage::try_get_pending_market_config(env, market)
        .unwrap_or_else(|| panic_with_error!(env, PositionManagerError::NoPendingConfig));
    require_applicable(env, pending.effective_at, now);
    storage::clear_pending_market_config(env, market);
    pending.config
}

pub fn take_due_price_feed(env: &Env, now: u64) -> Address {
    let pending = storage::try_get_pending_price_feed(env)
        .unwrap_or_else(|| panic_with_error!(env, PositionManagerError::NoPendingConfig));
    require_applicable(env, pending.effective_at, now);
    storage::clear_pending_price_feed(env);
    pending.price_feed
}

pub fn cancel_global_proposal(env: &Env, actor: &Address) {
    if storage::try_get_pending_global_config(env).is_none() {
        panic_with_error!(env, PositionManagerError::NoPendingConfig);
    }
    storage::clear_pending_global_config(env);
    events::emit_proposal_cancelled(env, actor, None, false);
}

pub fn cancel_market_proposal(env: &Env, actor: &Address, market: &Symbol) {
    if storage::try_get_pending_market_config(env, market).is_none() {
        panic_with_error!(env, PositionManagerError::NoPendingConfig);
    }
    storage::clear_pending_market_config(env, market);
    events::emit_proposal_cancelled(env, actor, Some(market.clone()), false);
}

pub fn cancel_price_feed_proposal(env: &Env, actor: &Address) {
    if storage::try_get_pending_price_feed(env).is_none() {
        panic_with_error!(env, PositionManagerError::NoPendingConfig);
    }
    storage::clear_pending_price_feed(env);
    events::emit_proposal_cancelled(env, actor, None, true);
}

pub fn store_price_feed_proposal(env: &Env, actor: &Address, price_feed: &Address, effective_at: u64) {
    storage::save_pending_price_feed(
        env,
        &PendingPriceFeed {
            price_feed: price_feed.clone(),
            effective_at,
        },
    );
    events::emit_price_feed_proposed(env, actor, price_feed, effective_at);
}

pub fn store_global_proposal(
    env: &Env,
    actor: &Address,
    config: &GlobalConfig,
    effective_at: u64,
) {
    storage::save_pending_global_config(
        env,
        &PendingGlobalConfig {
            config: config.clone(),
            effective_at,
        },
    );
    events::emit_config_proposed(env, actor, None, effective_at);
}

pub fn store_market_proposal(
    env: &Env,
    actor: &Address,
    market: &Symbol,
    config: &MarketConfig,
    effective_at: u64,
) {
    storage::save_pending_market_config(
        env,
        market,
        &PendingMarketConfig {
            config: config.clone(),
            effective_at,
        },
    );
    events::emit_config_proposed(env, actor, Some(market.clone()), effective_at);
}
