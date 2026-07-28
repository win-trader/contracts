use interfaces::{GlobalConfig, MarketInfo, Position};
use shared::constants::{SHARED_BUMP, SHARED_THRESHOLD};
use soroban_sdk::{contracttype, Address, Env, Symbol, Vec};

#[contracttype]
#[derive(Clone)]
pub enum Key {
    ConfigManager,
    OracleRouter,
    Vault,
    GlobalConfig,
    Initialized,
    Paused,
    NextPositionId,
    ActiveMarkets,
    Position(u64),
    Market(Symbol),
    MarketDisabled(Symbol),
    BorrowIndex,
    BorrowIndexRemainder,
    CurrentBorrowRate,
    GlobalReceiverFlow,
    GlobalReceiverRemainder,
    LastGlobalCheckpoint,
    StoredCollateralTotal,
    PendingReceiverFundingTotal,
    ExecutionBudgetTotal,
    ProtocolClaimableTotal,
    RiskKeeperReserveTotal,
    TotalRiskUnits,
    OpenPositionCount,
    LpBlockedSideCount,
}

pub fn set<T: soroban_sdk::IntoVal<Env, soroban_sdk::Val> + Clone>(
    env: &Env,
    key: &Key,
    value: &T,
) {
    env.storage().instance().set(key, value);
}

pub fn get<T: soroban_sdk::TryFromVal<Env, soroban_sdk::Val>>(env: &Env, key: &Key) -> Option<T> {
    env.storage().instance().get(key)
}

pub fn get_i128(env: &Env, key: &Key) -> i128 {
    get(env, key).unwrap_or(0)
}

pub fn get_u64(env: &Env, key: &Key) -> u64 {
    get(env, key).unwrap_or(0)
}

pub fn get_u32(env: &Env, key: &Key) -> u32 {
    get(env, key).unwrap_or(0)
}

pub fn position(env: &Env, id: u64) -> Option<Position> {
    env.storage().persistent().get(&Key::Position(id))
}

pub fn save_position(env: &Env, position: &Position) {
    let key = Key::Position(position.id);
    env.storage().persistent().set(&key, position);
    env.storage()
        .persistent()
        .extend_ttl(&key, SHARED_THRESHOLD, SHARED_BUMP);
    shared::bump_instance_ttl(env);
}

pub fn remove_position(env: &Env, id: u64) {
    env.storage().persistent().remove(&Key::Position(id));
}

pub fn market(env: &Env, symbol: &Symbol) -> Option<MarketInfo> {
    env.storage().persistent().get(&Key::Market(symbol.clone()))
}

pub fn save_market(env: &Env, symbol: &Symbol, market: &MarketInfo) {
    let key = Key::Market(symbol.clone());
    env.storage().persistent().set(&key, market);
    env.storage()
        .persistent()
        .extend_ttl(&key, SHARED_THRESHOLD, SHARED_BUMP);
}

pub fn config_manager(env: &Env) -> Address {
    get(env, &Key::ConfigManager).unwrap()
}

pub fn oracle_router(env: &Env) -> Address {
    get(env, &Key::OracleRouter).unwrap()
}

pub fn vault(env: &Env) -> Address {
    get(env, &Key::Vault).unwrap()
}

pub fn global_config(env: &Env) -> GlobalConfig {
    get(env, &Key::GlobalConfig).unwrap()
}

pub fn active_markets(env: &Env) -> Vec<Symbol> {
    get(env, &Key::ActiveMarkets).unwrap_or(Vec::new(env))
}
