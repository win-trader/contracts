//! §12.3 — the two-phase configuration change and its two exemptions.
//!
//! A parameter change is proposed, waits out `config_timelock_seconds`, and
//! is then applied. The delay exists because almost every parameter here can
//! move value between parties who cannot react instantly: raising
//! `close_pnl_fee_bps` taxes open positions at settlement, lowering
//! `hard_cap_pnl_factor_bps` reduces payouts on a side that is already
//! restricted, changing `maintenance_margin_bps` makes positions
//! liquidatable that were not. A timelock prevents none of that, but it
//! makes it observable in advance — which is the difference between a
//! governance action and a surprise.
//!
//! Applying is permissionless. The authorization happened at proposal, and
//! the delay is the protection; requiring the authority again at apply would
//! let a proposal be published and then quietly never land.

use soroban_sdk::{panic_with_error, Address, Env, Symbol};

use shared::{GlobalConfig, MarketConfig, PendingGlobalConfig, PendingMarketConfig};

use crate::auth::require_role;
use crate::errors::PositionManagerError;
use crate::{events, storage};

/// §12.3 — is this global proposal exempt from the timelock?
///
/// Only two categories are exempt, and this is the second: a change that
/// moves a bound in the more conservative direction. A protocol that must
/// wait 48 hours to become safer has the timelock pointed the wrong way.
///
/// Exemption requires **every other field to be unchanged**. A conservative
/// move bundled with an unrelated one is not exempt, because the bundle
/// would otherwise be the bypass.
pub fn global_is_conservative(current: &GlobalConfig, proposed: &GlobalConfig) -> bool {
    let mut probe = current.clone();
    probe.risk_capacity_limit_bps = proposed.risk_capacity_limit_bps;
    probe == *proposed && proposed.risk_capacity_limit_bps < current.risk_capacity_limit_bps
}

/// §12.3 — the per-market counterpart: lowering an exposure ceiling or
/// raising a margin requirement, with nothing else changed.
pub fn market_is_conservative(current: &MarketConfig, proposed: &MarketConfig) -> bool {
    let mut probe = current.clone();
    probe.max_long_size_open_interest = proposed.max_long_size_open_interest;
    probe.max_short_size_open_interest = proposed.max_short_size_open_interest;
    probe.max_long_base_exposure = proposed.max_long_base_exposure;
    probe.max_short_base_exposure = proposed.max_short_base_exposure;
    probe.initial_margin_bps = proposed.initial_margin_bps;
    probe.maintenance_margin_bps = proposed.maintenance_margin_bps;
    if probe != *proposed || *proposed == *current {
        return false;
    }
    let ceilings_not_raised = proposed.max_long_size_open_interest
        <= current.max_long_size_open_interest
        && proposed.max_short_size_open_interest <= current.max_short_size_open_interest
        && proposed.max_long_base_exposure <= current.max_long_base_exposure
        && proposed.max_short_base_exposure <= current.max_short_base_exposure;
    let margins_not_lowered = proposed.initial_margin_bps >= current.initial_margin_bps
        && proposed.maintenance_margin_bps >= current.maintenance_margin_bps;
    ceilings_not_raised && margins_not_lowered
}

/// The moment a validated proposal may be applied.
pub fn effective_at(env: &Env, now: u64) -> u64 {
    now.saturating_add(storage::get_global_config(env).config_timelock_seconds)
}

/// Authenticate the configuration authority (§12.3).
pub fn require_configuration_authority(env: &Env, caller: &Address) {
    require_role(env, caller, shared::constants::ROLE_ADMIN);
}

/// Take a stored global proposal whose timelock has elapsed.
pub fn take_due_global_proposal(env: &Env, now: u64) -> GlobalConfig {
    let pending = storage::try_get_pending_global_config(env)
        .unwrap_or_else(|| panic_with_error!(env, PositionManagerError::NoPendingConfig));
    if now < pending.effective_at {
        panic_with_error!(env, PositionManagerError::ConfigTimelockNotElapsed);
    }
    storage::clear_pending_global_config(env);
    pending.config
}

/// Take a stored market proposal whose timelock has elapsed.
pub fn take_due_market_proposal(env: &Env, market: &Symbol, now: u64) -> MarketConfig {
    let pending = storage::try_get_pending_market_config(env, market)
        .unwrap_or_else(|| panic_with_error!(env, PositionManagerError::NoPendingConfig));
    if now < pending.effective_at {
        panic_with_error!(env, PositionManagerError::ConfigTimelockNotElapsed);
    }
    storage::clear_pending_market_config(env, market);
    pending.config
}

/// Store a validated global proposal and announce it.
pub fn store_global_proposal(env: &Env, config: &GlobalConfig, effective_at: u64) {
    storage::save_pending_global_config(
        env,
        &PendingGlobalConfig {
            config: config.clone(),
            effective_at,
        },
    );
    events::emit_config_proposed(env, None, effective_at);
}

/// Store a validated market proposal and announce it.
pub fn store_market_proposal(env: &Env, market: &Symbol, config: &MarketConfig, effective_at: u64) {
    storage::save_pending_market_config(
        env,
        market,
        &PendingMarketConfig {
            config: config.clone(),
            effective_at,
        },
    );
    events::emit_config_proposed(env, Some(market.clone()), effective_at);
}
