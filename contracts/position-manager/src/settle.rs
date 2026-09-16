//! Position settlement — doc §12 (decrease, close, liquidation, ADL,
//! triggered orders).
//!
//! `settle` runs the §6.10 procedure as a pipeline of named phases on one
//! `Settlement` context — the function itself reads as the table of
//! contents. Available value is distributed in the §11.4 waterfall order —
//! receiver-backed funding, negative price PnL, LP-backed funding, borrow
//! (all inside `fees::capitalize`), then residual trader equity. Every money
//! move is a ledger verb, so the side aggregates and claim totals stay
//! conserved (§18.2) without a reconciliation step.
//!
//! A surviving decrease leaves realized profit in the position as stored
//! collateral (§7.9). There is no collateral-withdrawal operation: the ways
//! out of a position are to shrink it or to close it, and that is what makes
//! "a surviving position never realizes value the vault could not pay" a
//! property of one path rather than two.

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
pub struct RemovedExposure {
    pub full: bool,
    pub new_size: i128,
    pub base_after: i128,
    pub risk_after: i128,
    pub base_removed: i128,
    pub risk_removed: i128,
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
pub fn removed_exposure(
    env: &Env,
    position: &Position,
    size_removed: i128,
    config: &MarketConfig,
) -> RemovedExposure {
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
    /// §6.12 — what the settlement's one keeper was actually paid, and from
    /// where. Only liquidation can draw on the LP backstop or fall short
    /// (§8.11), so on every other path the last two are zero.
    pub keeper_reward: i128,
    pub keeper_from_lp_backstop: i128,
    pub keeper_unpaid: i128,
    /// Stored collateral the position is left holding. Zero on a full
    /// close, where `ClosedTail::collateral_payout` is the figure that
    /// matters instead.
    pub stored_collateral: i128,
    /// §12.6's liquidation fields, carried by **every** close rather than by
    /// a separate `Liquidated` event. They are the numbers that decide
    /// whether a close *could* have been a liquidation, so reporting them
    /// only when one happened would make the two paths unreconcilable.
    pub effective_collateral: i128,
    pub liquidation_threshold: i128,
    /// §6.5 — the side's stored hard-cap payout factor as this settlement
    /// applied it. `INDEX_PRECISION` unless the side is latched, which is
    /// §12.6's "payout factor applied" for the ADL path.
    pub payout_factor: i128,
    /// §6.5 — recognized profit the vault could not pay. The events are the
    /// only durable record of it: §5.6 removes the pending record and §5.14
    /// removes the position.
    pub unpaid_profit: i128,
}

impl SettleHeader {
    /// §2.8's payable PnL, signed — which is the quantity §12.6 requires a
    /// settlement event to report.
    ///
    /// The stored `payable_pnl` is the **positive part alone**, because both
    /// the closing-fee base (§6.9) and the credit step need it clamped at
    /// zero. A loss passes through unreduced under §2.8 and has to reach a
    /// consumer as a negative number: an indexer summing the emitted field
    /// would otherwise see winners only, and §12.6 makes the event stream the
    /// only durable record there is.
    pub fn signed_payable_pnl(&self) -> i128 {
        if self.raw_pnl <= 0 {
            self.raw_pnl
        } else {
            self.payable_pnl
        }
    }
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
    Partial(SettleHeader),
    Closed(SettleHeader, ClosedTail),
}

/// §6.10 — settle a decrease, close, liquidation, ADL, or triggered exit.
/// The caller must have accrued the borrow clock and the market's funding
/// to `now`. Saves the market and position storage; the caller saves the
/// ledger.
#[allow(clippy::too_many_arguments)]
pub fn settle(
    env: &Env,
    ledger: &mut Ledger,
    position: Position,
    market: Market,
    size_removed: i128,
    price: i128,
    keeper: Keeper,
    closing_fee: ClosingFee,
) -> Settled {
    let mut s = Settlement::begin(
        env,
        ledger,
        position,
        market,
        size_removed,
        price,
        keeper,
        closing_fee,
    );
    s.credit_payable();
    s.capitalize();
    s.pay_keeper();
    s.charge_closing_fee();
    s.reduce_exposure();
    if s.removed.full {
        let tail = s.finalize_close();
        Settled::Closed(s.finish(), tail)
    } else {
        s.finalize_partial();
        Settled::Partial(s.finish())
    }
}

/// Who is paid for this settlement, and how much. `liquidation` selects
/// §6.12's capped path — the only close whose reward may fall short and
/// still complete.
///
/// Not optional: §6.12 gives every settlement exactly one keeper reward, and
/// §12.6's envelope needs an actor to credit. A settlement with no one to pay
/// and no one to name is not a shape this protocol has.
pub struct Keeper<'a> {
    pub recipient: &'a Address,
    pub reward: i128,
    pub liquidation: bool,
}

