#![no_std]

pub mod config_manager;
pub mod events;
pub mod oracle;
pub mod oracle_router;
pub mod position_manager;
pub mod types;
pub mod upgrade;
pub mod vault;

// Re-export traits and clients at crate root
pub use config_manager::{ConfigManager, ConfigManagerClient};
pub use oracle::{Oracle, OracleClient};
pub use oracle_router::{OracleRouter, OracleRouterClient};
pub use position_manager::{PositionManager, PositionManagerClient};
pub use vault::{VaultClient, VaultInterface};

// Re-export types used in trait signatures
pub use types::{MarketInfo, MigrationData, OracleConfig, PendingUpgrade, Position};

// Re-export the upgrade flow trait + helpers
pub use upgrade::{TimelockedUpgradeable, UpgradeFailure};

// Re-export shared types that appear in trait signatures
pub use shared::{BorrowRateConfig, FeeConfig, FeeSplits, ProtocolLimits};
