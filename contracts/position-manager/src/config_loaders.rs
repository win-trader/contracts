//! Cross-contract loaders for state owned by other protocol contracts.
//! Concentrates the "what does the config look like right now" question and
//! the Vault's asset address lookup so the rest of position-manager doesn't
//! constantly re-thread `storage::get_config_manager(env)` / `ConfigManagerClient::new`.

use soroban_sdk::{Address, Env};

use interfaces::{ConfigManagerClient, VaultClient};
use shared::{BorrowRateConfig, FeeConfig, FeeSplits, ProtocolLimits};

use crate::storage;

// ---------------------------------------------------------------------------
// ConfigManager loaders
// ---------------------------------------------------------------------------

fn config_client(env: &Env) -> ConfigManagerClient<'_> {
    let addr = storage::get_config_manager(env);
    ConfigManagerClient::new(env, &addr)
}

/// Protocol risk + timing limits.
pub fn limits(env: &Env) -> ProtocolLimits {
    config_client(env).get_protocol_limits()
}

/// Revenue split (lp/dev/staker bps).
pub fn fee_splits(env: &Env) -> FeeSplits {
    config_client(env).get_fee_splits()
}

/// Execution-bounty parameters (open fee, liquidation bounty, TP/SL escrow).
pub fn fee_config(env: &Env) -> FeeConfig {
    config_client(env).get_fee_config()
}

/// Borrow + funding rate curve.
#[allow(dead_code)]
pub fn borrow_rate(env: &Env) -> BorrowRateConfig {
    config_client(env).get_borrow_rate_config()
}

// ---------------------------------------------------------------------------
// Vault loaders
// ---------------------------------------------------------------------------

/// Address of the USDC token the Vault holds. Cached only by Vault — read
/// here on demand because the asset address never changes after init and the
/// extra call is trivial.
pub fn vault_asset(env: &Env) -> Address {
    let vault_addr = storage::get_vault_address(env);
    VaultClient::new(env, &vault_addr).query_asset()
}
