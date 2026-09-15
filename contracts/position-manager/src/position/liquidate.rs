//! §7.13 — liquidation.
//!
//! A forced safety action: no trader commitment, no execution delay, and no
//! post-commit observation requirement. It uses one authenticated
//! current-price snapshot for **both** eligibility and settlement, so the
//! function cannot reassess with a later price midway through — the
//! assessment that authorized the liquidation is the one it settles against.

use soroban_sdk::{panic_with_error, Address, Env};

use crate::auth::require_initialized;
use crate::errors::PositionManagerError;
use crate::events::CloseReason;
use crate::settle::{self, ClosingFee};
use crate::{borrow, funding, keeper, ledger, risk, snapshot, storage};

pub fn liquidate_position(env: Env, keeper_address: Address, position_id: u64) {
    require_initialized(&env);
    // §7.0 — permissionless: the caller authenticates the address that will
    // receive the reward, and nothing more.
    keeper_address.require_auth();

    let position = storage::get_position(&env, position_id);
    let mut market = storage::get_market(&env, &position.market);
    let mut ledger = storage::get_ledger(&env);

    let now = env.ledger().timestamp();
    borrow::accrue(&env, &mut ledger, now);
    funding::accrue(&env, &mut ledger, &mut market, now);

    let price = snapshot::read_stamped_price(&env, &position.market).price;
    let physical = ledger::physical_cash(&env);
    let equity = ledger.cash_lp_equity(&env, physical);

    // §6.5 — refresh the side risk state from **this** snapshot before
    // anything reads a payout factor. Refreshing afterwards would let the
    // first position out of a newly-crossed side settle unscaled and latch
    // the side on its way out.
    risk::evaluate_market_risk(&env, &mut ledger, &position.market, &mut market, price, equity);

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
        Some(settle::Keeper {
            recipient: &keeper_address,
            // §6.12 — the one payment that may fall short and still
            // complete. A position nobody will liquidate because the reward
            // is short keeps accruing losses against the vault, so liveness
            // outranks paying the keeper in full.
            reward: keeper::reward_for(
                &storage::get_global_config(&env),
                keeper::RewardKind::Liquidation,
            ),
            liquidation: true,
        }),
        // §7.13 — no closing fee. The trader did not choose this exit.
        ClosingFee::Waived,
    );

    storage::save_ledger(&env, &ledger);
    super::emit_terminal(&env, &settled, CloseReason::Liquidation);
}
