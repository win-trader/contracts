#![no_std]

pub mod config_manager;
pub mod constants;
pub mod events;
pub mod oracle;
pub mod oracle_router;
pub mod position_manager;
pub mod request_router;
pub mod types;
pub mod upgrade;
pub mod vault;

use constants::{INSTANCE_BUMP, INSTANCE_THRESHOLD};
use soroban_sdk::{contractclient, Address, Env, Symbol};

pub use config_manager::{ConfigManager, ConfigManagerClient};
pub use oracle::{Oracle, OracleClient};
pub use oracle_router::{OracleRouter, OracleRouterClient};
pub use position_manager::{PositionManager, PositionManagerClient};
pub use request_router::{RequestRouter, RequestRouterClient};
pub use types::{
    AccountingSnapshot, GlobalConfig, LpConfig, LpRequest, LpRequestKind, LpRequestStatus,
    MarketConfig, MarketInfo, MarketSide, MigrationData, OracleConfig, OracleRound, PendingUpgrade,
    Position, RiskState, RoundPrice, SettlementResult, SettlementStatus,
};
pub use upgrade::{TimelockedUpgradeable, UpgradeFailure};
pub use vault::{VaultClient, VaultInterface};

/// Extend instance storage TTL to prevent archival.
pub fn bump_instance_ttl(env: &Env) {
    env.storage()
        .instance()
        .extend_ttl(INSTANCE_THRESHOLD, INSTANCE_BUMP);
}

// ---------------------------------------------------------------------------
// Access control — cross-contract role checking via ConfigManager
//
// Uses a minimal client surface to avoid coupling role checks to the full
// ConfigManager interface.
// ---------------------------------------------------------------------------

/// Minimal ConfigManager interface — only the has_role selector is needed.
#[contractclient(name = "AccessControlClient")]
pub trait AccessControlInterface {
    fn has_role(env: Env, role: Symbol, account: Address) -> bool;
}

/// Return true if `caller` holds `role` in the given ConfigManager contract.
///
/// Cross-contract auth primitive — does NOT call `require_auth` and does NOT
/// panic. Callers compose this with `caller.require_auth()` and a typed panic
/// using their own contract-local `Unauthorized` error so failures point to
/// the source contract via its error code.
pub fn has_role(env: &Env, config_manager: &Address, role: &str, caller: &Address) -> bool {
    AccessControlClient::new(env, config_manager).has_role(&Symbol::new(env, role), caller)
}
