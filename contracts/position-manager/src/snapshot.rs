//! Accounting snapshot, marked NAV, and the external price read.
//!
//! The protocol owns no oracle. Prices come from a third-party feed through
//! `read_stamped_price`; aggregation across sources is the provider's
//! concern, and what stays here is the staleness bound and the
//! non-positive-price rejection.
//!
//! NAV recognition (§2.3): trader profit in full, trader loss not at all.
//! Marked NAV is `max(cash_lp_equity - Σ max(raw_side_pnl, 0), 0)`, so LP
//! share price understates while traders are collectively winning-on-paper
//! and steps up only as losses are actually realized. That asymmetry is the
//! point: an LP never buys in against a loss the vault has not collected.
//! The loop is over the bounded active-market registry — never over
//! positions (§17).

use soroban_sdk::{panic_with_error, Env, Symbol};

use shared::constants::{BPS, PRICE_PRECISION};
use shared::{AccountingSnapshot, PriceFeedClient, StampedPrice};

use crate::errors::PositionManagerError;
use crate::ledger::Ledger;
use crate::risk;
use crate::{math, storage};

/// Read `symbol`'s price from the external feed, with its observation
/// cursor (§7.0).
///
/// Two checks are this protocol's own and stay here. The feed decides what
/// the price *is*; only this protocol can decide how old a price may be
/// before an action refuses it, and that a non-positive price is never a
/// price.
///
/// The staleness bound is the one parameter that silently widens the window
/// a trader commits against, which is why it is a governed §10 value rather
/// than a constant.
pub fn read_stamped_price(env: &Env, symbol: &Symbol) -> StampedPrice {
    let feed = storage::get_price_feed(env);
    let data = PriceFeedClient::new(env, &feed)
        .lastprice(symbol)
        .unwrap_or_else(|| panic_with_error!(env, PositionManagerError::PriceUnavailable));
    if data.price <= 0 {
        panic_with_error!(env, PositionManagerError::PriceUnavailable);
    }
    let max_age = storage::get_global_config(env).max_price_age_seconds;
    if env.ledger().timestamp() > data.timestamp.saturating_add(max_age) {
        panic_with_error!(env, PositionManagerError::StalePrice);
    }
    StampedPrice {
        price: data.price,
        observed_at: data.timestamp,
    }
}

/// Build the full accounting snapshot for one synchronized oracle round.
///
/// With `mutate_risk` set (LP settlement path, §13.5/§13.6 step 3) each
/// market's risk states are transitioned and persisted; otherwise the states
/// are evaluated hypothetically and only reported. The reported
/// `restricted_side_count` is always the fresh evaluation, not the stored
/// counter.
pub fn build_snapshot(
    env: &Env,
    ledger: &mut Ledger,
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

    let mut i = 0u32;
    while i < markets.len() {
        let symbol = markets.get(i).unwrap();
        // Read inside the loop, inside one transaction: every market is
        // priced at the same ledger timestamp, which is the synchronized
        // snapshot LP accounting needs. `max_active_markets` is what keeps
        // the read count bounded.
        let price = read_stamped_price(env, &symbol).price;
        let mut market = storage::get_market(env, &symbol);

        // §7.2 raw side PnL numerators (one extra PRICE_PRECISION factor),
        // §2.3 recognition: `max(raw_side_pnl, 0)` per side. Unrealized
        // trader loss is not recognized at all — it is not cash the vault
        // holds, and marking it would let an LP deposit buy into profit
        // that has not been collected.
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

        // One pure assessment serves both modes: the LP settlement path
        // persists it, the reporting path only counts it.
        let assessment = risk::assess(env, &market, price, equity);
        if mutate_risk {
            risk::apply(env, ledger, &symbol, &mut market, &assessment);
            storage::save_market(env, &symbol, &market);
        }
        restricted_side_count += assessment.restricted_sides();
        deleveraging_side_count += assessment.deleveraging_sides();
        i += 1;
    }

    // §2.3 marked NAV = max(cash LP equity − recognized trader profit, 0),
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
    }
}
