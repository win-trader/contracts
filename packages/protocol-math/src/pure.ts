// Direct ports of contracts/position-manager/src/math.rs. Same shape, same
// semantics, same rounding (integer division everywhere). All amounts are
// protocol-scaled bigints.

import { BPS, EXPOSURE_PRECISION, INDEX_PRECISION, SECONDS_PER_YEAR } from "./constants.js";

export function calcBaseExposure(size: bigint, price: bigint): bigint {
  if (size <= 0n || price <= 0n) return 0n;
  return (size * EXPOSURE_PRECISION) / price;
}

export function deriveEntryPrice(size: bigint, base_exposure: bigint): bigint {
  if (size <= 0n || base_exposure <= 0n) return 0n;
  return (size * EXPOSURE_PRECISION) / base_exposure;
}

export function calcMarkValue(base_exposure: bigint, mark_price: bigint): bigint {
  if (base_exposure <= 0n) return 0n;
  return (base_exposure * mark_price) / EXPOSURE_PRECISION;
}

export function calcUnrealizedPnl(
  size: bigint,
  base_exposure: bigint,
  mark_price: bigint,
  is_long: boolean,
): bigint {
  const mark_value = calcMarkValue(base_exposure, mark_price);
  return is_long ? mark_value - size : size - mark_value;
}

export function calcFeeDebt(size: bigint, current_index: bigint): bigint {
  return (size * current_index) / INDEX_PRECISION;
}

export function calcFeeFromDebt(
  size: bigint,
  current_index: bigint,
  debt: bigint,
): bigint {
  const accrued = calcFeeDebt(size, current_index) - debt;
  return accrued > 0n ? accrued : 0n;
}

export function calcHealth(
  collateral: bigint,
  unrealized_pnl: bigint,
  borrow_fee: bigint,
  skew_fee: bigint,
): bigint {
  return collateral + unrealized_pnl - borrow_fee - skew_fee;
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

export function calcSkewRate(
  long_oi: bigint,
  short_oi: bigint,
  utilization_bps: bigint,
  max_skew_rate_bps: bigint,
): bigint {
  const total = long_oi + short_oi;
  if (total <= 0n || long_oi === short_oi || utilization_bps <= 0n) return 0n;
  const skew = long_oi > short_oi ? long_oi - short_oi : short_oi - long_oi;
  const concentration = ((skew * BPS) / total) < BPS ? (skew * BPS) / total : BPS;
  const quadratic = (concentration * concentration) / BPS;
  const concentrated_rate = (max_skew_rate_bps * quadratic) / BPS;
  const util = utilization_bps < BPS ? utilization_bps : BPS;
  return (concentrated_rate * util) / BPS;
}

export function accumulateFeeIndex(
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
