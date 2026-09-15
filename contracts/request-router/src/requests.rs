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
    // §7.17 — strictly greater than the resolve reward, so a deposit always
    // has something left to convert after the executor is paid. Equal would
    // mint zero shares and fail for a reason the depositor could have been
    // told at creation.
    if assets <= VaultClient::new(env, &storage::vault(env)).lp_resolve_reward() {
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
        .lp_request_delay_seconds;
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

/// §7.17 — resolve the FIFO head.
///
/// Permissionless and paid, "exactly like every other settlement in this
/// protocol". No LP depends on a third party: because only the head is
/// resolvable, an owner queued behind others clears the queue by calling
/// this once per request ahead of theirs, collecting each reward on the way,
/// so the manual path funds itself. The reward exists to make that path
/// unnecessary, not to make it exclusive.
pub(crate) fn resolve_next(env: &Env, executor: Address) -> SettlementResult {
    executor.require_auth();
    let id = storage::next_to_resolve(env);
    let mut request = storage::load_request(env, id);
    if request.status != LpRequestStatus::Pending {
        panic_with_error!(env, RequestRouterError::InvalidRequest);
    }
    // §7.17 — the delay is a wall-clock wait, not a wait for a published
    // round. The vault prices every market from the external feed inside the
    // settling transaction, so there is no separate publication step whose
    // stalling would strand LP requests while positions keep trading.
    //
    // Not ready is a **return**, not a revert: there is no `Expired`
    // outcome for an LP request, so a premature call simply leaves the head
    // pending, changes nothing, and pays nothing.
    if env.ledger().timestamp() < request.execute_after {
        return SettlementResult {
            status: SettlementStatus::NotReady,
            amount: 0,
            reward: 0,
        };
    }

    // Mark and advance before external effects. A panic rolls the complete
    // transaction back; an expected business failure is a terminal outcome
    // that still advances the queue.
    request.status = LpRequestStatus::Settled;
    storage::save_request(env, &request);
    storage::advance_next_to_resolve(env, id);

    let vault_address = storage::vault(env);
    let vault_client = VaultClient::new(env, &vault_address);
    let current = env.current_contract_address();
    let result = if request.kind == LpRequestKind::Deposit {
        // §7.17 — a deposit's reward comes out of its **asset escrow,
        // before conversion**, so the depositor mints shares for the assets
        // that actually reach the vault and no share is created against
        // value paid to the executor. It is paid on every terminal outcome,
        // success or failure, which is why it is taken here rather than in
        // each branch.
        let reward = core::cmp::min(vault_client.lp_resolve_reward(), request.amount);
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
            // Only the un-deposited remainder goes back; the reward has
            // already left, exactly as `fail_lp_request` specifies.
            transfer(env, &asset, &current, &request.owner, deposit_assets);
        }
        settled
    } else {
        // §7.17 — a failed withdrawal pays **no** reward. Its escrow is
        // shares, not cash, and a failed withdrawal releases no assets, so
        // there is nothing to pay from; taking the reward in shares would
        // confiscate part of an LP's stake for an outcome they did not
        // cause. The executor is compensated by the deposits and successful
        // withdrawals in the same queue.
        let settled =
            vault_client.settle_withdrawal(&current, &request.owner, &request.amount, &executor);
        if settled.status == SettlementStatus::Failed {
            transfer(env, &vault_address, &current, &request.owner, request.amount);
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
