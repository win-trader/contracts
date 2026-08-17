use crate::{
    auth::{require_auth, require_initialized, require_market_active, require_not_paused},
    borrow,
    errors::PositionManagerError,
    events, fees, funding, ledger, math, risk, snapshot, storage, validation,
};
use shared::RiskState;
use soroban_sdk::{panic_with_error, Env};

fn require_valid_input(env: &Env, size_added: i128, collateral_added: i128) {
    if size_added < 0 || collateral_added < 0 || (size_added == 0 && collateral_added == 0) {
        panic_with_error!(&env, PositionManagerError::InvalidAmount);
    }
}

pub fn increase_position(
    env: Env,
    position_id: u64,
    size_added: i128,
    collateral_added: i128,
    acceptable_price: i128,
) {
    require_initialized(&env);
    require_valid_input(&env, size_added, collateral_added);
    require_not_paused(&env);

    let mut position = storage::get_position(&env, position_id);
    let mut market = storage::get_market(&env, &position.market);
    let mut ledger = storage::get_ledger(&env);
    let now = env.ledger().timestamp();
    let price = snapshot::authenticated_price(&env, &position.market);

    require_auth(&position.owner);
    require_market_active(&env, &position.market);

    borrow::accrue(&env, &mut ledger, now);
    funding::accrue(&env, &mut ledger, &mut market, now);

    validation::check_slippage(&env, position.is_long, true, price, acceptable_price);

    // can we increase positons without adding colleteral?
    if collateral_added > 0 {
        ledger::receive(&env, &position.owner, collateral_added);
        let is_long = position.is_long;

        ledger::add_stored_collateral(
            &env,
            &mut ledger,
            &mut position,
            market.side_mut(is_long),
            collateral_added,
        );
    }

    let collected = fees::capitalize(&env, &mut ledger, &mut position, &mut market, 0);

    if collected.unpaid > 0 {
        panic_with_error!(&env, PositionManagerError::InsufficientCollateral);
    }

    let base = math::base_added(&env, size_added, price, position.is_long);
    let risk_units = math::risk_added(&env, size_added, market.config.market_risk_factor_bps);
    let physical = ledger::physical_cash(&env);
    let equity = ledger.cash_lp_equity(&env, physical);
    risk::evaluate_market_risk(
        &env,
        &mut ledger,
        &position.market,
        &mut market,
        price,
        equity,
    );
    if size_added > 0 && market.side(position.is_long).risk_state != RiskState::Normal {
        panic_with_error!(&env, PositionManagerError::RiskStateBlocked);
    }
    position.size = math::add(&env, position.size, size_added);
    position.base_exposure = math::add(&env, position.base_exposure, base);
    position.risk_units = math::add(&env, position.risk_units, risk_units);
    if size_added > 0 {
        position.last_increased_time = now;
    }
    {
        let side = market.side_mut(position.is_long);
        side.size_open_interest = math::add(&env, side.size_open_interest, size_added);
        side.base_exposure = math::add(&env, side.base_exposure, base);
        side.risk_units = math::add(&env, side.risk_units, risk_units);
    }
    ledger.total_risk_units = math::add(&env, ledger.total_risk_units, risk_units);
    risk::enforce_capacity(&env, &ledger, physical, ledger.total_risk_units);
    risk::enforce_market_limits(&env, &market, position.is_long);
    let health = math::add(
        &env,
        position.stored_collateral,
        math::pnl(
            &env,
            position.is_long,
            position.size,
            position.base_exposure,
            price,
        ),
    );
    // Adding size is held to the initial margin; a pure collateral top-up
    // only de-risks and must clear just the maintenance floor (§12.3).
    let required = if size_added > 0 {
        risk::initial_requirement(&env, position.size, &market.config)
    } else {
        risk::maintenance_requirement(&env, position.size, &market.config)
    };
    if health < required {
        panic_with_error!(&env, PositionManagerError::InsufficientCollateral);
    }
    funding::reset_debts(&env, &ledger, &mut position, &market);
    storage::save_position(&env, &position);
    funding::refresh_display(&env, &mut market);
    storage::save_market(&env, &position.market, &market);
    borrow::refresh_rate(&env, &mut ledger, physical);
    storage::save_ledger(&env, &ledger);
    events::emit_increased(
        &env,
        &position,
        size_added,
        base,
        collateral_added,
        price,
        &collected,
    );
}
