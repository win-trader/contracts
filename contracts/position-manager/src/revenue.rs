//! Revenue routing — where new revenue dollars enter the Vault and how
//! already-vaulted dollars are sliced between LPs and the dev+staker pool.
//!
//! Two distinct callers are easy to confuse, so they get distinct entry
//! points:
//!
//! - [`recv_revenue`] — *new* dollars arriving at the Vault (Open fee from
//!   trader, escrow forfeit on Liquidation). Moves USDC PM→Vault, then
//!   slices.
//! - [`reslice_revenue`] — dollars already in the Vault (Close-time borrow
//!   fee + Funding cut against absorbed collateral). No transfer; just
//!   bumps `unclaimed_fees` by the non-LP slice.
//!
//! Both share the same slicing rule: `amount * (dev_bps + staker_bps) / BPS`
//! accrues to `unclaimed_fees`; the remainder stays in `total_assets` as the
//! LP slice. See `CONTEXT.md` ("Revenue split") for the canonical definition.

use soroban_sdk::{token::TokenClient, Address, Env};

use interfaces::VaultClient;

use crate::config_loaders;

/// Move `amount` USDC from PM into `vault`, then accrue the non-LP slice to
/// `unclaimed_fees`. Use this for fees the trader has just paid (Open fee)
/// or escrows the protocol is forfeiting to revenue on Liquidation.
///
/// No-op on `amount <= 0`.
pub fn recv_revenue(env: &Env, vault_addr: &Address, amount: i128) {
    if amount <= 0 {
        return;
    }
    let asset = config_loaders::vault_asset(env);
    let token = TokenClient::new(env, &asset);
    let contract_addr = env.current_contract_address();
    token.transfer(&contract_addr, vault_addr, &amount);

    let vault = VaultClient::new(env, vault_addr);
    accrue_non_lp_slice(env, &vault, amount);
}

/// Accrue the non-LP slice of `amount` to `unclaimed_fees` for revenue that
/// is already physically in the Vault. Used at Close to slice borrow/funding
/// fees out of the trader's absorbed collateral — the collateral has already
/// moved (via `record_absorbed_collateral`), so all we do here is the
/// LP-vs-dev+staker re-tag.
///
/// No-op on `amount <= 0`.
pub fn reslice_revenue(env: &Env, vault: &VaultClient, amount: i128) {
    if amount <= 0 {
        return;
    }
    accrue_non_lp_slice(env, vault, amount);
}

fn accrue_non_lp_slice(env: &Env, vault: &VaultClient, amount: i128) {
    let splits = config_loaders::fee_splits(env);
    let non_lp_bps = (splits.dev_bps + splits.staker_bps) as i128;
    let non_lp = amount * non_lp_bps / shared::constants::BPS;
    if non_lp > 0 {
        let contract_addr = env.current_contract_address();
        vault.accrue_fees(&contract_addr, &non_lp);
    }
}
