use interfaces::{
    ConfigManagerClient, MarketInfo, MigrationData, Position, PositionManager,
    TimelockedUpgradeable, UpgradeFailure,
};
use soroban_sdk::{contract, contractimpl, panic_with_error, Address, BytesN, Env, Symbol};

use stellar_contract_utils::upgradeable::{
    complete_migration, ensure_can_complete_migration,
};

use crate::close;
use crate::errors::PositionManagerError;
use crate::events;
use crate::guards;
use crate::increase;
use crate::pnl_refresh;
use crate::storage;
use crate::tick::MarketTick;
use crate::tp_sl;

#[contract]
pub struct PositionManagerContract;

// ---------------------------------------------------------------------------
// Implementation — thin routing layer
// ---------------------------------------------------------------------------

#[contractimpl]
impl PositionManagerContract {
    /// Atomic-with-deploy initialization (Soroban constructor). Binds the
    /// linked ConfigManager and OracleRouter once, inside the deploy
    /// transaction, so init cannot be front-run. The Vault link is wired
    /// afterwards via `set_vault` — Vault binds its trusted PositionManager in
    /// its own constructor, so PM is deployed first and cannot know the Vault
    /// address until the Vault exists.
    pub fn __constructor(env: Env, config_manager: Address, oracle_router: Address) {
        storage::set_config_manager(&env, &config_manager);
        storage::set_oracle_router(&env, &oracle_router);
        storage::set_paused(&env, false);
        storage::set_initialized(&env);
        shared::bump_instance_ttl(&env);
    }
}

#[contractimpl]
impl PositionManager for PositionManagerContract {
    /// Wire the linked Vault. ADMIN-only and one-shot: reverts with
    /// `AlreadyInitialized` once the Vault is set, so the trusted Vault link
    /// is immutable after deploy. Called once by the deployer immediately
    /// after the Vault is deployed.
    fn set_vault(env: Env, caller: Address, vault_address: Address) {
        guards::require_admin(&env, &caller);
        if storage::has_vault_address(&env) {
            panic_with_error!(&env, PositionManagerError::AlreadyInitialized);
        }
        storage::set_vault_address(&env, &vault_address);
        shared::bump_instance_ttl(&env);
    }

    fn increase_position(
        env: Env,
        trader: Address,
        symbol: Symbol,
        size: i128,
        collateral: i128,
        is_long: bool,
        take_profit: i128,
        stop_loss: i128,
        acceptable_price: i128,
    ) {
        guards::require_initialized(&env);
        guards::require_not_paused(&env);
        if storage::is_market_disabled(&env, &symbol) {
            panic_with_error!(&env, PositionManagerError::MarketDisabled);
        }
        trader.require_auth();
        guards::require_positive(&env, size);
        guards::require_positive(&env, collateral);
        increase::do_increase_position(
            &env,
            &trader,
            &symbol,
            size,
            collateral,
            is_long,
            take_profit,
            stop_loss,
            acceptable_price,
        );
        shared::bump_instance_ttl(&env);
    }

    fn decrease_position(
        env: Env,
        trader: Address,
        symbol: Symbol,
        size_delta: i128,
        acceptable_price: i128,
    ) {
        guards::require_initialized(&env);
        // Intentionally no pause check — traders must always be able to close.
        trader.require_auth();
        guards::require_positive(&env, size_delta);
        close::do_decrease_position(&env, &trader, &symbol, size_delta, acceptable_price);
        shared::bump_instance_ttl(&env);
    }

    fn liquidate_position(env: Env, caller: Address, trader: Address, symbol: Symbol) {
        guards::require_initialized(&env);
        // Intentionally no pause check — liquidations must always work to prevent bad debt
        caller.require_auth();
        close::do_liquidate_position(&env, &caller, &trader, &symbol);
        shared::bump_instance_ttl(&env);
    }

    fn update_indices(env: Env, caller: Address, symbol: Symbol) {
        guards::require_initialized(&env);
        // No pause check — the keeper checkpoints accrual during pause too.
        // `MarketTick::refresh` clamps the billable window at the pause
        // boundary, so a paused checkpoint bills exactly the pre-pause
        // segment and nothing inside the pause window.
        guards::require_keeper(&env, &caller);
        let vault_addr = storage::get_vault_address(&env);
        let view = crate::vault_view::VaultView::refresh(&env, &vault_addr);
        let tick = MarketTick::refresh(&env, &symbol, &view);
        // Trade paths refresh PnL at the end of their own state machine;
        // `update_indices` has no trailing trade so it pushes the refreshed
        // PnL to the vault here explicitly.
        pnl_refresh::refresh_market_unrealized_pnl(&env, &symbol, tick.mark_price);
        shared::bump_instance_ttl(&env);
    }

