use soroban_sdk::{contracttype, Address, Env};

#[contracttype]
#[derive(Clone)]
pub struct PendingAdminProposal {
    pub admin: Address,
    pub proposed_at: u64,
}

#[contracttype]
pub enum StorageKey {
    Initialized,
    UpgradeTimelock,
    PendingAdmin,
    Version,
}

pub fn set_initialized(env: &Env) {
    env.storage()
        .instance()
        .set(&StorageKey::Initialized, &true);
}

pub fn save_pending_admin(env: &Env, addr: &Address) {
    env.storage().instance().set(
        &StorageKey::PendingAdmin,
        &PendingAdminProposal {
            admin: addr.clone(),
            proposed_at: env.ledger().timestamp(),
        },
    );
}

pub fn load_pending_admin(env: &Env) -> Option<PendingAdminProposal> {
    env.storage().instance().get(&StorageKey::PendingAdmin)
}

pub fn clear_pending_admin(env: &Env) {
    env.storage().instance().remove(&StorageKey::PendingAdmin);
}

pub fn load_upgrade_timelock(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get(&StorageKey::UpgradeTimelock)
        .unwrap_or(shared::constants::DEFAULT_UPGRADE_TIMELOCK)
}

pub fn save_upgrade_timelock(env: &Env, seconds: u64) {
    env.storage()
        .instance()
        .set(&StorageKey::UpgradeTimelock, &seconds);
}

pub fn save_version(env: &Env, version: u32) {
    env.storage().instance().set(&StorageKey::Version, &version);
}
