use shared::constants::{BPS, ROLE_ADMIN, ROLE_PAUSER, ROLE_UPGRADER};
use shared::{
    AccountingSnapshot, ConfigManagerClient, LpConfig, MigrationData, OracleRound,
    PositionManagerClient, SettlementResult, SettlementStatus, TimelockedUpgradeable,
    UpgradeFailure, VaultInterface,
};
use soroban_sdk::{
    contract, contractimpl, panic_with_error, token::Client as TokenClient, Address, BytesN, Env,
    MuxedAddress, String,
};
use stellar_contract_utils::upgradeable::{complete_migration, ensure_can_complete_migration};
use stellar_tokens::{
    fungible::{Base, FungibleToken},
    vault::Vault,
};

use crate::errors::VaultError;
use crate::storage;

const VIRTUAL_ASSETS: i128 = 1;
const VIRTUAL_SHARES: i128 = 1_000_000;

#[contract]
pub struct VaultContract;

fn require_role(env: &Env, caller: &Address, role: &str) {
    caller.require_auth();
    if !shared::has_role(env, &storage::config_manager(env), role, caller) {
        panic_with_error!(env, VaultError::Unauthorized);
    }
    shared::bump_instance_ttl(env);
}

fn require_pm(env: &Env, caller: &Address) {
    caller.require_auth();
    if *caller != storage::position_manager(env) {
        panic_with_error!(env, VaultError::InvalidCaller);
    }
    shared::bump_instance_ttl(env);
}

fn require_router(env: &Env, caller: &Address) {
    caller.require_auth();
    if *caller != storage::request_router(env) {
        panic_with_error!(env, VaultError::InvalidCaller);
    }
    shared::bump_instance_ttl(env);
}

fn validate_config(env: &Env, config: &LpConfig) {
    if config.max_withdraw_utilization_bps > BPS as u32
        || config.min_deposit_nav_factor_bps > BPS as u32
        || config.lp_request_delay == 0
        || config.lp_request_delay > shared::constants::SHARED_BUMP_SECONDS
    {
        panic_with_error!(env, VaultError::InvalidConfig);
    }
}

fn asset(env: &Env) -> Address {
    Vault::query_asset(env)
}

fn cash(env: &Env) -> i128 {
    TokenClient::new(env, &asset(env)).balance(&env.current_contract_address())
}

fn mul_div_floor(env: &Env, a: i128, b: i128, d: i128) -> i128 {
    if a < 0 || b < 0 || d <= 0 {
        panic_with_error!(env, VaultError::ArithmeticError);
    }
    a.checked_mul(b)
        .and_then(|v| v.checked_div(d))
        .unwrap_or_else(|| panic_with_error!(env, VaultError::ArithmeticError))
}

fn mul_div_ceil(env: &Env, a: i128, b: i128, d: i128) -> i128 {
    if a == 0 {
        return 0;
    }
    let p = a
        .checked_mul(b)
        .unwrap_or_else(|| panic_with_error!(env, VaultError::ArithmeticError));
    p.checked_add(d - 1)
        .and_then(|v| v.checked_div(d))
        .unwrap_or_else(|| panic_with_error!(env, VaultError::ArithmeticError))
}

fn transfer_asset(env: &Env, from: &Address, to: &Address, amount: i128) {
    TokenClient::new(env, &asset(env)).transfer(from, to, &amount);
}

fn snapshot(env: &Env, round: &OracleRound, mutating: bool) -> AccountingSnapshot {
    let physical = cash(env);
    let pm = PositionManagerClient::new(env, &storage::position_manager(env));
    if mutating {
        pm.prepare_lp_snapshot(&env.current_contract_address(), round, &physical)
    } else {
        pm.accounting_snapshot(round, &physical)
    }
}

fn settlement_blocked(s: &AccountingSnapshot) -> bool {
    s.cash_shortfall > 0 || s.lp_blocked_side_count > 0
}

#[contractimpl(contracttrait)]
impl FungibleToken for VaultContract {
    type ContractType = Vault;

    fn decimals(env: &Env) -> u32 {
        Vault::decimals(env)
    }

    fn transfer(env: &Env, from: Address, to: MuxedAddress, amount: i128) {
        Base::transfer(env, &from, &to, amount);
    }

    fn transfer_from(env: &Env, spender: Address, from: Address, to: Address, amount: i128) {
        Base::transfer_from(env, &spender, &from, &to, amount);
    }
}

