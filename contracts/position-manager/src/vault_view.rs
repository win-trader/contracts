//! `VaultView` — a snapshot of Vault state for the risk-gating computations
//! PM performs (utilization gate, ADL trigger).
//!
//! Parallels `MarketTick`: refresh-once at the top of an entrypoint, query
//! many times for derived ratios. Concentrates the choice of *which* vault
//! asset basis is safe for *which* derivation — utilization uses
//! `total_assets_excl_pnl` (mark-price-insensitive) to defend against the
//! audit C-2 feedback loop; ADL PnL ratio uses the same safe basis for the
//! denominator so an oracle wick can't bias both numerator and denominator
//! against the trader.
//!
//! Invariant relied on by `utilization_bps`: Vault enforces `unclaimed_fees +
//! reserved_usdc <= total_assets` at `accrue_fees` (see
//! `contracts/vault/src/contract.rs::accrue_fees`), so `safe_basis >=
//! reserved` whenever the protocol is solvent and `utilization_bps <= BPS`.
//!
//! Constructed in two `VaultClient` cross-call reads and consumed within the
//! same operation — never persisted.

use interfaces::VaultClient;
use shared::constants::BPS;
use soroban_sdk::{Address, Env};
use stellar_contract_utils::math::{mul_div_i128, Rounding};

use crate::math;

pub struct VaultView {
    /// USDC reserved against open Position size.
    pub reserved: i128,
    /// `total_assets - unclaimed_fees`. The mark-price-insensitive basis for
    /// utilization and ADL PnL-ratio denominators.
    pub safe_basis: i128,
}

impl VaultView {
    /// Snapshot vault state. Two reads against the linked Vault.
    pub fn refresh(env: &Env, vault_addr: &Address) -> Self {
        let vault = VaultClient::new(env, vault_addr);
        Self {
            reserved: vault.reserved_usdc(),
            safe_basis: vault.total_assets_excl_pnl(),
        }
    }

    /// Utilization in basis points against the safe (PnL-excluded) basis.
    /// Used by the borrow-rate update, the increase utilization gate, and
    /// the ADL utilization trigger — every utilization read in PM funnels
    /// through here so a future entrypoint cannot pick the wrong basis.
    pub fn utilization_bps(&self) -> i128 {
        math::calc_utilization_bps(self.reserved, self.safe_basis)
    }

    /// ADL PnL trigger ratio in basis points: `unrealized_pnl * BPS /
    /// safe_basis`, where the numerator is the live `TotalUnrealizedPnl`
    /// liability (never realized history — that is already reflected in
    /// `total_assets`). Returns 0 for zero/negative PnL or zero basis.
    /// Denominator uses the safe basis so an oracle wick cannot reduce the
    /// denominator and spuriously trigger ADL.
    pub fn adl_pnl_ratio_bps(&self, env: &Env, unrealized_pnl: i128) -> i128 {
        if self.safe_basis > 0 && unrealized_pnl > 0 {
            // mul_div widens through I256, so `unrealized_pnl * BPS` cannot
            // trap before the divide brings the ratio back into range.
            mul_div_i128(env, unrealized_pnl, BPS, self.safe_basis, Rounding::Floor)
        } else {
            0
        }
    }
}