    fn sync_unrealized_pnl(env: Env) {
        guards::require_initialized(&env);
        // Permissionless and no pause gate: a pure repricing of the trader
        // PnL liability from current marks. It only sharpens the Vault's
        // freshness, so anyone (keeper or a withdrawing LP) may call it.
        pnl_refresh::sync_all_unrealized_pnl(&env);
        shared::bump_instance_ttl(&env);
    }

    fn execute_order(env: Env, caller: Address, trader: Address, symbol: Symbol) {
        guards::require_initialized(&env);
        // TP/SL orders protect traders and must execute during emergencies
        caller.require_auth();
        close::do_execute_order(&env, &caller, &trader, &symbol);
        shared::bump_instance_ttl(&env);
    }

    fn set_tp_sl(env: Env, trader: Address, symbol: Symbol, take_profit: i128, stop_loss: i128) {
        guards::require_initialized(&env);
        guards::require_not_paused(&env);
        trader.require_auth();
        tp_sl::do_set_tp_sl(&env, &trader, &symbol, take_profit, stop_loss);
        shared::bump_instance_ttl(&env);
    }

    fn deleverage_position(env: Env, caller: Address, trader: Address, symbol: Symbol) {
        guards::require_initialized(&env);
        // No pause check — ADL must work during crises, like liquidations
        guards::require_keeper(&env, &caller);
        close::do_deleverage_position(&env, &trader, &symbol);
        shared::bump_instance_ttl(&env);
    }

    fn bump_position(env: Env, user_address: Address, symbol: Symbol) {
        guards::require_initialized(&env);
        // Verify position exists
        storage::get_position(&env, &user_address, &symbol)
            .unwrap_or_else(|| panic_with_error!(&env, PositionManagerError::PositionNotFound));
        storage::bump_position_ttl(&env, &user_address, &symbol);
        storage::bump_market_ttl(&env, &symbol);
        storage::bump_market_unrealized_pnl_ttl(&env, &symbol);
    }

    fn pause(env: Env, caller: Address) {
        guards::require_initialized(&env);
        guards::require_pauser(&env, &caller);
        // Idempotent: repeat-calls preserve the original pause boundary so a
        // re-pause during an emergency does not advance `last_pause_time` past
        // the real start of the pause window, which would re-open the
        // fee-accrual gap fixed by the LastPauseTime clamp.
        if storage::get_paused(&env) {
            return;
        }
        storage::set_paused(&env, true);
        storage::set_last_pause_time(&env, env.ledger().timestamp());
        events::Pause { is_paused: true, caller: caller.clone() }.publish(&env);
    }

    fn unpause(env: Env, caller: Address) {
        guards::require_initialized(&env);
        guards::require_pauser(&env, &caller);
        // Idempotent: a re-unpause must not advance `last_unpause_time` past
        // a valid `market.last_index_update`, which would drop fees that
        // should have been charged.
        if !storage::get_paused(&env) {
            return;
        }
        storage::set_paused(&env, false);
        storage::set_last_unpause_time(&env, env.ledger().timestamp());
        events::Pause { is_paused: false, caller: caller.clone() }.publish(&env);
    }

    fn set_max_leverage(env: Env, caller: Address, symbol: Symbol, max_leverage: i128) {
        guards::require_initialized(&env);
        guards::require_admin(&env, &caller);
        // MIN_LEVERAGE floor stops the admin from using
        // set_max_leverage(symbol, 1) as a silent per-market kill-switch.
        // Use disable_market for that — it emits a distinct event.
        if max_leverage < (shared::constants::MIN_LEVERAGE as i128) {
            panic_with_error!(&env, PositionManagerError::LeverageBelowFloor);
        }
        if max_leverage > shared::constants::MAX_LEVERAGE_CAP {
            panic_with_error!(&env, PositionManagerError::LeverageCapExceeded);
        }
        storage::set_max_leverage(&env, &symbol, max_leverage);
        // Register the market so `sync_unrealized_pnl` can enumerate it. This
        // is the config gate every market clears before any position opens,
        // so the registry covers every market that can carry open interest.
        storage::add_market_to_registry(&env, &symbol);
        events::SetMaxLeverage { symbol: symbol.clone(), max_leverage }.publish(&env);
        shared::bump_instance_ttl(&env);
    }

    fn disable_market(env: Env, caller: Address, symbol: Symbol) {
        guards::require_initialized(&env);
        guards::require_pauser(&env, &caller);
        storage::set_market_disabled(&env, &symbol, true);
        events::MarketDisabled { symbol: symbol.clone(), caller: caller.clone() }.publish(&env);
        shared::bump_instance_ttl(&env);
    }

