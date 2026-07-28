use soroban_sdk::{contractclient, Address, BytesN, Env, Symbol};

/// Protocol role authority and upgrade-timelock owner.
#[contractclient(name = "ConfigManagerClient")]
pub trait ConfigManager {
    fn grant_role(env: Env, caller: Address, role: Symbol, account: Address);
    fn revoke_role(env: Env, caller: Address, role: Symbol, account: Address);
    fn has_role(env: Env, role: Symbol, account: Address) -> bool;
    fn bump_config_state(env: Env);

    fn propose_admin(env: Env, caller: Address, new_admin: Address);
    fn accept_admin(env: Env, new_admin: Address);
    fn cancel_admin_proposal(env: Env, caller: Address);
    fn get_pending_admin(env: Env) -> Option<Address>;

    fn set_upgrade_timelock(env: Env, caller: Address, seconds: u64);
    fn get_upgrade_timelock(env: Env) -> u64;
    fn propose_upgrade(env: Env, caller: Address, wasm_hash: BytesN<32>);
    fn cancel_upgrade(env: Env, caller: Address);
}
