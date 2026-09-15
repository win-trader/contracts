//! Position settlement — doc §12 (decrease, close, liquidation, ADL,
//! triggered orders).
//!
//! `settle_close` runs the §12.2 procedure as a pipeline of named phases on
//! one `Settlement` context — the function itself reads as the table of
//! contents. Available value is distributed in the §12.2 waterfall order —
//! receiver-backed funding, negative price PnL, LP-backed funding, borrow
//! (all inside `fees::capitalize`), then residual trader equity. The
//! closing fee and the keeper reward were deleted with their old schedules
//! (P1-05, P1-06) and return in Phases 5 and 6. Every money move is a
//! ledger verb, so the side aggregates and claim totals stay conserved
//! (§18.2) without a reconciliation step.

use soroban_sdk::{panic_with_error, Address, Env, Symbol};

use shared::{Market, MarketConfig, Position};

use crate::borrow;
use crate::errors::PositionManagerError;
use crate::events::FeeSource;
use crate::fees::{self, CollectedFees};
use crate::funding;
use crate::ledger::{self, Ledger};
use crate::risk;
use crate::{events, math, storage};

/// §6.13 — the exposure a decrease removes.
struct RemovedExposure {
    full: bool,
    new_size: i128,
    base_after: i128,
    risk_after: i128,
    base_removed: i128,
    risk_removed: i128,
}

/// §6.13 `derive_partial_removal` — remaining base **floors** and the
/// removed portion takes the difference, so the two always sum to the
/// pre-reduction base and nothing strands.
///
/// Remaining risk units are re-derived from the resulting size rather than
/// pro-rated from the old risk units: risk is a function of size, and
/// accumulating independently rounded tranches would let the stored value
/// drift from the value the size implies.
///
/// A full close removes the exact remainder with no proportional rounding at
/// all.
fn removed_exposure(env: &Env, position: &Position, size_removed: i128, config: &MarketConfig) -> RemovedExposure {
    let full = size_removed == position.size;
    let new_size = math::sub(env, position.size, size_removed);
    let (base_after, risk_after) = if full {
        (0, 0)
    } else {
        (
            math::mul_div_floor(env, position.base_exposure, new_size, position.size),
            math::risk_units_for(env, new_size, config.market_risk_factor_bps),
        )
    };
    RemovedExposure {
        full,
        new_size,
        base_after,
        risk_after,
        base_removed: math::sub(env, position.base_exposure, base_after),
        risk_removed: math::sub(env, position.risk_units, risk_after),
    }
}

/// What every settlement produces, whatever its shape.
#[derive(Clone, Debug)]
pub struct SettleHeader {
    pub position_id: u64,
    pub owner: Address,
    pub market: Symbol,
    pub size_removed: i128,
    pub price: i128,
    /// Signed raw price PnL on the removed exposure (§12.2).
    pub raw_pnl: i128,
    /// Positive PnL actually credited after the §14 payout factor.
    pub payable_pnl: i128,
    pub fees: CollectedFees,
    /// Closing fee collected out of the realized winnings.
    pub closing_fee: i128,
    /// §6.5 — recognized profit the vault could not pay. The events are the
    /// only durable record of it: §5.6 removes the pending record and §5.14
    /// removes the position.
    pub unpaid_profit: i128,
}

/// What only a partial close produces.
#[derive(Clone, Debug, Default)]
pub struct PartialTail {
    /// Realized profit transferred to the owner.
    pub realized_payout: i128,
    /// Explicit collateral withdrawal transferred.
    pub collateral_withdrawn: i128,
}

/// What only a full close produces.
#[derive(Clone, Debug, Default)]
pub struct ClosedTail {
    /// Residual collateral transferred to the owner.
    pub collateral_payout: i128,
    /// Accrued obligations the position could not cover.
    pub bad_debt: i128,
}

