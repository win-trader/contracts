use crate::{
    auth::{require_auth, require_initialized},
    borrow,
    errors::PositionManagerError,
    events, funding, settle, snapshot, storage, validation,
};
use soroban_sdk::{panic_with_error, Env};

fn require_valid_input(
    env: &Env,
    size_removed: i128,
    position_size: i128,
    collateral_withdrawn: i128,
) {
    if size_removed < 0
        || size_removed > position_size
        || collateral_withdrawn < 0
        || (size_removed == 0 && collateral_withdrawn == 0)
    {
        panic_with_error!(&env, PositionManagerError::InvalidAmount);
    }
}

fn require_min_position_age(
    env: &Env,
    now: u64,
    last_increased_time: u64,
    min_position_lifetime: u64,
) {
    if now < last_increased_time.saturating_add(min_position_lifetime) {
        panic_with_error!(&env, PositionManagerError::TooEarly);
    }
}

pub fn decrease_position(
    env: Env,
    position_id: u64,
    size_removed: i128,
    collateral_withdrawn: i128,
    acceptable_price: i128,
) {
    require_initialized(&env);

    let position = storage::get_position(&env, position_id);

    require_auth(&position.owner);
    require_valid_input(&env, size_removed, position.size, collateral_withdrawn);

    let config = storage::get_global_config(&env);

    let now = env.ledger().timestamp();

    require_min_position_age(
        &env,
        now,
        position.last_increased_time,
        config.min_position_lifetime,
    );

    let mut market = storage::get_market(&env, &position.market);
    let mut ledger = storage::get_ledger(&env);

    borrow::accrue(&env, &mut ledger, now);
    funding::accrue(&env, &mut ledger, &mut market, now);

    let price = snapshot::authenticated_price(&env, &position.market);

    validation::check_slippage(&env, position.is_long, false, price, acceptable_price);

    let summary = settle::settle_close(
        &env,
        &mut ledger,
        position,
        market,
        size_removed,
        collateral_withdrawn,
        price,
        None,
    );
    storage::save_ledger(&env, &ledger);
    if summary.closed {
        events::emit_closed(&env, &summary, events::CloseReason::Trader);
    } else {
        events::emit_decreased(&env, &summary);
    }
}
