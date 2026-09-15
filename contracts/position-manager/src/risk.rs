//! The risk register and per-side emergency states — doc §9.1 (capacity)
//! and §14 (emergency payout and ADL).
//!
//! This module owns everything "how exposed are we": the global risk
//! counters (via the register/release mutators), the margin floors, the
//! side risk-state machine split into pure `assess` and effectful `apply`,
//! and the §14 hard-cap payout factor.

use soroban_sdk::{panic_with_error, Env, Symbol, Vec};

use shared::constants::{BPS, INDEX_PRECISION};
use shared::{Market, MarketConfig, MarketSide, Position, RiskState};

use crate::errors::PositionManagerError;
use crate::events;
use crate::ledger::Ledger;
use crate::{math, storage};

/// Both sides of one market can reach their hard cap simultaneously, so a
/// market contributes twice its per-side factor to the global bound (§14:
/// "Limit the sum of all side hard-cap factors").
const SIDES_PER_MARKET: u64 = 2;

// ---------------------------------------------------------------------------
// The risk register: the global counters, mutated only here.
// ---------------------------------------------------------------------------

/// §9.1 — register newly opened exposure in the global risk register.
pub fn register_exposure(env: &Env, ledger: &mut Ledger, risk_units: i128) {
    ledger.total_risk_units = math::add(env, ledger.total_risk_units, risk_units);
}

/// §9.1 — remove settled exposure from the global risk register.
pub fn release_exposure(env: &Env, ledger: &mut Ledger, risk_units: i128) {
    ledger.total_risk_units = math::sub(env, ledger.total_risk_units, risk_units);
}

/// Count a newly opened position.
pub fn register_position(ledger: &mut Ledger) {
    ledger.open_position_count += 1;
}

/// Count a fully closed position. Underflow is an invariant break — more
/// positions released than registered — so it reverts rather than clamping
/// (§2.1.1).
pub fn release_position(env: &Env, ledger: &mut Ledger) {
    ledger.open_position_count = ledger
        .open_position_count
        .checked_sub(1)
        .unwrap_or_else(|| panic_with_error!(env, PositionManagerError::InvariantViolation));
}

/// §9.1 — the global capacity gate: new total risk must stay within the
/// configured share of cash LP equity.
pub fn enforce_capacity(env: &Env, ledger: &Ledger, physical_cash: i128, risk_after: i128) {
    let config = storage::get_global_config(env);
    let limit = math::mul_div_floor(
        env,
        ledger.cash_lp_equity(env, physical_cash),
        config.risk_capacity_limit_bps as i128,
        BPS,
    );
    if risk_after > limit {
        panic_with_error!(env, PositionManagerError::CapacityExceeded);
    }
}

/// §9.1 — hard per-side size and base-exposure caps.
pub fn enforce_market_limits(env: &Env, market: &Market, is_long: bool) {
    let side = market.side(is_long);
    let (size_cap, base_cap) = if is_long {
        (
            market.config.max_long_size_open_interest,
            market.config.max_long_base_exposure,
        )
    } else {
        (
            market.config.max_short_size_open_interest,
            market.config.max_short_base_exposure,
        )
    };
    if side.size_open_interest > size_cap || side.base_exposure > base_cap {
        panic_with_error!(env, PositionManagerError::MarketLimitExceeded);
    }
}

/// §12.3 — margin requirement for a position of `size` at `margin_bps`.
pub fn margin_requirement(env: &Env, size: i128, margin_bps: u32) -> i128 {
    math::mul_div_ceil(env, size, margin_bps as i128, BPS)
}

/// §12.3 — the floor below which a position of `size` is liquidatable.
pub fn maintenance_requirement(env: &Env, size: i128, config: &MarketConfig) -> i128 {
    margin_requirement(env, size, config.maintenance_margin_bps)
}

/// §12.3 — the margin a position of `size` needs to open or add risk.
pub fn initial_requirement(env: &Env, size: i128, config: &MarketConfig) -> i128 {
    margin_requirement(env, size, config.initial_margin_bps)
}

/// §12.3 — the margin floor a position of `size` must clear after an
/// action: anything that adds risk (new size, or a pure collateral
/// withdrawal raising leverage) re-underwrites at the initial margin;
/// pure de-risking (shrinking, or topping up collateral) needs only the
/// maintenance floor.
pub fn required_margin(env: &Env, size: i128, config: &MarketConfig, adds_risk: bool) -> i128 {
    if adds_risk {
        initial_requirement(env, size, config)
    } else {
        maintenance_requirement(env, size, config)
    }
}

