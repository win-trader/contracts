//! Shared oracle contract interface.

use soroban_sdk::{contractclient, Address, BytesN, Env, Symbol};

/// SEP-40 price oracle interface.
#[contractclient(name = "OracleClient")]
pub trait Oracle {
    // Initialization is the contract `__constructor(config_manager, publisher)`
    // — atomic with deploy, closing the first-caller front-running window. Not
    // a trait method (Soroban constructors are inherent). `config_manager`
    // supplies role lookups (for `set_publisher`); `publisher` is the single
    // address authorized to push prices into THIS instance. Per-instance
    // publisher binding keeps each source's signing key independent, so one
    // compromised key cannot move every source's price in lockstep.

    /// Set the price for `symbol` (scaled by `shared::constants::PRECISION`).
    /// Authorized to the stored per-instance publisher only.
    fn set_price(env: Env, caller: Address, symbol: Symbol, price: i128);

    /// Rotate the per-instance publisher. ADMIN role (via ConfigManager) only.
    fn set_publisher(env: Env, caller: Address, new_publisher: Address);

    /// Return the stored price for `symbol`. SEP-40 compatible.
    fn get_price(env: Env, symbol: Symbol) -> i128;

    /// Return the ledger timestamp when the price was last set. SEP-40 compatible.
    fn last_update(env: Env, symbol: Symbol) -> u64;

    /// Decimal scale of the prices this source reports. The OracleRouter
    /// rejects any source whose `decimals()` differs from
    /// `shared::constants::PRICE_DECIMALS`.
    fn decimals(env: Env) -> u32;

    /// Propose a WASM upgrade. UPGRADER role only; subject to the protocol
    /// upgrade timelock and PAUSER veto, identical to the other contracts.
    fn propose_upgrade(env: Env, caller: Address, wasm_hash: BytesN<32>);

    /// PAUSER veto of a pending upgrade.
    fn cancel_upgrade(env: Env, caller: Address);
}
