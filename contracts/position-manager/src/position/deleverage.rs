use crate::{
    auth::{require_initialized, require_role},
    borrow,
    errors::PositionManagerError,
    events, funding, keeper, ledger, math, risk, settle, snapshot, storage,
};
use shared::constants::ROLE_KEEPER;
use shared::RiskState;
use soroban_sdk::{panic_with_error, Address, Env};

pub fn deleverage_position(env: Env, caller: Address, position_id: u64) {
    require_initialized(&env);
    require_role(&env, &caller, ROLE_KEEPER);

    let position = storage::get_position(&env, position_id);
    let mut market = storage::get_market(&env, &position.market);
    let mut ledger = storage::get_ledger(&env);

    let now = env.ledger().timestamp();
    borrow::accrue(&env, &mut ledger, now);
    funding::accrue(&env, &mut ledger, &mut market, now);

    let price = snapshot::authenticated_price(&env, &position.market);
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

    let side_state = market.side(position.is_long).risk_state;
    if side_state != RiskState::Adl && side_state != RiskState::HardCap {
        panic_with_error!(&env, PositionManagerError::RiskStateBlocked);
    }
    if math::pnl(
        &env,
        position.is_long,
        position.size,
        position.base_exposure,
        price,
    ) <= 0
    {
        panic_with_error!(&env, PositionManagerError::RiskStateBlocked);
    }

    let size = position.size;
    let reward = keeper::reward_for(
        &storage::get_global_config(&env),
        keeper::RewardKind::Adl,
    );
    let settled = settle::settle_close(
        &env,
        &mut ledger,
        position,
        market,
        size,
        0,
        price,
        Some(settle::Keeper {
            recipient: &caller,
            reward,
            liquidation: false,
        }),
    );

    storage::save_ledger(&env, &ledger);
    match &settled {
        settle::Settled::Closed(header, tail) => {
            events::emit_closed(&env, header, tail, events::CloseReason::Deleverage)
        }
        settle::Settled::Partial(header, tail) => events::emit_decreased(&env, header, tail),
    }
}
