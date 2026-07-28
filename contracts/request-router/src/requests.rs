use shared::{
    LpRequest, LpRequestKind, LpRequestStatus, OracleRouterClient, SettlementResult,
    SettlementStatus, VaultClient,
};
use soroban_sdk::{
    auth::{ContractContext, InvokerContractAuthEntry, SubContractInvocation},
    panic_with_error,
    token::Client as TokenClient,
    Address, Env, IntoVal, Symbol, Vec,
};

use crate::errors::RequestRouterError;
use crate::{events, storage};

fn transfer(env: &Env, token: &Address, from: &Address, to: &Address, amount: i128) {
    TokenClient::new(env, token).transfer(from, to, &amount);
}

fn latest_valid_round(env: &Env) -> shared::OracleRound {
    let client = OracleRouterClient::new(env, &storage::oracle(env));
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
    if !VaultClient::new(env, &storage::vault(env)).can_create_lp_request() {
        panic_with_error!(env, RequestRouterError::LpActionBlocked);
    }
}

fn refund(env: &Env, request: &LpRequest) {
    let token = if request.kind == LpRequestKind::Deposit {
        storage::asset(env)
    } else {
        storage::vault(env)
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
    let vault_address = storage::vault(env);
    env.authorize_as_current_contract(Vec::from_array(
        env,
        [InvokerContractAuthEntry::Contract(SubContractInvocation {
            context: ContractContext {
                contract: storage::asset(env),
                fn_name: Symbol::new(env, "transfer"),
                args: (current, vault_address, amount).into_val(env),
            },
            sub_invocations: Vec::new(env),
        })],
    ));
}

pub(crate) fn request_deposit(env: &Env, owner: Address, assets: i128) -> u64 {
    owner.require_auth();
    if assets <= 0 {
        panic_with_error!(env, RequestRouterError::InvalidAmount);
    }
    ensure_lp_actions_open(env);
    transfer(
        env,
        &storage::asset(env),
        &owner,
        &env.current_contract_address(),
        assets,
    );
    let id = storage::next_id(env);
    storage::advance_next_id(env, id);
    let now = env.ledger().timestamp();
    let delay = VaultClient::new(env, &storage::vault(env))
        .get_lp_config()
        .lp_request_delay;
    let request = LpRequest {
        id,
        owner,
        kind: LpRequestKind::Deposit,
        amount: assets,
        request_time: now,
        execute_after: now.saturating_add(delay),
        status: LpRequestStatus::Pending,
    };
    storage::save_request(env, &request);
    events::LpRequestCreated {
        request_id: id,
        owner: request.owner,
        kind: request.kind,
        amount: assets,
        execute_after: request.execute_after,
    }
    .publish(env);
    id
}

pub(crate) fn request_withdrawal(env: &Env, owner: Address, shares: i128) -> u64 {
    owner.require_auth();
    if shares <= 0 {
        panic_with_error!(env, RequestRouterError::InvalidAmount);
    }
    ensure_lp_actions_open(env);
    transfer(
        env,
        &storage::vault(env),
        &owner,
        &env.current_contract_address(),
        shares,
    );
    let id = storage::next_id(env);
    storage::advance_next_id(env, id);
    let now = env.ledger().timestamp();
    let delay = VaultClient::new(env, &storage::vault(env))
        .get_lp_config()
        .lp_request_delay;
    let request = LpRequest {
        id,
        owner,
        kind: LpRequestKind::Withdrawal,
        amount: shares,
        request_time: now,
        execute_after: now.saturating_add(delay),
        status: LpRequestStatus::Pending,
    };
    storage::save_request(env, &request);
    events::LpRequestCreated {
        request_id: id,
        owner: request.owner,
        kind: request.kind,
        amount: shares,
        execute_after: request.execute_after,
    }
    .publish(env);
    id
}

pub(crate) fn resolve_next(env: &Env, executor: Address) -> SettlementResult {
    executor.require_auth();
    let id = storage::next_to_resolve(env);
    let mut request = storage::load_request(env, id);
    if request.status != LpRequestStatus::Pending {
        panic_with_error!(env, RequestRouterError::InvalidRequest);
    }
    let round = latest_valid_round(env);
    if round.timestamp < request.execute_after {
        panic_with_error!(env, RequestRouterError::TooEarly);
    }
    if round.previous_timestamp >= request.execute_after {
        request.status = LpRequestStatus::Expired;
        storage::save_request(env, &request);
        storage::advance_next_to_resolve(env, id);
        refund(env, &request);
        events::LpRequestResolved {
            request_id: id,
            owner: request.owner,
            kind: request.kind,
            status: request.status,
            settled_amount: 0,
        }
        .publish(env);
        return SettlementResult {
            status: SettlementStatus::Failed,
            amount: 0,
        };
    }

    // Mark and advance before external effects. A panic rolls the complete
    // transaction back, while an expected business failure is refunded.
    request.status = LpRequestStatus::Settled;
    storage::save_request(env, &request);
    storage::advance_next_to_resolve(env, id);
    let vault_client = VaultClient::new(env, &storage::vault(env));
    let result = if request.kind == LpRequestKind::Deposit {
        authorize_vault_asset_pull(env, request.amount);
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
        storage::save_request(env, &request);
        refund(env, &request);
    }
    events::LpRequestResolved {
        request_id: id,
        owner: request.owner,
        kind: request.kind,
        status: request.status,
        settled_amount: result.amount,
    }
    .publish(env);
    result
}
