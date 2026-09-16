use soroban_sdk::{Address, BytesN, Env, Symbol};
use stellar_contract_utils::upgradeable::enable_migration;

use crate::events::{UpgradeCancelled, UpgradeProposed};
use crate::types::PendingUpgrade;

pub fn pending_upgrade_key(env: &Env) -> Symbol {
    Symbol::new(env, "pending_upgrade")
}

pub fn load_pending_upgrade(env: &Env) -> Option<PendingUpgrade> {
    env.storage().instance().get(&pending_upgrade_key(env))
}

pub fn save_pending_upgrade(env: &Env, pending: &PendingUpgrade) {
    env.storage()
        .instance()
        .set(&pending_upgrade_key(env), pending);
}

pub fn clear_pending_upgrade(env: &Env) {
    env.storage().instance().remove(&pending_upgrade_key(env));
}

#[derive(Copy, Clone, Debug)]
pub enum UpgradeFailure {
    NoPendingUpgrade,
    TimelockNotElapsed,
    HashMismatch,
}

pub trait TimelockedUpgradeable {
    fn _require_proposer(env: &Env, caller: &Address);

    fn _require_executor(env: &Env, caller: &Address);

    fn _require_canceller(env: &Env, caller: &Address);

    fn _timelock_seconds(env: &Env) -> u64;

    fn _panic_with_upgrade_error(env: &Env, err: UpgradeFailure) -> !;

    fn propose(env: &Env, caller: Address, wasm_hash: BytesN<32>) {
        Self::_require_proposer(env, &caller);
        let eta = env.ledger().timestamp() + Self::_timelock_seconds(env);
        let pending = PendingUpgrade {
            wasm_hash: wasm_hash.clone(),
            eta,
        };
        save_pending_upgrade(env, &pending);
        UpgradeProposed { wasm_hash, eta }.publish(env);
    }

    fn execute(env: &Env, caller: Address, new_wasm_hash: BytesN<32>) {
        Self::_require_executor(env, &caller);
        let pending = match load_pending_upgrade(env) {
            Some(p) => p,
            None => Self::_panic_with_upgrade_error(env, UpgradeFailure::NoPendingUpgrade),
        };
        if env.ledger().timestamp() < pending.eta {
            Self::_panic_with_upgrade_error(env, UpgradeFailure::TimelockNotElapsed);
        }
        if pending.wasm_hash != new_wasm_hash {
            Self::_panic_with_upgrade_error(env, UpgradeFailure::HashMismatch);
        }
        clear_pending_upgrade(env);
        enable_migration(env);
        env.deployer().update_current_contract_wasm(new_wasm_hash);
    }

    fn cancel(env: &Env, caller: Address) {
        Self::_require_canceller(env, &caller);
        clear_pending_upgrade(env);
        UpgradeCancelled { caller }.publish(env);
    }
}
