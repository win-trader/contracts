use soroban_sdk::{panic_with_error, Address, Env};

use crate::errors::PositionManagerError;
use crate::events::CloseReason;
use crate::settle::{self, ClosingFee};
use crate::{borrow, funding, keeper, ledger, risk, snapshot, storage};

pub fn liquidate_position(env: Env, keeper_address: Address, position_id: u64) {
    keeper_address.require_auth();

    let position = storage::get_position(&env, position_id);
    let mut market = storage::get_market(&env, &position.market);
    let mut ledger = storage::get_ledger(&env);

    let now = env.ledger().timestamp();
    borrow::accrue(&env, &mut ledger, Some(&keeper_address), now);
    funding::accrue(&env, &mut ledger, &position.market, Some(&keeper_address), &mut market, now);

    let price = snapshot::read_stamped_price(&env, &position.market).price;
    let physical = ledger::physical_cash(&env);
    let equity = ledger.cash_lp_equity(&env, physical);

    risk::evaluate_market_risk(
        &env,
        &mut ledger,
        &position.market,
        &keeper_address,
        &mut market,
        price,
        equity,
    );

    let assessment = risk::evaluate_liquidation(&env, &ledger, &position, &market, price);
    if !assessment.liquidatable {
        panic_with_error!(&env, PositionManagerError::PositionHealthy);
    }

    let size = position.size;
    let settled = settle::settle(
        &env,
        &mut ledger,
        position,
        market,
        size,
        price,
        settle::Keeper {
            recipient: &keeper_address,
            reward: keeper::reward_for(
                &storage::get_global_config(&env),
                keeper::RewardKind::Liquidation,
            ),
            liquidation: true,
        },
        ClosingFee::Waived,
    );

    storage::save_ledger(&env, &ledger);
    super::emit_terminal(&env, &keeper_address, &settled, CloseReason::Liquidation);
}
