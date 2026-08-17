use crate::{
    auth::{require_auth, require_initialized, require_market_active, require_not_paused},
    checkpoint::{checkpoint_global, checkpoint_market},
    errors::PositionManagerError,
    events, funding, ledger, math, risk, snapshot, storage,
    validation::{check_slippage, validate_orders},
};
use shared::{MarketConfig, Position, RiskState, VaultClient};
use soroban_sdk::{panic_with_error, Address, Env, Symbol};

fn require_valid_input(env: &Env, size: i128, collateral: i128, execution_budget: i128) {
    if size <= 0 || collateral <= 0 || execution_budget < 0 {
        panic_with_error!(&env, PositionManagerError::InvalidAmount);
    }
}

fn require_sufficient_collateral(
    env: &Env,
    position: &Position,
    market_config: &MarketConfig,
    size: i128,
) {
    let min_collateral = storage::get_global_config(&env).min_collateral;
    let risk_requirement = risk::initial_requirement(&env, size, market_config);

    if position.stored_collateral < min_collateral || position.stored_collateral < risk_requirement
    {
        panic_with_error!(&env, PositionManagerError::InsufficientCollateral);
    }
}

pub fn open_position(
    env: Env,
    owner: Address,
    market_symbol: Symbol,
    is_long: bool,
    size: i128,
    collateral: i128,
    execution_budget: i128,
    take_profit: i128,
    stop_loss: i128,
    acceptable_price: i128,
) -> u64 {
    require_initialized(&env);
    require_auth(&owner);
    require_not_paused(&env);
    require_valid_input(&env, size, collateral, execution_budget);
    require_market_active(&env, &market_symbol);

    let mut market = storage::get_market(&env, &market_symbol);
    let mut ledger = storage::get_ledger(&env);
    let vault_address = storage::get_vault(&env);
    let vault = VaultClient::new(&env, &vault_address);

    let now = env.ledger().timestamp();

    checkpoint_global(&env, &mut ledger, now);
    checkpoint_market(&env, &mut ledger, &mut market, now);

    let price = snapshot::authenticated_price(&env, &market_symbol);

    check_slippage(&env, is_long, true, price, acceptable_price);
    validate_orders(&env, is_long, take_profit, stop_loss, price);

    let total_transfer = math::add(&env, collateral, execution_budget);

    vault.receive_collateral(&env.current_contract_address(), &owner, &total_transfer);

    ledger.execution_budget_total =
        math::add(&env, ledger.execution_budget_total, execution_budget);

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
        execution_budget,
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

    require_sufficient_collateral(&env, &position, &market.config, size);

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
        // §8.1 cold start — an empty book carries no history, and zero is
        // not "no information": it would grant a one-sided launch a
        // decaying discount. The EMA starts at the skew this open creates.
        market.skew_ema =
            math::skew_frac(&env, market.long.base_exposure, market.short.base_exposure);
    }

    ledger.total_risk_units = math::add(&env, ledger.total_risk_units, risk_units);
    risk::enforce_capacity(&env, &ledger, physical, ledger.total_risk_units);
    risk::enforce_market_limits(&env, &market, is_long);
    funding::reset_debts(&env, &ledger, &mut position, &market);
    storage::save_position(&env, &position);
    ledger.open_position_count += 1;
    funding::refresh_display(&env, &mut market);
    storage::save_market(&env, &market_symbol, &market);
    risk::refresh_rate(&env, &mut ledger, physical);
    storage::save_ledger(&env, &ledger);

    events::emit_opened(&env, &position, price);

    position_id
}
