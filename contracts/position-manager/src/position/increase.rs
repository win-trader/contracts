use crate::{
    auth::{require_auth, require_initialized, require_market_active, require_not_paused},
    borrow,
    errors::PositionManagerError,
    events, fees, funding, ledger, math, risk, snapshot, storage, validation,
};
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

    // §7.8 — the completed old window is settled **first**, from
    // pre-existing collateral, before any added collateral joins it. An
    // increase must not be able to use new money to cover an obligation the
    // position could not already meet: that would let a position that
    // should have been liquidated buy its way past the check.
    let collected =
        fees::capitalize_for_surviving_mutation(&env, &mut ledger, &mut position, &mut market);

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

    let added = if size_added > 0 {
        math::derive_added_exposure(
            &env,
            position.is_long,
            position.size,
            position.risk_units,
            size_added,
            price,
            market.config.market_risk_factor_bps,
        )
    } else {
        math::AddedExposure {
            base_added: 0,
            risk_added: 0,
        }
    };
    let (base, risk_units) = (added.base_added, added.risk_added);
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
    if size_added > 0 && !risk::side_accepts_new_exposure(&env, market.side(position.is_long)) {
        panic_with_error!(&env, PositionManagerError::RiskStateBlocked);
    }
    // §6.8 — an increase pays the opening fee on the **added size only**. A
    // collateral-only addition never calls it.
    if size_added > 0 {
        let opening_fee = fees::calculate_opening_fee(&env, size_added, &market.config);
        if opening_fee > 0 {
            let is_long = position.is_long;
            let charged = ledger::collect_stored_collateral(
                &env,
                &mut ledger,
                &mut position,
                market.side_mut(is_long),
                opening_fee,
            );
            if charged < opening_fee {
                panic_with_error!(&env, PositionManagerError::InsufficientCollateral);
            }
            let owner = position.owner.clone();
            fees::distribute_open_close_revenue(
                &env,
                &mut ledger,
                charged,
                &owner,
                events::FeeSource::Opening,
                position.id,
            );
        }
    }
    // §4.9 step 7 — reset the opposite stream's distribution carry before
    // this side's size changes.
    funding::reset_receiver_distribution_remainder(&mut market, position.is_long);
    position.size = math::add(&env, position.size, size_added);
    position.base_exposure = math::add(&env, position.base_exposure, base);
    position.risk_units = math::add(&env, position.risk_units, risk_units);
    if size_added > 0 {
        position.last_size_increase_at = now;
    }
    {
        let side = market.side_mut(position.is_long);
        side.size_open_interest = math::add(&env, side.size_open_interest, size_added);
        side.base_exposure = math::add(&env, side.base_exposure, base);
        side.risk_units = math::add(&env, side.risk_units, risk_units);
    }
    risk::register_exposure(&env, &mut ledger, risk_units);
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
    let required = risk::required_margin(&env, position.size, &market.config, size_added > 0);
    if health < required {
        panic_with_error!(&env, PositionManagerError::InsufficientCollateral);
    }
    funding::reset_debts(&env, &mut position, &market);
    funding::refresh_display(&env, &mut ledger, &mut market);
    storage::save_market(&env, &position.market, &market);
    borrow::refresh_rate(&env, &mut ledger, physical);
    // §3.3.3 — a fresh window for the resulting risk units. No tranche,
    // proportional remainder, or old minimum carries forward.
    borrow::initialize_window(&env, &ledger, &mut position);
    storage::save_position(&env, &position);
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
