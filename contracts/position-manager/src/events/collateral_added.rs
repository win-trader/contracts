use super::EventHeader;
use shared::Position;
use soroban_sdk::{contractevent, Address, Env};

/// §7.7 — an immediate collateral top-up. It settles nothing and charges
/// nothing, so the only cash movement to report is the deposit itself; the
/// resulting stored collateral is carried so an indexer can reconcile the
/// position without a second read.
#[contractevent(topics = ["colladd"], data_format = "map")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CollateralAdded {
    #[topic]
    pub position_id: u64,
    pub header: EventHeader,
    pub owner: Address,
    pub amount: i128,
    pub stored_collateral: i128,
}

pub fn emit_collateral_added(env: &Env, position: &Position, amount: i128) {
    CollateralAdded {
        position_id: position.id,
        // The owner is the actor: §7.7 needs no keeper.
        header: super::header(env, &position.market, &position.owner),
        owner: position.owner.clone(),
        amount,
        stored_collateral: position.stored_collateral,
    }
    .publish(env);
}
