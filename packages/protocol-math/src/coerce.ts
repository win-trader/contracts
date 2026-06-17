// Row → bigint-typed state coercers. Single home for the "stringly-typed row
// → MarketTick-shaped state" transform that keeper, indexer-derived API
// responses, and frontend hooks all need. Inputs are structural — any object
// with the right column names (drizzle row, API response, fixture literal)
// works. Outputs are the bigint shapes that the rest of protocol-math
// consumes (MarketState, PositionState, VaultLiquidity, BorrowRateConfig).

import type {
  BorrowRateConfig,
  MarketState,
  PositionState,
  VaultLiquidity,
} from "./types.js";

export type Stringy = string | null | undefined;

export function toBigInt(value: Stringy): bigint {
  if (value == null || value === "") return 0n;
  return BigInt(value);
}

export interface MarketRowShape {
  acc_borrow_index: Stringy;
  acc_funding_index: Stringy;
  last_index_update: Stringy;
  long_open_interest: Stringy;
  short_open_interest: Stringy;
}

export function toMarketState(row: MarketRowShape): MarketState {
  return {
    acc_borrow_index: toBigInt(row.acc_borrow_index),
    acc_funding_index: toBigInt(row.acc_funding_index),
    last_index_update: toBigInt(row.last_index_update),
    long_open_interest: toBigInt(row.long_open_interest),
    short_open_interest: toBigInt(row.short_open_interest),
  };
}

export interface PositionRowShape {
  is_long: boolean;
  size: Stringy;
  collateral: Stringy;
  entry_price: Stringy;
  entry_borrow_index: Stringy;
  entry_funding_index: Stringy;
}

export function toPositionState(row: PositionRowShape): PositionState {
  return {
    is_long: row.is_long,
    size: toBigInt(row.size),
    collateral: toBigInt(row.collateral),
    entry_price: toBigInt(row.entry_price),
    entry_borrow_index: toBigInt(row.entry_borrow_index),
    entry_funding_index: toBigInt(row.entry_funding_index),
  };
}

export interface VaultRowShape {
  reserved_usdc: Stringy;
  total_assets: Stringy;
}

export function toVaultLiquidity(row: VaultRowShape | undefined): VaultLiquidity {
  return {
    reserved_usdc: toBigInt(row?.reserved_usdc),
    total_assets: toBigInt(row?.total_assets),
  };
}

export interface BorrowRateRowShape {
  base_borrow_rate_bps: Stringy;
  slope1_bps: Stringy;
  slope2_bps: Stringy;
  optimal_utilization_bps: Stringy;
  base_funding_rate_bps: Stringy;
}

export function toBorrowRateConfig(
  row: BorrowRateRowShape | undefined,
): BorrowRateConfig {
  return {
    base_borrow_rate_bps: toBigInt(row?.base_borrow_rate_bps),
    slope1_bps: toBigInt(row?.slope1_bps),
    slope2_bps: toBigInt(row?.slope2_bps),
    optimal_utilization_bps: toBigInt(row?.optimal_utilization_bps),
    base_funding_rate_bps: toBigInt(row?.base_funding_rate_bps),
  };
}
