use shared::constants::{SHARED_BUMP, SHARED_THRESHOLD};
use shared::{
    GlobalConfig, Market, PendingAction, PendingGlobalConfig, PendingMarketConfig, Position,
};
use soroban_sdk::{contracttype, panic_with_error, Address, Env, Symbol, Vec};

use crate::errors::PositionManagerError;
use crate::ledger::Ledger;

#[contracttype]
#[derive(Clone)]
pub enum StorageKey {
    ConfigManager,
    PriceFeed,
    Vault,
    GlobalConfig,
    Initialized,
    Paused,
    NextPositionId,
    ActiveMarkets,
    Ledger,
    Version,
    Position(u64),
    Market(Symbol),
    MarketDisabled(Symbol),
    PendingAction(u64),
    NextActionId,
    PendingGlobalConfig,
    PendingMarketConfig(Symbol),
    ReferralCode(Symbol),
    Referrer(Address),
    ReferralBalance(Address),
    UnclaimedPayout(Address),
    PendingPriceFeed,
}

pub fn is_paused(env: &Env) -> bool {
    env.storage()
        .instance()
        .get(&StorageKey::Paused)
        .unwrap_or(false)
}

pub fn save_paused(env: &Env, paused: bool) {
    env.storage().instance().set(&StorageKey::Paused, &paused);
}

pub fn get_position(env: &Env, id: u64) -> Position {
    env.storage()
        .persistent()
        .get(&StorageKey::Position(id))
        .unwrap_or_else(|| panic_with_error!(env, PositionManagerError::PositionNotFound))
}

pub fn save_position(env: &Env, position: &Position) {
    let key = StorageKey::Position(position.id);
    env.storage().persistent().set(&key, position);
    env.storage()
        .persistent()
        .extend_ttl(&key, SHARED_THRESHOLD, SHARED_BUMP);
}

pub fn remove_position(env: &Env, id: u64) {
    env.storage().persistent().remove(&StorageKey::Position(id));
}

pub fn try_get_referral_code_owner(env: &Env, code: &Symbol) -> Option<Address> {
    env.storage()
        .persistent()
        .get(&StorageKey::ReferralCode(code.clone()))
}

pub fn save_referral_code_owner(env: &Env, code: &Symbol, owner: &Address) {
    let key = StorageKey::ReferralCode(code.clone());
    env.storage().persistent().set(&key, owner);
    env.storage()
        .persistent()
        .extend_ttl(&key, SHARED_THRESHOLD, SHARED_BUMP);
}

pub fn get_referrer(env: &Env, trader: &Address) -> Option<Address> {
    env.storage()
        .persistent()
        .get(&StorageKey::Referrer(trader.clone()))
}

pub fn save_referrer(env: &Env, trader: &Address, referrer: &Address) {
    let key = StorageKey::Referrer(trader.clone());
    env.storage().persistent().set(&key, referrer);
    env.storage()
        .persistent()
        .extend_ttl(&key, SHARED_THRESHOLD, SHARED_BUMP);
}

pub fn get_referral_balance(env: &Env, referrer: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&StorageKey::ReferralBalance(referrer.clone()))
        .unwrap_or(0)
}

pub fn save_referral_balance(env: &Env, referrer: &Address, amount: i128) {
    let key = StorageKey::ReferralBalance(referrer.clone());
    env.storage().persistent().set(&key, &amount);
    env.storage()
        .persistent()
        .extend_ttl(&key, SHARED_THRESHOLD, SHARED_BUMP);
}

pub fn get_unclaimed_payout(env: &Env, owner: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&StorageKey::UnclaimedPayout(owner.clone()))
        .unwrap_or(0)
}

pub fn save_unclaimed_payout(env: &Env, owner: &Address, amount: i128) {
    let key = StorageKey::UnclaimedPayout(owner.clone());
    env.storage().persistent().set(&key, &amount);
    env.storage()
        .persistent()
        .extend_ttl(&key, SHARED_THRESHOLD, SHARED_BUMP);
}

pub fn get_market(env: &Env, symbol: &Symbol) -> Market {
    try_get_market(env, symbol)
        .unwrap_or_else(|| panic_with_error!(env, PositionManagerError::MarketNotConfigured))
}

pub fn try_get_market(env: &Env, symbol: &Symbol) -> Option<Market> {
    env.storage()
        .persistent()
        .get(&StorageKey::Market(symbol.clone()))
}

