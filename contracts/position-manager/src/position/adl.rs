//! §7.14 — automatic deleveraging.
//!
//! A forced safety action like liquidation: no commitment, no delay, one
//! authenticated snapshot. **Permissionless**, deliberately. §7.0 defines
//! keeper authorization as the caller authenticating the address that
//! receives the reward and nothing more, and what bounds this mechanism is
//! the state gate, not who calls it: `next_state is ADL or HardCap` is
//! re-evaluated from the current book on every call, so each execution that
//! removes profitable exposure lowers the side's PnL factor, and once it
//! falls below `adl_pnl_factor_bps` no further ADL is permitted on that
//! side.
//!
//! Candidate selection is unranked. Any position on the restricted side with
//! positive raw PnL is eligible; the protocol does not require the keeper to
//! pick the largest winner or any particular order. Ranking would mean
//! sorting positions on chain, whose cost grows with the number of traders
//! and which §4.1 rules out for exactly that reason. The accepted cost is
//! fairness between winners: being deleveraged is not proportional to how
//! much of the liability a trader represents, and the protection a trader
//! has is the state gate and the fixed reward, not a queue position.

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

    // The gate, evaluated purely from the current book before anything is
    // applied. `next_state`, not the stored state: a side that has already
    // recovered must not be deleveraged because it was restricted an hour
    // ago, and a side that has just crossed must be reachable immediately.
    let assessment = risk::assess(&env, &market, price, equity);
    let side_next = if position.is_long {
        assessment.long.next_state
    } else {
        assessment.short.next_state
    };
    if side_next != RiskState::Adl && side_next != RiskState::HardCap {
        panic_with_error!(&env, PositionManagerError::RiskStateBlocked);
    }
    // Only a winner may be deleveraged: ADL exists to remove the profit
    // pressing on LP equity, and taking a loser would move no liability.
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

    // Apply the transition before anything reads a payout factor (§6.5),
    // then assess liquidation against the applied state.
    risk::apply(
        &env,
        &mut ledger,
        &position.market,
        &keeper_address,
        &mut market,
        &assessment,
    );

    // §8.12 — liquidation outranks ADL. A liquidatable position is left for
    // the liquidation path; this returns non-terminally and pays nothing.
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
        // §7.14 — no closing fee, and no liquidation or close reward on top
        // of the fixed ADL reward.
        ClosingFee::Waived,
    );

    storage::save_ledger(&env, &ledger);
    super::emit_terminal(&env, &keeper_address, &settled, CloseReason::Adl);
    ActionOutcome::Executed
}
