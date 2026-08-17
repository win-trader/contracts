//! Position settlement — doc §12 (decrease, close, liquidation, ADL,
//! triggered orders).
//!
//! `settle_close` runs the §12.2 procedure as a pipeline of named phases on
//! one `Settlement` context — the function itself reads as the table of
//! contents. Available value is distributed in the §12.2 waterfall order —
//! receiver-backed funding, negative price PnL, LP-backed funding, borrow
//! (all inside `fees::capitalize`), then the closing fee, then the
//! liquidation reward, then residual trader equity. Every money move is a
//! ledger verb, so the side aggregates and claim totals stay conserved
//! (§18.2) without a reconciliation step.

use soroban_sdk::{panic_with_error, Address, Env, Symbol};

use shared::constants::BPS;
use shared::{Market, Position};

use crate::borrow;
use crate::errors::PositionManagerError;
use crate::events::FeeSource;
use crate::fees::{self, CollectedFees};
use crate::funding;
use crate::ledger::{self, Ledger};
use crate::risk;
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

/// What the full- or partial-close tail paid out.
#[derive(Default)]
struct Finalized {
    collateral_payout: i128,
    bad_debt: i128,
    liquidation_reward: i128,
    budget_refund: i128,
    collateral_withdrawn: i128,
}

/// §12.2 — settle a decrease, close, liquidation, ADL, or triggered order.
/// The caller must have accrued the borrow clock and the market's funding
/// to `now`. Saves the market and position storage; the caller saves the
/// ledger.
#[allow(clippy::too_many_arguments)]
pub fn settle_close(
    env: &Env,
    ledger: &mut Ledger,
    position: Position,
    market: Market,
    size_removed: i128,
    collateral_withdrawn: i128,
    price: i128,
    reward_recipient: Option<&Address>,
) -> CloseSummary {
    let mut s = Settlement::begin(env, ledger, position, market, size_removed, price);
    s.credit_payable();
    s.capitalize();
    s.charge_closing_fee();
    s.pay_partial_realized();
    s.reduce_exposure();
    let tail = if s.removed.full {
        s.finalize_close(reward_recipient)
    } else {
        s.finalize_partial(collateral_withdrawn)
    };
    s.finish(tail)
}

/// The working state one settlement threads through its phases.
struct Settlement<'a> {
    env: &'a Env,
    ledger: &'a mut Ledger,
    position: Position,
    market: Market,
    size_removed: i128,
    price: i128,
    removed: RemovedExposure,
    raw_pnl: i128,
    /// Positive PnL after the §14 payout factor; zero for losers.
    payable: i128,
    /// Magnitude of negative raw PnL; zero for winners.
    negative: i128,
    collected: CollectedFees,
    closing_fee: i128,
    realized_payout: i128,
}

