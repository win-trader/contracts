use crate::{
    auth::{require_auth, require_initialized},
    borrow,
    errors::PositionManagerError,
    events, funding, ledger, math, risk, settle, snapshot, storage,
};
use shared::MarketConfig;
use soroban_sdk::{panic_with_error, Address, Env};

fn require_unhealthy_position(
    env: &Env,
    effective: i128,
    position_size: i128,
    market_config: &MarketConfig,
) {
    if effective >= risk::maintenance_requirement(&env, position_size, market_config) {
        panic_with_error!(env, PositionManagerError::PositionHealthy);
    }
}

pub fn liquidate_position(env: Env, caller: Address, position_id: u64) {
    require_initialized(&env);
    require_auth(&caller);

    let position = storage::get_position(&env, position_id);
    let mut market = storage::get_market(&env, &position.market);
    let mut ledger = storage::get_ledger(&env);
    let config = storage::get_global_config(&env);

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

    let pending = funding::pending_fees(&env, &ledger, &position, &market);
    let payable = risk::payable_pnl(
        &env,
        &ledger,
        &position,
        &market,
        position.size,
        position.base_exposure,
        price,
        physical,
    );

    let effective = math::add(
        &env,
        math::sub(
            &env,
            math::sub(
                &env,
                math::sub(
                    &env,
                    math::add(&env, position.stored_collateral, pending.funding_received),
                    pending.funding_paid_to_receivers,
                ),
                pending.funding_paid_to_lps,
            ),
            pending.borrow,
        ),
        payable,
    );

    require_unhealthy_position(&env, effective, position.size, &market.config);

    let insolvent = effective < 0;
    let size = position.size;
    let settled = settle::settle_close(
        &env,
        &mut ledger,
        position,
        market,
        size,
        0,
        price,
        Some(&caller),
    );
    if matches!(settled, settle::Settled::Closed(..)) && insolvent {
        let reward = core::cmp::min(
            ledger.risk_keeper_reserve_total,
            config.max_insolvent_touch_reward,
        );
        if reward > 0 {
            ledger::payout(
                &env,
                &mut ledger,
                ledger::Bucket::KeeperReserve,
                &caller,
                reward,
            );
            events::emit_insolvency_reward(&env, position_id, &caller, reward);
        }
    }

    storage::save_ledger(&env, &ledger);

    match &settled {
        settle::Settled::Closed(header, tail) => {
            events::emit_closed(&env, header, tail, events::CloseReason::Liquidation)
        }
        settle::Settled::Partial(header, tail) => events::emit_decreased(&env, header, tail),
    }
}
