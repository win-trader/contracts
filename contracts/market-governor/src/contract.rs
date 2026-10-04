use shared::constants::{ROLE_ADMIN, ROLE_ORACLE, ROLE_PAUSER, ROLE_UPGRADER};
use shared::{
    ConfigManagerClient, GlobalConfig, MarketConfig, MarketGovernor, MigrationData,
    PendingGlobalConfig, PendingMarketConfig, PendingPriceFeed, PositionManagerClient,
    PriceFeedClient, TimelockedUpgradeable, UpgradeFailure,
};
use soroban_sdk::{contract, contractimpl, panic_with_error, Address, BytesN, Env, Symbol};
use stellar_contract_utils::upgradeable::{complete_migration, ensure_can_complete_migration};

use crate::errors::MarketGovernorError;
use crate::{events, storage};

#[contract]
pub struct MarketGovernorContract;

fn has_role(env: &Env, caller: &Address, role: &str) -> bool {
    shared::has_role(env, &storage::config_manager(env), role, caller)
}

fn require_role(env: &Env, caller: &Address, role: &str) {
    caller.require_auth();
    if !has_role(env, caller, role) {
        panic_with_error!(env, MarketGovernorError::Unauthorized);
    }
    shared::bump_instance_ttl(env);
}

fn pm(env: &Env) -> PositionManagerClient<'_> {
    PositionManagerClient::new(env, &storage::position_manager(env))
}

fn config_timelock(env: &Env) -> u64 {
    pm(env).global_config().config_timelock_seconds
}

fn effective_at(env: &Env) -> u64 {
    env.ledger().timestamp().saturating_add(config_timelock(env))
}

// Due once the timelock has passed, and only for one more timelock after that.
fn require_applicable(env: &Env, effective_at: u64) {
    let now = env.ledger().timestamp();
    if now < effective_at {
        panic_with_error!(env, MarketGovernorError::ConfigTimelockNotElapsed);
    }
    if now > effective_at.saturating_add(config_timelock(env)) {
        panic_with_error!(env, MarketGovernorError::ConfigProposalExpired);
    }
}

fn require_valid_global(env: &Env, config: &GlobalConfig) {
    if !shared::validation::global_is_valid(config) {
        panic_with_error!(env, MarketGovernorError::InvalidConfig);
    }
}

fn require_valid_market(env: &Env, config: &MarketConfig) {
    if !shared::validation::market_is_valid(config) {
        panic_with_error!(env, MarketGovernorError::InvalidConfig);
    }
}

fn require_feed_decimals(env: &Env, price_feed: &Address) {
    match PriceFeedClient::new(env, price_feed).try_decimals() {
        Ok(Ok(decimals)) if decimals == shared::constants::PRICE_DECIMALS => {}
        _ => panic_with_error!(env, MarketGovernorError::PriceUnavailable),
    }
}

/// Only lowering `risk_capacity_limit_bps` skips the timelock.
pub fn global_is_conservative(current: &GlobalConfig, proposed: &GlobalConfig) -> bool {
    let mut probe = current.clone();
    probe.risk_capacity_limit_bps = proposed.risk_capacity_limit_bps;
    probe == *proposed && proposed.risk_capacity_limit_bps < current.risk_capacity_limit_bps
}

/// Only lowering caps or raising initial margin skips the timelock.
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

#[contractimpl]
impl MarketGovernorContract {
    pub fn __constructor(env: Env, config_manager: Address, position_manager: Address) {
        storage::init(&env, &config_manager, &position_manager);
        shared::bump_instance_ttl(&env);
    }
}

#[contractimpl]
impl MarketGovernor for MarketGovernorContract {
    fn propose_global_config(env: Env, caller: Address, config: GlobalConfig) {
        require_role(&env, &caller, ROLE_ADMIN);
        require_valid_global(&env, &config);
        let current = &env.current_contract_address();
        if global_is_conservative(&pm(&env).global_config(), &config) {
            pm(&env).install_global_config(current, &caller, &config);
            return;
        }
        let effective_at = effective_at(&env);
        storage::save_pending_global(&env, &PendingGlobalConfig { config, effective_at });
        events::emit_config_proposed(&env, &caller, None, effective_at);
    }

    fn apply_global_config(env: Env, caller: Address) {
        caller.require_auth();
        let pending = storage::pending_global(&env)
            .unwrap_or_else(|| panic_with_error!(&env, MarketGovernorError::NoPendingConfig));
        require_applicable(&env, pending.effective_at);
        storage::clear_pending_global(&env);
        pm(&env).install_global_config(&env.current_contract_address(), &caller, &pending.config);
    }

    fn cancel_global_config(env: Env, caller: Address) {
        require_role(&env, &caller, ROLE_ADMIN);
        if storage::pending_global(&env).is_none() {
            panic_with_error!(&env, MarketGovernorError::NoPendingConfig);
        }
        storage::clear_pending_global(&env);
        events::emit_proposal_cancelled(&env, &caller, None, false);
    }

    fn propose_market_config(env: Env, caller: Address, market_symbol: Symbol, config: MarketConfig) {
        require_role(&env, &caller, ROLE_ADMIN);
        require_valid_market(&env, &config);
        let pm = pm(&env);
        // A brand-new market installs now. A deregistered one comes back only
        // through the timelock (THREAT_MODEL T-04).
        let immediate = match pm.try_get_market(&market_symbol) {
            Ok(Ok(market)) => {
                pm.active_markets().iter().any(|s| s == market_symbol)
                    && market_is_conservative(&market.config, &config)
            }
            _ => true,
        };
        if immediate {
            pm.install_market_config(&env.current_contract_address(), &caller, &market_symbol, &config);
            return;
        }
        let effective_at = effective_at(&env);
        storage::save_pending_market(&env, &market_symbol, &PendingMarketConfig { config, effective_at });
        events::emit_config_proposed(&env, &caller, Some(market_symbol), effective_at);
    }

