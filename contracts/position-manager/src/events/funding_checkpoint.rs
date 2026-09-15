use super::EventHeader;
use shared::PayerSide;
use soroban_sdk::{contractevent, Address, Env, Symbol};

/// §12.6 — one funding accrual segment.
///
/// Emitted **per segment**, not per window, and that is the point. §6.2.1
/// splits a window at a funding sign change, and the two halves have
/// opposite payers; collapsing them into one event would attribute funding
/// generated on one side of the crossing to the other, which is exactly the
/// defect the split exists to fix. The index deltas are stored state and
/// could be read back, but their attribution across a crossing cannot.
#[contractevent(topics = ["fundchk"], data_format = "map")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FundingCheckpoint {
    #[topic]
    pub market: Symbol,
    pub header: EventHeader,
    /// `0` for the first segment of the window, `1` for the second. A window
    /// has at most two (§6.2.1).
    pub segment: u32,
    pub payer_side: PayerSide,
    /// Growth in the payer index whose collection restores cash backing an
    /// already-accrued receiver claim (§8.2).
    pub receiver_backed_delta: i128,
    /// Growth in the payer index that is LP revenue on collection.
    pub lp_backed_delta: i128,
    /// Growth in the receiver credit index.
    pub receiver_delta: i128,
    /// New guaranteed receiver liability created by this segment.
    pub liability_delta: i128,
    /// The skew EMA after the **whole** window. Repeated on each segment
    /// rather than attributed to one, because the EMA advances across the
    /// window as a unit.
    pub ema_after: i128,
    pub elapsed: u64,
}

#[allow(clippy::too_many_arguments)]
pub fn emit_funding_checkpoint(
    env: &Env,
    market: &Symbol,
    actor: &Address,
    segment: u32,
    payer_side: PayerSide,
    receiver_backed_delta: i128,
    lp_backed_delta: i128,
    receiver_delta: i128,
    liability_delta: i128,
    ema_after: i128,
    elapsed: u64,
) {
    FundingCheckpoint {
        market: market.clone(),
        header: super::header(env, market, actor),
        segment,
        payer_side,
        receiver_backed_delta,
        lp_backed_delta,
        receiver_delta,
        liability_delta,
        ema_after,
        elapsed,
    }
    .publish(env);
}
