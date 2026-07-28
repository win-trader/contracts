#![no_std]

pub mod config_manager;
pub mod events;
pub mod oracle;
pub mod oracle_router;
pub mod position_manager;
pub mod request_router;
pub mod types;
pub mod upgrade;
pub mod vault;

// Re-export traits and clients at crate root
pub use config_manager::{ConfigManager, ConfigManagerClient};
pub use oracle::{Oracle, OracleClient};
pub use oracle_router::{OracleRouter, OracleRouterClient};
pub use position_manager::{PositionManager, PositionManagerClient};
pub use request_router::{RequestRouter, RequestRouterClient};
pub use vault::{VaultClient, VaultInterface};

// Re-export types used in trait signatures
pub use types::{
    AccountingSnapshot, GlobalConfig, LpConfig, LpRequest, LpRequestKind, LpRequestStatus,
    MarketConfig, MarketInfo, MarketSide, MigrationData, OracleConfig, OracleRound, PendingUpgrade,
    Position, RiskState, RoundPrice, SettlementResult, SettlementStatus,
};

// Re-export the upgrade flow trait + helpers
pub use upgrade::{TimelockedUpgradeable, UpgradeFailure};
