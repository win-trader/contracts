use soroban_sdk::{contracttype, panic_with_error, Address, Env};

use shared::{MarketSide, Position, VaultClient};

use crate::errors::PositionManagerError;
use crate::{math, storage};

pub const STATE_VERSION: u32 = 3;

#[contracttype]
#[derive(Clone, Debug)]
pub struct Ledger {
    pub position_collateral_total: i128,
    pub pending_receiver_funding_total: i128,
    pub action_escrow_total: i128,
    pub protocol_claimable_total: i128,
    pub referral_claimable_total: i128,
    pub unclaimed_payout_total: i128,
    pub total_risk_units: i128,
    pub open_position_count: u64,
    pub restricted_market_side_count: u32,
    pub state_version: u32,
    pub borrow_index: i128,
    pub borrow_index_remainder: i128,
    pub current_borrow_rate: i128,
    pub last_global_checkpoint: u64,
}

impl Ledger {
    pub fn new(now: u64, initial_borrow_rate: i128) -> Self {
        Ledger {
            position_collateral_total: 0,
            pending_receiver_funding_total: 0,
            action_escrow_total: 0,
            protocol_claimable_total: 0,
            referral_claimable_total: 0,
            unclaimed_payout_total: 0,
            total_risk_units: 0,
            open_position_count: 0,
            restricted_market_side_count: 0,
            state_version: STATE_VERSION,
            borrow_index: 0,
            borrow_index_remainder: 0,
            current_borrow_rate: initial_borrow_rate,
            last_global_checkpoint: now,
        }
    }

    pub fn non_lp_claims(&self, env: &Env) -> i128 {
        let mut total = self.position_collateral_total;
        total = math::add(env, total, self.pending_receiver_funding_total);
        total = math::add(env, total, self.action_escrow_total);
        total = math::add(env, total, self.protocol_claimable_total);
        total = math::add(env, total, self.referral_claimable_total);
        math::add(env, total, self.unclaimed_payout_total)
    }

    pub fn cash_lp_equity(&self, env: &Env, physical_cash: i128) -> i128 {
        let claims = self.non_lp_claims(env);
        core::cmp::max(
            math::sub(env, physical_cash, core::cmp::min(physical_cash, claims)),
            0,
        )
    }
}

pub fn physical_cash(env: &Env) -> i128 {
    VaultClient::new(env, &storage::get_vault(env)).physical_cash()
}

fn vault(env: &Env) -> VaultClient<'_> {
    VaultClient::new(env, &storage::get_vault(env))
}

#[derive(Clone, Copy, Debug)]
pub enum Bucket {
    ReceiverFunding,
    ActionEscrow,
    ProtocolClaimable,
    Referral,
    UnclaimedPayout,
}

impl Ledger {
    fn bucket_mut(&mut self, bucket: Bucket) -> &mut i128 {
        match bucket {
            Bucket::ReceiverFunding => &mut self.pending_receiver_funding_total,
            Bucket::ActionEscrow => &mut self.action_escrow_total,
            Bucket::ProtocolClaimable => &mut self.protocol_claimable_total,
            Bucket::Referral => &mut self.referral_claimable_total,
            Bucket::UnclaimedPayout => &mut self.unclaimed_payout_total,
        }
    }

    pub fn credit(&mut self, env: &Env, bucket: Bucket, amount: i128) {
        if amount == 0 {
            return;
        }
        if amount < 0 {
            panic_with_error!(env, PositionManagerError::InvariantViolation);
        }
        let total = self.bucket_mut(bucket);
        *total = math::add(env, *total, amount);
    }

    pub fn release(&mut self, env: &Env, bucket: Bucket, amount: i128) -> i128 {
        let total = self.bucket_mut(bucket);
        let released = core::cmp::min(*total, amount);
        if released <= 0 {
            return 0;
        }
        *total = math::sub(env, *total, released);
        released
    }

    fn debit(&mut self, env: &Env, bucket: Bucket, amount: i128) {
        let total = self.bucket_mut(bucket);
        if amount < 0 || *total < amount {
            panic_with_error!(env, PositionManagerError::InvariantViolation);
        }
        *total = math::sub(env, *total, amount);
    }
}

pub fn payout(env: &Env, ledger: &mut Ledger, bucket: Bucket, recipient: &Address, amount: i128) {
    if amount <= 0 {
        return;
    }
    ledger.debit(env, bucket, amount);
    vault(env).transfer_safety_claim(&env.current_contract_address(), recipient, &amount);
}

pub fn payout_checked(
    env: &Env,
    ledger: &mut Ledger,
    bucket: Bucket,
    recipient: &Address,
    amount: i128,
) {
    if amount <= 0 {
        return;
    }
    ledger.debit(env, bucket, amount);
    let claims_after = ledger.non_lp_claims(env);
    vault(env).transfer_claim(
        &env.current_contract_address(),
        recipient,
        &amount,
        &claims_after,
    );
}

