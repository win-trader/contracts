//! §6.12 — keeper rewards: the one-to-one map from settlement kind to
//! amount, and the three payment paths.
//!
//! No keeper payment creates a claim or a reserve entry. Rewards are paid
//! atomically out of whatever source §5.10 assigns them, which is why they
//! do not appear in §5.12's liability equation at all. A reverted action
//! reverts the transfer and its source debit together.

use soroban_sdk::{panic_with_error, Address, Env};

use shared::{GlobalConfig, MarketSide, Position};

use crate::errors::PositionManagerError;
use crate::ledger::{self, Ledger};
use crate::math;

/// The settlement kinds that pay a reward. Exactly one reward per settlement
/// call (§6.12); internal cleanup adds none.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub enum RewardKind {
    MarketOpen,
    LimitOpen,
    Increase,
    Decrease,
    Close,
    TakeProfit,
    StopLoss,
    Expiry,
    Liquidation,
    Adl,
    LpResolve,
}

/// §6.12 — the one-to-one map. The eleven fields are independent even though
/// their initial values match, so this is a match, not an arithmetic.
pub fn reward_for(config: &GlobalConfig, kind: RewardKind) -> i128 {
    let r = &config.keeper_rewards;
    match kind {
        RewardKind::MarketOpen => r.open,
        RewardKind::LimitOpen => r.limit_order,
        RewardKind::Increase => r.increase,
        RewardKind::Decrease => r.decrease,
        RewardKind::Close => r.close,
        RewardKind::TakeProfit => r.tp,
        RewardKind::StopLoss => r.sl,
        RewardKind::Expiry => r.expiry,
        RewardKind::Liquidation => r.liquidation,
        RewardKind::Adl => r.adl,
        RewardKind::LpResolve => r.lp_resolve,
    }
}

/// §6.12 — pay from a pending action's escrow. Requires the full amount:
/// entry creation guarantees the escrow can cover the applicable reward
/// (§10.3.1), so a shortfall here is a broken invariant, not a business
/// outcome.
pub fn pay_from_escrow(
    env: &Env,
    ledger: &mut Ledger,
    escrowed: &mut i128,
    keeper: &Address,
    reward: i128,
) {
    if reward <= 0 {
        return;
    }
    if *escrowed < reward {
        panic_with_error!(env, PositionManagerError::InvariantViolation);
    }
    *escrowed = math::sub(env, *escrowed, reward);
    ledger::payout(env, ledger, ledger::Bucket::ActionEscrow, keeper, reward);
}

/// §6.12 — pay from stored position collateral, in full or not at all.
///
/// Every payment except the three §5.10 names requires its full amount and
/// reverts otherwise: a voluntary settlement that cannot pay for itself
/// should not complete. The blanket `reward <= min_collateral` bound is what
/// keeps that from stranding a position — see §5.10.
pub fn pay_from_position(
    env: &Env,
    ledger: &mut Ledger,
    position: &mut Position,
    side: &mut MarketSide,
    keeper: &Address,
    reward: i128,
) -> i128 {
    if reward <= 0 {
        return 0;
    }
    if position.stored_collateral < reward {
        panic_with_error!(env, PositionManagerError::InsufficientCollateral);
    }
    ledger::payout_collateral(env, ledger, position, side, keeper, reward)
}

/// What a liquidation reward actually came from.
#[derive(Clone, Copy, Debug, Default)]
pub struct KeeperPayment {
    pub from_position: i128,
    /// LP residual cash used to cover a price-gap shortfall. Liquidation is
    /// the only path that may draw on it (§8.11).
    pub from_lp_backstop: i128,
    /// The part of a liquidation reward neither the position nor LP
    /// residual could cover. Reported by §12.6's liquidation event.
    pub unpaid: i128,
}

impl KeeperPayment {
    pub fn paid(&self) -> i128 {
        self.from_position + self.from_lp_backstop
    }
}

/// §6.12 — liquidation takes what the position has, then tops up from LP
/// residual, capped at what exists.
///
/// This is one of only three payments capped at its source rather than
/// required in full, and the reason is liveness: the liquidation must
/// complete either way. A position nobody will liquidate because the reward
/// is short is a position that keeps accruing losses against the vault.
pub fn pay_liquidation(
    env: &Env,
    ledger: &mut Ledger,
    position: &mut Position,
    side: &mut MarketSide,
    physical_cash: i128,
    keeper: &Address,
    reward: i128,
) -> KeeperPayment {
    if reward <= 0 {
        return KeeperPayment::default();
    }
    let from_position = ledger::payout_collateral(
        env,
        ledger,
        position,
        side,
        keeper,
        core::cmp::min(position.stored_collateral, reward),
    );
    let shortfall = math::sub(env, reward, from_position);
    let equity = ledger.cash_lp_equity(env, physical_cash);
    let lp_backstop = core::cmp::min(shortfall, equity);
    if lp_backstop > 0 {
        // The backstop is LP residual cash, which carries no claim label —
        // it leaves the vault without a bucket debit, on the safety path.
        ledger::payout_lp_residual(env, keeper, lp_backstop);
    }
    KeeperPayment {
        from_position,
        from_lp_backstop: lp_backstop,
        unpaid: math::sub(env, shortfall, lp_backstop),
    }
}
