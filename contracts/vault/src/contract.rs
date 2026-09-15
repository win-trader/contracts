use shared::constants::{BPS, ROLE_ADMIN, ROLE_PAUSER, ROLE_UPGRADER};
use shared::{
    AccountingSnapshot, ConfigManagerClient, LpConfig, MigrationData,
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
use crate::{events, storage};

/// §7.17's conversion offsets. Minting divides by `marked_vault_nav + 1`:
/// as NAV falls toward zero with shares still outstanding that denominator
/// collapses, and a deposit of any size would mint an unbounded number of
/// shares. The offsets keep the arithmetic **defined**, not fair — the
/// `min_deposit_nav_factor_bps` gate is what keeps it away from that regime.
const NAV_OFFSET: i128 = 1;
const SHARE_OFFSET: i128 = shared::constants::SHARE_SCALE;

/// §12.1 — the collateral token's decimals are part of the trust boundary.
/// Every cash amount in the specification is stated at `PRICE_PRECISION` and
/// compared directly against the token balance, and §2.2 forbids a second
/// authoritative cash counter, which is what a conversion factor would
/// amount to.
const REQUIRED_ASSET_DECIMALS: u32 = shared::constants::PRICE_DECIMALS;
/// §12.1 — `7` collateral decimals plus the six-decimal share offset.
const REQUIRED_SHARE_DECIMALS: u32 = REQUIRED_ASSET_DECIMALS + 6;

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

/// §10.3.1's three LP rules, checked together. The delay's upper bound is
/// this implementation's own: an LP request may not outlive the storage
/// entry that holds it.
fn validate_config(env: &Env, config: &LpConfig) {
    if config.max_withdraw_utilization_bps > BPS as u32
        || config.min_deposit_nav_factor_bps > BPS as u32
        || config.lp_request_delay_seconds == 0
        || config.lp_request_delay_seconds > shared::constants::SHARED_BUMP_SECONDS
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
    shared::math::mul_div_floor(env, a, b, d)
        .unwrap_or_else(|| panic_with_error!(env, VaultError::ArithmeticError))
}

fn mul_div_ceil(env: &Env, a: i128, b: i128, d: i128) -> i128 {
    shared::math::mul_div_ceil(env, a, b, d)
        .unwrap_or_else(|| panic_with_error!(env, VaultError::ArithmeticError))
}

fn transfer_asset(env: &Env, from: &Address, to: &Address, amount: i128) {
    TokenClient::new(env, &asset(env)).transfer(from, to, &amount);
}

/// The position manager prices every active market from the external feed
/// inside this call, so the snapshot is synchronized by virtue of being one
/// transaction rather than by a separately published round.
fn snapshot(env: &Env, mutating: bool) -> AccountingSnapshot {
    let physical = cash(env);
    let pm = PositionManagerClient::new(env, &storage::position_manager(env));
    if mutating {
        pm.prepare_lp_snapshot(&env.current_contract_address(), &physical)
    } else {
        pm.accounting_snapshot(&physical)
    }
}

/// §7.17 — the shared prefix of both gates: the vault must not be short of
/// its own claims. A shortfall means physical cash no longer covers what has
/// already been promised, and neither minting against it nor paying out of
/// it is defensible until it is cured.
fn vault_is_short(s: &AccountingSnapshot) -> bool {
    s.cash_shortfall > 0
}

/// §5.10 `keeper_lp_resolve_reward`, read from the position manager's live
/// configuration rather than cached here: §5.10 owns every reward, and a
/// second copy would be a second thing to keep in step.
fn lp_resolve_reward(env: &Env) -> i128 {
    PositionManagerClient::new(env, &storage::position_manager(env))
        .global_config()
        .keeper_rewards
        .lp_resolve
}

fn failed(env: &Env) -> SettlementResult {
    let _ = env;
    SettlementResult {
        status: SettlementStatus::Failed,
        amount: 0,
        reward: 0,
    }
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
        // §12.1 — checked at the moment the token is wired rather than
        // discovered at the first deposit. A token with different decimals
        // would make every claim wrong by a power of ten.
        if TokenClient::new(&env, &asset(&env)).decimals() != REQUIRED_ASSET_DECIMALS
            || Vault::decimals(&env) != REQUIRED_SHARE_DECIMALS
        {
            panic_with_error!(&env, VaultError::InvalidConfig);
        }
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

    fn pull_from_allowance(env: Env, caller: Address, from: Address, amount: i128) -> bool {
        require_pm(&env, &caller);
        if amount <= 0 {
            panic_with_error!(&env, VaultError::InvalidAmount);
        }
        // The vault is the approved spender; `try_transfer_from` catches a
        // revoked/expired allowance or insufficient balance into `Err` so
        // the caller can drop the order without reverting.
        let current = env.current_contract_address();
        TokenClient::new(&env, &asset(&env))
            .try_transfer_from(&current, &from, &current, &amount)
            .is_ok()
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
    ) -> SettlementResult {
        require_router(&env, &caller);
        if assets <= 0 {
            panic_with_error!(&env, VaultError::InvalidAmount);
        }
        if storage::get::<bool>(&env, &storage::Key::Paused).unwrap_or(false) {
            return failed(&env);
        }
        let s = snapshot(&env, true);
        let supply = Base::total_supply(&env);

        // §7.17 `deposit_eligible`, and nothing else. It is a guard on the
        // conversion arithmetic, not a judgement about market conditions:
        // the depositor is already protected because marked NAV deducts
        // recognized trader profit before conversion, so depositing into a
        // vault under stress is priced rather than subsidized.
        //
        // A deposit is deliberately **not** gated on side risk state. It
        // adds LP equity and therefore lowers every side's PnL factor, which
        // is the direction the vault wants in exactly the conditions that
        // would make such a gate bind.
        let eligible = if supply == 0 {
            // No holders to dilute.
            true
        } else if s.cash_lp_equity == 0 {
            // Shares outstanding against no cash equity. Recapitalization is
            // governance's job (§15.2); this operation is not the path.
            false
        } else {
            mul_div_floor(&env, s.vault_nav, BPS, s.cash_lp_equity)
                >= storage::lp_config(&env).min_deposit_nav_factor_bps as i128
        };
        if vault_is_short(&s) || !eligible {
            return failed(&env);
        }

        let shares = mul_div_floor(
            &env,
            assets,
            supply + SHARE_OFFSET,
            s.vault_nav + NAV_OFFSET,
        );
        if shares <= 0 {
            return failed(&env);
        }
        transfer_asset(&env, &caller, &env.current_contract_address(), assets);
        Base::mint(&env, &owner, shares);
        let new_cash = cash(&env);
        PositionManagerClient::new(&env, &storage::position_manager(&env))
            .refresh_borrow_rate(&env.current_contract_address(), &new_cash);
        events::DepositSettled {
            owner,
            assets,
            shares,
            share_supply: supply + shares,
            vault_nav: s.vault_nav + assets,
        }
        .publish(&env);
        SettlementResult {
            status: SettlementStatus::Settled,
            amount: shares,
            // The router pays a deposit's reward out of its own asset
            // escrow, before conversion, so no share is minted against value
            // that went to the executor. It reports it, not the vault.
            reward: 0,
        }
    }

    fn settle_withdrawal(
        env: Env,
        caller: Address,
        owner: Address,
        shares: i128,
        executor: Address,
    ) -> SettlementResult {
        require_router(&env, &caller);
        if shares <= 0 || Base::balance(&env, &caller) < shares {
            panic_with_error!(&env, VaultError::InvalidAmount);
        }
        if storage::get::<bool>(&env, &storage::Key::Paused).unwrap_or(false) {
            return failed(&env);
        }
        let s = snapshot(&env, true);
        let supply = Base::total_supply(&env);
        let mut assets = mul_div_floor(
            &env,
            shares,
            s.vault_nav + NAV_OFFSET,
            supply + SHARE_OFFSET,
        );
        // §7.17 — in a clean terminal vault the final LP takes all residual
        // cash LP equity, so conversion rounding cannot strand ownerless
        // assets behind a supply of zero.
        let empties_supply = shares == supply;
        let clean_terminal = empties_supply
            && s.open_position_count == 0
            && s.total_risk_units == 0
            && s.non_lp_claims == 0;
        if clean_terminal {
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
        // A withdrawal removes LP equity, and LP equity is the denominator
        // of every side's PnL factor, so paying one out pushes every side
        // closer to restriction. `Warning` does not block it (§6.16.1); a
        // side actually in `ADL` or `HardCap` does.
        if vault_is_short(&s)
            || s.deleveraging_side_count > 0
            || assets > s.free_lp_capital
            || post_util > config.max_withdraw_utilization_bps as i128
            // Burning the last share while the vault still owes anything
            // would leave those claims with no equity behind them.
            || (empties_supply && !clean_terminal)
        {
            return failed(&env);
        }

        // §7.17 — the reward comes out of the assets this withdrawal
        // releases, after every capacity and health check has been satisfied
        // on the full amount. A withdrawal worth less than the reward pays
        // the executor everything it releases; it is never topped up from LP
        // equity.
        let reward = core::cmp::min(lp_resolve_reward(&env), assets);
        let to_owner = assets - reward;

        Base::burn(&env, &caller, shares);
        let current = env.current_contract_address();
        if reward > 0 {
            transfer_asset(&env, &current, &executor, reward);
        }
        if to_owner > 0 {
            transfer_asset(&env, &current, &owner, to_owner);
        }
        let new_cash = cash(&env);
        PositionManagerClient::new(&env, &storage::position_manager(&env))
            .refresh_borrow_rate(&current, &new_cash);
        events::WithdrawalSettled {
            owner,
            shares,
            assets,
            share_supply: supply - shares,
            vault_nav: s.vault_nav - assets,
        }
        .publish(&env);
        SettlementResult {
            status: SettlementStatus::Settled,
            amount: assets,
            reward,
        }
    }

    fn lp_resolve_reward(env: Env) -> i128 {
        lp_resolve_reward(&env)
    }

    fn set_lp_config(env: Env, caller: Address, config: LpConfig) {
        require_role(&env, &caller, ROLE_ADMIN);
        validate_config(&env, &config);
        storage::set(&env, &storage::Key::LpConfig, &config);
        events::LpConfigUpdated { config }.publish(&env);
    }

    fn get_lp_config(env: Env) -> LpConfig {
        storage::lp_config(&env)
    }

    fn can_create_lp_request(env: Env) -> bool {
        let physical = cash(&env);
        PositionManagerClient::new(&env, &storage::position_manager(&env))
            .can_create_lp_request(&env.current_contract_address(), &physical)
    }

    fn accounting_snapshot(env: Env) -> AccountingSnapshot {
        snapshot(&env, false)
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
        events::PauseChanged { paused: true }.publish(&env);
    }

    fn unpause(env: Env, caller: Address) {
        require_role(&env, &caller, ROLE_PAUSER);
        storage::set(&env, &storage::Key::Paused, &false);
        events::PauseChanged { paused: false }.publish(&env);
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
    fn _panic_with_upgrade_error(env: &Env, failure: UpgradeFailure) -> ! {
        match failure {
            UpgradeFailure::NoPendingUpgrade => {
                panic_with_error!(env, VaultError::UpgradeNoPending)
            }
            UpgradeFailure::TimelockNotElapsed => {
                panic_with_error!(env, VaultError::UpgradeTimelockNotElapsed)
            }
            UpgradeFailure::HashMismatch => {
                panic_with_error!(env, VaultError::UpgradeHashMismatch)
            }
        }
    }
}
