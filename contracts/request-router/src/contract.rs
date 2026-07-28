use shared::constants::{ROLE_PAUSER, ROLE_UPGRADER};
use shared::{
    ConfigManagerClient, LpRequest, MigrationData, RequestRouter, SettlementResult,
    TimelockedUpgradeable, UpgradeFailure,
};
use soroban_sdk::{contract, contractimpl, panic_with_error, Address, BytesN, Env};
use stellar_contract_utils::upgradeable::{complete_migration, ensure_can_complete_migration};

use crate::errors::RequestRouterError;
use crate::{requests, storage};

#[contract]
pub struct RequestRouterContract;

fn require_role(env: &Env, caller: &Address, role: &str) {
    caller.require_auth();
    if !shared::has_role(env, &storage::config_manager(env), role, caller) {
        panic_with_error!(env, RequestRouterError::Unauthorized);
    }
}

#[contractimpl]
impl RequestRouterContract {
    pub fn __constructor(
        env: Env,
        asset_address: Address,
        vault_address: Address,
        oracle_router: Address,
        config_manager_address: Address,
    ) {
        storage::set(&env, &storage::Key::Asset, &asset_address);
        storage::set(&env, &storage::Key::Vault, &vault_address);
        storage::set(&env, &storage::Key::OracleRouter, &oracle_router);
        storage::set(&env, &storage::Key::ConfigManager, &config_manager_address);
        storage::set(&env, &storage::Key::NextId, &1u64);
        storage::set(&env, &storage::Key::NextToResolve, &1u64);
        shared::bump_instance_ttl(&env);
    }
}

#[contractimpl]
impl RequestRouter for RequestRouterContract {
    fn request_deposit(env: Env, owner: Address, assets: i128) -> u64 {
        requests::request_deposit(&env, owner, assets)
    }

    fn request_withdrawal(env: Env, owner: Address, shares: i128) -> u64 {
        requests::request_withdrawal(&env, owner, shares)
    }

    fn resolve_next(env: Env, executor: Address) -> SettlementResult {
        requests::resolve_next(&env, executor)
    }

    fn get_request(env: Env, request_id: u64) -> LpRequest {
        storage::load_request(&env, request_id)
    }

    fn next_request_to_resolve(env: Env) -> u64 {
        storage::next_to_resolve(&env)
    }

    fn propose_upgrade(env: Env, caller: Address, wasm_hash: BytesN<32>) {
        <Self as TimelockedUpgradeable>::propose(&env, caller, wasm_hash);
    }

    fn cancel_upgrade(env: Env, caller: Address) {
        <Self as TimelockedUpgradeable>::cancel(&env, caller);
    }
}

#[contractimpl]
impl RequestRouterContract {
    pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>, operator: Address) {
        <Self as TimelockedUpgradeable>::execute(&env, operator, new_wasm_hash);
    }

    pub fn migrate(env: Env, data: MigrationData, operator: Address) {
        require_role(&env, &operator, ROLE_UPGRADER);
        ensure_can_complete_migration(&env);
        storage::save_version(&env, &data);
        complete_migration(&env);
    }
}

impl TimelockedUpgradeable for RequestRouterContract {
    fn _require_proposer(env: &Env, caller: &Address) {
        require_role(env, caller, ROLE_UPGRADER);
    }

    fn _require_executor(env: &Env, caller: &Address) {
        require_role(env, caller, ROLE_UPGRADER);
    }

    fn _require_canceller(env: &Env, caller: &Address) {
        require_role(env, caller, ROLE_PAUSER);
    }

    fn _timelock_seconds(env: &Env) -> u64 {
        ConfigManagerClient::new(env, &storage::config_manager(env)).get_upgrade_timelock()
    }

    fn _panic_with_upgrade_error(env: &Env, failure: UpgradeFailure) -> ! {
        match failure {
            UpgradeFailure::NoPendingUpgrade => {
                panic_with_error!(env, RequestRouterError::UpgradeNoPending)
            }
            UpgradeFailure::TimelockNotElapsed => {
                panic_with_error!(env, RequestRouterError::UpgradeTimelockNotElapsed)
            }
            UpgradeFailure::HashMismatch => {
                panic_with_error!(env, RequestRouterError::UpgradeHashMismatch)
            }
        }
    }
}
