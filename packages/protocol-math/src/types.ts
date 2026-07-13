// Structural types accepted by the off-chain MarketTick projection and
// PositionEvaluation. Callers pass row-shaped data; the math doesn't care
// where it came from (DB row, RPC response, fixture).

export interface MarketState {
  acc_borrow_index: bigint;
  acc_long_skew_index: bigint;
  acc_short_skew_index: bigint;
  last_index_update: bigint;
  long_open_interest: bigint;
  short_open_interest: bigint;
}

export interface PositionState {
  is_long: boolean;
  size: bigint;
  collateral: bigint;
  entry_price: bigint;
  base_exposure: bigint;
  borrow_fee_debt: bigint;
  skew_fee_debt: bigint;
}

export interface VaultLiquidity {
  reserved_usdc: bigint;
  total_assets: bigint;
}

export interface CarryingFeeConfig {
  base_borrow_rate_bps: bigint;
  slope1_bps: bigint;
  slope2_bps: bigint;
  optimal_utilization_bps: bigint;
  max_skew_rate_bps: bigint;
}

export interface PositionEvaluation {
  pnl: bigint;
  borrow_fee: bigint;
  skew_fee: bigint;
  effective_health: bigint;
}

/** Optional exact/pro-rata slice — defaults to the whole Position. */
export interface Slice {
  size: bigint;
  collateral: bigint;
  base_exposure: bigint;
  borrow_fee_debt: bigint;
  skew_fee_debt: bigint;
}

/** Inputs to project a MarketTick from cached state forward to `now`. */
export interface ProjectInput {
  market: MarketState;
  mark_price: bigint;
  vault: VaultLiquidity;
  rate_config: CarryingFeeConfig;
  /** Unix seconds. */
  now: bigint;
  /**
   * Unix seconds of the last unpause. Indices don't accumulate during pauses,
   * so the projection clamps `effective_start = max(last_index_update, last_unpause_time)`.
   * Pass 0n if the protocol has never paused.
   */
  last_unpause_time: bigint;
}
