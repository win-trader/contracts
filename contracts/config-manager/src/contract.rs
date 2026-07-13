use interfaces::{ConfigManager, MigrationData, TimelockedUpgradeable, UpgradeFailure};
use soroban_sdk::{contract, contractimpl, panic_with_error, Address, BytesN, Env, Symbol};
use stellar_contract_utils::upgradeable::{complete_migration, ensure_can_complete_migration};

use crate::{
    errors::ConfigManagerError,
    events,
    logic::{
        admin_role_symbol, bump_instance_ttl, grant_role_internal, has_role_local, init_admin,
        load_admin, require_admin_with_auth, revoke_role_internal, rotate_admin,
    },
    storage,
    types::{roles, CarryingFeeConfig, FeeConfig, FeeSplits, ProtocolLimits},
    validate::Validate,
};

/// ConfigManager-local role check. Unlike Vault/PM/OracleRouter (which
/// cross-call ConfigManager via `shared::has_role`), ConfigManager *is*
/// the role authority — so the membership read is local.
fn require_local_role(env: &Env, caller: &Address, role: &str) {
    caller.require_auth();
    let role_sym = Symbol::new(env, role);
    if !has_role_local(env, &role_sym, caller) {
        panic_with_error!(env, ConfigManagerError::Unauthorized);
    }
}

#[contract]
pub struct ConfigManagerContract;

// ---------------------------------------------------------------------------
// Contract implementation
// ---------------------------------------------------------------------------

#[contractimpl]
impl ConfigManagerContract {
    /// Atomic-with-deploy initialization (Soroban constructor). Runs exactly
    /// once, inside the deploy transaction, so there is no window in which a
    /// third party could front-run init and claim the admin role. Grants
    /// DEFAULT_ADMIN_ROLE to `admin_address` and seeds validated defaults.
    pub fn __constructor(env: Env, admin_address: Address) {
        init_admin(&env, &admin_address);
        let admin_role = admin_role_symbol(&env);
        grant_role_internal(&env, &admin_role, &admin_address, &admin_address);
        events::RoleChange {
            role: admin_role.clone(),
            account: admin_address.clone(),
            is_grant: true,
        }
        .publish(&env);

        // Set sensible defaults so PM never panics reading unconfigured
        // values. Each default passes the same `Validate` bounds as the
        // admin setters, so a drifted DEFAULT_* constant cannot ship an
        // invalid genesis config.
        let fee_splits = FeeSplits {
            lp_bps: shared::constants::DEFAULT_LP_BPS,
            dev_bps: shared::constants::DEFAULT_DEV_BPS,
            staker_bps: shared::constants::DEFAULT_STAKER_BPS,
        };
        fee_splits.validate(&env);
        storage::save_fee_splits(&env, &fee_splits);

        let fee_config = FeeConfig {
            open_fee_bps: shared::constants::DEFAULT_OPEN_FEE_BPS,
            liquidation_bounty_bps: shared::constants::DEFAULT_LIQUIDATION_BOUNTY_BPS,
            tp_sl_execution_fee: shared::constants::DEFAULT_TP_SL_EXECUTION_FEE,
        };
        fee_config.validate(&env);
        storage::save_fee_config(&env, &fee_config);

        let protocol_limits = ProtocolLimits {
            min_collateral: shared::constants::DEFAULT_MIN_COLLATERAL,
            cooldown_duration: shared::constants::DEFAULT_COOLDOWN_DURATION,
            min_position_lifetime: shared::constants::DEFAULT_MIN_POSITION_LIFETIME,
            max_utilization_ratio: shared::constants::DEFAULT_MAX_UTILIZATION_RATIO,
            adl_pnl_bps: shared::constants::DEFAULT_ADL_PNL_BPS,
            adl_utilization_bps: shared::constants::DEFAULT_ADL_UTILIZATION_BPS,
            liquidation_threshold_bps: shared::constants::DEFAULT_LIQUIDATION_THRESHOLD_BPS,
        };
        protocol_limits.validate(&env);
        storage::save_protocol_limits(&env, &protocol_limits);

        let carrying_fee_config = CarryingFeeConfig {
            base_borrow_rate_bps: shared::constants::DEFAULT_BASE_BORROW_RATE_BPS,
            slope1_bps: shared::constants::DEFAULT_SLOPE1_BPS,
            slope2_bps: shared::constants::DEFAULT_SLOPE2_BPS,
            optimal_utilization_bps: shared::constants::DEFAULT_OPTIMAL_UTILIZATION_BPS,
            max_skew_rate_bps: shared::constants::DEFAULT_MAX_SKEW_RATE_BPS,
        };
        carrying_fee_config.validate(&env);
        storage::save_carrying_fee_config(&env, &carrying_fee_config);

        // Emit the seeded defaults so off-chain indexers populate
        // `protocol_config` from ledger 0 — without these, the keeper's
        // env-var fallback would mask a partially-empty config row.
        events::FeeSplitsUpdate {
            lp_bps: fee_splits.lp_bps,
            dev_bps: fee_splits.dev_bps,
            staker_bps: fee_splits.staker_bps,
        }
        .publish(&env);
        events::FeeConfigUpdate {
            open_fee_bps: fee_config.open_fee_bps,
            liquidation_bounty_bps: fee_config.liquidation_bounty_bps,
            tp_sl_execution_fee: fee_config.tp_sl_execution_fee,
        }
        .publish(&env);
        events::LimitsUpdate {
            min_collateral: protocol_limits.min_collateral,
            cooldown_duration: protocol_limits.cooldown_duration,
            min_position_lifetime: protocol_limits.min_position_lifetime,
            max_utilization_ratio: protocol_limits.max_utilization_ratio,
            adl_pnl_bps: protocol_limits.adl_pnl_bps,
            adl_utilization_bps: protocol_limits.adl_utilization_bps,
            liquidation_threshold_bps: protocol_limits.liquidation_threshold_bps,
        }
        .publish(&env);
        events::CarryingFeeUpdate {
            base_borrow_rate_bps: carrying_fee_config.base_borrow_rate_bps,
            slope1_bps: carrying_fee_config.slope1_bps,
            slope2_bps: carrying_fee_config.slope2_bps,
            optimal_utilization_bps: carrying_fee_config.optimal_utilization_bps,
            max_skew_rate_bps: carrying_fee_config.max_skew_rate_bps,
        }
        .publish(&env);

        storage::save_upgrade_timelock(&env, shared::constants::DEFAULT_UPGRADE_TIMELOCK);
        events::UpgradeTimelockUpdate {
            timelock_seconds: shared::constants::DEFAULT_UPGRADE_TIMELOCK,
        }
        .publish(&env);

        storage::set_initialized(&env);
        bump_instance_ttl(&env);
    }
}