pub fn save_market(env: &Env, symbol: &Symbol, market: &Market) {
    let key = StorageKey::Market(symbol.clone());
    env.storage().persistent().set(&key, market);
    env.storage()
        .persistent()
        .extend_ttl(&key, SHARED_THRESHOLD, SHARED_BUMP);
}

pub fn is_market_disabled(env: &Env, market: &Symbol) -> bool {
    env.storage()
        .instance()
        .get(&StorageKey::MarketDisabled(market.clone()))
        .unwrap_or(false)
}

pub fn set_market_disabled(env: &Env, market: &Symbol, disabled: bool) {
    env.storage()
        .instance()
        .set(&StorageKey::MarketDisabled(market.clone()), &disabled);
}

pub fn get_active_markets(env: &Env) -> Vec<Symbol> {
    env.storage()
        .instance()
        .get(&StorageKey::ActiveMarkets)
        .unwrap_or(Vec::new(env))
}

pub fn is_market_registered(env: &Env, market: &Symbol) -> bool {
    get_active_markets(env).iter().any(|s| s == *market)
}

pub fn save_active_markets(env: &Env, markets: &Vec<Symbol>) {
    env.storage()
        .instance()
        .set(&StorageKey::ActiveMarkets, markets);
}

pub fn get_ledger_unguarded(env: &Env) -> Ledger {
    env.storage()
        .instance()
        .get(&StorageKey::Ledger)
        .unwrap_or_else(|| panic_with_error!(env, PositionManagerError::NotInitialized))
}

pub fn get_ledger(env: &Env) -> Ledger {
    let ledger = get_ledger_unguarded(env);
    if ledger.state_version != crate::ledger::STATE_VERSION {
        panic_with_error!(env, PositionManagerError::StateVersionMismatch);
    }
    ledger
}

pub fn save_ledger(env: &Env, ledger: &Ledger) {
    env.storage().instance().set(&StorageKey::Ledger, ledger);
    shared::bump_instance_ttl(env);
}

pub fn get_config_manager(env: &Env) -> Address {
    env.storage()
        .instance()
        .get(&StorageKey::ConfigManager)
        .unwrap_or_else(|| panic_with_error!(env, PositionManagerError::NotInitialized))
}

pub fn save_config_manager(env: &Env, config_manager: &Address) {
    env.storage()
        .instance()
        .set(&StorageKey::ConfigManager, config_manager);
}

pub fn get_price_feed(env: &Env) -> Address {
    env.storage()
        .instance()
        .get(&StorageKey::PriceFeed)
        .unwrap_or_else(|| panic_with_error!(env, PositionManagerError::NotInitialized))
}

pub fn save_price_feed(env: &Env, price_feed: &Address) {
    env.storage()
        .instance()
        .set(&StorageKey::PriceFeed, price_feed);
}

pub fn get_vault(env: &Env) -> Address {
    try_get_vault(env)
        .unwrap_or_else(|| panic_with_error!(env, PositionManagerError::NotInitialized))
}

pub fn try_get_vault(env: &Env) -> Option<Address> {
    env.storage().instance().get(&StorageKey::Vault)
}

pub fn save_vault(env: &Env, vault: &Address) {
    env.storage().instance().set(&StorageKey::Vault, vault);
}

pub fn get_global_config(env: &Env) -> GlobalConfig {
    env.storage()
        .instance()
        .get(&StorageKey::GlobalConfig)
        .unwrap_or_else(|| panic_with_error!(env, PositionManagerError::NotInitialized))
}

pub fn save_global_config(env: &Env, config: &GlobalConfig) {
    env.storage()
        .instance()
        .set(&StorageKey::GlobalConfig, config);
}

pub fn get_initialized(env: &Env) -> bool {
    env.storage()
        .instance()
        .get(&StorageKey::Initialized)
        .unwrap_or_else(|| panic_with_error!(env, PositionManagerError::NotInitialized))
}

pub fn save_initialized(env: &Env) {
    env.storage()
        .instance()
        .set(&StorageKey::Initialized, &true);
}

pub fn save_version(env: &Env, version: u32) {
    env.storage().instance().set(&StorageKey::Version, &version);
}

pub fn get_next_position_id(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get(&StorageKey::NextPositionId)
        .unwrap_or(1)
}

