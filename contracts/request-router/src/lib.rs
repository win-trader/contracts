#![no_std]

use interfaces::{
    ConfigManagerClient, LpRequest, LpRequestKind, LpRequestStatus, MigrationData,
    OracleRouterClient, RequestRouter, SettlementResult, SettlementStatus, TimelockedUpgradeable,
    UpgradeFailure, VaultClient,
};
use shared::constants::{ROLE_PAUSER, ROLE_UPGRADER};
use shared::constants::{SHARED_BUMP, SHARED_THRESHOLD};
use soroban_sdk::{
    auth::{ContractContext, InvokerContractAuthEntry, SubContractInvocation},
    contract, contracterror, contractimpl, contracttype, panic_with_error,
    token::Client as TokenClient,
    Address, BytesN, Env, IntoVal, Symbol, Vec,
};
use stellar_contract_utils::upgradeable::{complete_migration, ensure_can_complete_migration};

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum RequestRouterError {
    InvalidAmount = 1,
    InvalidRequest = 2,
    TooEarly = 3,
    QueueBlocked = 4,
    LpActionBlocked = 5,
    NoOracleRound = 6,
    Unauthorized = 7,
}

#[contracttype]
#[derive(Clone)]
enum Key {
    Asset,
    Vault,
    OracleRouter,
    ConfigManager,
    NextId,
    NextToResolve,
    Request(u64),
    Version,
}

#[contract]
pub struct RequestRouterContract;

fn get<T: soroban_sdk::TryFromVal<Env, soroban_sdk::Val>>(env: &Env, key: &Key) -> Option<T> {
    env.storage().instance().get(key)
}

fn set<T: soroban_sdk::IntoVal<Env, soroban_sdk::Val> + Clone>(env: &Env, key: &Key, value: &T) {
    env.storage().instance().set(key, value);
}

fn asset(env: &Env) -> Address {
    get(env, &Key::Asset).unwrap()
}

fn vault(env: &Env) -> Address {
    get(env, &Key::Vault).unwrap()
}

fn oracle(env: &Env) -> Address {
    get(env, &Key::OracleRouter).unwrap()
}

fn config_manager(env: &Env) -> Address {
    get(env, &Key::ConfigManager).unwrap()
}

fn require_role(env: &Env, caller: &Address, role: &str) {
    caller.require_auth();
    if !shared::has_role(env, &config_manager(env), role, caller) {
        panic_with_error!(env, RequestRouterError::Unauthorized);
    }
}

fn save_request(env: &Env, request: &LpRequest) {
    let key = Key::Request(request.id);
    env.storage().persistent().set(&key, request);
    env.storage()
        .persistent()
        .extend_ttl(&key, SHARED_THRESHOLD, SHARED_BUMP);
    shared::bump_instance_ttl(env);
}

fn load_request(env: &Env, id: u64) -> LpRequest {
    env.storage()
        .persistent()
        .get(&Key::Request(id))
        .unwrap_or_else(|| panic_with_error!(env, RequestRouterError::InvalidRequest))
}

fn transfer(env: &Env, token: &Address, from: &Address, to: &Address, amount: i128) {
    TokenClient::new(env, token).transfer(from, to, &amount);
}

fn latest_valid_round(env: &Env) -> interfaces::OracleRound {
    let client = OracleRouterClient::new(env, &oracle(env));
    let latest = client.latest_round_id();
    if latest == 0 {
        panic_with_error!(env, RequestRouterError::NoOracleRound);
    }
    let round = client.get_round(&latest);
    let max_age = client.get_oracle_config().staleness_threshold;
    if env.ledger().timestamp() > round.timestamp.saturating_add(max_age) {
        panic_with_error!(env, RequestRouterError::NoOracleRound);
    }
    round
}

fn ensure_lp_actions_open(env: &Env) {
    if !VaultClient::new(env, &vault(env)).can_create_lp_request() {
        panic_with_error!(env, RequestRouterError::LpActionBlocked);
    }
}

fn refund(env: &Env, request: &LpRequest) {
    let token = if request.kind == LpRequestKind::Deposit {
        asset(env)
    } else {
        vault(env)
    };
    transfer(
        env,
        &token,
        &env.current_contract_address(),
        &request.owner,
        request.amount,
    );
}

fn authorize_vault_asset_pull(env: &Env, amount: i128) {
    let current = env.current_contract_address();
    let vault_address = vault(env);
    env.authorize_as_current_contract(Vec::from_array(
        env,
        [InvokerContractAuthEntry::Contract(SubContractInvocation {
            context: ContractContext {
                contract: asset(env),
                fn_name: Symbol::new(env, "transfer"),
                args: (current, vault_address, amount).into_val(env),
            },
            sub_invocations: Vec::new(env),
        })],
    ));
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
        set(&env, &Key::Asset, &asset_address);
        set(&env, &Key::Vault, &vault_address);
        set(&env, &Key::OracleRouter, &oracle_router);
        set(&env, &Key::ConfigManager, &config_manager_address);
        set(&env, &Key::NextId, &1u64);
        set(&env, &Key::NextToResolve, &1u64);
        shared::bump_instance_ttl(&env);
    }
}