#[contractimpl]
impl ConfigManager for ConfigManagerContract {
    fn grant_role(env: Env, caller: Address, role: Symbol, account: Address) {
        require_admin_with_auth(&env, &caller);
        // ADMIN role is managed exclusively via propose_admin/accept_admin
        let admin_role = admin_role_symbol(&env);
        if role == admin_role {
            panic_with_error!(&env, ConfigManagerError::Unauthorized);
        }
        if grant_role_internal(&env, &role, &account, &caller) {
            events::RoleChange {
                role: role.clone(),
                account: account.clone(),
                is_grant: true,
            }
            .publish(&env);
        }
    }

    fn revoke_role(env: Env, caller: Address, role: Symbol, account: Address) {
        require_admin_with_auth(&env, &caller);
        let admin_role = admin_role_symbol(&env);
        if role == admin_role {
            panic_with_error!(&env, ConfigManagerError::Unauthorized);
        }
        if revoke_role_internal(&env, &role, &account, &caller) {
            events::RoleChange {
                role: role.clone(),
                account: account.clone(),
                is_grant: false,
            }
            .publish(&env);
        }
    }

    fn has_role(env: Env, role: Symbol, account: Address) -> bool {
        has_role_local(&env, &role, &account)
    }

    fn update_fee_splits(env: Env, caller: Address, fee_splits: FeeSplits) {
        require_admin_with_auth(&env, &caller);
        fee_splits.validate(&env);
        storage::save_fee_splits(&env, &fee_splits);
        events::FeeSplitsUpdate {
            lp_bps: fee_splits.lp_bps,
            dev_bps: fee_splits.dev_bps,
            staker_bps: fee_splits.staker_bps,
        }
        .publish(&env);
        bump_instance_ttl(&env);
    }

