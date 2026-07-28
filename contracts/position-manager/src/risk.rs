//! Risk capacity, borrow rate, and per-side emergency states — doc §9
//! (borrow mechanics) and §14 (emergency payout and ADL).

use soroban_sdk::{panic_with_error, Env, Symbol, Vec};

use shared::constants::BPS;
use shared::{Market, MarketConfig, RiskState};

use crate::errors::PositionManagerError;
use crate::events::RiskStateChanged;
use crate::ledger::Ledger;
use crate::{math, storage};

/// Both sides of one market can reach their hard cap simultaneously, so a
/// market contributes twice its per-side factor to the global bound (§14:
/// "Limit the sum of all side hard-cap factors").
const SIDES_PER_MARKET: u64 = 2;

/// §9.2 — recompute the stored borrow rate from current utilization. Call
/// after any mutation that changes risk units or cash LP equity (§10.3
/// step 7).
pub fn refresh_rate(env: &Env, ledger: &mut Ledger, physical_cash: i128) {
    let config = storage::global_config(env);
    let utilization = math::utilization_bps(
        env,
        ledger.total_risk_units,
        ledger.cash_lp_equity(env, physical_cash),
    );
    ledger.current_borrow_rate = math::borrow_rate(
        env,
        config.base_borrow_rate_bps_day,
        config.max_variable_borrow_bps_day,
        utilization,
    );
}

/// §9.1 — the global capacity gate: new total risk must stay within the
/// configured share of cash LP equity.
pub fn enforce_capacity(env: &Env, ledger: &Ledger, physical_cash: i128, risk_after: i128) {
    let config = storage::global_config(env);
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

/// §12.3 — maintenance margin requirement for a position of `size`.
pub fn maintenance_requirement(env: &Env, size: i128, config: &MarketConfig) -> i128 {
    math::mul_div_ceil(env, size, config.maintenance_margin_bps as i128, BPS)
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

/// §14 — evaluate both sides' risk states at `price` and apply the
/// transitions to the market and the blocked-side count, emitting an event
/// per side that actually changed state.
pub fn evaluate_market_risk(
    env: &Env,
    ledger: &mut Ledger,
    symbol: &Symbol,
    market: &mut Market,
    price: i128,
    equity: i128,
) {
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
    let long_new = risk_state_for(
        env,
        market.long.risk_state,
        long_pnl,
        equity,
        &market.config,
    );
    let short_new = risk_state_for(
        env,
        market.short.risk_state,
        short_pnl,
        equity,
        &market.config,
    );
    update_blocked_count(ledger, market.long.risk_state, long_new);
    update_blocked_count(ledger, market.short.risk_state, short_new);
    if long_new != market.long.risk_state {
        RiskStateChanged {
            market: symbol.clone(),
            is_long: true,
            state: long_new,
        }
        .publish(env);
    }
    if short_new != market.short.risk_state {
        RiskStateChanged {
            market: symbol.clone(),
            is_long: false,
            state: short_new,
        }
        .publish(env);
    }
    market.long.risk_state = long_new;
    market.short.risk_state = short_new;
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
                storage::market(env, &symbol)
                    .unwrap_or_else(|| {
                        panic_with_error!(env, PositionManagerError::MarketNotConfigured)
                    })
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
