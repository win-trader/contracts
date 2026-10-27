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

pub struct RemovedExposure {
    pub full: bool,
    pub new_size: i128,
    pub base_after: i128,
    pub risk_after: i128,
    pub base_removed: i128,
    pub risk_removed: i128,
}

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

#[derive(Clone, Debug)]
pub struct SettleHeader {
    pub position_id: u64,
    pub owner: Address,
    pub market: Symbol,
    pub size_removed: i128,
    pub price: i128,
    pub raw_pnl: i128,
    pub payable_pnl: i128,
    pub fees: CollectedFees,
    pub closing_fee: i128,
    pub keeper_reward: i128,
    pub keeper_from_lp_backstop: i128,
    pub keeper_unpaid: i128,
    pub stored_collateral: i128,
    pub effective_collateral: i128,
    pub liquidation_threshold: i128,
    pub payout_factor: i128,
    pub unpaid_profit: i128,
}

impl SettleHeader {
    pub fn signed_payable_pnl(&self) -> i128 {
        if self.raw_pnl <= 0 {
            self.raw_pnl
        } else {
            self.payable_pnl
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ClosedTail {
    pub collateral_payout: i128,
    pub bad_debt: i128,
}

#[derive(Clone, Debug)]
pub enum Settled {
    Partial(SettleHeader),
    Closed(SettleHeader, ClosedTail),
}

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

pub struct Keeper<'a> {
    pub recipient: &'a Address,
    pub reward: i128,
    pub liquidation: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClosingFee {
    Charged,
    Waived,
}

struct Settlement<'a> {
    env: &'a Env,
    ledger: &'a mut Ledger,
    position: Position,
    market: Market,
    size_removed: i128,
    price: i128,
    keeper: Keeper<'a>,
    closing_fee_policy: ClosingFee,
    pending: crate::funding::PendingFees,
    effective_collateral: i128,
    liquidation_threshold: i128,
    payout_factor: i128,
    keeper_payment: crate::keeper::KeeperPayment,
    removed: RemovedExposure,
    raw_pnl: i128,
    payable: i128,
    negative: i128,
    collected: CollectedFees,
    closing_fee: i128,
    unpaid_profit: i128,
    survives: bool,
}

impl<'a> Settlement<'a> {
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
        let payable = core::cmp::max(
            risk::payable_pnl(env, raw_pnl, market.side(position.is_long)),
            0,
        );
        let negative = core::cmp::max(-raw_pnl, 0);
        let pending = crate::funding::pending_fees(env, ledger, &position, &market);
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
        if !self.removed.full && (self.collected.unpaid > 0 || self.unpaid_profit > 0) {
            panic_with_error!(self.env, PositionManagerError::InsufficientCollateral);
        }
    }

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
            // Terminal exits cap the reward: the profit credit is cash-limited, health is not.
            let payable = if self.removed.full {
                core::cmp::min(reward, self.position.stored_collateral)
            } else {
                reward
            };
            let from_position = crate::keeper::pay_from_position(
                self.env,
                self.ledger,
                &mut self.position,
                self.market.side_mut(is_long),
                &recipient,
                payable,
            );
            crate::keeper::KeeperPayment {
                from_position,
                from_lp_backstop: 0,
                unpaid: math::sub(self.env, reward, from_position),
            }
        };
    }

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
        if fee <= 0 {
            return;
        }
        let is_long = self.position.is_long;
        self.closing_fee = ledger::collect_stored_collateral(
            self.env,
            self.ledger,
            &mut self.position,
            self.market.side_mut(is_long),
            fee,
        );
        let (market_id, actor) = (self.position.market.clone(), self.keeper.recipient.clone());
        fees::distribute_open_close_revenue(
            self.env,
            self.ledger,
            &market_id,
            &actor,
            self.closing_fee,
            FeeSource::Closing,
            self.position.id,
        );
    }

    fn reduce_exposure(&mut self) {
        let is_long = self.position.is_long;
        funding::reset_receiver_distribution_remainder(&mut self.market, is_long);
        let side = self.market.side_mut(is_long);
        side.size_open_interest = math::sub(self.env, side.size_open_interest, self.size_removed);
        side.base_exposure = math::sub(self.env, side.base_exposure, self.removed.base_removed);
        side.risk_units = math::sub(self.env, side.risk_units, self.removed.risk_removed);
        risk::release_exposure(self.env, self.ledger, self.removed.risk_removed);
    }

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
            let residual = self.position.stored_collateral;
            tail.collateral_payout = ledger::payout_collateral_to_owner(
                self.env,
                self.ledger,
                &mut self.position,
                self.market.side_mut(is_long),
                residual,
            );
        }
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

    fn finalize_partial(&mut self) {
        self.position.size = self.removed.new_size;
        self.position.base_exposure = self.removed.base_after;
        self.position.risk_units = self.removed.risk_after;
        let config = storage::get_global_config(self.env);
        if self.position.stored_collateral < config.min_collateral {
            panic_with_error!(self.env, PositionManagerError::InsufficientCollateral);
        }
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
        let threshold = core::cmp::max(
            risk::maintenance_requirement(self.env, self.removed.new_size, &self.market.config),
            config.keeper_rewards.liquidation,
        );
        if effective <= threshold {
            panic_with_error!(self.env, PositionManagerError::InsufficientCollateral);
        }
        funding::snapshot_funding_indices(&mut self.position, &self.market);
        self.survives = true;
    }

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
        borrow::refresh_rate(self.env, self.ledger, physical_after);
        if self.survives {
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
