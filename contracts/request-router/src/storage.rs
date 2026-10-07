use shared::constants::{SHARED_BUMP, SHARED_THRESHOLD};
use shared::{LpRequest, MigrationData};
use soroban_sdk::{contracttype, panic_with_error, Address, Env, IntoVal, TryFromVal, Val};

use crate::errors::RequestRouterError;

#[contracttype]
#[derive(Clone)]
pub(crate) enum Key {
    Asset,
    Vault,
    ConfigManager,
    NextId,
    NextToResolve,
    Request(u64),
    Version,
    Claimable(Address),
    ClaimableTotal,
}

pub(crate) fn get<T: TryFromVal<Env, Val>>(env: &Env, key: &Key) -> Option<T> {
    env.storage().instance().get(key)
}

pub(crate) fn set<T: IntoVal<Env, Val> + Clone>(env: &Env, key: &Key, value: &T) {
    env.storage().instance().set(key, value);
}

pub(crate) fn asset(env: &Env) -> Address {
    get(env, &Key::Asset).unwrap()
}

pub(crate) fn vault(env: &Env) -> Address {
    get(env, &Key::Vault).unwrap()
}

pub(crate) fn config_manager(env: &Env) -> Address {
    get(env, &Key::ConfigManager).unwrap()
}

pub(crate) fn next_id(env: &Env) -> u64 {
    get(env, &Key::NextId).unwrap_or(1)
}

pub(crate) fn advance_next_id(env: &Env, id: u64) {
    set(env, &Key::NextId, &(id + 1));
}

pub(crate) fn next_to_resolve(env: &Env) -> u64 {
    get(env, &Key::NextToResolve).unwrap_or(1)
}

pub(crate) fn advance_next_to_resolve(env: &Env, id: u64) {
    set(env, &Key::NextToResolve, &(id + 1));
}

pub(crate) fn save_request(env: &Env, request: &LpRequest) {
    let key = Key::Request(request.id);
    env.storage().persistent().set(&key, request);
    env.storage()
        .persistent()
        .extend_ttl(&key, SHARED_THRESHOLD, SHARED_BUMP);
    shared::bump_instance_ttl(env);
}

pub(crate) fn load_request(env: &Env, id: u64) -> LpRequest {
    env.storage()
        .persistent()
        .get(&Key::Request(id))
        .unwrap_or_else(|| panic_with_error!(env, RequestRouterError::InvalidRequest))
}

pub(crate) fn save_version(env: &Env, data: &MigrationData) {
    set(env, &Key::Version, &data.version);
}

pub(crate) fn claimable(env: &Env, owner: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&Key::Claimable(owner.clone()))
        .unwrap_or(0)
}

pub(crate) fn save_claimable(env: &Env, owner: &Address, amount: i128) {
    let key = Key::Claimable(owner.clone());
    env.storage().persistent().set(&key, &amount);
    env.storage()
        .persistent()
        .extend_ttl(&key, SHARED_THRESHOLD, SHARED_BUMP);
    shared::bump_instance_ttl(env);
}

pub(crate) fn claimable_total(env: &Env) -> i128 {
    get(env, &Key::ClaimableTotal).unwrap_or(0)
}

pub(crate) fn save_claimable_total(env: &Env, amount: i128) {
    set(env, &Key::ClaimableTotal, &amount);
}