#[contractimpl]
impl VaultContract {
    pub fn __constructor(
        env: Env,
        asset_address: Address,
        config_manager: Address,
        position_manager: Address,
        lp_config: LpConfig,
    ) {
        validate_config(&env, &lp_config);
        Vault::set_asset(&env, asset_address);
        Vault::set_decimals_offset(&env, 6);
        Base::set_metadata(
            &env,
            Vault::decimals(&env),
            String::from_str(&env, "Stellars LP"),
            String::from_str(&env, "sLP"),
        );
        storage::set(&env, &storage::Key::ConfigManager, &config_manager);
        storage::set(&env, &storage::Key::PositionManager, &position_manager);
        storage::set(&env, &storage::Key::LpConfig, &lp_config);
        storage::set(&env, &storage::Key::Paused, &false);
        storage::set(&env, &storage::Key::Initialized, &true);
        shared::bump_instance_ttl(&env);
    }
}

#[contractimpl]
impl VaultInterface for VaultContract {
    fn set_request_router(env: Env, caller: Address, request_router: Address) {
        require_role(&env, &caller, ROLE_ADMIN);
        if storage::get::<Address>(&env, &storage::Key::RequestRouter).is_some() {
            panic_with_error!(&env, VaultError::AlreadyInitialized);
        }
        storage::set(&env, &storage::Key::RequestRouter, &request_router);
    }

    fn receive_collateral(env: Env, caller: Address, from: Address, amount: i128) {
        require_pm(&env, &caller);
        if amount <= 0 {
            panic_with_error!(&env, VaultError::InvalidAmount);
        }
        transfer_asset(&env, &from, &env.current_contract_address(), amount);
    }

    fn transfer_claim(
        env: Env,
        caller: Address,
        recipient: Address,
        amount: i128,
        claims_after: i128,
    ) {
        require_pm(&env, &caller);
        let physical = cash(&env);
        if amount <= 0 || amount > physical || claims_after < 0 || physical - amount < claims_after
        {
            panic_with_error!(&env, VaultError::InsufficientCash);
        }
        transfer_asset(&env, &env.current_contract_address(), &recipient, amount);
    }

    fn transfer_safety_claim(env: Env, caller: Address, recipient: Address, amount: i128) {
        require_pm(&env, &caller);
        if amount <= 0 || amount > cash(&env) {
            panic_with_error!(&env, VaultError::InsufficientCash);
        }
        transfer_asset(&env, &env.current_contract_address(), &recipient, amount);
    }

    fn settle_deposit(
        env: Env,
        caller: Address,
        owner: Address,
        assets: i128,
        round: OracleRound,
    ) -> SettlementResult {
        require_router(&env, &caller);
        if assets <= 0 {
            panic_with_error!(&env, VaultError::InvalidAmount);
        }
        if storage::get::<bool>(&env, &storage::Key::Paused).unwrap_or(false) {
            return SettlementResult {
                status: SettlementStatus::Failed,
                amount: 0,
            };
        }
        let s = snapshot(&env, &round, true);
        let supply = Base::total_supply(&env);
        let clean_first = s.physical_cash == 0
            && s.non_lp_claims == 0
            && s.total_risk_units == 0
            && s.open_position_count == 0
            && supply == 0;
        let config = storage::lp_config(&env);
        let later_ok = s.cash_lp_equity > 0
            && s.vault_nav > 0
            && mul_div_floor(&env, s.vault_nav, BPS, s.cash_lp_equity)
                >= config.min_deposit_nav_factor_bps as i128;
        let restores_capacity = s.cash_lp_equity.saturating_add(assets) >= s.required_risk_backing;
        if settlement_blocked(&s) || !restores_capacity || (!clean_first && !later_ok) {
            return SettlementResult {
                status: SettlementStatus::Failed,
                amount: 0,
            };
        }
        let shares = mul_div_floor(
            &env,
            assets,
            supply + VIRTUAL_SHARES,
            s.vault_nav + VIRTUAL_ASSETS,
        );
        if shares <= 0 {
            return SettlementResult {
                status: SettlementStatus::Failed,
                amount: 0,
            };
        }
        transfer_asset(&env, &caller, &env.current_contract_address(), assets);
        Base::mint(&env, &owner, shares);
        let new_cash = cash(&env);
        PositionManagerClient::new(&env, &storage::position_manager(&env))
            .refresh_borrow_rate(&env.current_contract_address(), &new_cash);
        SettlementResult {
            status: SettlementStatus::Settled,
            amount: shares,
        }
    }

