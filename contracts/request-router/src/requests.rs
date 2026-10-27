use shared::{
    LpRequest, LpRequestKind, LpRequestStatus, SettlementResult, SettlementStatus, VaultClient,
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

fn deliver_or_hold(env: &Env, owner: &Address, amount: i128) {
    if amount <= 0 {
        return;
    }
    let current = env.current_contract_address();
    // A failed push is held for the owner so one LP can't block the queue head.
    let delivered = TokenClient::new(env, &storage::asset(env))
        .try_transfer(&current, owner, &amount)
        .is_ok();
    if delivered {
        return;
    }
    storage::save_claimable(env, owner, storage::claimable(env, owner) + amount);
    storage::save_claimable_total(env, storage::claimable_total(env) + amount);
    events::LpPayoutDeferred {
        owner: owner.clone(),
        amount,
    }
    .publish(env);
}

pub(crate) fn claim_lp_payout(env: &Env, owner: Address) -> i128 {
    owner.require_auth();
    let amount = storage::claimable(env, &owner);
    if amount <= 0 {
        panic_with_error!(env, RequestRouterError::InvalidAmount);
    }
    storage::save_claimable(env, &owner, 0);
    storage::save_claimable_total(env, storage::claimable_total(env) - amount);
    transfer(
        env,
        &storage::asset(env),
        &env.current_contract_address(),
        &owner,
        amount,
    );
    events::LpPayoutClaimed {
        owner,
        amount,
    }
    .publish(env);
    amount
}

fn ensure_lp_actions_open(env: &Env) {
    if !VaultClient::new(env, &storage::vault(env)).can_create_lp_request() {
        panic_with_error!(env, RequestRouterError::LpActionBlocked);
    }
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
    // The reward is fixed now, so a later config change cannot reprice a queued request (T-22).
    let reward = VaultClient::new(env, &storage::vault(env)).lp_resolve_reward();
    if assets <= reward {
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
        .lp_request_delay_seconds;
    let request = LpRequest {
        id,
        owner,
        kind: LpRequestKind::Deposit,
        amount: assets,
        reward,
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
        reward,
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
    let reward = VaultClient::new(env, &storage::vault(env)).lp_resolve_reward();
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
        .lp_request_delay_seconds;
    let request = LpRequest {
        id,
        owner,
        kind: LpRequestKind::Withdrawal,
        amount: shares,
        reward,
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
        reward,
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
    let not_ready = SettlementResult {
        status: SettlementStatus::NotReady,
        amount: 0,
        reward: 0,
    };
    if env.ledger().timestamp() < request.execute_after {
        return not_ready;
    }
    if VaultClient::new(env, &storage::vault(env)).lp_paused() {
        return not_ready;
    }

    request.status = LpRequestStatus::Settled;
    storage::save_request(env, &request);
    storage::advance_next_to_resolve(env, id);

    let vault_address = storage::vault(env);
    let vault_client = VaultClient::new(env, &vault_address);
    let current = env.current_contract_address();
    let result = if request.kind == LpRequestKind::Deposit {
        let reward = core::cmp::min(request.reward, request.amount);
        let deposit_assets = request.amount - reward;
        let asset = storage::asset(env);
        if reward > 0 {
            transfer(env, &asset, &current, &executor, reward);
        }
        authorize_vault_asset_pull(env, deposit_assets);
        let mut settled =
            vault_client.settle_deposit(&current, &request.owner, &deposit_assets);
        settled.reward = reward;
        if settled.status == SettlementStatus::Failed {
            deliver_or_hold(env, &request.owner, deposit_assets);
        }
        settled
    } else {
        let settled =
            vault_client.settle_withdrawal(
                &current,
                &request.owner,
                &request.amount,
                &executor,
                &request.reward,
            );
        match settled.status {
            SettlementStatus::Settled => {
                deliver_or_hold(env, &request.owner, settled.amount - settled.reward)
            }
            SettlementStatus::Failed => {
                transfer(env, &vault_address, &current, &request.owner, request.amount)
            }
            SettlementStatus::NotReady => {}
        }
        settled
    };

    if result.status == SettlementStatus::Failed {
        request.status = LpRequestStatus::Failed;
        storage::save_request(env, &request);
    }
    events::LpRequestResolved {
        request_id: id,
        owner: request.owner,
        kind: request.kind,
        status: request.status,
        settled_amount: result.amount,
        reward: result.reward,
    }
    .publish(env);
    result
}

/// How long a request must have been resolvable before the pause key may skip it.
pub(crate) const SKIP_GRACE_SECONDS: u64 = 86_400;

// The escape hatch for a queue head that cannot resolve (THREAT_MODEL T-02):
// the request is refunded in full and the queue moves on. It can only return
// escrow to its owner, never redirect it.
pub(crate) fn skip_head(env: &Env, caller: Address) {
    let id = storage::next_to_resolve(env);
    let mut request = storage::load_request(env, id);
    if request.status != LpRequestStatus::Pending {
        panic_with_error!(env, RequestRouterError::InvalidRequest);
    }
    if env.ledger().timestamp() < request.execute_after.saturating_add(SKIP_GRACE_SECONDS) {
        panic_with_error!(env, RequestRouterError::TooEarly);
    }
    request.status = LpRequestStatus::Failed;
    storage::save_request(env, &request);
    storage::advance_next_to_resolve(env, id);
    if request.kind == LpRequestKind::Deposit {
        deliver_or_hold(env, &request.owner, request.amount);
    } else {
        let current = env.current_contract_address();
        transfer(env, &storage::vault(env), &current, &request.owner, request.amount);
    }
    events::LpRequestResolved {
        request_id: id,
        owner: request.owner,
        kind: request.kind,
        status: request.status,
        settled_amount: 0,
        reward: 0,
    }
    .publish(env);
    events::LpRequestSkipped { request_id: id, caller }.publish(env);
}