/// §9.13 — which settlements may charge a closing fee.
///
/// A voluntary decrease, a voluntary close, a take-profit, and a stop-loss
/// do. Liquidation and ADL do not: both are forced on the trader, and
/// charging a fee for an exit they did not choose would take value from the
/// side the protocol is already penalising.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClosingFee {
    Charged,
    Waived,
}

/// The working state one settlement threads through its phases.
struct Settlement<'a> {
    env: &'a Env,
    ledger: &'a mut Ledger,
    position: Position,
    market: Market,
    size_removed: i128,
    price: i128,
    keeper: Keeper<'a>,
    closing_fee_policy: ClosingFee,
    /// The obligations as they stood before capitalization consumed them.
    /// §6.9's closing fee is measured against these, not against what was
    /// actually collected.
    pending: crate::funding::PendingFees,
    effective_collateral: i128,
    liquidation_threshold: i128,
    payout_factor: i128,
    keeper_payment: crate::keeper::KeeperPayment,
    removed: RemovedExposure,
    raw_pnl: i128,
    /// Positive PnL after the §14 payout factor; zero for losers.
    payable: i128,
    /// Magnitude of negative raw PnL; zero for winners.
    negative: i128,
    collected: CollectedFees,
    closing_fee: i128,
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
        keeper: Keeper<'a>,
        closing_fee_policy: ClosingFee,
    ) -> Self {
        let physical = ledger::physical_cash(env);
        let equity = ledger.cash_lp_equity(env, physical);
        risk::evaluate_market_risk(
            env,
            ledger,
            &position.market,
            keeper.recipient,
            &mut market,
            price,
            equity,
        );

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
        // Measured against the same snapshot everything else here uses, so
        // the reported health cannot come from a different instant than the
        // settlement it describes.
        let effective_collateral =
            risk::effective_collateral(env, position.stored_collateral, payable, &pending);
        let liquidation_threshold = core::cmp::max(
            risk::maintenance_requirement(env, position.size, &market.config),
            storage::get_global_config(env).keeper_rewards.liquidation,
        );
        let payout_factor = market.side(position.is_long).hard_cap_payout_factor;
        Settlement {
            env,
            ledger,
            position,
            market,
            size_removed,
            price,
            keeper,
            closing_fee_policy,
            pending,
            effective_collateral,
            liquidation_threshold,
            payout_factor,
            keeper_payment: crate::keeper::KeeperPayment::default(),
            removed,
            raw_pnl,
            payable,
            negative,
            collected: CollectedFees::default(),
            closing_fee: 0,
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
        let (market_id, actor) = (self.position.market.clone(), self.keeper.recipient.clone());
        self.collected = fees::capitalize(
            self.env,
            self.ledger,
            &market_id,
            &actor,
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
        let (recipient, reward, liquidation) = (
            self.keeper.recipient.clone(),
            self.keeper.reward,
            self.keeper.liquidation,
        );
        let is_long = self.position.is_long;
        self.keeper_payment = if liquidation {
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
        } else {
            crate::keeper::KeeperPayment {
                from_position: crate::keeper::pay_from_position(
                    self.env,
                    self.ledger,
                    &mut self.position,
                    self.market.side_mut(is_long),
                    &recipient,
                    reward,
                ),
                from_lp_backstop: 0,
                unpaid: 0,
            }
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
    /// Liquidation and ADL pay no closing fee at all (§7.13, §7.14).
    fn charge_closing_fee(&mut self) {
        if self.closing_fee_policy == ClosingFee::Waived {
            return;
        }
        let fee = fees::calculate_closing_fee(
            self.env,
            self.size_removed,
            self.payable,
            &self.pending,
            self.pending.borrow,
            self.keeper_payment.paid(),
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
        let (market_id, actor) = (self.position.market.clone(), self.keeper.recipient.clone());
        fees::distribute_open_close_revenue(
            self.env,
            self.ledger,
            &market_id,
            &actor,
            self.closing_fee,
            &owner,
            FeeSource::Closing,
            self.position.id,
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

    /// §11.4 full-close waterfall tail: bad debt, then residual trader
    /// equity; the position leaves storage, taking both attached triggers
    /// and any pending voluntary mutation with it (§5.14).
    fn finalize_close(&mut self) -> ClosedTail {
        let mut tail = ClosedTail::default();
        if self.collected.unpaid > 0 {
            tail.bad_debt = self.collected.unpaid;
            let (market_id, actor) =
                (self.position.market.clone(), self.keeper.recipient.clone());
            events::emit_bad_debt(
                self.env,
                &market_id,
                &actor,
                self.position.id,
                tail.bad_debt,
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
        // §8.12 steps 4-6 — the position is leaving, so any pending
        // voluntary mutation is superseded and its complete added-collateral
        // escrow goes back to the owner. It pays no reward of its own: a
        // terminal settlement is one keeper action (§8.14).
        let actor = self.keeper.recipient.clone();
        crate::action::supersede_pending_mutation(
            self.env,
            self.ledger,
            &actor,
            &mut self.position,
        );
        storage::remove_position(self.env, self.position.id);
        risk::release_position(self.env, self.ledger);
        tail
    }

    /// §7.9 — the surviving tail: resize, then the two requirements a
    /// survivor must meet, then the baseline reset (§11.5: no historical
    /// debt carries forward). The position is saved in `finish`.
    ///
    /// Realized profit is **not** paid out. It stays as stored collateral,
    /// which is what makes the §7.9 guards local: a survivor may not carry a
    /// loss its collateral could not absorb, and may not realize profit the
    /// vault could not pay, because unlike a terminal settlement it has no
    /// result in which to report the shortfall.
    ///
    /// Both checks are `require`s rather than outcomes: the decrease
    /// preflight has already evaluated them against this same snapshot, so
    /// reaching one here would mean the projection and the settlement
    /// disagree — a defect, not a business outcome.
    fn finalize_partial(&mut self) {
        self.position.size = self.removed.new_size;
        self.position.base_exposure = self.removed.base_after;
        self.position.risk_units = self.removed.risk_after;
        let config = storage::get_global_config(self.env);
        if self.position.stored_collateral < config.min_collateral {
            panic_with_error!(self.env, PositionManagerError::InsufficientCollateral);
        }
        // Pending obligations are all settled at this point, so effective
        // collateral is stored collateral plus the payable PnL still riding
        // on the remaining exposure.
        let remaining_pnl = risk::payable_pnl(
            self.env,
            math::pnl(
                self.env,
                self.position.is_long,
                self.removed.new_size,
                self.removed.base_after,
                self.price,
            ),
            self.market.side(self.position.is_long),
        );
        let effective = math::add(self.env, self.position.stored_collateral, remaining_pnl);
        // §7.9 — strictly above the threshold. Liquidation eligibility is
        // `effective <= threshold`, so a survivor left exactly at it would
        // be liquidatable the instant the decrease committed.
        let threshold = core::cmp::max(
            risk::maintenance_requirement(self.env, self.removed.new_size, &self.market.config),
            config.keeper_rewards.liquidation,
        );
        if effective <= threshold {
            panic_with_error!(self.env, PositionManagerError::InsufficientCollateral);
        }
        funding::reset_debts(self.env, &mut self.position, &self.market);
        // The position is *not* saved here. §4.10 opens its replacement
        // borrow window after the global rate refresh, and that refresh
        // happens in `finish`; saving now would store a position whose
        // window minimum was quoted at the pre-mutation utilization.
        self.survives = true;
    }

    /// §4.9 steps 8-10 — refresh the market display and risk state from the
    /// resulting book, refresh the global borrow rate from the resulting
    /// risk units, open the survivor's replacement borrow window, and store
    /// everything atomically.
    fn finish(mut self) -> SettleHeader {
        funding::refresh_display(self.env, self.ledger, &mut self.market);
        let physical_after = ledger::physical_cash(self.env);
        let equity_after = self.ledger.cash_lp_equity(self.env, physical_after);
        let actor = self.keeper.recipient.clone();
        risk::evaluate_market_risk(
            self.env,
            self.ledger,
            &self.position.market,
            &actor,
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
            keeper_reward: self.keeper_payment.paid(),
            keeper_from_lp_backstop: self.keeper_payment.from_lp_backstop,
            keeper_unpaid: self.keeper_payment.unpaid,
            stored_collateral: self.position.stored_collateral,
            effective_collateral: self.effective_collateral,
            liquidation_threshold: self.liquidation_threshold,
            payout_factor: self.payout_factor,
            unpaid_profit: self.unpaid_profit,
        }
    }
}
