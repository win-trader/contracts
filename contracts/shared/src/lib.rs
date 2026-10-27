#![no_std]

pub mod config_manager;
pub mod constants;
pub mod defaults;
pub mod events;
pub mod fixed;
pub mod market_governor;
pub mod math;
pub mod position_manager;
pub mod price_feed;
pub mod request_router;
pub mod types;
pub mod upgrade;
pub mod validation;
pub mod vault;

use constants::{INSTANCE_BUMP, INSTANCE_THRESHOLD};
use soroban_sdk::{contractclient, Address, Env, Symbol};

pub use config_manager::{ConfigManager, ConfigManagerClient};
pub use market_governor::{MarketGovernor, MarketGovernorClient};
pub use position_manager::{PositionManager, PositionManagerClient};
pub use price_feed::{Asset, PriceData, PriceFeed, PriceFeedClient, StampedPrice};
pub use request_router::{RequestRouter, RequestRouterClient};
pub use types::{
    AccountingSnapshot, ActionKind, ActionOutcome, ActionPayload, ClosePayload, DecreasePayload,
    FailureReason, FundingIndices, GlobalConfig,
    KeeperRewards, LpConfig, LpRequest, LpRequestKind, LpRequestStatus, Market, MarketConfig,
    MarketSide, MigrationData, OpenPayload, PayerSide, PendingAction,
    PendingFeesView, PendingGlobalConfig, PendingMarketConfig, PendingPriceFeed, PendingUpgrade,
    Position,
    RemainderGroup, RiskState,
    SettlementResult, SettlementStatus, Trigger, TriggerCondition,
    TriggerInstruction,
};
pub use upgrade::{TimelockedUpgradeable, UpgradeFailure};
pub use vault::{VaultClient, VaultInterface};

pub fn bump_instance_ttl(env: &Env) {
    env.storage()
        .instance()
        .extend_ttl(INSTANCE_THRESHOLD, INSTANCE_BUMP);
}

#[contractclient(name = "AccessControlClient")]
pub trait AccessControlInterface {
    fn has_role(env: Env, role: Symbol, account: Address) -> bool;
}

pub fn has_role(env: &Env, config_manager: &Address, role: &str, caller: &Address) -> bool {
    AccessControlClient::new(env, config_manager).has_role(&Symbol::new(env, role), caller)
}
