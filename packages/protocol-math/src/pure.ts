// Direct ports of contracts/position-manager/src/math.rs. Same shape, same
// semantics, same rounding (integer division everywhere). All amounts are
// protocol-scaled bigints.

import { BPS, INDEX_PRECISION, SECONDS_PER_YEAR } from "./constants.js";

export function calcUnrealizedPnl(
  size: bigint,
  entry_price: bigint,
  mark_price: bigint,
  is_long: boolean,
): bigint {
  if (entry_price === 0n || size === 0n) return 0n;
  const price_diff = is_long ? mark_price - entry_price : entry_price - mark_price;
  return (size * price_diff) / entry_price;
}

export function calcBorrowFee(
  size: bigint,
  entry_borrow_index: bigint,
  current_borrow_index: bigint,
): bigint {
  return ((current_borrow_index - entry_borrow_index) * size) / INDEX_PRECISION;
}

export function calcFundingFee(
  size: bigint,
  entry_funding_index: bigint,
  current_funding_index: bigint,
  is_long: boolean,
): bigint {
  const delta = current_funding_index - entry_funding_index;
  return is_long
    ? -((delta * size) / INDEX_PRECISION)
    : (delta * size) / INDEX_PRECISION;
}

export function calcHealth(
  collateral: bigint,
  unrealized_pnl: bigint,
  borrow_fee: bigint,
  funding_fee: bigint,
): bigint {
  return collateral + unrealized_pnl - borrow_fee + funding_fee;
}

export function calcUtilizationBps(reserved: bigint, total_assets: bigint): bigint {
  if (total_assets <= 0n) return 0n;
  return (reserved * BPS) / total_assets;
}

export function calcBorrowRate(
  utilization_bps: bigint,
  base_borrow_rate: bigint,
  slope1: bigint,
  slope2: bigint,
  optimal_util: bigint,
): bigint {
  if (utilization_bps <= optimal_util) {
    return base_borrow_rate + (utilization_bps * slope1) / BPS;
  }
  return (
    base_borrow_rate +
    (optimal_util * slope1) / BPS +
    ((utilization_bps - optimal_util) * slope2) / BPS
  );
}

export function calcFundingRate(
  long_oi: bigint,
  short_oi: bigint,
  base_funding_rate: bigint,
): bigint {
  const total = long_oi + short_oi;
  if (total === 0n) return 0n;
  // bigint is unbounded, so the Rust contract's progressive-halving fallback
  // for i128 overflow is unnecessary. Direct division gives identical results
  // for any input that wouldn't have overflowed i128 in the contract.
  const imbalance = long_oi - short_oi;
  return (imbalance * base_funding_rate) / total;
}

export function accumulateBorrowIndex(
  current_index: bigint,
  rate_bps: bigint,
  time_delta: bigint,
): bigint {
  return (
    current_index +
    (rate_bps * INDEX_PRECISION * time_delta) / (BPS * SECONDS_PER_YEAR)
  );
}

export function accumulateFundingIndex(
  current_index: bigint,
  rate_bps: bigint,
  time_delta: bigint,
): bigint {
  return (
    current_index +
    (rate_bps * INDEX_PRECISION * time_delta) / (BPS * SECONDS_PER_YEAR)
  );
}

export function isTpTriggered(
  take_profit: bigint,
  mark_price: bigint,
  is_long: boolean,
): boolean {
  if (take_profit <= 0n) return false;
  return is_long ? mark_price >= take_profit : mark_price <= take_profit;
}

export function isSlTriggered(
  stop_loss: bigint,
  mark_price: bigint,
  is_long: boolean,
): boolean {
  if (stop_loss <= 0n) return false;
  return is_long ? mark_price <= stop_loss : mark_price >= stop_loss;
}
