use crate::{
    auth::{require_initialized, require_role},
    checkpoint,
    errors::PositionManagerError,
    events, ledger, math, risk, settle, snapshot, storage,
};
use shared::constants::{BPS, ROLE_KEEPER};
use shared::RiskState;
use soroban_sdk::{panic_with_error, Address, Env};

pub fn deleverage_position(env: Env, caller: Address, position_id: u64) {
    require_initialized(&env);
    require_role(&env, &caller, ROLE_KEEPER);

    let position = storage::get_position(&env, position_id);
    let mut market = storage::get_market(&env, &position.market);
    let mut ledger = storage::get_ledger(&env);

    let now = env.ledger().timestamp();
    checkpoint::checkpoint_global(&env, &mut ledger, now);
    checkpoint::checkpoint_market(&env, &mut ledger, &mut market, now);

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
    let reward_bps = market.config.adl_reward_bps;
    let summary = settle::settle_close(&env, &mut ledger, position, market, size, 0, price, None);

    let configured_reward = math::mul_div_floor(&env, size, reward_bps as i128, BPS);
    let reward = core::cmp::min(
        core::cmp::min(
            ledger.risk_keeper_reserve_total,
            storage::get_global_config(&env).max_adl_reward,
        ),
        configured_reward,
    );
    if reward > 0 {
        ledger::payout(
            &env,
            &mut ledger,
            ledger::Bucket::KeeperReserve,
            &caller,
            reward,
        );
        events::emit_adl_reward(&env, position_id, &caller, reward);
    }

    storage::save_ledger(&env, &ledger);
    events::emit_closed(&env, &summary, events::CloseReason::Deleverage);
}
