//! Accounting snapshot and marked NAV — doc §4 (sources of truth) and §7
//! (price exposure and PnL).
//!
//! NAV recognition (§7.3): trader profit in full, trader loss only up to the
//! side's stored collateral aggregate. The loop is over the bounded
//! active-market registry — never over positions (§17).

use soroban_sdk::{panic_with_error, Env, Symbol};

use shared::constants::{BPS, PRICE_PRECISION};
use shared::{AccountingSnapshot, OracleRound, OracleRouterClient, RiskState};

use crate::errors::PositionManagerError;
use crate::ledger::Ledger;
use crate::risk;
use crate::{math, storage};

/// The authenticated cached price for `symbol` from the OracleRouter — used
/// by every position action (§13.1: one canonical price source).
pub fn authenticated_price(env: &Env, symbol: &Symbol) -> i128 {
    let price = OracleRouterClient::new(env, &storage::oracle_router(env)).get_price(symbol);
    if price <= 0 {
        panic_with_error!(env, PositionManagerError::InvalidOracleRound);
    }
    price
}

/// The price for `market` at position `index` of a synchronized round.
/// The round must list prices in active-market order.
pub fn round_price(env: &Env, round: &OracleRound, market: &Symbol, index: u32) -> i128 {
    let item = round
        .prices
        .get(index)
        .unwrap_or_else(|| panic_with_error!(env, PositionManagerError::InvalidOracleRound));
    if item.symbol != *market || item.price <= 0 {
        panic_with_error!(env, PositionManagerError::InvalidOracleRound);
    }
    item.price
}

/// Build the full accounting snapshot for one synchronized oracle round.
///
/// With `mutate_risk` set (LP settlement path, §13.5/§13.6 step 3) each
/// market's risk states are transitioned and persisted; otherwise the states
/// are evaluated hypothetically and only reported. The reported
/// `lp_blocked_side_count` is always the fresh evaluation, not the stored
/// counter.
pub fn build_snapshot(
    env: &Env,
    ledger: &mut Ledger,
    round: &OracleRound,
    physical: i128,
    mutate_risk: bool,
) -> AccountingSnapshot {
    let markets = storage::active_markets(env);
    if round.prices.len() != markets.len() {
        panic_with_error!(env, PositionManagerError::InvalidOracleRound);
    }
    let claims = ledger.non_lp_claims(env);
    let shortfall = core::cmp::max(math::sub(env, claims, physical), 0);
    let equity = ledger.cash_lp_equity(env, physical);
    let mut aggregate_pnl_numerator = 0i128;
    let mut blocked_side_count = 0u32;

    let mut i = 0u32;
    while i < markets.len() {
        let symbol = markets.get(i).unwrap();
        let price = round_price(env, round, &symbol, i);
        let mut market = storage::market(env, &symbol)
            .unwrap_or_else(|| panic_with_error!(env, PositionManagerError::MarketNotConfigured));

        // §7.2 raw side PnL numerators (one extra PRICE_PRECISION factor),
        // §7.3 recognition: profit in full, loss capped at side collateral.
        let long_num = math::sub(
            env,
            math::mul(env, market.long.base_exposure, price),
            math::mul(env, market.long.size_open_interest, PRICE_PRECISION),
        );
        let short_num = math::sub(
            env,
            math::mul(env, market.short.size_open_interest, PRICE_PRECISION),
            math::mul(env, market.short.base_exposure, price),
        );
        let long_recognized = if long_num >= 0 {
            long_num
        } else {
            -core::cmp::min(
                long_num.abs(),
                math::mul(env, market.long.stored_collateral_total, PRICE_PRECISION),
            )
        };
        let short_recognized = if short_num >= 0 {
            short_num
        } else {
            -core::cmp::min(
                short_num.abs(),
                math::mul(env, market.short.stored_collateral_total, PRICE_PRECISION),
            )
        };
        aggregate_pnl_numerator = math::add(
            env,
            aggregate_pnl_numerator,
            math::add(env, long_recognized, short_recognized),
        );

        if mutate_risk {
            risk::evaluate_market_risk(env, ledger, &mut market, price, equity);
            storage::save_market(env, &symbol, &market);
        } else {
            market.long.risk_state = risk::risk_state_for(
                env,
                market.long.risk_state,
                core::cmp::max(
                    math::pnl(
                        env,
                        true,
                        market.long.size_open_interest,
                        market.long.base_exposure,
                        price,
                    ),
                    0,
                ),
                equity,
                &market.config,
            );
            market.short.risk_state = risk::risk_state_for(
                env,
                market.short.risk_state,
                core::cmp::max(
                    math::pnl(
                        env,
                        false,
                        market.short.size_open_interest,
                        market.short.base_exposure,
                        price,
                    ),
                    0,
                ),
                equity,
                &market.config,
            );
        }
        if market.long.risk_state != RiskState::Normal {
            blocked_side_count += 1;
        }
        if market.short.risk_state != RiskState::Normal {
            blocked_side_count += 1;
        }
        i += 1;
    }

    // §18.6 marked NAV = max(cash LP equity − recognized trader PnL, 0),
    // converted to cash exactly once.
    let nav_num = math::sub(
        env,
        math::mul(env, equity, PRICE_PRECISION),
        aggregate_pnl_numerator,
    );
    let nav = if nav_num <= 0 {
        0
    } else {
        nav_num / PRICE_PRECISION
    };
    let config = storage::global_config(env);
    let required = if ledger.total_risk_units == 0 {
        0
    } else {
        math::mul_div_ceil(
            env,
            ledger.total_risk_units,
            BPS,
            config.risk_capacity_limit_bps as i128,
        )
    };
    AccountingSnapshot {
        physical_cash: physical,
        non_lp_claims: claims,
        cash_lp_equity: equity,
        cash_shortfall: shortfall,
        required_risk_backing: required,
        free_lp_capital: core::cmp::max(
            math::sub(env, equity, core::cmp::min(equity, required)),
            0,
        ),
        vault_nav: nav,
        total_risk_units: ledger.total_risk_units,
        open_position_count: ledger.open_position_count,
        lp_blocked_side_count: blocked_side_count,
    }
}