    fn apply_market_config(env: Env, caller: Address, market_symbol: Symbol) {
        caller.require_auth();
        let pending = storage::pending_market(&env, &market_symbol)
            .unwrap_or_else(|| panic_with_error!(&env, MarketGovernorError::NoPendingConfig));
        require_applicable(&env, pending.effective_at);
        storage::clear_pending_market(&env, &market_symbol);
        pm(&env).install_market_config(
            &env.current_contract_address(),
            &caller,
            &market_symbol,
            &pending.config,
        );
    }

    fn cancel_market_config(env: Env, caller: Address, market_symbol: Symbol) {
        require_role(&env, &caller, ROLE_ADMIN);
        if storage::pending_market(&env, &market_symbol).is_none() {
            panic_with_error!(&env, MarketGovernorError::NoPendingConfig);
        }
        storage::clear_pending_market(&env, &market_symbol);
        events::emit_proposal_cancelled(&env, &caller, Some(market_symbol), false);
    }

    fn propose_price_feed(env: Env, caller: Address, price_feed: Address) {
        require_role(&env, &caller, ROLE_ORACLE);
        require_feed_decimals(&env, &price_feed);
        let effective_at = effective_at(&env);
        storage::save_pending_price_feed(
            &env,
            &PendingPriceFeed { price_feed: price_feed.clone(), effective_at },
        );
        events::emit_price_feed_proposed(&env, &caller, &price_feed, effective_at);
    }

    fn apply_price_feed(env: Env, caller: Address) {
        caller.require_auth();
        let pending = storage::pending_price_feed(&env)
            .unwrap_or_else(|| panic_with_error!(&env, MarketGovernorError::NoPendingConfig));
        require_applicable(&env, pending.effective_at);
        storage::clear_pending_price_feed(&env);
        pm(&env).install_price_feed(&env.current_contract_address(), &caller, &pending.price_feed);
    }

    // ADMIN can cancel too, so revoking a compromised ORACLE key is enough to
    // stop its proposal (THREAT_MODEL T-07).
    fn cancel_price_feed(env: Env, caller: Address) {
        caller.require_auth();
        if !has_role(&env, &caller, ROLE_ORACLE) && !has_role(&env, &caller, ROLE_ADMIN) {
            panic_with_error!(&env, MarketGovernorError::Unauthorized);
        }
        if storage::pending_price_feed(&env).is_none() {
            panic_with_error!(&env, MarketGovernorError::NoPendingConfig);
        }
        storage::clear_pending_price_feed(&env);
        events::emit_proposal_cancelled(&env, &caller, None, true);
    }

    fn deregister_market(env: Env, caller: Address, market_symbol: Symbol) {
        require_role(&env, &caller, ROLE_ADMIN);
        pm(&env).deregister_market(&env.current_contract_address(), &caller, &market_symbol);
    }

    fn position_manager(env: Env) -> Address {
        storage::position_manager(&env)
    }

    fn propose_upgrade(env: Env, caller: Address, wasm_hash: BytesN<32>) {
        <Self as TimelockedUpgradeable>::propose(&env, caller, wasm_hash);
    }

    fn cancel_upgrade(env: Env, caller: Address) {
        <Self as TimelockedUpgradeable>::cancel(&env, caller);
    }
}

#[contractimpl]
impl MarketGovernorContract {
    pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>, operator: Address) {
        <Self as TimelockedUpgradeable>::execute(&env, operator, new_wasm_hash);
    }

    pub fn migrate(env: Env, data: MigrationData, operator: Address) {
        require_role(&env, &operator, ROLE_UPGRADER);
        ensure_can_complete_migration(&env);
        storage::save_version(&env, data.version);
        complete_migration(&env);
    }
}

impl TimelockedUpgradeable for MarketGovernorContract {
    fn _require_proposer(env: &Env, caller: &Address) {
        require_role(env, caller, ROLE_UPGRADER);
    }
    fn _require_executor(env: &Env, caller: &Address) {
        require_role(env, caller, ROLE_UPGRADER);
    }
    fn _require_canceller(env: &Env, caller: &Address) {
        require_role(env, caller, ROLE_PAUSER);
    }
    // Never shorter than the config timelock: upgrading the governor must not
    // be a faster path to new config than proposing it.
    fn _timelock_seconds(env: &Env) -> u64 {
        let upgrade =
            ConfigManagerClient::new(env, &storage::config_manager(env)).get_upgrade_timelock();
        core::cmp::max(upgrade, config_timelock(env))
    }
    fn _panic_with_upgrade_error(env: &Env, failure: UpgradeFailure) -> ! {
        match failure {
            UpgradeFailure::NoPendingUpgrade => {
                panic_with_error!(env, MarketGovernorError::UpgradeNoPending)
            }
            UpgradeFailure::TimelockNotElapsed => {
                panic_with_error!(env, MarketGovernorError::UpgradeTimelockNotElapsed)
            }
            UpgradeFailure::HashMismatch => {
                panic_with_error!(env, MarketGovernorError::UpgradeHashMismatch)
            }
        }
    }
}
