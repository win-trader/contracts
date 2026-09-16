use soroban_sdk::{panic_with_error, Address, Env};

use shared::{ActionOutcome, RiskState};

use crate::auth::require_initialized;
use crate::errors::PositionManagerError;
use crate::events::CloseReason;
use crate::settle::{self, ClosingFee};
use crate::{borrow, funding, keeper, ledger, math, risk, snapshot, storage};

pub fn execute_adl(env: Env, keeper_address: Address, position_id: u64) -> ActionOutcome {
    require_initialized(&env);
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

    let assessment = risk::assess(&env, &market, price, equity);
    let side_next = if position.is_long {
        assessment.long.next_state
    } else {
        assessment.short.next_state
    };
    if side_next != RiskState::Adl && side_next != RiskState::HardCap {
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

    risk::apply(
        &env,
        &mut ledger,
        &position.market,
        &keeper_address,
        &mut market,
        &assessment,
    );

    if risk::evaluate_liquidation(&env, &ledger, &position, &market, price).liquidatable {
        return ActionOutcome::RequiresLiquidation;
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
                keeper::RewardKind::Adl,
            ),
            liquidation: false,
        },
        ClosingFee::Waived,
    );

    storage::save_ledger(&env, &ledger);
    super::emit_terminal(&env, &keeper_address, &settled, CloseReason::Adl);
    ActionOutcome::Executed
}
