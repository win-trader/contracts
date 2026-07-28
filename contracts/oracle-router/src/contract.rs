use shared::bump_instance_ttl;
use shared::constants::MAX_ORACLE_SOURCES;
use shared::{
    ConfigManagerClient, MigrationData, OracleConfig, OracleRound, OracleRouter,
    PositionManagerClient, RoundPrice, TimelockedUpgradeable, UpgradeFailure,
};
use soroban_sdk::{contract, contractimpl, panic_with_error, Address, BytesN, Env, Symbol, Vec};
use stellar_contract_utils::upgradeable::{complete_migration, ensure_can_complete_migration};

use crate::errors::OracleRouterError;
use crate::{events, logic, storage};

#[contract]
pub struct OracleRouterContract;

#[contractimpl]
impl OracleRouterContract {
    /// Atomic-with-deploy initialization (Soroban constructor). Binds the
    /// linked ConfigManager once, inside the deploy transaction, so no third
    /// party can front-run init and point the router at a malicious role
    /// authority. The router stores no admin of its own — every role check
    /// cross-calls the linked ConfigManager.
    pub fn __constructor(env: Env, config_manager_address: Address) {
        storage::set_config_manager(&env, &config_manager_address);
        storage::set_initialized(&env);
        bump_instance_ttl(&env);
    }
}

#[contractimpl]
impl OracleRouter for OracleRouterContract {
    fn get_price(env: Env, symbol: Symbol) -> i128 {
        logic::fetch_and_validate_price(&env, symbol)
    }

    fn set_position_manager(env: Env, caller: Address, position_manager: Address) {
        logic::require_oracle_admin(&env, &caller);
        if storage::has_position_manager(&env) {
            panic_with_error!(&env, OracleRouterError::PositionManagerAlreadySet);
        }
        storage::set_position_manager(&env, &position_manager);
        shared::bump_instance_ttl(&env);
    }

    fn publish_round(env: Env, caller: Address) -> u64 {
        logic::require_keeper(&env, &caller);
        let markets = PositionManagerClient::new(&env, &storage::load_position_manager(&env))
            .active_markets();
        let mut prices = Vec::new(&env);
        let mut i = 0u32;
        while i < markets.len() {
            let symbol = markets.get(i).unwrap();
            let price = logic::fetch_and_validate_price(&env, symbol.clone());
            prices.push_back(RoundPrice { symbol, price });
            i += 1;
        }
        let previous_id = storage::latest_round_id(&env);
        let previous_timestamp = if previous_id == 0 {
            0
        } else {
            storage::load_round(&env, previous_id).timestamp
        };
        let id = previous_id
            .checked_add(1)
            .unwrap_or_else(|| panic_with_error!(&env, OracleRouterError::InvalidConfig));
        let round = OracleRound {
            id,
            timestamp: env.ledger().timestamp(),
            previous_id,
            previous_timestamp,
            prices,
        };
        storage::save_round(&env, &round);
        shared::bump_instance_ttl(&env);
        events::RoundPublished {
            id,
            timestamp: round.timestamp,
            previous_id,
        }
        .publish(&env);
        id
    }

    fn latest_round_id(env: Env) -> u64 {
        storage::latest_round_id(&env)
    }

    fn get_round(env: Env, round_id: u64) -> OracleRound {
        storage::load_round(&env, round_id)
    }

    fn set_oracle_sources(env: Env, caller: Address, symbol: Symbol, sources: Vec<Address>) {
        logic::require_oracle_admin(&env, &caller);
        if sources.len() > MAX_ORACLE_SOURCES {
            panic_with_error!(&env, OracleRouterError::TooManySources);
        }
        let deduped = logic::dedup_sources(&env, &sources);
        logic::validate_source_decimals(&env, &deduped);
        storage::save_sources(&env, &symbol, &deduped);
        // Drop any cached median so a source rotation/disable takes effect
        // immediately rather than serving the stale aggregate.
        storage::remove_cached_price(&env, &symbol);
        events::OracleSourcesUpdate {
            symbol: symbol.clone(),
            sources: deduped,
        }
        .publish(&env);
        bump_instance_ttl(&env);
    }

    fn set_oracle_config(env: Env, caller: Address, config: OracleConfig) {
        use logic::Validate;
        logic::require_oracle_admin(&env, &caller);
        config.validate(&env);
        storage::save_oracle_config(&env, &config);
        storage::bump_config_version(&env);
        events::OracleConfigUpdate {
            staleness: config.staleness_threshold,
            deviation: config.max_deviation_bps,
            cache_duration: config.cache_duration,
            min_required_sources: config.min_required_sources,
        }
        .publish(&env);
        bump_instance_ttl(&env);
    }

    fn get_oracle_config(env: Env) -> OracleConfig {
        storage::load_oracle_config(&env)
    }

    fn bump_oracle_state(env: Env) {
        bump_instance_ttl(&env);
    }

    fn propose_upgrade(env: Env, caller: Address, wasm_hash: BytesN<32>) {
        storage::require_initialized(&env);
        <Self as TimelockedUpgradeable>::propose(&env, caller, wasm_hash);
        bump_instance_ttl(&env);
    }

    fn cancel_upgrade(env: Env, caller: Address) {
        storage::require_initialized(&env);
        <Self as TimelockedUpgradeable>::cancel(&env, caller);
        bump_instance_ttl(&env);
    }
}

// ---------------------------------------------------------------------------
// Upgrade / migrate entrypoints — `upgrade` delegates to the trait's
// `execute`; `migrate` keeps its OZ-driven post-upgrade migration logic.
// ---------------------------------------------------------------------------
#[contractimpl]
impl OracleRouterContract {
    pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>, operator: Address) {
        storage::require_initialized(&env);
        <Self as TimelockedUpgradeable>::execute(&env, operator, new_wasm_hash);
    }

    pub fn migrate(env: Env, migration_data: MigrationData, operator: Address) {
        storage::require_initialized(&env);
        logic::require_upgrader(&env, &operator);
        ensure_can_complete_migration(&env);
        Self::_migrate(&env, &migration_data);
        complete_migration(&env);
    }
}

impl OracleRouterContract {
    pub(crate) fn _migrate(env: &Env, data: &MigrationData) {
        storage::save_version(env, data.version);
    }
}

// ---------------------------------------------------------------------------
// TimelockedUpgradeable impl — hooks supply the contract-specific bits.
// ---------------------------------------------------------------------------
impl TimelockedUpgradeable for OracleRouterContract {
    fn _require_proposer(env: &Env, caller: &Address) {
        logic::require_upgrader(env, caller);
    }
    fn _require_executor(env: &Env, caller: &Address) {
        logic::require_upgrader(env, caller);
    }
    fn _require_canceller(env: &Env, caller: &Address) {
        logic::require_pauser_for_upgrade(env, caller);
    }
    fn _timelock_seconds(env: &Env) -> u64 {
        let config_mgr = storage::load_config_manager(env);
        ConfigManagerClient::new(env, &config_mgr).get_upgrade_timelock()
    }
    fn _panic_with_upgrade_error(env: &Env, err: UpgradeFailure) -> ! {
        match err {
            UpgradeFailure::NoPendingUpgrade => {
                panic_with_error!(env, OracleRouterError::NoPendingUpgrade)
            }
            UpgradeFailure::TimelockNotElapsed => {
                panic_with_error!(env, OracleRouterError::UpgradeTimelockNotElapsed)
            }
            UpgradeFailure::HashMismatch => {
                panic_with_error!(env, OracleRouterError::UpgradeHashMismatch)
            }
        }
    }
}
