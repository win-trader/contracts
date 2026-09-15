use crate::{
    auth::{require_auth, require_initialized},
    borrow,
    errors::PositionManagerError,
    events, funding, settle, snapshot, storage,
};
use soroban_sdk::{panic_with_error, Address, Env};

pub fn execute_order(env: Env, caller: Address, position_id: u64) {
    require_initialized(&env);
    require_auth(&caller);

    let position = storage::get_position(&env, position_id);
    let price = snapshot::authenticated_price(&env, &position.market);

    let triggered = if position.is_long {
        (position.take_profit > 0 && price >= position.take_profit)
            || (position.stop_loss > 0 && price <= position.stop_loss)
    } else {
        (position.take_profit > 0 && price <= position.take_profit)
            || (position.stop_loss > 0 && price >= position.stop_loss)
    };
    if !triggered {
        panic_with_error!(&env, PositionManagerError::InvalidOrder);
    }
    let mut ledger = storage::get_ledger(&env);
    let mut market = storage::get_market(&env, &position.market);
    let now = env.ledger().timestamp();
    borrow::accrue(&env, &mut ledger, now);
    funding::accrue(&env, &mut ledger, &mut market, now);

    let size = position.size;
    let settled = settle::settle_close(&env, &mut ledger, position, market, size, 0, price, None);
    storage::save_ledger(&env, &ledger);

    events::emit_order_executed(&env, position_id, &caller, 0);
    match &settled {
        settle::Settled::Closed(header, tail) => {
            events::emit_closed(&env, header, tail, events::CloseReason::Order)
        }
        settle::Settled::Partial(header, tail) => events::emit_decreased(&env, header, tail),
    }
}
