use crate::{
    auth::{require_auth, require_initialized},
    borrow,
    errors::PositionManagerError,
    events, funding, keeper, ledger, risk, settle, snapshot, storage,
};
use soroban_sdk::{panic_with_error, Address, Env};

pub fn liquidate_position(env: Env, caller: Address, position_id: u64) {
    require_initialized(&env);
    require_auth(&caller);

    let position = storage::get_position(&env, position_id);
    let mut market = storage::get_market(&env, &position.market);
    let mut ledger = storage::get_ledger(&env);

    let now = env.ledger().timestamp();

    borrow::accrue(&env, &mut ledger, now);
    funding::accrue(&env, &mut ledger, &mut market, now);

    let price = snapshot::authenticated_price(&env, &position.market);
    let physical = ledger::physical_cash(&env);
    let equity = ledger.cash_lp_equity(&env, physical);

    // §6.5 — refresh the side risk state from **this** price snapshot
    // before anything reads a payout factor. Refreshing afterwards would let
    // the first position out of a newly-crossed side settle unscaled and
    // latch the side on its way out.
    risk::evaluate_market_risk(
        &env,
        &mut ledger,
        &position.market,
        &mut market,
        price,
        equity,
    );

    // §6.15 — one assessment for eligibility *and* settlement.
    let assessment = risk::evaluate_liquidation(&env, &ledger, &position, &market, price);
    if !assessment.liquidatable {
        panic_with_error!(&env, PositionManagerError::PositionHealthy);
    }

    let size = position.size;
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
            // §6.12 — the one payment that may fall short and still
            // complete. A position nobody will liquidate because the reward
            // is short keeps accruing losses against the vault.
            reward: keeper::reward_for(
                &storage::get_global_config(&env),
                keeper::RewardKind::Liquidation,
            ),
            liquidation: true,
        }),
    );

    storage::save_ledger(&env, &ledger);

    match &settled {
        settle::Settled::Closed(header, tail) => {
            events::emit_closed(&env, header, tail, events::CloseReason::Liquidation)
        }
        settle::Settled::Partial(header, tail) => events::emit_decreased(&env, header, tail),
    }
}
