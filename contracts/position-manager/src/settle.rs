//! Position settlement — doc §12 (decrease, close, liquidation) and the §14
//! hard-cap payout factor.
//!
//! `settle_close` executes the §12.2 procedure. Available value is
//! distributed in the §12.2 waterfall order — receiver-backed funding,
//! negative price PnL, LP-backed funding, borrow (all inside
//! `fees::capitalize`), then the liquidation reward, then residual trader
//! equity. Every collateral movement goes through the ledger choke point, so
//! the side aggregates and claim totals stay conserved (§18.2) without a
//! reconciliation step.

use soroban_sdk::{panic_with_error, Address, Env, Symbol};

use shared::constants::BPS;
use shared::{Market, Position, RiskState, VaultClient};

use crate::errors::PositionManagerError;
use crate::fees::{self, CollectedFees};
use crate::funding;
use crate::ledger::{self, Ledger};
use crate::risk;
use crate::events::FeeSource;
use crate::{events, math, storage};

/// §11.5 — the exposure a decrease removes, pro-rata by size; the final
/// close removes the complete remainder so nothing strands (§7.1).
struct RemovedExposure {
    full: bool,
    new_size: i128,
    base_after: i128,
    risk_after: i128,
    base_removed: i128,
    risk_removed: i128,
}

fn removed_exposure(env: &Env, position: &Position, size_removed: i128) -> RemovedExposure {
    let full = size_removed == position.size;
    let new_size = math::sub(env, position.size, size_removed);
    let base_after = math::remaining(env, position.base_exposure, position.size, new_size);
    let risk_after = math::remaining(env, position.risk_units, position.size, new_size);
    RemovedExposure {
        full,
        new_size,
        base_after,
        risk_after,
        base_removed: math::sub(env, position.base_exposure, base_after),
        risk_removed: math::sub(env, position.risk_units, risk_after),
    }
}

/// §14 — positive price PnL after the hard-cap payout factor. Below the
/// hard-cap state the raw PnL passes through; at hard cap every profitable
/// position on the side is scaled by the same factor from one price
/// snapshot. Negative PnL always passes through.
pub fn payable_price_pnl(
    env: &Env,
    ledger: &Ledger,
    position: &Position,
    market: &Market,
    size: i128,
    base: i128,
    price: i128,
    physical_cash: i128,
) -> i128 {
    let raw = math::pnl(env, position.is_long, size, base, price);
    if raw <= 0 {
        return raw;
    }
    let side = market.side(position.is_long);
    if side.risk_state != RiskState::HardCap {
        return raw;
    }
    let side_positive = core::cmp::max(
        math::pnl(
            env,
            position.is_long,
            side.size_open_interest,
            side.base_exposure,
            price,
        ),
        0,
    );
    if side_positive == 0 {
        return 0;
    }
    let hard_cap_value = math::mul_div_floor(
        env,
        ledger.cash_lp_equity(env, physical_cash),
        market.config.hard_cap_pnl_factor_bps as i128,
        BPS,
    );
    math::mul_div_floor(
        env,
        raw,
        core::cmp::min(hard_cap_value, side_positive),
        side_positive,
    )
}

/// Everything one settlement moved — consumed by the entry points to emit
/// the action's event and pay keeper rewards.
#[derive(Clone, Debug)]
pub struct CloseSummary {
    pub position_id: u64,
    pub owner: Address,
    pub market: Symbol,
    pub closed: bool,
    pub size_removed: i128,
    pub price: i128,
    /// Signed raw price PnL on the removed exposure (§12.2).
    pub raw_pnl: i128,
    /// Positive PnL actually credited after the §14 payout factor.
    pub payable_pnl: i128,
    pub fees: CollectedFees,
    /// §11.1 closing fee collected out of the realized winnings.
    pub closing_fee: i128,
    /// Partial close: realized profit transferred to the owner.
    pub realized_payout: i128,
    /// Partial close: explicit collateral withdrawal transferred.
    pub collateral_withdrawn: i128,
    /// Full close: residual collateral transferred to the owner.
    pub collateral_payout: i128,
    /// Full close: accrued obligations the position could not cover.
    pub bad_debt: i128,
    pub liquidation_reward: i128,
    pub execution_budget_refunded: i128,
}

fn transfer_safety(env: &Env, recipient: &Address, amount: i128) {
    VaultClient::new(env, &storage::vault(env)).transfer_safety_claim(
        &env.current_contract_address(),
        recipient,
        &amount,
    );
}

