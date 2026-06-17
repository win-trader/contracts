use soroban_sdk::{panic_with_error, token::TokenClient, Address, Env};

use interfaces::ConfigManagerClient;
use stellar_contract_utils::math::{mul_div_i128, Rounding};
use stellar_tokens::{fungible::Base, vault::Vault};

use crate::errors::VaultError;
use crate::storage;

/// Maximum age of the PM-synced `NetGlobalTraderPnl` accepted on the LP
/// exit path. `free_liquidity`'s PnL deduction is only as fresh as the last
/// `update_net_pnl` push; beyond this age, withdrawals refuse to trust it
/// while positions are open.
pub const PNL_SYNC_MAX_AGE_SECS: u64 = 900;

// ---------------------------------------------------------------------------
// Initialization guards
// ---------------------------------------------------------------------------

pub fn require_initialized(env: &Env) {
    if !storage::is_initialized(env) {
        panic_with_error!(env, VaultError::NotInitialized);
    }
}

// ---------------------------------------------------------------------------
// Pause guards
// ---------------------------------------------------------------------------

pub fn require_not_paused(env: &Env) {
    if storage::get_paused(env) {
        panic_with_error!(env, VaultError::Paused);
    }
}

// ---------------------------------------------------------------------------
// Role guards (via ConfigManager cross-contract call)
// ---------------------------------------------------------------------------

/// Cross-contract role check + per-contract panic. Panics with
/// `VaultError::Unauthorized` (code 5) on failure so the panic code
/// identifies the source contract.
fn require_role_or_panic(env: &Env, caller: &Address, role: &str) {
    caller.require_auth();
    let config_mgr = storage::get_config_manager(env);
    if !shared::has_role(env, &config_mgr, role, caller) {
        panic_with_error!(env, VaultError::Unauthorized);
    }
}

pub fn require_pauser(env: &Env, caller: &Address) {
    require_role_or_panic(env, caller, shared::constants::ROLE_PAUSER);
}

pub fn require_admin(env: &Env, caller: &Address) {
    require_role_or_panic(env, caller, shared::constants::ROLE_ADMIN);
}

pub fn require_upgrader(env: &Env, caller: &Address) {
    require_role_or_panic(env, caller, shared::constants::ROLE_UPGRADER);
}

// ---------------------------------------------------------------------------
// Lockup guard
// ---------------------------------------------------------------------------

/// Compute lockup expiry as `now + cooldown_duration` (read from ConfigManager)
/// and persist it for `user`. Emits a `Lockup` event.
pub fn record_lockup(env: &Env, user: &Address) {
    let config_mgr = storage::get_config_manager(env);
    let limits = ConfigManagerClient::new(env, &config_mgr).get_protocol_limits();
    let now = env.ledger().timestamp();
    let expires_at = now + limits.cooldown_duration;
    storage::set_lockup_expires_at(env, user, expires_at);
    crate::events::Lockup { user: user.clone(), expires_at }.publish(env);
}

/// Panics with `CooldownNotElapsed` if `now < stored_expiry`. Users without
/// a stored expiry (never deposited) bypass the check.
pub fn require_lockup_elapsed(env: &Env, user: &Address) {
    if let Some(expiry) = storage::get_lockup_expires_at(env, user) {
        let now = env.ledger().timestamp();
        if now < expiry {
            panic_with_error!(env, VaultError::CooldownNotElapsed);
        }
    }
}

/// Inherit `from`'s remaining lockup to `to` on share transfers — but only
/// when `to` held no shares before the transfer. The propagation closes the
/// deposit → transfer-to-fresh-address → withdraw cooldown bypass; existing
/// holders keep the expiry their own deposits set, so an unsolicited dust
/// transfer cannot extend another LP's lockup.
pub fn propagate_lockup_on_transfer(
    env: &Env,
    from: &Address,
    to: &Address,
    to_pre_balance: i128,
) {
    if to_pre_balance > 0 {
        return;
    }
    let from_expiry = storage::get_lockup_expires_at(env, from).unwrap_or(0);
    if from_expiry == 0 {
        return;
    }
    let now = env.ledger().timestamp();
    if from_expiry <= now {
        return;
    }
    let to_expiry = storage::get_lockup_expires_at(env, to).unwrap_or(0);
    if from_expiry > to_expiry {
        storage::set_lockup_expires_at(env, to, from_expiry);
        crate::events::Lockup { user: to.clone(), expires_at: from_expiry }.publish(env);
    }
}

/// Panics with `StalePnlSync` when positions are open (`reserved_usdc > 0`)
/// and the last `update_net_pnl` push is older than `PNL_SYNC_MAX_AGE_SECS`.
/// With nothing reserved there are no open positions, so the synced PnL is
/// irrelevant and an idle protocol never blocks LP exits.
pub fn require_fresh_pnl_sync(env: &Env) {
    if storage::get_reserved_usdc(env) == 0 {
        return;
    }
    let last = storage::get_last_pnl_sync(env);
    let now = env.ledger().timestamp();
    if now.saturating_sub(last) > PNL_SYNC_MAX_AGE_SECS {
        panic_with_error!(env, VaultError::StalePnlSync);
    }
}