/// One settlement's outcome. The two shapes carry only the fields their
/// path can actually produce — partial payouts on a full close (and vice
/// versa) are unrepresentable.
#[derive(Clone, Debug)]
pub enum Settled {
    Partial(SettleHeader, PartialTail),
    Closed(SettleHeader, ClosedTail),
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
    keeper: Option<Keeper>,
) -> Settled {
    let mut s = Settlement::begin(env, ledger, position, market, size_removed, price, keeper);
    s.credit_payable();
    s.capitalize();
    s.pay_keeper();
    s.charge_closing_fee();
    s.pay_partial_realized();
    s.reduce_exposure();
    if s.removed.full {
        let tail = s.finalize_close();
        Settled::Closed(s.finish(), tail)
    } else {
        let tail = s.finalize_partial(collateral_withdrawn);
        Settled::Partial(s.finish(), tail)
    }
}

/// Who is paid for this settlement, and how much. `liquidation` selects
/// §6.12's capped path — the only close whose reward may fall short and
/// still complete.
pub struct Keeper<'a> {
    pub recipient: &'a Address,
    pub reward: i128,
    pub liquidation: bool,
}

/// The working state one settlement threads through its phases.
struct Settlement<'a> {
    env: &'a Env,
    ledger: &'a mut Ledger,
    position: Position,
    market: Market,
    size_removed: i128,
    price: i128,
    keeper: Option<Keeper<'a>>,
    /// The obligations as they stood before capitalization consumed them.
    /// §6.9's closing fee is measured against these, not against what was
    /// actually collected.
    pending: crate::funding::PendingFees,
    keeper_paid: i128,
    removed: RemovedExposure,
    raw_pnl: i128,
    /// Positive PnL after the §14 payout factor; zero for losers.
    payable: i128,
    /// Magnitude of negative raw PnL; zero for winners.
    negative: i128,
    collected: CollectedFees,
    closing_fee: i128,
    realized_payout: i128,
    /// §6.5 — recognized profit the vault had no cash to pay.
    unpaid_profit: i128,
    /// Set by `finalize_partial`: the position lives on and needs a
    /// replacement borrow window opened in `finish`.
    survives: bool,
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
        keeper: Option<Keeper<'a>>,
    ) -> Self {
        let physical = ledger::physical_cash(env);
        let equity = ledger.cash_lp_equity(env, physical);
        risk::evaluate_market_risk(env, ledger, &position.market, &mut market, price, equity);

        let removed = removed_exposure(env, &position, size_removed, &market.config);
        let raw_pnl = math::pnl(
            env,
            position.is_long,
            size_removed,
            removed.base_removed,
            price,
        );
        // §6.5 — the side risk state was refreshed from this same price
        // snapshot immediately above, so the payout factor this reads is
        // current. The factor is read, never recomputed.
        let payable = core::cmp::max(
            risk::payable_pnl(env, raw_pnl, market.side(position.is_long)),
            0,
        );
        // The payment-time cash limit is deliberately **not** applied here.
        // It belongs to the payment, in `credit_payable`, and must not reach
        // effective collateral or any health check: a position's health is a
        // property of that position and must not change because the vault is
        // temporarily short of cash. Applied here it would also contaminate
        // the closing-fee base and the reported payable figure.
        let negative = core::cmp::max(-raw_pnl, 0);
        let pending = crate::funding::pending_fees(env, ledger, &position, &market);
        Settlement {
            env,
            ledger,
            position,
            market,
            size_removed,
            price,
            keeper,
            pending,
            keeper_paid: 0,
            removed,
            raw_pnl,
            payable,
            negative,
            collected: CollectedFees::default(),
            closing_fee: 0,
            realized_payout: 0,
            unpaid_profit: 0,
            survives: false,
        }
    }

    /// §6.5 `apply_payable_pnl` — a winner's payable PnL becomes stored
    /// collateral (LP equity pays, a pure label move) so the waterfall below
    /// has value to collect from.
    ///
    /// This is where the payment-time cash limit of §2.8 is applied, and the
    /// **only** place it is applied. Crediting is capped by the equity that
    /// exists at the moment of the credit, and any shortfall becomes
    /// `unpaid_profit` so the caller can report it. Profit the vault could
    /// not pay is neither a claim nor a receivable — there is no cash to owe
    /// it from — but it is never silently dropped: terminal settlement emits
    /// it, because a trader receiving less than their recognized profit is
    /// the single outcome most likely to be mistaken for an accounting
    /// error.
    fn credit_payable(&mut self) {
        if self.payable <= 0 {
            return;
        }
        let physical = ledger::physical_cash(self.env);
        let equity = self.ledger.cash_lp_equity(self.env, physical);
        let credited = core::cmp::min(self.payable, equity);
        self.unpaid_profit = math::sub(self.env, self.payable, credited);
        let is_long = self.position.is_long;
        ledger::add_stored_collateral(
            self.env,
            self.ledger,
            &mut self.position,
            self.market.side_mut(is_long),
            credited,
        );
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
        // §6.5 / §6.10 — a surviving path may leave neither an uncollectible
        // loss nor unpaid profit behind. Only terminal settlement may
        // consume a remainder and report it as bad debt, and only terminal
        // settlement may report profit the vault could not pay. A survivor
        // carrying either would be a claim with nothing backing it.
        if !self.removed.full && (self.collected.unpaid > 0 || self.unpaid_profit > 0) {
            panic_with_error!(self.env, PositionManagerError::InsufficientCollateral);
        }
    }

    /// §6.12 — the settlement's one keeper reward, paid before the closing
    /// fee so the fee is measured against what is left.
    fn pay_keeper(&mut self) {
        let Some(keeper) = self.keeper.as_ref() else {
            return;
        };
        let (recipient, reward, liquidation) =
            (keeper.recipient.clone(), keeper.reward, keeper.liquidation);
        let is_long = self.position.is_long;
        self.keeper_paid = if liquidation {
            let physical = ledger::physical_cash(self.env);
            crate::keeper::pay_liquidation(
                self.env,
                self.ledger,
                &mut self.position,
                self.market.side_mut(is_long),
                physical,
                &recipient,
                reward,
            )
            .paid()
        } else {
            crate::keeper::pay_from_position(
                self.env,
                self.ledger,
                &mut self.position,
                self.market.side_mut(is_long),
                &recipient,
                reward,
            )
        };
    }

    /// §6.9 — `max(size component, PnL component)`, capped at payable
    /// profit, then capped again at the profit left after every senior item.
    ///
    /// Only `collectible` is debited. `nominal - collectible` is waived
    /// immediately, is never stored, and is **not** bad debt: the fee is
    /// junior to everything above it, so a settlement that cannot pay it
    /// simply does not.
    ///
    /// Liquidation pays no closing fee at all (§7.13).
    fn charge_closing_fee(&mut self) {
        if self.keeper.as_ref().is_some_and(|k| k.liquidation) {
            return;
        }
        let fee = fees::calculate_closing_fee(
            self.env,
            self.size_removed,
            self.payable,
            &self.pending,
            self.pending.borrow,
            self.keeper_paid,
            &self.market.config,
        );
        if fee.collectible <= 0 {
            return;
        }
        let is_long = self.position.is_long;
        self.closing_fee = ledger::collect_stored_collateral(
            self.env,
            self.ledger,
            &mut self.position,
            self.market.side_mut(is_long),
            fee.collectible,
        );
        let owner = self.position.owner.clone();
        fees::distribute_open_close_revenue(
            self.env,
            self.ledger,
            self.closing_fee,
            &owner,
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
        // §4.9 step 7 — reset the opposite payer stream's distribution
        // carry before this side's size changes.
        funding::reset_receiver_distribution_remainder(&mut self.market, is_long);
        let side = self.market.side_mut(is_long);
        side.size_open_interest = math::sub(self.env, side.size_open_interest, self.size_removed);
        side.base_exposure = math::sub(self.env, side.base_exposure, self.removed.base_removed);
        side.risk_units = math::sub(self.env, side.risk_units, self.removed.risk_removed);
        risk::release_exposure(self.env, self.ledger, self.removed.risk_removed);
    }

    /// §12.2 full-close waterfall tail: bad debt, then residual trader
    /// equity; the position leaves storage. The keeper reward that used to
    /// sit between them is deleted with the bps schedule (P1-05) and comes
    /// back as a fixed cash amount in Phase 6.
    fn finalize_close(&mut self) -> ClosedTail {
        let mut tail = ClosedTail::default();
        if self.collected.unpaid > 0 {
            tail.bad_debt = self.collected.unpaid;
            events::emit_bad_debt(self.env, self.position.id, tail.bad_debt);
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
        storage::remove_position(self.env, self.position.id);
        risk::release_position(self.env, self.ledger);
        tail
    }

    /// §12.2 partial close: explicit withdrawal, resize, health check,
    /// baseline reset (§11.5: no historical debt carries forward); the
    /// position is saved back.
    fn finalize_partial(&mut self, collateral_withdrawn: i128) -> PartialTail {
        if collateral_withdrawn > self.position.stored_collateral {
            panic_with_error!(self.env, PositionManagerError::InsufficientCollateral);
        }
        if collateral_withdrawn > 0 {
            let is_long = self.position.is_long;
            let owner = self.position.owner.clone();
            // An explicit withdrawal is an outflow, not close proceeds, so
            // it always takes the conservation-checked path — it can never
            // create or deepen a shortfall, whether or not a size decrease
            // rides along. (Realized close profit still takes the safety
            // path in `pay_partial_realized`, as a risk-reduction proceed.)
            ledger::payout_collateral_checked(
                self.env,
                self.ledger,
                &mut self.position,
                self.market.side_mut(is_long),
                &owner,
                collateral_withdrawn,
            );
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
        // Any explicit collateral withdrawal raises leverage and
        // re-underwrites the remaining position at the initial margin — a
        // dust size decrease must not downgrade the floor to maintenance.
        // A pure shrink (no withdrawal) de-risks and keeps the maintenance
        // floor. `require_valid_input` forbids both being zero.
        let required = risk::required_margin(
            self.env,
            self.removed.new_size,
            &self.market.config,
            collateral_withdrawn > 0,
        );
        if health < required {
            panic_with_error!(self.env, PositionManagerError::InsufficientCollateral);
        }
        funding::reset_debts(self.env, &mut self.position, &self.market);
        // The position is *not* saved here. §4.10 opens its replacement
        // borrow window after the global rate refresh, and that refresh
        // happens in `finish`; saving now would store a position whose
        // window minimum was quoted at the pre-mutation utilization.
        self.survives = true;
        PartialTail {
            realized_payout: self.realized_payout,
            collateral_withdrawn,
        }
    }

    /// §4.9 steps 8-10 — refresh the market display and risk state from the
    /// resulting book, refresh the global borrow rate from the resulting
    /// risk units, open the survivor's replacement borrow window, and store
    /// everything atomically.
    fn finish(mut self) -> SettleHeader {
        funding::refresh_display(self.env, self.ledger, &mut self.market);
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

        funding::verify_no_final_receiver_residue(self.env, self.ledger);
        storage::save_market(self.env, &self.position.market, &self.market);
        // §4.9 step 9 — the rate is refreshed from the resulting risk units
        // and cash LP equity, never before the mutation that changed them.
        borrow::refresh_rate(self.env, self.ledger, physical_after);
        if self.survives {
            // §3.3.3 / §4.10 — the replacement window, quoted from the
            // refreshed rate.
            borrow::initialize_window(self.env, self.ledger, &mut self.position);
            storage::save_position(self.env, &self.position);
        }

        SettleHeader {
            position_id: self.position.id,
            owner: self.position.owner.clone(),
            market: self.position.market.clone(),
            size_removed: self.size_removed,
            price: self.price,
            raw_pnl: self.raw_pnl,
            payable_pnl: self.payable,
            fees: self.collected,
            closing_fee: self.closing_fee,
            unpaid_profit: self.unpaid_profit,
        }
    }
}