/// §6.5 — positive raw PnL after the side's **stored** payout factor, and
/// nothing else. Negative PnL always passes through.
///
/// The factor is **read, not recomputed**. Each settlement pays out and
/// removes exposure, so both cash LP equity and the side's aggregate
/// positive PnL move; a factor derived again afterwards would measure the
/// next position against a shrunken denominator, payouts would become
/// order-dependent, and their sum would bear no relation to the cap. With
/// one snapshot every position on the side is scaled by the same number and
/// settlement order cannot change any individual outcome.
///
/// The payment-time cash limit is deliberately **not** here — see
/// `settle::apply_payable_pnl`. A position's health is a property of that
/// position; it must not change because the vault is temporarily short of
/// cash.
pub fn payable_pnl(env: &Env, raw_pnl: i128, side: &MarketSide) -> i128 {
    if raw_pnl <= 0 || side.risk_state != RiskState::HardCap {
        return raw_pnl;
    }
    math::mul_div_floor(env, raw_pnl, side.hard_cap_payout_factor, INDEX_PRECISION)
}

/// §6.5 — record the payout factor and the denominator it was measured
/// against. Serves both the initial latch and every later re-latch.
///
/// Capped at `INDEX_PRECISION`: a side entering `HardCap` while its
/// aggregate profit is still below the cap simply pays in full until that
/// changes.
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

/// §6.16.1 — may this side take on more exposure?
///
/// Two behaviours are deliberate. `Warning` **does not block**: it is a
/// latch that makes recovery sticky, keeping a side restricted until its
/// factor falls below `recovery_pnl_factor_bps` so it cannot oscillate on
/// small price moves. And the pause folds in here rather than being checked
/// separately at every call site, so pausing behaves as a vault-wide
/// restricted state.
///
/// The parameter is one side, which is the whole of §6.16's rule that the
/// opposite side is never restricted by this side's state: opening against a
/// restricted side reduces its net aggregate PnL, which is the trade that
/// resolves the condition. Blocking both sides would leave closure as the
/// only path back to `Normal`.
pub fn side_accepts_new_exposure(env: &Env, side: &MarketSide) -> bool {
    !storage::is_paused(env)
        && matches!(side.risk_state, RiskState::Normal | RiskState::Warning)
}

/// §6.16 — one side's assessment at an observed price and equity.
#[derive(Clone, Copy, Debug)]
pub struct SideRiskAssessment {
    pub previous_state: RiskState,
    pub next_state: RiskState,
    pub positive_pnl: i128,
}

/// §6.16 — the state a side belongs in given its positive PnL factor.
/// States latch: a restricted side stays at least `Warning` until the factor
/// falls below the recovery threshold.
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

/// §5.11 — keep `restricted_market_side_count` equal to the number of
/// restricted sides across a state transition.
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

/// §6.16 — apply one side's transition, including the two that move the
/// payout factor.
fn apply_side(
    env: &Env,
    ledger: &mut Ledger,
    symbol: &Symbol,
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
            // Entering: latch from the book as it stands right now.
            snapshot_hard_cap_factor(env, side, is_long, config, price, cash_lp_equity);
        } else {
            // §6.16 re-latch band. **One-directional**: it fires only when
            // the side's positive PnL has *grown* past the denominator the
            // current factor was measured against. A side whose profit
            // falls keeps its factor — lowering the denominator would raise
            // the factor and pay later exits more than earlier ones.
            //
            // Each re-latch reads the current equity, which is lower for
            // every payout already made, so the replacement factor is lower
            // than the one it replaces. That is what bounds a side whose
            // profit keeps running at `(BPS + band) / BPS` of the cap
            // measured at the most recent latch, instead of paying an
            // ever-larger multiple of a cap nobody re-measured.
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
        // Leaving clears both. A side that re-enters later takes a fresh
        // snapshot from the book at that moment.
        side.hard_cap_payout_factor = INDEX_PRECISION;
        side.hard_cap_reference_pnl = 0;
    }

    if next != previous {
        events::emit_risk_state_changed(env, symbol, is_long, next);
    }
    side.risk_state = next;
}

/// The pure output of one §6.16 evaluation for both sides. Nothing is
/// mutated until `apply`.
#[derive(Clone, Copy, Debug)]
pub struct RiskAssessment {
    pub long: SideRiskAssessment,
    pub short: SideRiskAssessment,
    /// The price and equity the assessment was measured against. Any latch
    /// `apply` triggers uses exactly these, so eligibility and the factor
    /// can never come from different snapshots.
    price: i128,
    cash_lp_equity: i128,
}

impl RiskAssessment {
    /// The number of restricted sides under this assessment.
    pub fn restricted_sides(&self) -> u32 {
        (self.long.next_state != RiskState::Normal) as u32
            + (self.short.next_state != RiskState::Normal) as u32
    }
}

/// §6.16 — compute both sides' states at `price` against `equity`. Pure.
pub fn assess(env: &Env, market: &Market, price: i128, equity: i128) -> RiskAssessment {
    RiskAssessment {
        long: evaluate_side_risk_state(env, &market.long, true, &market.config, price, equity),
        short: evaluate_side_risk_state(env, &market.short, false, &market.config, price, equity),
        price,
        cash_lp_equity: equity,
    }
}

