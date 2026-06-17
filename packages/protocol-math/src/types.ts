// Structural types accepted by the off-chain MarketTick projection and
// PositionEvaluation. Callers pass row-shaped data; the math doesn't care
// where it came from (DB row, RPC response, fixture).

export interface MarketState {
  acc_borrow_index: bigint;
  acc_funding_index: bigint;
  last_index_update: bigint;
  long_open_interest: bigint;
  short_open_interest: bigint;
}

export interface PositionState {
  is_long: boolean;
  size: bigint;
  collateral: bigint;
  entry_price: bigint;
  entry_borrow_index: bigint;
  entry_funding_index: bigint;
}

export interface VaultLiquidity {
  reserved_usdc: bigint;
  total_assets: bigint;
}

export interface BorrowRateConfig {
  base_borrow_rate_bps: bigint;
  slope1_bps: bigint;
  slope2_bps: bigint;
  optimal_utilization_bps: bigint;
  base_funding_rate_bps: bigint;
}

export interface PositionEvaluation {
  pnl: bigint;
  borrow_fee: bigint;
  /** Raw direction-corrected funding accrual. Display-only — settlement and
   *  the liquidation gate use `effective_funding`. */
  funding_fee: bigint;
  /** `funding_fee` after zero-sum scaling (`min(payer_oi, receiver_oi) /
   *  receiver_oi`) and the protocol funding cut. Matches the value the
   *  contract moves between accounts. */
  effective_funding: bigint;
  /** Protocol slice of positive funding accruals (zero when paying). */
  funding_protocol_cut: bigint;
  /** Display-only raw health using `funding_fee`. */
  health: bigint;
  /** Health using `effective_funding`. The on-chain liquidation gate
   *  compares this against the threshold — off-chain gates must too. */
  effective_health: bigint;
}

/** Optional slice — defaults to (size, collateral) of the whole Position. */
export interface Slice {
  size: bigint;
  collateral: bigint;
}

/** Inputs to project a MarketTick from cached state forward to `now`. */
export interface ProjectInput {
  market: MarketState;
  mark_price: bigint;
  vault: VaultLiquidity;
  rate_config: BorrowRateConfig;
  /** Unix seconds. */
  now: bigint;
  /**
   * Unix seconds of the last unpause. Indices don't accumulate during pauses,
   * so the projection clamps `effective_start = max(last_index_update, last_unpause_time)`.
   * Pass 0n if the protocol has never paused.
   */
  last_unpause_time: bigint;
}