pub fn payout_collateral_to_owner(
    env: &Env,
    ledger: &mut Ledger,
    position: &mut Position,
    side: &mut MarketSide,
    amount: i128,
) -> i128 {
    let collected = collect_stored_collateral(env, ledger, position, side, amount);
    let owner = position.owner.clone();
    deliver_to_owner(env, ledger, &owner, collected);
    collected
}

pub fn payout_collateral(
    env: &Env,
    ledger: &mut Ledger,
    position: &mut Position,
    side: &mut MarketSide,
    recipient: &Address,
    amount: i128,
) -> i128 {
    let collected = collect_stored_collateral(env, ledger, position, side, amount);
    if collected > 0 {
        vault(env).transfer_safety_claim(&env.current_contract_address(), recipient, &collected);
    }
    collected
}

pub fn deliver_to_owner(env: &Env, ledger: &mut Ledger, owner: &Address, amount: i128) -> i128 {
    if amount <= 0 {
        return 0;
    }
    // A failed push (e.g. no trustline) is held as a claim so the recipient can't veto the settlement.
    let delivered = vault(env)
        .try_transfer_safety_claim(&env.current_contract_address(), owner, &amount)
        .is_ok();
    if delivered {
        return amount;
    }
    ledger.credit(env, Bucket::UnclaimedPayout, amount);
    let held = storage::get_unclaimed_payout(env, owner);
    storage::save_unclaimed_payout(env, owner, math::add(env, held, amount));
    crate::events::emit_payout_deferred(env, owner, amount);
    0
}

pub fn claim_unclaimed_payout(env: &Env, ledger: &mut Ledger, owner: &Address) -> i128 {
    let amount = storage::get_unclaimed_payout(env, owner);
    if amount <= 0 {
        panic_with_error!(env, PositionManagerError::InvalidAmount);
    }
    storage::save_unclaimed_payout(env, owner, 0);
    payout(env, ledger, Bucket::UnclaimedPayout, owner, amount);
    amount
}

pub fn payout_lp_residual(env: &Env, recipient: &Address, amount: i128) {
    if amount <= 0 {
        return;
    }
    vault(env).transfer_safety_claim(&env.current_contract_address(), recipient, &amount);
}

pub fn escrow_in(env: &Env, ledger: &mut Ledger, owner: &Address, amount: i128) {
    if amount <= 0 {
        return;
    }
    receive(env, owner, amount);
    ledger.credit(env, Bucket::ActionEscrow, amount);
}

pub fn refund_escrow(env: &Env, ledger: &mut Ledger, owner: &Address, amount: i128) {
    if amount <= 0 {
        return;
    }
    ledger.debit(env, Bucket::ActionEscrow, amount);
    deliver_to_owner(env, ledger, owner, amount);
}

pub fn escrow_to_collateral(
    env: &Env,
    ledger: &mut Ledger,
    position: &mut Position,
    side: &mut MarketSide,
    amount: i128,
) {
    if amount <= 0 {
        return;
    }
    ledger.debit(env, Bucket::ActionEscrow, amount);
    position.stored_collateral = math::add(env, position.stored_collateral, amount);
    side.stored_collateral_total = math::add(env, side.stored_collateral_total, amount);
    ledger.position_collateral_total = math::add(env, ledger.position_collateral_total, amount);
}

pub fn spend_escrow(env: &Env, ledger: &mut Ledger, amount: i128) {
    if amount <= 0 {
        return;
    }
    ledger.debit(env, Bucket::ActionEscrow, amount);
}

pub fn receive(env: &Env, from: &Address, amount: i128) {
    vault(env).receive_collateral(&env.current_contract_address(), from, &amount);
}

pub fn add_stored_collateral(
    env: &Env,
    ledger: &mut Ledger,
    position: &mut Position,
    side: &mut MarketSide,
    amount: i128,
) {
    if amount == 0 {
        return;
    }
    if amount < 0 {
        panic_with_error!(env, PositionManagerError::InvariantViolation);
    }
    position.stored_collateral = math::add(env, position.stored_collateral, amount);
    side.stored_collateral_total = math::add(env, side.stored_collateral_total, amount);
    ledger.position_collateral_total = math::add(env, ledger.position_collateral_total, amount);
}

pub fn collect_stored_collateral(
    env: &Env,
    ledger: &mut Ledger,
    position: &mut Position,
    side: &mut MarketSide,
    amount: i128,
) -> i128 {
    let collected = core::cmp::min(position.stored_collateral, amount);
    if collected <= 0 {
        return 0;
    }
    position.stored_collateral = math::sub(env, position.stored_collateral, collected);
    side.stored_collateral_total = math::sub(env, side.stored_collateral_total, collected);
    ledger.position_collateral_total = math::sub(env, ledger.position_collateral_total, collected);
    collected
}