pub fn save_next_position_id(env: &Env, id: u64) {
    env.storage()
        .instance()
        .set(&StorageKey::NextPositionId, &id);
}

pub fn update_position_id(env: &Env) {
    save_next_position_id(env, get_next_position_id(env) + 1);
}

#[allow(dead_code)]
pub fn try_get_pending_action(env: &Env, id: u64) -> Option<PendingAction> {
    env.storage()
        .persistent()
        .get(&StorageKey::PendingAction(id))
}

pub fn get_pending_action(env: &Env, id: u64) -> PendingAction {
    try_get_pending_action(env, id)
        .unwrap_or_else(|| panic_with_error!(env, PositionManagerError::ActionNotFound))
}

pub fn save_pending_action(env: &Env, action: &PendingAction) {
    let key = StorageKey::PendingAction(action.action_id);
    env.storage().persistent().set(&key, action);
    env.storage()
        .persistent()
        .extend_ttl(&key, SHARED_THRESHOLD, SHARED_BUMP);
}

pub fn remove_pending_action(env: &Env, id: u64) {
    env.storage()
        .persistent()
        .remove(&StorageKey::PendingAction(id));
}

pub fn get_next_action_id(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get(&StorageKey::NextActionId)
        .unwrap_or(1)
}

pub fn take_next_action_id(env: &Env) -> u64 {
    let id = get_next_action_id(env);
    env.storage()
        .instance()
        .set(&StorageKey::NextActionId, &(id + 1));
    id
}

pub fn try_get_pending_price_feed(env: &Env) -> Option<shared::PendingPriceFeed> {
    env.storage().instance().get(&StorageKey::PendingPriceFeed)
}

pub fn save_pending_price_feed(env: &Env, pending: &shared::PendingPriceFeed) {
    env.storage()
        .instance()
        .set(&StorageKey::PendingPriceFeed, pending);
}

pub fn clear_pending_price_feed(env: &Env) {
    env.storage().instance().remove(&StorageKey::PendingPriceFeed);
}

pub fn try_get_pending_global_config(env: &Env) -> Option<PendingGlobalConfig> {
    env.storage()
        .instance()
        .get(&StorageKey::PendingGlobalConfig)
}

pub fn save_pending_global_config(env: &Env, pending: &PendingGlobalConfig) {
    env.storage()
        .instance()
        .set(&StorageKey::PendingGlobalConfig, pending);
}

pub fn clear_pending_global_config(env: &Env) {
    env.storage()
        .instance()
        .remove(&StorageKey::PendingGlobalConfig);
}

pub fn try_get_pending_market_config(env: &Env, market: &Symbol) -> Option<PendingMarketConfig> {
    env.storage()
        .instance()
        .get(&StorageKey::PendingMarketConfig(market.clone()))
}

pub fn save_pending_market_config(env: &Env, market: &Symbol, pending: &PendingMarketConfig) {
    env.storage()
        .instance()
        .set(&StorageKey::PendingMarketConfig(market.clone()), pending);
}

pub fn clear_pending_market_config(env: &Env, market: &Symbol) {
    env.storage()
        .instance()
        .remove(&StorageKey::PendingMarketConfig(market.clone()));
}

fn extend(env: &Env, key: &StorageKey) {
    env.storage()
        .persistent()
        .extend_ttl(key, SHARED_THRESHOLD, SHARED_BUMP);
}

pub fn bump_pending_action(env: &Env, id: u64) {
    extend(env, &StorageKey::PendingAction(id));
}

pub fn bump_market(env: &Env, market: &Symbol) {
    extend(env, &StorageKey::Market(market.clone()));
}

pub fn bump_referral_code(env: &Env, code: &Symbol) {
    extend(env, &StorageKey::ReferralCode(code.clone()));
}

pub fn bump_referrer(env: &Env, trader: &Address) {
    extend(env, &StorageKey::Referrer(trader.clone()));
}

pub fn bump_referral_balance(env: &Env, referrer: &Address) {
    extend(env, &StorageKey::ReferralBalance(referrer.clone()));
}

pub fn bump_unclaimed_payout(env: &Env, owner: &Address) {
    let key = StorageKey::UnclaimedPayout(owner.clone());
    if env.storage().persistent().has(&key) {
        extend(env, &key);
    }
}
