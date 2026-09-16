use soroban_sdk::{panic_with_error, Address, Env, Symbol};

use shared::constants::{BPS, PRICE_PRECISION};
use shared::{AccountingSnapshot, PriceFeedClient, StampedPrice};

use crate::errors::PositionManagerError;
use crate::ledger::Ledger;
use crate::risk;
use crate::{math, storage};

pub fn read_stamped_price(env: &Env, symbol: &Symbol) -> StampedPrice {
    let feed = storage::get_price_feed(env);
    let data = match PriceFeedClient::new(env, &feed).try_lastprice(symbol) {
        Ok(Ok(Some(data))) => data,
        _ => panic_with_error!(env, PositionManagerError::PriceUnavailable),
    };
    let now = env.ledger().timestamp();
    if data.price <= 0 || data.timestamp > now {
        panic_with_error!(env, PositionManagerError::PriceUnavailable);
    }
    let max_age = storage::get_global_config(env).max_price_age_seconds;
    if now > data.timestamp.saturating_add(max_age) {
        panic_with_error!(env, PositionManagerError::StalePrice);
    }
    StampedPrice {
        price: data.price,
        observed_at: data.timestamp,
    }
}

pub fn build_snapshot(
    env: &Env,
    ledger: &mut Ledger,
    actor: &Address,
    physical: i128,
    mutate_risk: bool,
) -> AccountingSnapshot {
    let markets = storage::get_active_markets(env);
    let claims = ledger.non_lp_claims(env);
    let shortfall = core::cmp::max(math::sub(env, claims, physical), 0);
    let equity = ledger.cash_lp_equity(env, physical);
    let mut aggregate_pnl_numerator = 0i128;
    let mut restricted_side_count = 0u32;
    let mut deleveraging_side_count = 0u32;
    let mut min_equity_clear_of_adl = 0i128;

    let mut i = 0u32;
    while i < markets.len() {
        let symbol = markets.get(i).unwrap();
        let price = read_stamped_price(env, &symbol).price;
        let mut market = storage::get_market(env, &symbol);

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
        aggregate_pnl_numerator = math::add(
            env,
            aggregate_pnl_numerator,
            math::add(
                env,
                core::cmp::max(long_num, 0),
                core::cmp::max(short_num, 0),
            ),
        );

        let assessment = risk::assess(env, &market, price, equity);
        if mutate_risk {
            risk::apply(env, ledger, &symbol, actor, &mut market, &assessment);
            storage::save_market(env, &symbol, &market);
        }
        restricted_side_count += assessment.restricted_sides();
        deleveraging_side_count += assessment.deleveraging_sides();
        for side_pnl in [assessment.long.positive_pnl, assessment.short.positive_pnl] {
            min_equity_clear_of_adl = core::cmp::max(
                min_equity_clear_of_adl,
                equity_clear_of_adl(env, side_pnl, market.config.adl_pnl_factor_bps),
            );
        }
        i += 1;
    }

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
    let config = storage::get_global_config(env);
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
        restricted_side_count,
        deleveraging_side_count,
        min_equity_clear_of_adl,
    }
}

fn equity_clear_of_adl(env: &Env, positive_pnl: i128, adl_pnl_factor_bps: u32) -> i128 {
    if positive_pnl <= 0 {
        return 0;
    }
    math::add(
        env,
        math::mul_div_floor(env, positive_pnl, BPS, adl_pnl_factor_bps as i128),
        1,
    )
}