#[contractimpl]
impl RequestRouter for RequestRouterContract {
    fn request_deposit(env: Env, owner: Address, assets: i128) -> u64 {
        owner.require_auth();
        if assets <= 0 {
            panic_with_error!(&env, RequestRouterError::InvalidAmount);
        }
        ensure_lp_actions_open(&env);
        transfer(
            &env,
            &asset(&env),
            &owner,
            &env.current_contract_address(),
            assets,
        );
        let id = get::<u64>(&env, &Key::NextId).unwrap_or(1);
        set(&env, &Key::NextId, &(id + 1));
        let now = env.ledger().timestamp();
        let delay = VaultClient::new(&env, &vault(&env))
            .get_lp_config()
            .lp_request_delay;
        save_request(
            &env,
            &LpRequest {
                id,
                owner,
                kind: LpRequestKind::Deposit,
                amount: assets,
                request_time: now,
                execute_after: now.saturating_add(delay),
                status: LpRequestStatus::Pending,
            },
        );
        id
    }

    fn request_withdrawal(env: Env, owner: Address, shares: i128) -> u64 {
        owner.require_auth();
        if shares <= 0 {
            panic_with_error!(&env, RequestRouterError::InvalidAmount);
        }
        ensure_lp_actions_open(&env);
        transfer(
            &env,
            &vault(&env),
            &owner,
            &env.current_contract_address(),
            shares,
        );
        let id = get::<u64>(&env, &Key::NextId).unwrap_or(1);
        set(&env, &Key::NextId, &(id + 1));
        let now = env.ledger().timestamp();
        let delay = VaultClient::new(&env, &vault(&env))
            .get_lp_config()
            .lp_request_delay;
        save_request(
            &env,
            &LpRequest {
                id,
                owner,
                kind: LpRequestKind::Withdrawal,
                amount: shares,
                request_time: now,
                execute_after: now.saturating_add(delay),
                status: LpRequestStatus::Pending,
            },
        );
        id
    }

    fn resolve_next(env: Env, executor: Address) -> SettlementResult {
        executor.require_auth();
        let id = get::<u64>(&env, &Key::NextToResolve).unwrap_or(1);
        let mut request = load_request(&env, id);
        if request.status != LpRequestStatus::Pending {
            panic_with_error!(&env, RequestRouterError::InvalidRequest);
        }
        let round = latest_valid_round(&env);
        if round.timestamp < request.execute_after {
            panic_with_error!(&env, RequestRouterError::TooEarly);
        }
        if round.previous_timestamp >= request.execute_after {
            request.status = LpRequestStatus::Expired;
            save_request(&env, &request);
            set(&env, &Key::NextToResolve, &(id + 1));
            refund(&env, &request);
            return SettlementResult {
                status: SettlementStatus::Failed,
                amount: 0,
            };
        }

        // Mark and advance before external effects. A panic rolls the complete
        // transaction back, while an expected business failure is refunded.
        request.status = LpRequestStatus::Settled;
        save_request(&env, &request);
        set(&env, &Key::NextToResolve, &(id + 1));
        let vault_client = VaultClient::new(&env, &vault(&env));
        let result = if request.kind == LpRequestKind::Deposit {
            authorize_vault_asset_pull(&env, request.amount);
            vault_client.settle_deposit(
                &env.current_contract_address(),
                &request.owner,
                &request.amount,
                &round,
            )
        } else {
            vault_client.settle_withdrawal(
                &env.current_contract_address(),
                &request.owner,
                &request.amount,
                &round,
            )
        };
        if result.status == SettlementStatus::Failed {
            request.status = LpRequestStatus::Failed;
            save_request(&env, &request);
            refund(&env, &request);
        }
        result
    }

    fn get_request(env: Env, request_id: u64) -> LpRequest {
        load_request(&env, request_id)
    }

    fn next_request_to_resolve(env: Env) -> u64 {
        get(&env, &Key::NextToResolve).unwrap_or(1)
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
        set(&env, &Key::Version, &data.version);
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
        ConfigManagerClient::new(env, &config_manager(env)).get_upgrade_timelock()
    }
    fn _panic_with_upgrade_error(env: &Env, _: UpgradeFailure) -> ! {
        panic_with_error!(env, RequestRouterError::Unauthorized)
    }
}
