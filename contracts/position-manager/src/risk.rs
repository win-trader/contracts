//! The risk register and per-side emergency states — doc §9.1 (capacity)
//! and §14 (emergency payout and ADL).
//!
//! This module owns everything "how exposed are we": the global risk
//! counters (via the register/release mutators), the margin floors, the
//! side risk-state machine split into pure `assess` and effectful `apply`,
//! and the §14 hard-cap payout factor.

use soroban_sdk::{panic_with_error, Env, Symbol, Vec};

use shared::constants::BPS;
use shared::{Market, MarketConfig, Position, RiskState};

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

/// Count a fully closed position.
pub fn release_position(ledger: &mut Ledger) {
    ledger.open_position_count = ledger.open_position_count.saturating_sub(1);
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

/// §14 — positive price PnL after the hard-cap payout factor. Below the
/// hard-cap state the raw PnL passes through; at hard cap every profitable
/// position on the side is scaled by the same factor from one price
/// snapshot. Negative PnL always passes through.
pub fn payable_pnl(
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

/// §14 — the risk state a side belongs in given its positive PnL factor.
/// States latch: a restricted side stays at least `Warning` until the factor
/// falls below the recovery threshold.
pub fn risk_state_for(
    env: &Env,
    current: RiskState,
    positive_pnl: i128,
    equity: i128,
    config: &MarketConfig,
) -> RiskState {
    let factor = if positive_pnl == 0 {
        0
    } else if equity == 0 {
        BPS
    } else {
        math::mul_div_floor(env, positive_pnl, BPS, equity)
    };
    if factor >= config.hard_cap_pnl_factor_bps as i128 {
        RiskState::HardCap
    } else if factor >= config.adl_pnl_factor_bps as i128 {
        RiskState::Adl
    } else if factor >= config.warning_pnl_factor_bps as i128 {
        RiskState::Warning
    } else if current != RiskState::Normal && factor >= config.recovery_pnl_factor_bps as i128 {
        RiskState::Warning
    } else {
        RiskState::Normal
    }
}

/// §14 — keep `lp_blocked_side_count` equal to the number of restricted
/// sides (§18.8) across a state transition.
pub fn update_blocked_count(ledger: &mut Ledger, old: RiskState, new: RiskState) {
    if old == RiskState::Normal && new != RiskState::Normal {
        ledger.lp_blocked_side_count += 1;
    } else if old != RiskState::Normal && new == RiskState::Normal {
        ledger.lp_blocked_side_count = ledger.lp_blocked_side_count.saturating_sub(1);
    }
}

/// The pure output of one §14 risk evaluation: the state each side
/// belongs in at the observed price and equity. Nothing is mutated until
/// `apply`.
#[derive(Clone, Copy, Debug)]
pub struct RiskAssessment {
    pub long: RiskState,
    pub short: RiskState,
}

impl RiskAssessment {
    /// The number of restricted sides under this assessment.
    pub fn blocked_sides(&self) -> u32 {
        (self.long != RiskState::Normal) as u32 + (self.short != RiskState::Normal) as u32
    }
}

/// §14 — compute both sides' risk states at `price` against `equity`.
/// Pure: reads the market, mutates nothing.
pub fn assess(env: &Env, market: &Market, price: i128, equity: i128) -> RiskAssessment {
    let long_pnl = core::cmp::max(
        math::pnl(
            env,
            true,
            market.long.size_open_interest,
            market.long.base_exposure,
            price,
        ),
        0,
    );
    let short_pnl = core::cmp::max(
        math::pnl(
            env,
            false,
            market.short.size_open_interest,
            market.short.base_exposure,
            price,
        ),
        0,
    );
    RiskAssessment {
        long: risk_state_for(
            env,
            market.long.risk_state,
            long_pnl,
            equity,
            &market.config,
        ),
        short: risk_state_for(
            env,
            market.short.risk_state,
            short_pnl,
            equity,
            &market.config,
        ),
    }
}

/// §14 — apply an assessment: transition the market's side states and the
/// blocked-side count, emitting an event per side that actually changed.
pub fn apply(
    env: &Env,
    ledger: &mut Ledger,
    symbol: &Symbol,
    market: &mut Market,
    assessment: &RiskAssessment,
) {
    update_blocked_count(ledger, market.long.risk_state, assessment.long);
    update_blocked_count(ledger, market.short.risk_state, assessment.short);
    if assessment.long != market.long.risk_state {
        events::emit_risk_state_changed(env, symbol, true, assessment.long);
    }
    if assessment.short != market.short.risk_state {
        events::emit_risk_state_changed(env, symbol, false, assessment.short);
    }
    market.long.risk_state = assessment.long;
    market.short.risk_state = assessment.short;
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
