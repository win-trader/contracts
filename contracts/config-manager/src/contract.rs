use shared::{ConfigManager, MigrationData, TimelockedUpgradeable, UpgradeFailure};
use soroban_sdk::{contract, contractimpl, panic_with_error, Address, BytesN, Env, Symbol};
use stellar_contract_utils::upgradeable::{complete_migration, ensure_can_complete_migration};

use crate::errors::ConfigManagerError;
use crate::events;
use crate::logic::{
    admin_role_symbol, bump_instance_ttl, grant_role_internal, has_role_local, init_admin,
    load_admin, require_admin_with_auth, revoke_role_internal, rotate_admin,
};
use crate::storage;
use crate::types::roles;

#[contract]
pub struct ConfigManagerContract;

fn require_local_role(env: &Env, caller: &Address, role: &str) {
    caller.require_auth();
    if !has_role_local(env, &Symbol::new(env, role), caller) {
        panic_with_error!(env, ConfigManagerError::Unauthorized);
    }
}

fn require_upgrader(env: &Env, caller: &Address) {
    require_local_role(env, caller, roles::UPGRADER);
}

#[contractimpl]
impl ConfigManagerContract {
    pub fn __constructor(env: Env, admin: Address) {
        init_admin(&env, &admin);
        let role = admin_role_symbol(&env);
        grant_role_internal(&env, &role, &admin, &admin);
        events::RoleChange {
            role,
            account: admin,
            is_grant: true,
        }
        .publish(&env);
        storage::save_upgrade_timelock(&env, shared::constants::DEFAULT_UPGRADE_TIMELOCK);
        storage::set_initialized(&env);
        bump_instance_ttl(&env);
    }
}

#[contractimpl]
impl ConfigManager for ConfigManagerContract {
    fn grant_role(env: Env, caller: Address, role: Symbol, account: Address) {
        require_admin_with_auth(&env, &caller);
        if role == admin_role_symbol(&env) {
            panic_with_error!(&env, ConfigManagerError::Unauthorized);
        }
        if grant_role_internal(&env, &role, &account, &caller) {
            events::RoleChange {
                role,
                account,
                is_grant: true,
            }
            .publish(&env);
        }
    }

    fn revoke_role(env: Env, caller: Address, role: Symbol, account: Address) {
        require_admin_with_auth(&env, &caller);
        if role == admin_role_symbol(&env) {
            panic_with_error!(&env, ConfigManagerError::Unauthorized);
        }
        if revoke_role_internal(&env, &role, &account, &caller) {
            events::RoleChange {
                role,
                account,
                is_grant: false,
            }
            .publish(&env);
        }
    }

    fn has_role(env: Env, role: Symbol, account: Address) -> bool {
        bump_instance_ttl(&env);
        has_role_local(&env, &role, &account)
    }

    fn bump_config_state(env: Env) {
        bump_instance_ttl(&env);
    }

    fn propose_admin(env: Env, caller: Address, new_admin: Address) {
        require_admin_with_auth(&env, &caller);
        if caller == new_admin {
            panic_with_error!(&env, ConfigManagerError::InvalidAdminProposal);
        }
        storage::save_pending_admin(&env, &new_admin);
        events::AdminProposed {
            proposer: caller,
            new_admin,
        }
        .publish(&env);
    }

    fn accept_admin(env: Env, new_admin: Address) {
        new_admin.require_auth();
        let pending = storage::load_pending_admin(&env)
            .unwrap_or_else(|| panic_with_error!(&env, ConfigManagerError::NoPendingAdmin));
        if pending.admin != new_admin {
            panic_with_error!(&env, ConfigManagerError::NotPendingAdmin);
        }
        if env.ledger().timestamp()
            > pending
                .proposed_at
                .saturating_add(shared::constants::ADMIN_PROPOSAL_TTL_SECS)
        {
            panic_with_error!(&env, ConfigManagerError::AdminProposalExpired);
        }
        let role = admin_role_symbol(&env);
        let old_admin = load_admin(&env);
        revoke_role_internal(&env, &role, &old_admin, &old_admin);
        grant_role_internal(&env, &role, &new_admin, &new_admin);
        rotate_admin(&env, &new_admin);
        storage::clear_pending_admin(&env);
        events::RoleChange {
            role: role.clone(),
            account: old_admin,
            is_grant: false,
        }
        .publish(&env);
        events::RoleChange {
            role,
            account: new_admin,
            is_grant: true,
        }
        .publish(&env);
    }

    fn cancel_admin_proposal(env: Env, caller: Address) {
        require_admin_with_auth(&env, &caller);
        storage::clear_pending_admin(&env);
        events::AdminProposalCancelled { canceller: caller }.publish(&env);
    }

    fn get_pending_admin(env: Env) -> Option<Address> {
        storage::load_pending_admin(&env).map(|p| p.admin)
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
    }

    fn get_upgrade_timelock(env: Env) -> u64 {
        storage::load_upgrade_timelock(&env)
    }

    fn propose_upgrade(env: Env, caller: Address, wasm_hash: BytesN<32>) {
        <Self as TimelockedUpgradeable>::propose(&env, caller, wasm_hash);
    }

    fn cancel_upgrade(env: Env, caller: Address) {
        <Self as TimelockedUpgradeable>::cancel(&env, caller);
    }
}

#[contractimpl]
impl ConfigManagerContract {
    pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>, operator: Address) {
        <Self as TimelockedUpgradeable>::execute(&env, operator, new_wasm_hash);
    }

    pub fn migrate(env: Env, data: MigrationData, operator: Address) {
        require_upgrader(&env, &operator);
        ensure_can_complete_migration(&env);
        storage::save_version(&env, data.version);
        complete_migration(&env);
    }
}

impl TimelockedUpgradeable for ConfigManagerContract {
    fn _require_proposer(env: &Env, caller: &Address) {
        require_upgrader(env, caller);
    }
    fn _require_executor(env: &Env, caller: &Address) {
        require_upgrader(env, caller);
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