/// §6.16 — apply an assessment: transition both sides, maintain the
/// restricted-side count, move the payout factors, and emit per changed
/// side.
pub fn apply(
    env: &Env,
    ledger: &mut Ledger,
    symbol: &Symbol,
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
        &mut market.short,
        false,
        &config,
        &assessment.short,
        price,
        equity,
        band,
    );
}

/// §14 — assess-then-apply in one call, for the entry points that always
/// persist the evaluation.
pub fn evaluate_market_risk(
    env: &Env,
    ledger: &mut Ledger,
    symbol: &Symbol,
    market: &mut Market,
    price: i128,
    equity: i128,
) {
    let assessment = assess(env, market, price, equity);
    apply(env, ledger, symbol, market, &assessment);
}

/// §14 — the sum of every market's hard-cap factor contribution, with an
/// optional replacement factor for one symbol (used when validating a config
/// change before it is stored). A `replace` symbol not yet in `markets` is
/// counted as an addition.
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

/// §6.6 — effective collateral: stored collateral, plus payable PnL and
/// funding received, less every pending obligation.
///
/// **One formula**, used identically by previews, admission checks,
/// liquidation eligibility, and final settlement. Pending borrow already
/// includes the active window's minimum (§6.3). A proposed action subtracts
/// its fixed keeper reward and any other charge that must be paid before the
/// post-action health check; the closing fee is not included in liquidation
/// health, because liquidation pays no closing fee and a voluntary closing
/// fee cannot consume original collateral.
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

/// §6.15 — one assessment, reused by liquidation settlement so eligibility
/// and payment cannot use different prices or fee snapshots.
#[derive(Clone, Copy, Debug)]
pub struct LiquidationAssessment {
    pub liquidatable: bool,
    /// Reported by §12.6's liquidation event; §7.13 also reads it to decide
    /// whether the close books bad debt.
    #[allow(dead_code)]
    pub insolvent: bool,
    #[allow(dead_code)]
    pub effective_collateral: i128,
    #[allow(dead_code)]
    pub threshold: i128,
    #[allow(dead_code)]
    pub payable_pnl: i128,
}

/// §6.15 — is this position liquidatable at `price`?
///
/// Two differences from the superseded check. The threshold is
/// `max(maintenance, keeper_liquidation_reward)`, so the reward is reserved
/// rather than assumed available; and the comparison is `effective <=
/// threshold`, not a strict inequality against maintenance alone — a
/// position sitting exactly at maintenance was previously not liquidatable.
///
/// The caller must have checkpointed the indices and refreshed the side risk
/// state to this timestamp and price first (§6.5): the payout factor this
/// reads is only as current as the last action that touched the market.
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

    /// A long side holding `size` notional against `base` units of the
    /// underlying, latched in `HardCap` with the given factor.
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

    /// The property the stored factor exists for: two winners on the same
    /// latched side are scaled by the same number whatever order they
    /// settle in.
    ///
    /// A factor recomputed per settlement would measure the second position
    /// against a denominator both the first payout and the first exposure
    /// removal had already shrunk, so the two orders would disagree.
    #[test]
    fn payouts_on_a_latched_side_do_not_depend_on_settlement_order() {
        let e = env();
        let side = latched_side(INDEX_PRECISION * 3 / 4, 80_000);
        let (first, second) = (30_000i128, 12_000i128);

        let a = payable_pnl(&e, first, &side) + payable_pnl(&e, second, &side);
        let b = payable_pnl(&e, second, &side) + payable_pnl(&e, first, &side);
        assert_eq!(a, b);
        // And the factor is actually applied: three quarters of each.
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

    /// §6.5 — the snapshot cannot exceed INDEX_PRECISION, so a side that
    /// enters HardCap while its aggregate profit is still under the cap
    /// simply pays in full until that changes.
    #[test]
    fn a_latch_below_the_cap_value_pays_in_full() {
        let e = env();
        let mut side = MarketSide::new();
        // 1 base unit against 0 size: raw PnL = price, i.e. $1 of profit.
        side.size_open_interest = 0;
        side.base_exposure = PRICE_PRECISION;
        let config = shared::defaults::market_config();
        // Equity large enough that 6% of it dwarfs the side's $1 profit.
        snapshot_hard_cap_factor(&e, &mut side, true, &config, PRICE_PRECISION, 1_000_000_000);
        assert_eq!(side.hard_cap_payout_factor, INDEX_PRECISION);
        assert_eq!(side.hard_cap_reference_pnl, PRICE_PRECISION);
    }

    /// §6.5 — above the cap the factor is `cap_value / side_positive_pnl`,
    /// and the denominator it used is recorded for the §6.16 band.
    #[test]
    fn a_latch_above_the_cap_value_scales_and_records_its_denominator() {
        let e = env();
        let mut side = MarketSide::new();
        side.base_exposure = 100 * PRICE_PRECISION;
        let config = shared::defaults::market_config();
        // hard_cap_pnl_factor_bps is 600 (6%), so the cap value is 6% of
        // equity. With equity 100 units of profit-scale cash, cap = 6.
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
