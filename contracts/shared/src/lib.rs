#![no_std]

use soroban_sdk::{contractclient, contracttype, Address, Env, Symbol};

pub mod constants;

use constants::{INSTANCE_BUMP, INSTANCE_THRESHOLD};

/// Extend instance storage TTL to prevent archival.
pub fn bump_instance_ttl(env: &Env) {
    env.storage()
        .instance()
        .extend_ttl(INSTANCE_THRESHOLD, INSTANCE_BUMP);
}

// ---------------------------------------------------------------------------
// Access control — cross-contract role checking via ConfigManager
//
// Uses a minimal contractclient trait (NOT the full config-manager crate) so
// shared has zero dependency on any protocol contract, preventing circular deps.
// ---------------------------------------------------------------------------

/// Minimal ConfigManager interface — only the has_role selector is needed.
#[contractclient(name = "AccessControlClient")]
pub trait AccessControlInterface {
    fn has_role(env: Env, role: Symbol, account: Address) -> bool;
}

/// Return true if `caller` holds `role` in the given ConfigManager contract.
///
/// Cross-contract auth primitive — does NOT call `require_auth` and does NOT
/// panic. Callers compose this with `caller.require_auth()` and a typed panic
/// using their own contract-local `Unauthorized` error so failures point to
/// the source contract via its error code.
pub fn has_role(env: &Env, config_manager: &Address, role: &str, caller: &Address) -> bool {
    AccessControlClient::new(env, config_manager).has_role(&Symbol::new(env, role), caller)
}

// ---------------------------------------------------------------------------
// Protocol-wide types (single source of truth — used by ConfigManager,
// PositionManager, Vault, and tests)
// ---------------------------------------------------------------------------

/// Defines how protocol revenue is split between parties.
/// All values are in basis points (bps). Must sum to 10_000.
#[contracttype]
#[derive(Clone, Debug)]
pub struct FeeSplits {
    pub lp_bps: u32,
    pub dev_bps: u32,
    pub staker_bps: u32,
}

/// Execution-bounty and open-fee parameters charged to traders.
/// `open_fee_bps` and `liquidation_bounty_bps` are in basis points;
/// `tp_sl_execution_fee` is a flat USDC amount at PRECISION scale.
#[contracttype]
#[derive(Clone, Debug)]
pub struct FeeConfig {
    pub open_fee_bps: u32,
    pub liquidation_bounty_bps: u32,
    pub tp_sl_execution_fee: i128,
}

/// Global protocol risk and timing parameters.
#[contracttype]
#[derive(Clone, Debug)]
pub struct ProtocolLimits {
    pub min_collateral: i128,
    pub cooldown_duration: u64,
    pub min_position_lifetime: u64,
    pub max_utilization_ratio: i128,
    pub funding_cut_bps: u32,
    pub adl_pnl_bps: u32,
    pub adl_utilization_bps: u32,
    pub liquidation_threshold_bps: u32,
}

/// Borrow rate kink curve and funding rate parameters (all in basis points).
#[contracttype]
#[derive(Clone, Debug)]
pub struct BorrowRateConfig {
    pub base_borrow_rate_bps: i128,
    pub slope1_bps: i128,
    pub slope2_bps: i128,
    pub optimal_utilization_bps: i128,
    pub base_funding_rate_bps: i128,
}
