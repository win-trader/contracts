//! `TimelockedUpgradeable` — the propose / execute / cancel flow used by
//! every protocol contract. Mirrors OZ's `Upgradeable` + `UpgradeableInternal`
//! split: three explicit auth hooks (proposer/executor/canceller) plus a
//! `_timelock_seconds` hook supply the contract-specific bits; the default
//! methods own the storage layout, event emission, and ordering invariants
//! ("every proposal emits exactly one `UpgradeProposed` with `eta = now +
//! timelock`", "execute requires a matured proposal whose `wasm_hash`
//! matches the argument").
//!
//! Soroban constraint: `env.deployer().update_current_contract_wasm()` is
//! contract-local, so this is a Rust trait composed at compile time inside
//! each contract — not a separate contract. Each contract declares 3
//! one-line entrypoint shims in its `#[contractimpl]` block that delegate
//! to `<Self as TimelockedUpgradeable>::propose / execute / cancel`.

use soroban_sdk::{Address, BytesN, Env, Symbol};
use stellar_contract_utils::upgradeable::enable_migration;

use crate::events::{UpgradeCancelled, UpgradeProposed};
use crate::types::PendingUpgrade;

/// Shared storage symbol — every contract's pending upgrade lives at the
/// same key so the trait's default methods can read/write without a
/// per-contract hook. (Each contract still has its own instance storage;
/// only the key name is shared.)
pub fn pending_upgrade_key(env: &Env) -> Symbol {
    Symbol::new(env, "pending_upgrade")
}

pub fn load_pending_upgrade(env: &Env) -> Option<PendingUpgrade> {
    env.storage().instance().get(&pending_upgrade_key(env))
}

pub fn save_pending_upgrade(env: &Env, pending: &PendingUpgrade) {
    env.storage().instance().set(&pending_upgrade_key(env), pending);
}

pub fn clear_pending_upgrade(env: &Env) {
    env.storage().instance().remove(&pending_upgrade_key(env));
}

/// Failure modes the trait's default methods can hit. Each contract maps
/// these to its own typed error via `_panic_with_upgrade_error` so panic
/// codes identify the source contract.
#[derive(Copy, Clone, Debug)]
pub enum UpgradeFailure {
    NoPendingUpgrade,
    TimelockNotElapsed,
    HashMismatch,
}

/// The propose/execute/cancel surface every protocol contract exposes.
///
/// To implement: provide the four hooks (three auth checks + timelock
/// fetcher) and a panic mapper. Default methods do the rest.
pub trait TimelockedUpgradeable {
    /// Auth gate for `propose_upgrade`. Typically: caller holds UPGRADER.
    fn _require_proposer(env: &Env, caller: &Address);

    /// Auth gate for `upgrade` (consuming a matured proposal). Typically:
    /// caller holds UPGRADER.
    fn _require_executor(env: &Env, caller: &Address);

    /// Auth gate for `cancel_upgrade` (veto path). Typically: caller holds
    /// PAUSER. PAUSER veto is intentional — the protocol's emergency brake
    /// must be able to stop a malicious or accidental upgrade.
    fn _require_canceller(env: &Env, caller: &Address);

    /// Timelock duration in seconds. Typically fetched from ConfigManager.
    /// Captured at proposal time so a later admin-raised timelock cannot
    /// rush an in-flight upgrade.
    fn _timelock_seconds(env: &Env) -> u64;

    /// Map a trait-level `UpgradeFailure` to a contract-local typed panic.
    /// Diverges (never returns).
    fn _panic_with_upgrade_error(env: &Env, err: UpgradeFailure) -> !;

    // -------------------------------------------------------------------
    // Default methods — wire-protocol entrypoints delegate to these.
    // -------------------------------------------------------------------

    /// Record a new pending upgrade. Computes `eta = now + timelock`,
    /// persists `PendingUpgrade { wasm_hash, eta }`, emits `UpgradeProposed`.
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

    /// Consume a matured proposal: verify it exists, `eta` has passed, and
    /// the `wasm_hash` matches; then arm migration and swap the WASM.
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

    /// Veto a pending upgrade. Clears storage and emits `UpgradeCancelled`.
    /// Idempotent (no-op if no upgrade is pending) — matches existing
    /// `cancel_upgrade` semantics.
    fn cancel(env: &Env, caller: Address) {
        Self::_require_canceller(env, &caller);
        clear_pending_upgrade(env);
        UpgradeCancelled { caller }.publish(env);
    }
}

