use super::EventHeader;
use shared::PayerSide;
use soroban_sdk::{contractevent, Address, Env, Symbol};

#[contractevent(topics = ["fundchk"], data_format = "map")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FundingCheckpoint {
    #[topic]
    pub market: Symbol,
    pub header: EventHeader,
    pub segment: u32,
    pub payer_side: PayerSide,
    pub receiver_backed_delta: i128,
    pub lp_backed_delta: i128,
    pub receiver_delta: i128,
    pub liability_delta: i128,
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
