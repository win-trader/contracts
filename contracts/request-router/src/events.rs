//! LP request queue events — one per request creation and one per
//! resolution outcome, so the indexer can reconstruct the FIFO queue state
//! without polling.

use soroban_sdk::{contractevent, Address};

use shared::{LpRequestKind, LpRequestStatus};

#[contractevent(topics = ["lpreq"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LpRequestCreated {
    pub request_id: u64,
    pub owner: Address,
    pub kind: LpRequestKind,
    /// Escrowed collateral for a deposit; escrowed shares for a withdrawal.
    pub amount: i128,
    pub execute_after: u64,
}

/// Terminal outcome of the FIFO head: `Settled` with the minted shares /
/// paid assets, or `Failed` with the escrow returned.
#[contractevent(topics = ["lpres"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LpRequestResolved {
    pub request_id: u64,
    pub owner: Address,
    pub kind: LpRequestKind,
    pub status: LpRequestStatus,
    /// Shares minted (deposit) or assets paid (withdrawal); 0 on failure.
    pub settled_amount: i128,
    /// §7.17 `keeper_lp_resolve_reward` paid to the executor. Zero on a
    /// failed withdrawal, which releases no assets to pay from.
    pub reward: i128,
}
