use soroban_sdk::{contracttype, panic_with_error, Address, Env};
use shared::constants::{SHARED_THRESHOLD, SHARED_BUMP};

use crate::errors::VaultError;

#[contracttype]
pub enum VaultDataKey {
    Initialized,
    ConfigManager,
    PositionManager,
    ReservedUsdc,
    UnclaimedFees,
    NetGlobalTraderPnl,
    /// Ledger timestamp of the most recent full-book PM PnL sync. Partial
    /// per-market `update_net_pnl` pushes update the amount but not this
    /// timestamp, so LP exits cannot pass on a freshly-updated single market
    /// while another open market remains stale.
    LastPnlSyncTime,
    IsPaused,
    Version,
    /// Per-user lockup expiry timestamp (persistent storage). Frozen at
    /// deposit time as `now + cooldown_duration`; subsequent admin changes
    /// to `cooldown_duration` MUST NOT alter already-stored values.
    LockupExpiresAt(Address),
}

// ---------------------------------------------------------------------------
// Initialization
// ---------------------------------------------------------------------------

pub fn is_initialized(env: &Env) -> bool {
    env.storage().instance().has(&VaultDataKey::Initialized)
}

pub fn set_initialized(env: &Env) {
    env.storage().instance().set(&VaultDataKey::Initialized, &true);
}

// ---------------------------------------------------------------------------
// Config Manager
// ---------------------------------------------------------------------------

pub fn get_config_manager(env: &Env) -> Address {
    env.storage()
        .instance()
        .get(&VaultDataKey::ConfigManager)
        .unwrap_or_else(|| panic_with_error!(env, VaultError::NotInitialized))
}

pub fn set_config_manager(env: &Env, addr: &Address) {
    env.storage()
        .instance()
        .set(&VaultDataKey::ConfigManager, addr);
}

// ---------------------------------------------------------------------------
// Position Manager
// ---------------------------------------------------------------------------

pub fn get_position_manager(env: &Env) -> Address {
    env.storage()
        .instance()
        .get(&VaultDataKey::PositionManager)
        .unwrap_or_else(|| panic_with_error!(env, VaultError::NotInitialized))
}

pub fn set_position_manager(env: &Env, addr: &Address) {
    env.storage()
        .instance()
        .set(&VaultDataKey::PositionManager, addr);
}

// ---------------------------------------------------------------------------
// Reserved USDC
// ---------------------------------------------------------------------------

pub fn get_reserved_usdc(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get(&VaultDataKey::ReservedUsdc)
        .unwrap_or(0)
}

pub fn set_reserved_usdc(env: &Env, amount: i128) {
    env.storage()
        .instance()
        .set(&VaultDataKey::ReservedUsdc, &amount);
}

// ---------------------------------------------------------------------------
// Unclaimed Fees
// ---------------------------------------------------------------------------

pub fn get_unclaimed_fees(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get(&VaultDataKey::UnclaimedFees)
        .unwrap_or(0)
}

pub fn set_unclaimed_fees(env: &Env, amount: i128) {
    env.storage()
        .instance()
        .set(&VaultDataKey::UnclaimedFees, &amount);
}

// ---------------------------------------------------------------------------
// Net Global Trader PnL
// ---------------------------------------------------------------------------

pub fn get_net_global_trader_pnl(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get(&VaultDataKey::NetGlobalTraderPnl)
        .unwrap_or(0)
}

pub fn set_net_global_trader_pnl(env: &Env, pnl: i128) {
    env.storage()
        .instance()
        .set(&VaultDataKey::NetGlobalTraderPnl, &pnl);
}

pub fn get_last_pnl_sync(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get(&VaultDataKey::LastPnlSyncTime)
        .unwrap_or(0)
}

pub fn set_last_pnl_sync(env: &Env, ts: u64) {
    env.storage()
        .instance()
        .set(&VaultDataKey::LastPnlSyncTime, &ts);
}

// ---------------------------------------------------------------------------
// Pause State
// ---------------------------------------------------------------------------

pub fn get_paused(env: &Env) -> bool {
    env.storage()
        .instance()
        .get(&VaultDataKey::IsPaused)
        .unwrap_or(false)
}

pub fn set_paused(env: &Env, paused: bool) {
    env.storage()
        .instance()
        .set(&VaultDataKey::IsPaused, &paused);
}

// ---------------------------------------------------------------------------
// Version (upgrade tracking)
// ---------------------------------------------------------------------------

pub fn save_version(env: &Env, version: u32) {
    env.storage()
        .instance()
        .set(&VaultDataKey::Version, &version);
}

// Pending upgrade storage now lives in `interfaces::upgrade` under a shared
// Symbol key — used by the `TimelockedUpgradeable` trait's default methods.

// ---------------------------------------------------------------------------
// Persistent storage: LockupExpiresAt (per-user)
// ---------------------------------------------------------------------------

pub fn get_lockup_expires_at(env: &Env, user: &Address) -> Option<u64> {
    let key = VaultDataKey::LockupExpiresAt(user.clone());
    env.storage().persistent().get(&key)
}

pub fn set_lockup_expires_at(env: &Env, user: &Address, expires_at: u64) {
    let key = VaultDataKey::LockupExpiresAt(user.clone());
    env.storage().persistent().set(&key, &expires_at);
    env.storage()
        .persistent()
        .extend_ttl(&key, SHARED_THRESHOLD, SHARED_BUMP);
}
