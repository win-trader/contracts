use soroban_sdk::{panic_with_error, Address, Env};

use shared::{GlobalConfig, MarketSide, Position};

use crate::errors::PositionManagerError;
use crate::ledger::{self, Ledger};
use crate::math;

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

#[derive(Clone, Copy, Debug, Default)]
pub struct KeeperPayment {
    pub from_position: i128,
    pub from_lp_backstop: i128,
    pub unpaid: i128,
}

impl KeeperPayment {
    pub fn paid(&self) -> i128 {
        self.from_position + self.from_lp_backstop
    }
}

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
    // from_position has already left the vault.
    let equity = ledger.cash_lp_equity(env, math::sub(env, physical_cash, from_position));
    let lp_backstop = core::cmp::min(shortfall, equity);
    if lp_backstop > 0 {
        ledger::payout_lp_residual(env, keeper, lp_backstop);
    }
    KeeperPayment {
        from_position,
        from_lp_backstop: lp_backstop,
        unpaid: math::sub(env, shortfall, lp_backstop),
    }
}