    fn settle_withdrawal(
        env: Env,
        caller: Address,
        owner: Address,
        shares: i128,
        round: OracleRound,
    ) -> SettlementResult {
        require_router(&env, &caller);
        if shares <= 0 || Base::balance(&env, &caller) < shares {
            panic_with_error!(&env, VaultError::InvalidAmount);
        }
        if storage::get::<bool>(&env, &storage::Key::Paused).unwrap_or(false) {
            return SettlementResult {
                status: SettlementStatus::Failed,
                amount: 0,
            };
        }
        let s = snapshot(&env, &round, true);
        let supply = Base::total_supply(&env);
        let mut assets = mul_div_floor(
            &env,
            shares,
            s.vault_nav + VIRTUAL_ASSETS,
            supply + VIRTUAL_SHARES,
        );
        if shares == supply
            && s.open_position_count == 0
            && s.total_risk_units == 0
            && s.non_lp_claims == 0
        {
            assets = s.cash_lp_equity;
        }
        let post_equity = s.cash_lp_equity.saturating_sub(assets);
        let post_util = if s.total_risk_units == 0 {
            0
        } else if post_equity <= 0 {
            BPS + 1
        } else {
            mul_div_ceil(&env, s.total_risk_units, BPS, post_equity)
        };
        let config = storage::lp_config(&env);
        let empties_supply = shares == supply;
        let unsafe_empty = empties_supply
            && (s.open_position_count > 0 || s.total_risk_units > 0 || s.non_lp_claims > 0);
        if settlement_blocked(&s)
            || assets > s.free_lp_capital
            || post_util > config.max_withdraw_utilization_bps as i128
            || unsafe_empty
        {
            return SettlementResult {
                status: SettlementStatus::Failed,
                amount: 0,
            };
        }
        Base::burn(&env, &caller, shares);
        transfer_asset(&env, &env.current_contract_address(), &owner, assets);
        let new_cash = cash(&env);
        PositionManagerClient::new(&env, &storage::position_manager(&env))
            .refresh_borrow_rate(&env.current_contract_address(), &new_cash);
        SettlementResult {
            status: SettlementStatus::Settled,
            amount: assets,
        }
    }

    fn set_lp_config(env: Env, caller: Address, config: LpConfig) {
        require_role(&env, &caller, ROLE_ADMIN);
        validate_config(&env, &config);
        storage::set(&env, &storage::Key::LpConfig, &config);
    }

    fn get_lp_config(env: Env) -> LpConfig {
        storage::lp_config(&env)
    }

    fn can_create_lp_request(env: Env) -> bool {
        let physical = cash(&env);
        PositionManagerClient::new(&env, &storage::position_manager(&env))
            .can_create_lp_request(&env.current_contract_address(), &physical)
    }

    fn accounting_snapshot(env: Env, round: OracleRound) -> AccountingSnapshot {
        snapshot(&env, &round, false)
    }

    fn physical_cash(env: Env) -> i128 {
        cash(&env)
    }

    fn query_asset(env: Env) -> Address {
        asset(&env)
    }

    fn total_share_supply(env: Env) -> i128 {
        Base::total_supply(&env)
    }

    fn pause(env: Env, caller: Address) {
        require_role(&env, &caller, ROLE_PAUSER);
        storage::set(&env, &storage::Key::Paused, &true);
    }

    fn unpause(env: Env, caller: Address) {
        require_role(&env, &caller, ROLE_PAUSER);
        storage::set(&env, &storage::Key::Paused, &false);
    }

    fn propose_upgrade(env: Env, caller: Address, wasm_hash: BytesN<32>) {
        <Self as TimelockedUpgradeable>::propose(&env, caller, wasm_hash);
    }

    fn cancel_upgrade(env: Env, caller: Address) {
        <Self as TimelockedUpgradeable>::cancel(&env, caller);
    }

    fn bump_vault_state(env: Env) {
        shared::bump_instance_ttl(&env);
    }
}

#[contractimpl]
impl VaultContract {
    pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>, operator: Address) {
        <Self as TimelockedUpgradeable>::execute(&env, operator, new_wasm_hash);
    }

    pub fn migrate(env: Env, data: MigrationData, operator: Address) {
        require_role(&env, &operator, ROLE_UPGRADER);
        ensure_can_complete_migration(&env);
        storage::save_version(&env, data.version);
        complete_migration(&env);
    }
}

impl TimelockedUpgradeable for VaultContract {
    fn _require_proposer(env: &Env, caller: &Address) {
        require_role(env, caller, ROLE_UPGRADER);
    }
    fn _require_executor(env: &Env, caller: &Address) {
        require_role(env, caller, ROLE_UPGRADER);
    }
    fn _require_canceller(env: &Env, caller: &Address) {
        require_role(env, caller, ROLE_PAUSER);
    }
    fn _timelock_seconds(env: &Env) -> u64 {
        ConfigManagerClient::new(env, &storage::config_manager(env)).get_upgrade_timelock()
    }
    fn _panic_with_upgrade_error(env: &Env, _: UpgradeFailure) -> ! {
        panic_with_error!(env, VaultError::InvalidCaller)
    }
}
