//! TP/SL execution-fee escrow lifecycle.
//!
//! See `CONTEXT.md` for the canonical definition of `TP/SL execution-fee
//! escrow`. The lifecycle has three observable transitions:
//!
//! 1. *Charge* on first TP or SL set (Increase or `set_tp_sl`): trader pays
//!    a flat USDC amount, attached to the Position via
//!    `Position.execution_fee_escrow`.
//! 2. *Refund* on `set_tp_sl(0, 0)`: trader gets the escrow back, Position's
//!    escrow returns to 0.
//! 3. *Settle* on Close: routed per Close kind (refund to trader, paid to
//!    Executor, forfeited to Vault, or carried on the surviving Position).
//!
//! All three live here so the lifecycle reads top-to-bottom in one file
//! instead of being recovered from three.
//!
//! Escrow is charged at most once per Position. A second TP/SL change does
//! not stack a second escrow: the predicate is "Position currently has no
//! escrow AND at least one of TP/SL is being set to a non-zero value."

use soroban_sdk::{token::TokenClient, Address, Env};

use crate::config_loaders;
use crate::revenue;
use crate::storage;
use crate::types::CloseType;

// ---------------------------------------------------------------------------
// Predicates / pure logic
// ---------------------------------------------------------------------------

/// Resolve the TP/SL values a Position *will end up with* after an Increase or
/// `set_tp_sl` call, given the existing-semantics rule that `0` means "leave
/// unchanged" on Increase. Pure; no env / storage access.
pub fn resulting_tp_sl(prior_tp: i128, prior_sl: i128, new_tp: i128, new_sl: i128) -> (i128, i128) {
    let resulting_tp = if new_tp > 0 { new_tp } else { prior_tp };
    let resulting_sl = if new_sl > 0 { new_sl } else { prior_sl };
    (resulting_tp, resulting_sl)
}

/// The signed delta to apply to a Position's `execution_fee_escrow`, given
/// the prior escrow and the resulting TP/SL after a state transition.
///
/// `+fee` when an escrow is being charged for the first time on this Position
/// (`prior_escrow == 0` and at least one of TP/SL is non-zero). `-prior_escrow`
/// when both TP and SL are being cleared and an escrow was held. `0` otherwise.
pub fn escrow_delta(
    prior_escrow: i128,
    resulting_tp: i128,
    resulting_sl: i128,
    tp_sl_execution_fee: i128,
) -> i128 {
    let has_orders = resulting_tp != 0 || resulting_sl != 0;
    if prior_escrow == 0 && has_orders {
        tp_sl_execution_fee
    } else if prior_escrow > 0 && !has_orders {
        -prior_escrow
    } else {
        0
    }
}

// ---------------------------------------------------------------------------
// Token movement
// ---------------------------------------------------------------------------

/// Apply an escrow delta computed by [`escrow_delta`]:
/// - positive: pull `delta` USDC from `trader` into PM.
/// - negative: refund `|delta|` USDC from PM to `trader`.
/// - zero: no-op.
///
/// Callers update `Position.execution_fee_escrow` themselves after this
/// returns; this function owns only the token movement.
pub fn apply_delta(env: &Env, trader: &Address, delta: i128) {
    if delta == 0 {
        return;
    }
    let asset = config_loaders::vault_asset(env);
    let token = TokenClient::new(env, &asset);
    let contract_addr = env.current_contract_address();
    if delta > 0 {
        token.transfer(trader, &contract_addr, &delta);
    } else {
        token.transfer(&contract_addr, trader, &(-delta));
    }
}

// ---------------------------------------------------------------------------
// Close-time routing
// ---------------------------------------------------------------------------

/// Settle a Position's escrow at Close. Routes per Close kind:
/// - `User` partial: carry — no-op (Caller copies the escrow onto the
///   surviving Position state).
/// - `User` (full) / `Deleverage`: refund to `trader`.
/// - `OrderExecution`: pay to `executor` (the keeper-or-anyone caller).
/// - `Liquidation`: forfeit to Vault, sliced by the Revenue split.
///
/// `escrow` is the amount held on the Position. `is_partial` is only
/// meaningful for `User`. `executor` is the caller of a permissionless
/// Close (Liquidation or OrderExecution).
pub fn settle_on_close(
    env: &Env,
    trader: &Address,
    kind: &CloseType,
    escrow: i128,
    executor: Option<&Address>,
    is_partial: bool,
) {
    if escrow <= 0 {
        return;
    }
    let asset = config_loaders::vault_asset(env);
    let token = TokenClient::new(env, &asset);
    let contract_addr = env.current_contract_address();

    match kind {
        CloseType::User if is_partial => {
            // Escrow stays attached to the surviving Position. Caller is
            // responsible for preserving `execution_fee_escrow` when it
            // writes back the partially-decremented Position.
        }
        CloseType::User | CloseType::Deleverage => {
            token.transfer(&contract_addr, trader, &escrow);
        }
        CloseType::OrderExecution => {
            if let Some(addr) = executor {
                token.transfer(&contract_addr, addr, &escrow);
            }
        }
        CloseType::Liquidation => {
            let vault_addr = storage::get_vault_address(env);
            revenue::recv_revenue(env, &vault_addr, escrow);
        }
    }
}