/// §12.2 — settle a decrease, close, liquidation, ADL, or triggered order.
/// The caller must have checkpointed global state and the market to `now`.
/// Saves the market and position storage; the caller saves the ledger.
#[allow(clippy::too_many_arguments)]
pub fn settle_close(
    env: &Env,
    ledger: &mut Ledger,
    mut position: Position,
    mut market: Market,
    size_removed: i128,
    collateral_withdrawn: i128,
    price: i128,
    reward_recipient: Option<&Address>,
) -> CloseSummary {
    // One physical-cash reading covers every pre-transfer step; the §14 risk
    // evaluation and hard-cap factor below observe the same instant.
    let physical = ledger::physical_cash(env);
    let equity = ledger.cash_lp_equity(env, physical);
    risk::evaluate_market_risk(env, ledger, &position.market, &mut market, price, equity);

    // §12.2 steps 4-6: removed exposure, raw PnL, emergency payout factor.
    let removed = removed_exposure(env, &position, size_removed);
    let raw_pnl = math::pnl(
        env,
        position.is_long,
        size_removed,
        removed.base_removed,
        price,
    );
    let payable = core::cmp::max(
        payable_price_pnl(
            env,
            ledger,
            &position,
            &market,
            size_removed,
            removed.base_removed,
            price,
            physical,
        ),
        0,
    );
    let negative = core::cmp::max(-raw_pnl, 0);

    // §12.2 steps 7-9: positive PnL supplies value to the waterfall, then
    // capitalization settles credits and obligations in the §11.4 order.
    if payable > 0 {
        let is_long = position.is_long;
        ledger::add_stored_collateral(
            env,
            ledger,
            &mut position,
            market.side_mut(is_long),
            payable,
        );
    }
    let collected = fees::capitalize(env, ledger, &mut position, &mut market, negative);
    if collected.unpaid > 0 && !removed.full {
        panic_with_error!(env, PositionManagerError::InsufficientCollateral);
    }

    // §11.1 — the closing fee comes only out of realized price winnings:
    // `min(size × tier, payable)`, ranked below every waterfall obligation.
    // Losers pay nothing, so there is no shortfall path.
    let mut closing_fee = 0i128;
    if payable > 0 {
        let bps = fees::tiered_close_fee_bps(env, &market, position.is_long, removed.base_removed);
        let is_long = position.is_long;
        closing_fee = ledger::collect_stored_collateral(
            env,
            ledger,
            &mut position,
            market.side_mut(is_long),
            core::cmp::min(math::closing_fee(env, size_removed, bps), payable),
        );
        fees::split_revenue(env, ledger, closing_fee, FeeSource::Closing, position.id);
    }

    // Partial close: transfer the remaining realized profit to the trader.
    let mut realized_payout = 0i128;
    if !removed.full && payable > closing_fee {
        let is_long = position.is_long;
        realized_payout = core::cmp::min(
            math::sub(env, payable, closing_fee),
            position.stored_collateral,
        );
        if realized_payout > 0 {
            ledger::collect_stored_collateral(
                env,
                ledger,
                &mut position,
                market.side_mut(is_long),
                realized_payout,
            );
            transfer_safety(env, &position.owner, realized_payout);
        }
    }

    // §12.2 step 12: reduce the exposure aggregates (collateral aggregates
    // were kept in sync by the choke point above).
    {
        let is_long = position.is_long;
        let side = market.side_mut(is_long);
        side.size_open_interest = math::sub(env, side.size_open_interest, size_removed);
        side.base_exposure = math::sub(env, side.base_exposure, removed.base_removed);
        side.risk_units = math::sub(env, side.risk_units, removed.risk_removed);
    }
    ledger.total_risk_units = math::sub(env, ledger.total_risk_units, removed.risk_removed);

    let mut collateral_payout = 0i128;
    let mut liquidation_reward = 0i128;
    let mut budget_refund = 0i128;
    let mut bad_debt = 0i128;
    if removed.full {
        // §12.2 full-close waterfall tail: liquidation reward after borrow,
        // then residual trader equity, then the execution-budget refund.
        if collected.unpaid > 0 {
            bad_debt = collected.unpaid;
            events::BadDebt {
                position_id: position.id,
                amount: bad_debt,
            }
            .publish(env);
        }
        if let Some(liquidator) = reward_recipient {
            let is_long = position.is_long;
            let reward = core::cmp::min(
                position.stored_collateral,
                math::mul_div_floor(
                    env,
                    size_removed,
                    market.config.liquidation_reward_bps as i128,
                    BPS,
                ),
            );
            if reward > 0 {
                ledger::collect_stored_collateral(
                    env,
                    ledger,
                    &mut position,
                    market.side_mut(is_long),
                    reward,
                );
                transfer_safety(env, liquidator, reward);
                liquidation_reward = reward;
            }
        }
        {
            let is_long = position.is_long;
            let payout = position.stored_collateral;
            if payout > 0 {
                ledger::collect_stored_collateral(
                    env,
                    ledger,
                    &mut position,
                    market.side_mut(is_long),
                    payout,
                );
                transfer_safety(env, &position.owner, payout);
                collateral_payout = payout;
            }
        }
        if position.execution_budget > 0 {
            budget_refund = position.execution_budget;
            ledger.execution_budget_total =
                math::sub(env, ledger.execution_budget_total, budget_refund);
            transfer_safety(env, &position.owner, budget_refund);
        }
        storage::remove_position(env, position.id);
        ledger.open_position_count = ledger.open_position_count.saturating_sub(1);
    } else {
        // §12.2 partial close: explicit withdrawal, resize, health check,
        // baseline reset (§11.5: no historical debt carries forward).
        if collateral_withdrawn > position.stored_collateral {
            panic_with_error!(env, PositionManagerError::InsufficientCollateral);
        }
        if collateral_withdrawn > 0 {
            let is_long = position.is_long;
            ledger::collect_stored_collateral(
                env,
                ledger,
                &mut position,
                market.side_mut(is_long),
                collateral_withdrawn,
            );
            if size_removed > 0 {
                transfer_safety(env, &position.owner, collateral_withdrawn);
            } else {
                VaultClient::new(env, &storage::vault(env)).transfer_claim(
                    &env.current_contract_address(),
                    &position.owner,
                    &collateral_withdrawn,
                    &ledger.non_lp_claims(env),
                );
            }
        }
        position.size = removed.new_size;
        position.base_exposure = removed.base_after;
        position.risk_units = removed.risk_after;
        let health = math::add(
            env,
            position.stored_collateral,
            math::pnl(
                env,
                position.is_long,
                removed.new_size,
                removed.base_after,
                price,
            ),
        );
        // Shrinking the position de-risks, so the maintenance floor is
        // enough; a pure collateral withdrawal raises leverage and must
        // leave the position back above the initial margin (§12.3).
        let required = if size_removed > 0 {
            risk::maintenance_requirement(env, removed.new_size, &market.config)
        } else {
            risk::initial_requirement(env, removed.new_size, &market.config)
        };
        if health < required {
            panic_with_error!(env, PositionManagerError::InsufficientCollateral);
        }
        funding::reset_debts(env, ledger, &mut position, &market);
        storage::save_position(env, &position);
    }

    // §10.3 steps 5-7: new flows, fresh risk evaluation against the
    // post-transfer balance, new borrow rate.
    funding::recompute_market_flow(env, ledger, &mut market);
    let physical_after = ledger::physical_cash(env);
    let equity_after = ledger.cash_lp_equity(env, physical_after);
    risk::evaluate_market_risk(env, ledger, &position.market, &mut market, price, equity_after);

    // §8.3 — with no open positions and no receiver flow, aggregate
    // conservation makes every market size zero: release the unassigned
    // rounding residue to LP residual cash without a market loop.
    if ledger.open_position_count == 0 && ledger.receiver_flow_per_second == 0 {
        ledger.pending_receiver_funding_total = 0;
        ledger.receiver_accrual_remainder = 0;
    }
    storage::save_market(env, &position.market, &market);
    risk::refresh_rate(env, ledger, physical_after);

    CloseSummary {
        position_id: position.id,
        owner: position.owner.clone(),
        market: position.market.clone(),
        closed: removed.full,
        size_removed,
        price,
        raw_pnl,
        payable_pnl: payable,
        fees: collected,
        closing_fee,
        realized_payout,
        collateral_withdrawn: if removed.full {
            0
        } else {
            collateral_withdrawn
        },
        collateral_payout,
        bad_debt,
        liquidation_reward,
        execution_budget_refunded: budget_refund,
    }
}