impl<'a> Settlement<'a> {
    /// Mark the book and price the removal: one physical-cash reading
    /// covers every pre-transfer step, the §14 risk evaluation and payout
    /// factor observe that same instant, and the removed exposure prices
    /// into raw / payable / negative PnL.
    fn begin(
        env: &'a Env,
        ledger: &'a mut Ledger,
        position: Position,
        mut market: Market,
        size_removed: i128,
        price: i128,
    ) -> Self {
        let physical = ledger::physical_cash(env);
        let equity = ledger.cash_lp_equity(env, physical);
        risk::evaluate_market_risk(env, ledger, &position.market, &mut market, price, equity);

        let removed = removed_exposure(env, &position, size_removed);
        let raw_pnl = math::pnl(
            env,
            position.is_long,
            size_removed,
            removed.base_removed,
            price,
        );
        let payable = core::cmp::max(
            risk::payable_pnl(
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
        Settlement {
            env,
            ledger,
            position,
            market,
            size_removed,
            price,
            removed,
            raw_pnl,
            payable,
            negative,
            collected: CollectedFees::default(),
            closing_fee: 0,
            realized_payout: 0,
        }
    }

    /// §12.2 step 7 — a winner's payable PnL becomes stored collateral (LP
    /// equity pays, a pure label move) so the waterfall below has value to
    /// collect from.
    fn credit_payable(&mut self) {
        if self.payable > 0 {
            let is_long = self.position.is_long;
            ledger::add_stored_collateral(
                self.env,
                self.ledger,
                &mut self.position,
                self.market.side_mut(is_long),
                self.payable,
            );
        }
    }

    /// §12.2 steps 8-9 — capitalization settles credits and obligations in
    /// the §11.4 order. A partial close must cover everything; only a full
    /// close may leave an unpaid remainder (booked as bad debt in the
    /// tail).
    fn capitalize(&mut self) {
        self.collected = fees::capitalize(
            self.env,
            self.ledger,
            &mut self.position,
            &mut self.market,
            self.negative,
        );
        if self.collected.unpaid > 0 && !self.removed.full {
            panic_with_error!(self.env, PositionManagerError::InsufficientCollateral);
        }
    }

    /// §11.1 — the closing fee is a share of realized price winnings:
    /// `ceil(payable × tier / BPS)`, ranked below every waterfall
    /// obligation. Losers pay nothing, so there is no shortfall path; the
    /// tier is validated ≤ BPS, so the fee never exceeds the payable
    /// profit.
    fn charge_closing_fee(&mut self) {
        if self.payable <= 0 {
            return;
        }
        let bps = fees::tiered_close_fee_bps(
            self.env,
            &self.market,
            self.position.is_long,
            self.removed.base_removed,
        );
        let fee = math::closing_fee(self.env, self.payable, bps);
        let is_long = self.position.is_long;
        self.closing_fee = ledger::collect_stored_collateral(
            self.env,
            self.ledger,
            &mut self.position,
            self.market.side_mut(is_long),
            fee,
        );
        fees::split_revenue(
            self.env,
            self.ledger,
            self.closing_fee,
            FeeSource::Closing,
            self.position.id,
        );
    }

    /// Partial close only: transfer the remaining realized profit to the
    /// trader.
    fn pay_partial_realized(&mut self) {
        if self.removed.full || self.payable <= self.closing_fee {
            return;
        }
        let is_long = self.position.is_long;
        let owner = self.position.owner.clone();
        let amount = math::sub(self.env, self.payable, self.closing_fee);
        self.realized_payout = ledger::payout_collateral(
            self.env,
            self.ledger,
            &mut self.position,
            self.market.side_mut(is_long),
            &owner,
            amount,
        );
    }

    /// §12.2 step 12 — reduce the side exposure aggregates and the global
    /// risk register (collateral aggregates were kept in sync by the choke
    /// point above).
    fn reduce_exposure(&mut self) {
        let is_long = self.position.is_long;
        let side = self.market.side_mut(is_long);
        side.size_open_interest = math::sub(self.env, side.size_open_interest, self.size_removed);
        side.base_exposure = math::sub(self.env, side.base_exposure, self.removed.base_removed);
        side.risk_units = math::sub(self.env, side.risk_units, self.removed.risk_removed);
        risk::release_exposure(self.env, self.ledger, self.removed.risk_removed);
    }

    /// §12.2 full-close waterfall tail: bad debt, then the liquidation
    /// reward, then residual trader equity, then the execution-budget
    /// refund; the position leaves storage.
    fn finalize_close(&mut self, reward_recipient: Option<&Address>) -> Finalized {
        let mut tail = Finalized::default();
        if self.collected.unpaid > 0 {
            tail.bad_debt = self.collected.unpaid;
            events::emit_bad_debt(self.env, self.position.id, tail.bad_debt);
        }
        if let Some(liquidator) = reward_recipient {
            let is_long = self.position.is_long;
            let reward = math::mul_div_floor(
                self.env,
                self.size_removed,
                self.market.config.liquidation_reward_bps as i128,
                BPS,
            );
            tail.liquidation_reward = ledger::payout_collateral(
                self.env,
                self.ledger,
                &mut self.position,
                self.market.side_mut(is_long),
                liquidator,
                reward,
            );
        }
        {
            let is_long = self.position.is_long;
            let owner = self.position.owner.clone();
            let residual = self.position.stored_collateral;
            tail.collateral_payout = ledger::payout_collateral(
                self.env,
                self.ledger,
                &mut self.position,
                self.market.side_mut(is_long),
                &owner,
                residual,
            );
        }
        if self.position.execution_budget > 0 {
            tail.budget_refund = self.position.execution_budget;
            ledger::payout(
                self.env,
                self.ledger,
                ledger::Bucket::ExecutionBudget,
                &self.position.owner,
                tail.budget_refund,
            );
        }
        storage::remove_position(self.env, self.position.id);
        risk::release_position(self.ledger);
        tail
    }

    /// §12.2 partial close: explicit withdrawal, resize, health check,
    /// baseline reset (§11.5: no historical debt carries forward); the
    /// position is saved back.
    fn finalize_partial(&mut self, collateral_withdrawn: i128) -> Finalized {
        if collateral_withdrawn > self.position.stored_collateral {
            panic_with_error!(self.env, PositionManagerError::InsufficientCollateral);
        }
        if collateral_withdrawn > 0 {
            let is_long = self.position.is_long;
            let owner = self.position.owner.clone();
            // Shrinking the book may use the safety path; a pure collateral
            // withdrawal goes through the conservation-checked path.
            if self.size_removed > 0 {
                ledger::payout_collateral(
                    self.env,
                    self.ledger,
                    &mut self.position,
                    self.market.side_mut(is_long),
                    &owner,
                    collateral_withdrawn,
                );
            } else {
                ledger::payout_collateral_checked(
                    self.env,
                    self.ledger,
                    &mut self.position,
                    self.market.side_mut(is_long),
                    &owner,
                    collateral_withdrawn,
                );
            }
        }
        self.position.size = self.removed.new_size;
        self.position.base_exposure = self.removed.base_after;
        self.position.risk_units = self.removed.risk_after;
        let health = math::add(
            self.env,
            self.position.stored_collateral,
            math::pnl(
                self.env,
                self.position.is_long,
                self.removed.new_size,
                self.removed.base_after,
                self.price,
            ),
        );
        // Shrinking the position de-risks; a pure collateral withdrawal
        // raises leverage and re-underwrites at the initial margin (§12.3).
        let required = risk::required_margin(
            self.env,
            self.removed.new_size,
            &self.market.config,
            self.size_removed == 0,
        );
        if health < required {
            panic_with_error!(self.env, PositionManagerError::InsufficientCollateral);
        }
        funding::reset_debts(self.env, self.ledger, &mut self.position, &self.market);
        storage::save_position(self.env, &self.position);
        Finalized {
            collateral_withdrawn,
            ..Finalized::default()
        }
    }

    /// §10.3 steps 5-7 — refresh the funding display, re-evaluate risk
    /// against the post-transfer balance, release the empty-book residue,
    /// store the market, refresh the borrow rate, and assemble the
    /// summary.
    fn finish(mut self, tail: Finalized) -> CloseSummary {
        funding::refresh_display(self.env, &mut self.market);
        let physical_after = ledger::physical_cash(self.env);
        let equity_after = self.ledger.cash_lp_equity(self.env, physical_after);
        risk::evaluate_market_risk(
            self.env,
            self.ledger,
            &self.position.market,
            &mut self.market,
            self.price,
            equity_after,
        );

        funding::release_residue(self.env, self.ledger);
        storage::save_market(self.env, &self.position.market, &self.market);
        borrow::refresh_rate(self.env, self.ledger, physical_after);

        CloseSummary {
            position_id: self.position.id,
            owner: self.position.owner.clone(),
            market: self.position.market.clone(),
            closed: self.removed.full,
            size_removed: self.size_removed,
            price: self.price,
            raw_pnl: self.raw_pnl,
            payable_pnl: self.payable,
            fees: self.collected,
            closing_fee: self.closing_fee,
            realized_payout: self.realized_payout,
            collateral_withdrawn: tail.collateral_withdrawn,
            collateral_payout: tail.collateral_payout,
            bad_debt: tail.bad_debt,
            liquidation_reward: tail.liquidation_reward,
            execution_budget_refunded: tail.budget_refund,
        }
    }
}