    fn update_protocol_limits(env: Env, caller: Address, limits: ProtocolLimits) {
        require_admin_with_auth(&env, &caller);
        limits.validate(&env);
        storage::save_protocol_limits(&env, &limits);
        events::LimitsUpdate {
            min_collateral: limits.min_collateral,
            cooldown_duration: limits.cooldown_duration,
            min_position_lifetime: limits.min_position_lifetime,
            max_utilization_ratio: limits.max_utilization_ratio,
            adl_pnl_bps: limits.adl_pnl_bps,
            adl_utilization_bps: limits.adl_utilization_bps,
            liquidation_threshold_bps: limits.liquidation_threshold_bps,
        }
        .publish(&env);
        bump_instance_ttl(&env);
    }

    fn get_protocol_limits(env: Env) -> ProtocolLimits {
        storage::load_protocol_limits(&env)
    }

    fn get_fee_splits(env: Env) -> FeeSplits {
        storage::load_fee_splits(&env)
    }

    fn set_fee_config(env: Env, caller: Address, config: FeeConfig) {
        require_admin_with_auth(&env, &caller);
        config.validate(&env);
        storage::save_fee_config(&env, &config);
        events::FeeConfigUpdate {
            open_fee_bps: config.open_fee_bps,
            liquidation_bounty_bps: config.liquidation_bounty_bps,
            tp_sl_execution_fee: config.tp_sl_execution_fee,
        }
        .publish(&env);
        bump_instance_ttl(&env);
    }

    fn get_fee_config(env: Env) -> FeeConfig {
        storage::load_fee_config(&env)
    }

    fn bump_config_state(env: Env) {
        bump_instance_ttl(&env);
    }

    fn update_carrying_fee_config(env: Env, caller: Address, config: CarryingFeeConfig) {
        require_admin_with_auth(&env, &caller);
        config.validate(&env);
        storage::save_carrying_fee_config(&env, &config);
        events::CarryingFeeUpdate {
            base_borrow_rate_bps: config.base_borrow_rate_bps,
            slope1_bps: config.slope1_bps,
            slope2_bps: config.slope2_bps,
            optimal_utilization_bps: config.optimal_utilization_bps,
            max_skew_rate_bps: config.max_skew_rate_bps,
        }
        .publish(&env);
        bump_instance_ttl(&env);
    }

    fn get_carrying_fee_config(env: Env) -> CarryingFeeConfig {
        storage::load_carrying_fee_config(&env)
    }

    fn set_upgrade_timelock(env: Env, caller: Address, seconds: u64) {
        require_admin_with_auth(&env, &caller);
        if seconds < shared::constants::MIN_UPGRADE_TIMELOCK {
            panic_with_error!(&env, ConfigManagerError::UpgradeTimelockTooShort);
        }
        if seconds > shared::constants::MAX_UPGRADE_TIMELOCK_SECS {
            panic_with_error!(&env, ConfigManagerError::UpgradeTimelockTooLong);
        }
        storage::save_upgrade_timelock(&env, seconds);
        events::UpgradeTimelockUpdate {
            timelock_seconds: seconds,
        }
        .publish(&env);
        bump_instance_ttl(&env);
    }

    fn get_upgrade_timelock(env: Env) -> u64 {
        storage::load_upgrade_timelock(&env)
    }

    fn propose_admin(env: Env, caller: Address, new_admin: Address) {
        require_admin_with_auth(&env, &caller);
        if caller == new_admin {
            panic_with_error!(&env, ConfigManagerError::InvalidAdminProposal);
        }
        storage::save_pending_admin(&env, &new_admin);
        events::AdminProposed {
            proposer: caller.clone(),
            new_admin: new_admin.clone(),
        }
        .publish(&env);
        bump_instance_ttl(&env);
    }

