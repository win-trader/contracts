use soroban_sdk::{panic_with_error, Address, Env, Symbol, Vec};

use shared::constants::{BPS, INDEX_PRECISION};
use shared::{Market, MarketConfig, MarketSide, Position, RiskState};

use crate::errors::PositionManagerError;
use crate::events;
use crate::ledger::Ledger;
use crate::{math, storage};

const SIDES_PER_MARKET: u64 = 2;

pub fn register_exposure(env: &Env, ledger: &mut Ledger, risk_units: i128) {
    ledger.total_risk_units = math::add(env, ledger.total_risk_units, risk_units);
}

pub fn release_exposure(env: &Env, ledger: &mut Ledger, risk_units: i128) {
    ledger.total_risk_units = math::sub(env, ledger.total_risk_units, risk_units);
}

pub fn register_position(ledger: &mut Ledger) {
    ledger.open_position_count += 1;
}

pub fn release_position(env: &Env, ledger: &mut Ledger) {
    ledger.open_position_count = ledger
        .open_position_count
        .checked_sub(1)
        .unwrap_or_else(|| panic_with_error!(env, PositionManagerError::InvariantViolation));
}

pub fn margin_requirement(env: &Env, size: i128, margin_bps: u32) -> i128 {
    math::mul_div_ceil(env, size, margin_bps as i128, BPS)
}

pub fn maintenance_requirement(env: &Env, size: i128, config: &MarketConfig) -> i128 {
    margin_requirement(env, size, config.maintenance_margin_bps)
}

pub fn initial_requirement(env: &Env, size: i128, config: &MarketConfig) -> i128 {
    margin_requirement(env, size, config.initial_margin_bps)
}

pub fn payable_pnl(env: &Env, raw_pnl: i128, side: &MarketSide) -> i128 {
    // Read the stored factor; recomputing it per settlement makes payouts order-dependent.
    if raw_pnl <= 0 || side.risk_state != RiskState::HardCap {
        return raw_pnl;
    }
    math::mul_div_floor(env, raw_pnl, side.hard_cap_payout_factor, INDEX_PRECISION)
}

fn snapshot_hard_cap_factor(
    env: &Env,
    side: &mut MarketSide,
    is_long: bool,
    config: &MarketConfig,
    price: i128,
    cash_lp_equity: i128,
) {
    let side_positive_pnl = core::cmp::max(
        math::pnl(env, is_long, side.size_open_interest, side.base_exposure, price),
        0,
    );
    let hard_cap_value = math::mul_div_floor(
        env,
        cash_lp_equity,
        config.hard_cap_pnl_factor_bps as i128,
        BPS,
    );
    side.hard_cap_reference_pnl = side_positive_pnl;
    side.hard_cap_payout_factor = if side_positive_pnl <= hard_cap_value {
        INDEX_PRECISION
    } else {
        math::mul_div_floor(env, hard_cap_value, INDEX_PRECISION, side_positive_pnl)
    };
}

pub fn side_accepts_new_exposure(env: &Env, side: &MarketSide) -> bool {
    !storage::is_paused(env)
        && matches!(side.risk_state, RiskState::Normal | RiskState::Warning)
}

#[derive(Clone, Copy, Debug)]
pub struct SideRiskAssessment {
    pub previous_state: RiskState,
    pub next_state: RiskState,
    pub positive_pnl: i128,
}

pub fn evaluate_side_risk_state(
    env: &Env,
    side: &MarketSide,
    is_long: bool,
    config: &MarketConfig,
    price: i128,
    cash_lp_equity: i128,
) -> SideRiskAssessment {
    let positive_pnl = core::cmp::max(
        math::pnl(env, is_long, side.size_open_interest, side.base_exposure, price),
        0,
    );
    let factor = if positive_pnl == 0 {
        0
    } else if cash_lp_equity == 0 {
        BPS
    } else {
        math::mul_div_floor(env, positive_pnl, BPS, cash_lp_equity)
    };
    let next_state = if factor >= config.hard_cap_pnl_factor_bps as i128 {
        RiskState::HardCap
    } else if factor >= config.adl_pnl_factor_bps as i128 {
        RiskState::Adl
    } else if factor >= config.warning_pnl_factor_bps as i128 {
        RiskState::Warning
    } else if side.risk_state != RiskState::Normal
        && factor >= config.recovery_pnl_factor_bps as i128
    {
        RiskState::Warning
    } else {
        RiskState::Normal
    };
    SideRiskAssessment {
        previous_state: side.risk_state,
        next_state,
        positive_pnl,
    }
}