// ---------------------------------------------------------------------------
// Position Manager guard
// ---------------------------------------------------------------------------

pub fn require_position_manager(env: &Env, caller: &Address) {
    caller.require_auth();
    let pm = storage::get_position_manager(env);
    if *caller != pm {
        panic_with_error!(env, VaultError::NotPositionManager);
    }
}

// ---------------------------------------------------------------------------
// Free liquidity
// ---------------------------------------------------------------------------

pub fn free_liquidity(env: &Env) -> i128 {
    // Free liquidity is the LP basis net of reserved capital, floored at zero.
    let reserved = storage::get_reserved_usdc(env);
    (lp_total_assets(env) - reserved).max(0)
}

/// Free-liquidity gate for `pay_profit`. The `amount` being paid IS realized
/// trader profit, so it extinguishes that much of the synced PnL liability —
/// the deduction is taken on `net_pnl - amount`, not the full `net_pnl`.
/// Gating on the full deduction would count the payout twice and block a
/// fully-backed winner from closing.
pub fn require_payout_liquidity(env: &Env, amount: i128) {
    let total = Vault::total_assets(env);
    let reserved = storage::get_reserved_usdc(env);
    let unclaimed_fees = storage::get_unclaimed_fees(env);
    let net_pnl = storage::get_net_global_trader_pnl(env);
    let pnl_deduction = (net_pnl - amount).max(0);
    let free = total - reserved - unclaimed_fees - pnl_deduction;
    if amount > free {
        panic_with_error!(env, VaultError::InsufficientFreeLiquidity);
    }
}

// ---------------------------------------------------------------------------
// LP-basis share conversion
// ---------------------------------------------------------------------------

/// Assets owned by LP shareholders: raw token custody minus the two non-LP
/// claims — `unclaimed_fees` (dev+staker revenue awaiting claim) and, when
/// positive, the PM-synced net trader PnL liability. `reserved_usdc` stays
/// included: reserved liquidity is LP capital backing open positions.
pub fn lp_total_assets(env: &Env) -> i128 {
    let raw = Vault::total_assets(env);
    let unclaimed_fees = storage::get_unclaimed_fees(env);
    let net_pnl = storage::get_net_global_trader_pnl(env);
    (raw - unclaimed_fees - net_pnl.max(0)).max(0)
}

/// The two ERC-4626 virtual-offset terms shared by both share conversions:
/// `(supply + 10^offset, lp_assets + 1)`. The virtual offset (decimals offset
/// 6) keeps first-depositor share-price inflation attacks unprofitable,
/// exactly as in the OZ implementation this mirrors — only the asset basis
/// differs.
fn lp_share_basis(env: &Env) -> (i128, i128) {
    let virtual_shares = 10_i128.pow(Vault::get_decimals_offset(env));
    let supply = Base::total_supply(env) + virtual_shares;
    let lp_assets = lp_total_assets(env) + 1;
    (supply, lp_assets)
}

/// Shares for `assets` against the LP basis, using the virtual-offset formula
/// `assets * (supply + 10^offset) / (lp_assets + 1)`.
pub fn assets_to_shares(env: &Env, assets: i128, rounding: Rounding) -> i128 {
    if assets < 0 {
        panic_with_error!(env, VaultError::ZeroAmount);
    }
    if assets == 0 {
        return 0;
    }
    let (supply, lp_assets) = lp_share_basis(env);
    mul_div_i128(env, assets, supply, lp_assets, rounding)
}

/// Assets for `shares` against the LP basis:
/// `shares * (lp_assets + 1) / (supply + 10^offset)`.
pub fn shares_to_assets(env: &Env, shares: i128, rounding: Rounding) -> i128 {
    if shares < 0 {
        panic_with_error!(env, VaultError::ZeroAmount);
    }
    if shares == 0 {
        return 0;
    }
    let (supply, lp_assets) = lp_share_basis(env);
    mul_div_i128(env, shares, lp_assets, supply, rounding)
}

/// Total assets minus only the fee-buffer. PnL is intentionally excluded so
/// mark-price moves cannot feed back into consumers' utilization
/// denominator (PM's utilization gate, in particular). LP withdraw/pay_profit
/// still go through `free_liquidity`, which retains the PnL deduction.
pub fn total_assets_excl_pnl(env: &Env) -> i128 {
    let total = Vault::total_assets(env);
    let unclaimed_fees = storage::get_unclaimed_fees(env);
    let v = total - unclaimed_fees;
    if v < 0 { 0 } else { v }
}

pub fn require_free_liquidity(env: &Env, amount: i128) {
    if amount > free_liquidity(env) {
        panic_with_error!(env, VaultError::InsufficientFreeLiquidity);
    }
}

// ---------------------------------------------------------------------------
// Asset transfers
// ---------------------------------------------------------------------------

pub fn transfer_asset(env: &Env, asset: &Address, from: &Address, to: &Address, amount: i128) {
    TokenClient::new(env, asset).transfer(from, to, &amount);
}