    fn accept_admin(env: Env, new_admin: Address) {
        new_admin.require_auth();
        let pending = storage::load_pending_admin(&env)
            .unwrap_or_else(|| panic_with_error!(&env, ConfigManagerError::NoPendingAdmin));
        if pending.admin != new_admin {
            panic_with_error!(&env, ConfigManagerError::NotPendingAdmin);
        }
        let deadline = pending
            .proposed_at
            .saturating_add(shared::constants::ADMIN_PROPOSAL_TTL_SECS);
        if env.ledger().timestamp() > deadline {
            panic_with_error!(&env, ConfigManagerError::AdminProposalExpired);
        }
        let admin_role = admin_role_symbol(&env);
        let old_admin = load_admin(&env);
        // Invariant: old_admin must hold ADMIN at this point — `initialize`
        // grants it and `grant_role`/`revoke_role` reject `role == ADMIN`.
        // Asserting catches state corruption rather than letting the
        // defensive no-op in `revoke_role_internal` emit a misleading
        // RoleChange event for an account that never held the role.
        if !has_role_local(&env, &admin_role, &old_admin) {
            panic_with_error!(&env, ConfigManagerError::Unauthorized);
        }
        revoke_role_internal(&env, &admin_role, &old_admin, &old_admin);
        grant_role_internal(&env, &admin_role, &new_admin, &new_admin);
        rotate_admin(&env, &new_admin);
        storage::clear_pending_admin(&env);
        events::RoleChange {
            role: admin_role.clone(),
            account: old_admin,
            is_grant: false,
        }
        .publish(&env);
        events::RoleChange {
            role: admin_role,
            account: new_admin,
            is_grant: true,
        }
        .publish(&env);
        bump_instance_ttl(&env);
    }

    fn cancel_admin_proposal(env: Env, caller: Address) {
        require_admin_with_auth(&env, &caller);
        storage::clear_pending_admin(&env);
        events::AdminProposalCancelled {
            canceller: caller.clone(),
        }
        .publish(&env);
        bump_instance_ttl(&env);
    }

    fn get_pending_admin(env: Env) -> Option<Address> {
        storage::load_pending_admin(&env).map(|p| p.admin)
    }

    fn propose_upgrade(env: Env, caller: Address, wasm_hash: BytesN<32>) {
        <Self as TimelockedUpgradeable>::propose(&env, caller, wasm_hash);
        bump_instance_ttl(&env);
    }

    fn cancel_upgrade(env: Env, caller: Address) {
        <Self as TimelockedUpgradeable>::cancel(&env, caller);
        bump_instance_ttl(&env);
    }
}

// ---------------------------------------------------------------------------
// Upgrade / migrate entrypoints — `upgrade` delegates to the trait's
// `execute`; `migrate` keeps its OZ-driven post-upgrade migration logic.
// ---------------------------------------------------------------------------
#[contractimpl]
impl ConfigManagerContract {
    pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>, operator: Address) {
        <Self as TimelockedUpgradeable>::execute(&env, operator, new_wasm_hash);
    }

    pub fn migrate(env: Env, migration_data: MigrationData, operator: Address) {
        require_upgrader_auth(&env, &operator);
        ensure_can_complete_migration(&env);
        Self::_migrate(&env, &migration_data);
        complete_migration(&env);
    }
}

impl ConfigManagerContract {
    pub(crate) fn _migrate(env: &Env, data: &MigrationData) {
        storage::save_version(env, data.version);
    }
}

fn require_upgrader_auth(env: &Env, operator: &Address) {
    operator.require_auth();
    let upgrader_role = Symbol::new(env, roles::UPGRADER);
    if !has_role_local(env, &upgrader_role, operator) {
        panic_with_error!(env, ConfigManagerError::Unauthorized);
    }
}

// ---------------------------------------------------------------------------
// TimelockedUpgradeable impl — hooks supply the contract-specific bits.
// ---------------------------------------------------------------------------
impl TimelockedUpgradeable for ConfigManagerContract {
    fn _require_proposer(env: &Env, caller: &Address) {
        require_local_role(env, caller, roles::UPGRADER);
    }
    fn _require_executor(env: &Env, caller: &Address) {
        require_upgrader_auth(env, caller);
    }
    fn _require_canceller(env: &Env, caller: &Address) {
        require_local_role(env, caller, roles::PAUSER);
    }
    fn _timelock_seconds(env: &Env) -> u64 {
        storage::load_upgrade_timelock(env)
    }
    fn _panic_with_upgrade_error(env: &Env, err: UpgradeFailure) -> ! {
        match err {
            UpgradeFailure::NoPendingUpgrade => {
                panic_with_error!(env, ConfigManagerError::NoPendingUpgrade)
            }
            UpgradeFailure::TimelockNotElapsed => {
                panic_with_error!(env, ConfigManagerError::UpgradeTimelockNotElapsed)
            }
            UpgradeFailure::HashMismatch => {
                panic_with_error!(env, ConfigManagerError::UpgradeHashMismatch)
            }
        }
    }
}
