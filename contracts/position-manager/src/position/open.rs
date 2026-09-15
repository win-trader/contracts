use crate::{
    auth::{require_auth, require_initialized, require_market_active, require_not_paused},
    borrow,
    errors::PositionManagerError,
    events, funding, ledger, math, risk, snapshot, storage,
    validation::{check_slippage, validate_orders},
};
use shared::{MarketConfig, Position, RiskState};
use soroban_sdk::{panic_with_error, Address, Env, Symbol};

fn require_valid_input(env: &Env, size: i128, collateral: i128) {
    if size <= 0 || collateral <= 0 {
        panic_with_error!(&env, PositionManagerError::InvalidAmount);
    }
}

pub(crate) fn require_valid_open_input(env: &Env, size: i128, collateral: i128) {
    require_valid_input(env, size, collateral);
}

/// §12.3 — the collateral a position of `size` needs to open: at least the
/// global minimum and at least the initial margin. Price-independent (the
/// requirement is a share of notional), so it is checked identically at a
/// market open and when placing an entry order.
pub(crate) fn require_sufficient_collateral(
    env: &Env,
    collateral: i128,
    market_config: &MarketConfig,
    size: i128,
) {
    let min_collateral = storage::get_global_config(env).min_collateral;
    let risk_requirement = risk::initial_requirement(env, size, market_config);
    if collateral < min_collateral || collateral < risk_requirement {
        panic_with_error!(env, PositionManagerError::InsufficientCollateral);
    }
}

pub fn open_position(
    env: Env,
    owner: Address,
    market_symbol: Symbol,
    is_long: bool,
    size: i128,
    collateral: i128,
    take_profit: i128,
    stop_loss: i128,
    acceptable_price: i128,
) -> u64 {
    require_initialized(&env);
    require_auth(&owner);
    require_not_paused(&env);
    require_valid_input(&env, size, collateral);
    require_market_active(&env, &market_symbol);

    let mut market = storage::get_market(&env, &market_symbol);
    let mut ledger = storage::get_ledger(&env);

    let now = env.ledger().timestamp();

    borrow::accrue(&env, &mut ledger, now);
    funding::accrue(&env, &mut ledger, &mut market, now);

    let price = snapshot::authenticated_price(&env, &market_symbol);

    check_slippage(&env, is_long, true, price, acceptable_price);
    validate_orders(&env, is_long, take_profit, stop_loss, price);

    // Market open: the owner is present, so pull the collateral from them
    // directly. The fill path pulls via allowance instead, then joins the
    // shared core below.
    ledger::receive(&env, &owner, collateral);

    open_from_collateral(
        env,
        ledger,
        market,
        owner,
        market_symbol,
        is_long,
        size,
        collateral,
        take_profit,
        stop_loss,
        price,
        now,
    )
}

/// The shared open core, from "the cash is already in the vault" onward:
/// label the funds, build and store the position, run every risk gate, and
/// emit the open event. Used by both the market `open_position` (after a
/// direct `ledger::receive`) and the entry-order fill (after an allowance
/// pull). The `RiskState::Normal` gate, capacity, and market-limit checks
/// panic on violation — a full transaction rollback that returns the
/// already-pulled cash.
#[allow(clippy::too_many_arguments)]
pub(crate) fn open_from_collateral(
    env: Env,
    mut ledger: crate::ledger::Ledger,
    mut market: shared::Market,
    owner: Address,
    market_symbol: Symbol,
    is_long: bool,
    size: i128,
    collateral: i128,
    take_profit: i128,
    stop_loss: i128,
    price: i128,
    now: u64,
) -> u64 {
    let position_id = storage::get_next_position_id(&env);
    storage::update_position_id(&env);

    let base = math::base_added(&env, size, price, is_long);
    let risk_units = math::risk_added(&env, size, market.config.market_risk_factor_bps);
    let mut position = Position {
        id: position_id,
        owner: owner.clone(),
        market: market_symbol.clone(),
        is_long,
        size,
        base_exposure: base,
        stored_collateral: 0,
        risk_units,
        borrow_debt: 0,
        funding_paid_to_receivers_debt: 0,
        funding_paid_to_lps_debt: 0,
        funding_received_debt: 0,
        last_increased_time: now,
        take_profit,
        stop_loss,
    };

    ledger::add_stored_collateral(
        &env,
        &mut ledger,
        &mut position,
        market.side_mut(is_long),
        collateral,
    );

    require_sufficient_collateral(&env, position.stored_collateral, &market.config, size);

    let physical = ledger::physical_cash(&env);
    let equity = ledger.cash_lp_equity(&env, physical);

    risk::evaluate_market_risk(
        &env,
        &mut ledger,
        &market_symbol,
        &mut market,
        price,
        equity,
    );

    if market.side(is_long).risk_state != RiskState::Normal {
        panic_with_error!(&env, PositionManagerError::RiskStateBlocked);
    }

    let was_empty = market.long.size_open_interest == 0 && market.short.size_open_interest == 0;

    {
        let side = market.side_mut(is_long);
        side.size_open_interest = math::add(&env, side.size_open_interest, size);
        side.base_exposure = math::add(&env, side.base_exposure, base);
        side.risk_units = math::add(&env, side.risk_units, risk_units);
    }

    if was_empty {
        funding::cold_start(&env, &mut market);
    }

    risk::register_exposure(&env, &mut ledger, risk_units);
    risk::enforce_capacity(&env, &ledger, physical, ledger.total_risk_units);
    risk::enforce_market_limits(&env, &market, is_long);
    funding::reset_debts(&env, &ledger, &mut position, &market);
    storage::save_position(&env, &position);
    risk::register_position(&mut ledger);
    funding::refresh_display(&env, &mut market);
    storage::save_market(&env, &market_symbol, &market);
    borrow::refresh_rate(&env, &mut ledger, physical);
    storage::save_ledger(&env, &ledger);

    events::emit_opened(&env, &position, price);

    position_id
}
