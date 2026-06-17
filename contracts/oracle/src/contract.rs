use interfaces::{MigrationData, Oracle, TimelockedUpgradeable, UpgradeFailure};
use soroban_sdk::{contract, contractimpl, panic_with_error, Address, BytesN, Env, Symbol};
use stellar_contract_utils::upgradeable::{complete_migration, ensure_can_complete_migration};

use crate::errors::OracleError;
use crate::storage;

#[contract]
pub struct OracleContract;

#[contractimpl]
impl OracleContract {
    /// Atomic-with-deploy initialization (Soroban constructor). Binds the
    /// linked ConfigManager (role lookups for `set_publisher`) and the single
    /// per-instance `publisher` authorized to push prices. Runs once inside
    /// the deploy transaction, so init cannot be front-run.
    pub fn __constructor(env: Env, config_manager: Address, publisher: Address) {
        storage::set_config_manager(&env, &config_manager);
        storage::set_publisher(&env, &publisher);
        storage::set_initialized(&env);
        shared::bump_instance_ttl(&env);
    }
}

#[contractimpl]
impl Oracle for OracleContract {
    fn set_price(env: Env, caller: Address, symbol: Symbol, price: i128) {
        if !storage::is_initialized(&env) {
            panic_with_error!(&env, OracleError::NotInitialized);
        }
        require_publisher(&env, &caller);

        storage::set_price(&env, &symbol, price);
        storage::set_last_update(&env, &symbol, env.ledger().timestamp());
        shared::bump_instance_ttl(&env);
    }

    fn set_publisher(env: Env, caller: Address, new_publisher: Address) {
        if !storage::is_initialized(&env) {
            panic_with_error!(&env, OracleError::NotInitialized);
        }
        require_admin(&env, &caller);
        storage::set_publisher(&env, &new_publisher);
        shared::bump_instance_ttl(&env);
    }

    fn get_price(env: Env, symbol: Symbol) -> i128 {
        storage::get_price(&env, &symbol)
            .unwrap_or_else(|| panic_with_error!(&env, OracleError::NoPriceSet))
    }

    fn last_update(env: Env, symbol: Symbol) -> u64 {
        storage::get_last_update(&env, &symbol)
    }

    fn decimals(_env: Env) -> u32 {
        shared::constants::PRICE_DECIMALS
    }

    fn propose_upgrade(env: Env, caller: Address, wasm_hash: BytesN<32>) {
        if !storage::is_initialized(&env) {
            panic_with_error!(&env, OracleError::NotInitialized);
        }
        <Self as TimelockedUpgradeable>::propose(&env, caller, wasm_hash);
        shared::bump_instance_ttl(&env);
    }

    fn cancel_upgrade(env: Env, caller: Address) {
        if !storage::is_initialized(&env) {
            panic_with_error!(&env, OracleError::NotInitialized);
        }
        <Self as TimelockedUpgradeable>::cancel(&env, caller);
        shared::bump_instance_ttl(&env);
    }
}

// ---------------------------------------------------------------------------
// Upgrade / migrate entrypoints — `upgrade` delegates to the timelock's
// `execute`; `migrate` keeps the OZ-driven post-upgrade migration logic.
// ---------------------------------------------------------------------------
#[contractimpl]
impl OracleContract {
    pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>, operator: Address) {
        <Self as TimelockedUpgradeable>::execute(&env, operator, new_wasm_hash);
    }

    pub fn migrate(env: Env, migration_data: MigrationData, operator: Address) {
        require_upgrader(&env, &operator);
        ensure_can_complete_migration(&env);
        storage::save_version(&env, migration_data.version);
        complete_migration(&env);
    }
}

// ---------------------------------------------------------------------------
// Auth helpers
// ---------------------------------------------------------------------------

/// Require `caller` to be the stored per-instance publisher.
fn require_publisher(env: &Env, caller: &Address) {
    caller.require_auth();
    if *caller != storage::get_publisher(env) {
        panic_with_error!(env, OracleError::Unauthorized);
    }
}

/// Cross-contract role check against the linked ConfigManager. Panics with
/// `OracleError::Unauthorized` (code 3) on failure so the panic code
/// identifies the source contract.
fn require_role_or_panic(env: &Env, caller: &Address, role: &str) {
    caller.require_auth();
    let config_mgr = storage::get_config_manager(env);
    if !shared::has_role(env, &config_mgr, role, caller) {
        panic_with_error!(env, OracleError::Unauthorized);
    }
}

fn require_admin(env: &Env, caller: &Address) {
    require_role_or_panic(env, caller, shared::constants::ROLE_ADMIN);
}

fn require_upgrader(env: &Env, caller: &Address) {
    require_role_or_panic(env, caller, shared::constants::ROLE_UPGRADER);
}

fn require_pauser(env: &Env, caller: &Address) {
    require_role_or_panic(env, caller, shared::constants::ROLE_PAUSER);
}

// ---------------------------------------------------------------------------
// TimelockedUpgradeable impl — hooks supply the contract-specific bits.
// ---------------------------------------------------------------------------
impl TimelockedUpgradeable for OracleContract {
    fn _require_proposer(env: &Env, caller: &Address) {
        require_upgrader(env, caller);
    }
    fn _require_executor(env: &Env, caller: &Address) {
        require_upgrader(env, caller);
    }
    fn _require_canceller(env: &Env, caller: &Address) {
        require_pauser(env, caller);
    }
    fn _timelock_seconds(env: &Env) -> u64 {
        let config_mgr = storage::get_config_manager(env);
        interfaces::ConfigManagerClient::new(env, &config_mgr).get_upgrade_timelock()
    }
    fn _panic_with_upgrade_error(env: &Env, err: UpgradeFailure) -> ! {
        match err {
            UpgradeFailure::NoPendingUpgrade => {
                panic_with_error!(env, OracleError::NoPendingUpgrade)
            }
            UpgradeFailure::TimelockNotElapsed => {
                panic_with_error!(env, OracleError::UpgradeTimelockNotElapsed)
            }
            UpgradeFailure::HashMismatch => {
                panic_with_error!(env, OracleError::UpgradeHashMismatch)
            }
        }
    }
}
