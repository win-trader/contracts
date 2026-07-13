use soroban_sdk::{Address, Env, Symbol};

use shared::FeeConfig;

use crate::{types::roles, ConfigManagerClient, ConfigManagerContract, FeeSplits, ProtocolLimits};

/// Register the contract with `admin` passed as the constructor argument.
/// The Soroban constructor runs atomically during `register`, so the returned
/// client is already fully initialized (admin granted, defaults seeded).
pub fn deploy<'a>(env: &'a Env, admin: &Address) -> ConfigManagerClient<'a> {
    let contract_id = env.register(ConfigManagerContract, (admin.clone(),));
    ConfigManagerClient::new(env, &contract_id)
}

pub fn deploy_initialized(env: &Env) -> (ConfigManagerClient<'_>, Address) {
    use soroban_sdk::testutils::Address as _;
    let admin = Address::generate(env);
    let client = deploy(env, &admin);
    (client, admin)
}

pub fn valid_limits() -> ProtocolLimits {
    ProtocolLimits {
        min_collateral: 100,
        cooldown_duration: 60,
        min_position_lifetime: 60,
        max_utilization_ratio: 8_500,
        adl_pnl_bps: 9_000,
        adl_utilization_bps: 9_500,
        liquidation_threshold_bps: 200,
    }
}

pub fn valid_splits() -> FeeSplits {
    FeeSplits {
        lp_bps: 9_000,
        dev_bps: 1_000,
        staker_bps: 0,
    }
}

pub fn valid_fee_config() -> FeeConfig {
    FeeConfig {
        open_fee_bps: 10,
        liquidation_bounty_bps: 100,
        tp_sl_execution_fee: 5_000_000,
    }
}

// ---------------------------------------------------------------------------
// Role Symbol helpers — avoids repeating Symbol::new(&env, roles::*) in tests
// ---------------------------------------------------------------------------

pub fn role_admin(env: &Env) -> Symbol {
    Symbol::new(env, roles::DEFAULT_ADMIN)
}

pub fn role_keeper(env: &Env) -> Symbol {
    Symbol::new(env, roles::KEEPER)
}

pub fn role_pauser(env: &Env) -> Symbol {
    Symbol::new(env, roles::PAUSER)
}

pub fn role_upgrader(env: &Env) -> Symbol {
    Symbol::new(env, roles::UPGRADER)
}