    fn enable_market(env: Env, caller: Address, symbol: Symbol) {
        guards::require_initialized(&env);
        guards::require_pauser(&env, &caller);
        storage::set_market_disabled(&env, &symbol, false);
        events::MarketEnabled { symbol: symbol.clone(), caller: caller.clone() }.publish(&env);
        shared::bump_instance_ttl(&env);
    }

    fn is_market_disabled(env: Env, symbol: Symbol) -> bool {
        storage::is_market_disabled(&env, &symbol)
    }

    fn propose_upgrade(env: Env, caller: Address, wasm_hash: BytesN<32>) {
        guards::require_initialized(&env);
        <Self as TimelockedUpgradeable>::propose(&env, caller, wasm_hash);
        shared::bump_instance_ttl(&env);
    }

    fn cancel_upgrade(env: Env, caller: Address) {
        guards::require_initialized(&env);
        <Self as TimelockedUpgradeable>::cancel(&env, caller);
        shared::bump_instance_ttl(&env);
    }

    fn get_max_leverage(env: Env, symbol: Symbol) -> i128 {
        guards::require_initialized(&env);
        storage::get_max_leverage(&env, &symbol)
            .unwrap_or_else(|| panic_with_error!(&env, PositionManagerError::MarketNotConfigured))
    }

    fn get_position(env: Env, trader: Address, symbol: Symbol) -> Position {
        guards::require_initialized(&env);
        storage::get_position(&env, &trader, &symbol)
            .unwrap_or_else(|| panic_with_error!(&env, PositionManagerError::PositionNotFound))
    }

    fn get_market(env: Env, symbol: Symbol) -> MarketInfo {
        guards::require_initialized(&env);
        storage::get_market(&env, &symbol)
    }

    fn realized_pnl(env: Env) -> i128 {
        guards::require_initialized(&env);
        storage::get_realized_pnl(&env)
    }

    fn total_unrealized_pnl(env: Env) -> i128 {
        guards::require_initialized(&env);
        storage::get_total_unrealized_pnl(&env)
    }

    fn market_unrealized_pnl(env: Env, symbol: Symbol) -> i128 {
        guards::require_initialized(&env);
        storage::get_market_unrealized_pnl(&env, &symbol)
    }
}

// ---------------------------------------------------------------------------
// Upgrade / migrate entrypoints — `upgrade` delegates to the trait's
// `execute`; `migrate` keeps its OZ-driven post-upgrade migration logic.
// ---------------------------------------------------------------------------
#[contractimpl]
impl PositionManagerContract {
    pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>, operator: Address) {
        <Self as TimelockedUpgradeable>::execute(&env, operator, new_wasm_hash);
    }

    pub fn migrate(env: Env, migration_data: MigrationData, operator: Address) {
        guards::require_upgrader(&env, &operator);
        ensure_can_complete_migration(&env);
        Self::_migrate(&env, &migration_data);
        complete_migration(&env);
    }
}

impl PositionManagerContract {
    pub(crate) fn _migrate(env: &Env, data: &MigrationData) {
        storage::save_version(env, data.version);
        // Bootstrap LastPauseTime if upgrading from a version that did not
        // record pause timestamps. Predicate is naturally idempotent —
        // subsequent migrations see `last_pause_time > 0` and skip. The
        // runtime fee clamp in `MarketTick::refresh` relies on the
        // `is_paused ⟹ last_pause_time > 0` invariant established here.
        if storage::get_paused(env) && storage::get_last_pause_time(env) == 0 {
            storage::set_last_pause_time(env, env.ledger().timestamp());
        }
    }
}

// ---------------------------------------------------------------------------
// TimelockedUpgradeable impl — hooks supply the contract-specific bits.
// ---------------------------------------------------------------------------
impl TimelockedUpgradeable for PositionManagerContract {
    fn _require_proposer(env: &Env, caller: &Address) {
        guards::require_upgrader(env, caller);
    }
    fn _require_executor(env: &Env, caller: &Address) {
        guards::require_upgrader(env, caller);
    }
    fn _require_canceller(env: &Env, caller: &Address) {
        guards::require_pauser(env, caller);
    }
    fn _timelock_seconds(env: &Env) -> u64 {
        let config_mgr = storage::get_config_manager(env);
        ConfigManagerClient::new(env, &config_mgr).get_upgrade_timelock()
    }
    fn _panic_with_upgrade_error(env: &Env, err: UpgradeFailure) -> ! {
        match err {
            UpgradeFailure::NoPendingUpgrade => {
                panic_with_error!(env, PositionManagerError::NoPendingUpgrade)
            }
            UpgradeFailure::TimelockNotElapsed => {
                panic_with_error!(env, PositionManagerError::UpgradeTimelockNotElapsed)
            }
            UpgradeFailure::HashMismatch => {
                panic_with_error!(env, PositionManagerError::UpgradeHashMismatch)
            }
        }
    }
}