fn update_restricted_count(env: &Env, ledger: &mut Ledger, old: RiskState, new: RiskState) {
    if old == RiskState::Normal && new != RiskState::Normal {
        ledger.restricted_market_side_count += 1;
    } else if old != RiskState::Normal && new == RiskState::Normal {
        ledger.restricted_market_side_count = ledger
            .restricted_market_side_count
            .checked_sub(1)
            .unwrap_or_else(|| panic_with_error!(env, PositionManagerError::InvariantViolation));
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_side(
    env: &Env,
    ledger: &mut Ledger,
    symbol: &Symbol,
    actor: &Address,
    side: &mut MarketSide,
    is_long: bool,
    config: &MarketConfig,
    assessment: &SideRiskAssessment,
    price: i128,
    cash_lp_equity: i128,
    relatch_band_bps: u32,
) {
    let previous = assessment.previous_state;
    let next = assessment.next_state;
    update_restricted_count(env, ledger, previous, next);

    if next == RiskState::HardCap {
        if previous != RiskState::HardCap {
            snapshot_hard_cap_factor(env, side, is_long, config, price, cash_lp_equity);
        } else {
            let band = math::mul_div_floor(
                env,
                side.hard_cap_reference_pnl,
                math::add(env, BPS, relatch_band_bps as i128),
                BPS,
            );
            if assessment.positive_pnl >= band {
                snapshot_hard_cap_factor(env, side, is_long, config, price, cash_lp_equity);
            }
        }
    } else if previous == RiskState::HardCap {
        side.hard_cap_payout_factor = INDEX_PRECISION;
        side.hard_cap_reference_pnl = 0;
    }

    if next != previous {
        let factor = if assessment.positive_pnl == 0 {
            0
        } else if cash_lp_equity == 0 {
            BPS
        } else {
            math::mul_div_floor(env, assessment.positive_pnl, BPS, cash_lp_equity)
        };
        events::emit_risk_state_changed(env, symbol, actor, is_long, previous, next, factor);
    }
    side.risk_state = next;
}

#[derive(Clone, Copy, Debug)]
pub struct RiskAssessment {
    pub long: SideRiskAssessment,
    pub short: SideRiskAssessment,
    price: i128,
    cash_lp_equity: i128,
}

impl RiskAssessment {
    pub fn restricted_sides(&self) -> u32 {
        (self.long.next_state != RiskState::Normal) as u32
            + (self.short.next_state != RiskState::Normal) as u32
    }

    pub fn deleveraging_sides(&self) -> u32 {
        fn counts(state: RiskState) -> u32 {
            matches!(state, RiskState::Adl | RiskState::HardCap) as u32
        }
        counts(self.long.next_state) + counts(self.short.next_state)
    }
}

pub fn assess(env: &Env, market: &Market, price: i128, equity: i128) -> RiskAssessment {
    RiskAssessment {
        long: evaluate_side_risk_state(env, &market.long, true, &market.config, price, equity),
        short: evaluate_side_risk_state(env, &market.short, false, &market.config, price, equity),
        price,
        cash_lp_equity: equity,
    }
}

pub fn apply(
    env: &Env,
    ledger: &mut Ledger,
    symbol: &Symbol,
    actor: &Address,
    market: &mut Market,
    assessment: &RiskAssessment,
) {
    let band = storage::get_global_config(env).hard_cap_relatch_band_bps;
    let config = market.config.clone();
    let (price, equity) = (assessment.price, assessment.cash_lp_equity);
    apply_side(
        env,
        ledger,
        symbol,
        actor,
        &mut market.long,
        true,
        &config,
        &assessment.long,
        price,
        equity,
        band,
    );
    apply_side(
        env,
        ledger,
        symbol,
        actor,
        &mut market.short,
        false,
        &config,
        &assessment.short,
        price,
        equity,
        band,
    );
}

pub fn evaluate_market_risk(
    env: &Env,
    ledger: &mut Ledger,
    symbol: &Symbol,
    actor: &Address,
    market: &mut Market,
    price: i128,
    equity: i128,
) {
    let assessment = assess(env, market, price, equity);
    apply(env, ledger, symbol, actor, market, &assessment);
}

pub fn hard_cap_factor_sum(
    env: &Env,
    markets: &Vec<Symbol>,
    replace: Option<(&Symbol, u32)>,
) -> u64 {
    let mut sum = 0u64;
    let mut replaced = false;
    for symbol in markets.iter() {
        let factor = match replace {
            Some((replace_symbol, factor)) if symbol == *replace_symbol => {
                replaced = true;
                factor
            }
            _ => {
                storage::get_market(env, &symbol)
                    .config
                    .hard_cap_pnl_factor_bps
            }
        };
        sum += factor as u64 * SIDES_PER_MARKET;
    }
    if let Some((_, factor)) = replace {
        if !replaced {
            sum += factor as u64 * SIDES_PER_MARKET;
        }
    }
    sum
}

pub fn effective_collateral(
    env: &Env,
    stored_collateral: i128,
    payable_pnl: i128,
    pending: &crate::funding::PendingFees,
) -> i128 {
    let mut value = math::add(env, stored_collateral, payable_pnl);
    value = math::add(env, value, pending.funding_received);
    value = math::sub(env, value, pending.funding_paid_to_receivers);
    value = math::sub(env, value, pending.funding_paid_to_lps);
    math::sub(env, value, pending.borrow)
}

#[derive(Clone, Copy, Debug)]
pub struct LiquidationAssessment {
    pub liquidatable: bool,
    #[allow(dead_code)]
    pub insolvent: bool,
    pub effective_collateral: i128,
    pub threshold: i128,
    #[allow(dead_code)]
    pub payable_pnl: i128,
}

pub fn evaluate_liquidation(
    env: &Env,
    ledger: &Ledger,
    position: &Position,
    market: &Market,
    price: i128,
) -> LiquidationAssessment {
    let pending = crate::funding::pending_fees(env, ledger, position, market);
    let raw_pnl = math::pnl(
        env,
        position.is_long,
        position.size,
        position.base_exposure,
        price,
    );
    let payable_pnl = payable_pnl(env, raw_pnl, market.side(position.is_long));
    let effective = effective_collateral(env, position.stored_collateral, payable_pnl, &pending);
    let maintenance = maintenance_requirement(env, position.size, &market.config);
    let threshold = core::cmp::max(
        maintenance,
        storage::get_global_config(env).keeper_rewards.liquidation,
    );
    LiquidationAssessment {
        liquidatable: effective <= threshold,
        insolvent: effective < 0,
        effective_collateral: effective,
        threshold,
        payable_pnl,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::constants::PRICE_PRECISION;

    fn env() -> Env {
        Env::default()
    }

    fn latched_side(factor: i128, reference: i128) -> MarketSide {
        MarketSide {
            size_open_interest: 0,
            base_exposure: 0,
            stored_collateral_total: 0,
            risk_units: 0,
            risk_state: RiskState::HardCap,
            hard_cap_payout_factor: factor,
            hard_cap_reference_pnl: reference,
        }
    }

    #[test]
    fn payouts_on_a_latched_side_do_not_depend_on_settlement_order() {
        let e = env();
        let side = latched_side(INDEX_PRECISION * 3 / 4, 80_000);
        let (first, second) = (30_000i128, 12_000i128);

        let a = payable_pnl(&e, first, &side) + payable_pnl(&e, second, &side);
        let b = payable_pnl(&e, second, &side) + payable_pnl(&e, first, &side);
        assert_eq!(a, b);
        assert_eq!(payable_pnl(&e, first, &side), 22_500);
        assert_eq!(payable_pnl(&e, second, &side), 9_000);
    }

    #[test]
    fn a_side_outside_hard_cap_pays_raw_profit_and_losses_always_pass_through() {
        let e = env();
        let mut side = latched_side(INDEX_PRECISION / 2, 1_000);
        side.risk_state = RiskState::Adl;
        assert_eq!(payable_pnl(&e, 10_000, &side), 10_000, "ADL does not scale");
        side.risk_state = RiskState::HardCap;
        assert_eq!(payable_pnl(&e, -10_000, &side), -10_000, "losses pass through");
    }

    #[test]
    fn a_latch_below_the_cap_value_pays_in_full() {
        let e = env();
        let mut side = MarketSide::new();
        side.size_open_interest = 0;
        side.base_exposure = PRICE_PRECISION;
        let config = shared::defaults::market_config();
        snapshot_hard_cap_factor(&e, &mut side, true, &config, PRICE_PRECISION, 1_000_000_000);
        assert_eq!(side.hard_cap_payout_factor, INDEX_PRECISION);
        assert_eq!(side.hard_cap_reference_pnl, PRICE_PRECISION);
    }

    #[test]
    fn a_latch_above_the_cap_value_scales_and_records_its_denominator() {
        let e = env();
        let mut side = MarketSide::new();
        side.base_exposure = 100 * PRICE_PRECISION;
        let config = shared::defaults::market_config();
        let equity = 1_000_000_000i128;
        let cap_value = equity * 600 / 10_000;
        let side_pnl = 100 * PRICE_PRECISION;
        snapshot_hard_cap_factor(&e, &mut side, true, &config, PRICE_PRECISION, equity);
        assert_eq!(side.hard_cap_reference_pnl, side_pnl);
        assert_eq!(
            side.hard_cap_payout_factor,
            cap_value * INDEX_PRECISION / side_pnl
        );
        assert!(side.hard_cap_payout_factor < INDEX_PRECISION);
    }
}
