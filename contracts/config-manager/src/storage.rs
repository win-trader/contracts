use soroban_sdk::{contracttype, panic_with_error, Address, Env};

use crate::errors::ConfigManagerError;
use crate::types::{CarryingFeeConfig, FeeConfig, FeeSplits, ProtocolLimits};

/// Pending WASM upgrade is the same shape across every protocol contract —
/// re-exported here so callers can keep saying `storage::PendingUpgrade`.
pub use interfaces::PendingUpgrade;

/// Pending admin transfer awaiting `accept_admin`. The proposal timestamp
/// bounds its lifetime to `ADMIN_PROPOSAL_TTL_SECS`.
#[contracttype]
#[derive(Clone)]
pub struct PendingAdminProposal {
    pub admin: Address,
    pub proposed_at: u64,
}

#[contracttype]
pub enum StorageKey {
    /// Initialization flag — set to `true` after `initialize` succeeds.
    Initialized,
    /// Fee split configuration.
    FeeSplits,
    /// Execution-bounty and open-fee parameters.
    FeeConfig,
    /// Protocol risk and timing limits (single struct replaces four separate keys).
    ProtocolLimits,
    /// Borrow-rate curve and dominant-side skew carrying-rate ceiling.
    CarryingFeeConfig,
    /// Configurable upgrade timelock in seconds. Floor enforced at
    /// `shared::constants::MIN_UPGRADE_TIMELOCK`.
    UpgradeTimelock,
    /// Pending admin awaiting `accept_admin` — set by `propose_admin`.
    PendingAdmin,
    /// Current contract version (written by migration).
    Version,
}

// ---------------------------------------------------------------------------
// TTL constants — single source of truth lives in the `shared` crate.
// ---------------------------------------------------------------------------
pub use shared::constants::{INSTANCE_BUMP, INSTANCE_THRESHOLD, SHARED_BUMP, SHARED_THRESHOLD};

// ---------------------------------------------------------------------------
// Initialization helpers
// ---------------------------------------------------------------------------

pub fn set_initialized(env: &Env) {
    env.storage()
        .instance()
        .set(&StorageKey::Initialized, &true);
}

// ---------------------------------------------------------------------------
// PendingAdmin helpers — two-step admin transfer (propose → accept).
// ---------------------------------------------------------------------------

pub fn save_pending_admin(env: &Env, addr: &Address) {
    let proposal = PendingAdminProposal {
        admin: addr.clone(),
        proposed_at: env.ledger().timestamp(),
    };
    env.storage()
        .instance()
        .set(&StorageKey::PendingAdmin, &proposal);
}

pub fn load_pending_admin(env: &Env) -> Option<PendingAdminProposal> {
    env.storage().instance().get(&StorageKey::PendingAdmin)
}

pub fn clear_pending_admin(env: &Env) {
    env.storage().instance().remove(&StorageKey::PendingAdmin);
}

// Pending upgrade storage now lives in `interfaces::upgrade` under a shared
// Symbol key — used by the `TimelockedUpgradeable` trait's default methods.

// ---------------------------------------------------------------------------
// FeeSplits helpers
// ---------------------------------------------------------------------------

pub fn load_fee_splits(env: &Env) -> FeeSplits {
    env.storage()
        .instance()
        .get(&StorageKey::FeeSplits)
        .unwrap_or_else(|| panic_with_error!(env, ConfigManagerError::NotInitialized))
}

pub fn save_fee_splits(env: &Env, fee_splits: &FeeSplits) {
    env.storage()
        .instance()
        .set(&StorageKey::FeeSplits, fee_splits);
}

// ---------------------------------------------------------------------------
// FeeConfig helpers
// ---------------------------------------------------------------------------

pub fn load_fee_config(env: &Env) -> FeeConfig {
    env.storage()
        .instance()
        .get(&StorageKey::FeeConfig)
        .unwrap_or_else(|| panic_with_error!(env, ConfigManagerError::NotInitialized))
}

pub fn save_fee_config(env: &Env, config: &FeeConfig) {
    env.storage().instance().set(&StorageKey::FeeConfig, config);
}
// ---------------------------------------------------------------------------
// ProtocolLimits helpers
// ---------------------------------------------------------------------------

pub fn load_protocol_limits(env: &Env) -> ProtocolLimits {
    env.storage()
        .instance()
        .get(&StorageKey::ProtocolLimits)
        .unwrap_or_else(|| panic_with_error!(env, ConfigManagerError::NotInitialized))
}

pub fn save_protocol_limits(env: &Env, limits: &ProtocolLimits) {
    env.storage()
        .instance()
        .set(&StorageKey::ProtocolLimits, limits);
}

// ---------------------------------------------------------------------------
// CarryingFeeConfig helpers
// ---------------------------------------------------------------------------

pub fn load_carrying_fee_config(env: &Env) -> CarryingFeeConfig {
    env.storage()
        .instance()
        .get(&StorageKey::CarryingFeeConfig)
        .unwrap_or_else(|| panic_with_error!(env, ConfigManagerError::NotInitialized))
}

pub fn save_carrying_fee_config(env: &Env, config: &CarryingFeeConfig) {
    env.storage()
        .instance()
        .set(&StorageKey::CarryingFeeConfig, config);
}

// ---------------------------------------------------------------------------
// Upgrade timelock helpers
// ---------------------------------------------------------------------------

pub fn load_upgrade_timelock(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get(&StorageKey::UpgradeTimelock)
        .unwrap_or_else(|| panic_with_error!(env, ConfigManagerError::NotInitialized))
}

pub fn save_upgrade_timelock(env: &Env, seconds: u64) {
    env.storage()
        .instance()
        .set(&StorageKey::UpgradeTimelock, &seconds);
}

// ---------------------------------------------------------------------------
// Version helper
// ---------------------------------------------------------------------------

pub fn save_version(env: &Env, version: u32) {
    env.storage().instance().set(&StorageKey::Version, &version);
}
