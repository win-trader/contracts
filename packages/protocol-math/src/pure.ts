// Ports of the PositionManager's math (contracts/position-manager/src/math.rs,
// risk.rs, borrow.rs), with the same rounding directions. All amounts are
// protocol-scaled bigints (PRECISION for prices and USD, INDEX_PRECISION for
// rates and indices).
//
// Funding previews are not ported: they integrate a skew EMA over time
// (window.rs), so read accrued funding from the indexer instead.

import { BPS, INDEX_PRECISION, PRECISION, SECONDS_PER_DAY } from "./constants.js";

const ceilDiv = (a: bigint, b: bigint): bigint => (a + b - 1n) / b;

/** Base units a new position acquires: floor for longs, ceil for shorts (`base_added`). */
export function baseExposure(size: bigint, price: bigint, is_long: boolean): bigint {
  return is_long ? (size * PRECISION) / price : ceilDiv(size * PRECISION, price);
}

/** Raw PnL of `size` notional holding `base` units at `price` (`math::pnl`). */
export function pnl(size: bigint, base: bigint, price: bigint, is_long: boolean): bigint {
  return is_long ? (base * price) / PRECISION - size : size - ceilDiv(base * price, PRECISION);
}

/** Raw PnL of a position opened at `entry_price`, marked at `mark_price`. */
export function calcUnrealizedPnl(
  size: bigint,
  entry_price: bigint,
  mark_price: bigint,
  is_long: boolean,
): bigint {
  if (entry_price <= 0n || size <= 0n) return 0n;
  return pnl(size, baseExposure(size, entry_price, is_long), mark_price, is_long);
}

/** Risk units backing `size` (`risk_units_for`). */
export function riskUnits(size: bigint, market_risk_factor_bps: bigint): bigint {
  return (size * market_risk_factor_bps) / BPS;
}

/** The effective collateral at or below which a position is liquidatable. */
export function liquidationThreshold(
  size: bigint,
  maintenance_margin_bps: bigint,
  liquidation_reward: bigint,
): bigint {
  const maintenance = ceilDiv(size * maintenance_margin_bps, BPS);
  return maintenance > liquidation_reward ? maintenance : liquidation_reward;
}

/**
 * The first price at which a position opened at `entry_price` with
 * `collateral` becomes liquidatable, before any fee accrues. For a long it is
 * the highest liquidatable price; for a short, the lowest. `null` when no
 * positive price liquidates it.
 */
export function liquidationPriceAtOpen(
  entry_price: bigint,
  collateral: bigint,
  size: bigint,
  is_long: boolean,
  maintenance_margin_bps: bigint,
  liquidation_reward: bigint,
): bigint | null {
  if (collateral <= 0n || size <= 0n || entry_price <= 0n) return null;
  const base = baseExposure(size, entry_price, is_long);
  const threshold = liquidationThreshold(size, maintenance_margin_bps, liquidation_reward);
  if (is_long) {
    // Liquidatable while floor(base·p/P) ≤ size + threshold − collateral.
    const k = size + threshold - collateral;
    if (k < 0n) return null;
    return ceilDiv((k + 1n) * PRECISION, base) - 1n;
  }
  // Liquidatable once ceil(base·p/P) ≥ collateral + size − threshold.
  const m = collateral + size - threshold;
  if (m <= 0n) return 0n;
  return ((m - 1n) * PRECISION) / base + 1n;
}

/** Utilization of LP equity by open risk, capped at 100% (`utilization_bps`). */
export function utilizationBps(total_risk_units: bigint, cash_lp_equity: bigint): bigint {
  if (total_risk_units === 0n) return 0n;
  if (cash_lp_equity <= 0n) return BPS;
  const u = (total_risk_units * BPS) / cash_lp_equity;
  return u < BPS ? u : BPS;
}

/** Borrow rate in bps per day, scaled by INDEX_PRECISION: base + max_variable · u² (`borrow::rate_at`). */
export function borrowRate(
  utilization_bps: bigint,
  base_borrow_rate_bps_day: bigint,
  max_variable_borrow_bps_day: bigint,
): bigint {
  const u = (utilization_bps * INDEX_PRECISION) / BPS;
  return base_borrow_rate_bps_day * INDEX_PRECISION + max_variable_borrow_bps_day * ((u * u) / INDEX_PRECISION);
}

/** Borrow index growth over `seconds` at `rate` (whole units; the contract carries the remainder). */
export function borrowIndexDelta(rate: bigint, seconds: bigint): bigint {
  return (rate * seconds) / (BPS * SECONDS_PER_DAY);
}

/** Borrow fee due on a position: actual accrual, but never below its window minimum (`calculate_pending`). */
export function pendingBorrow(
  risk_units: bigint,
  index_delta: bigint,
  stored_minimum_borrow_fee: bigint,
): bigint {
  const actual = ceilDiv(risk_units * index_delta, INDEX_PRECISION);
  return actual > stored_minimum_borrow_fee ? actual : stored_minimum_borrow_fee;
}

export function isTpTriggered(take_profit: bigint, mark_price: bigint, is_long: boolean): boolean {
  if (take_profit <= 0n) return false;
  return is_long ? mark_price >= take_profit : mark_price <= take_profit;
}

export function isSlTriggered(stop_loss: bigint, mark_price: bigint, is_long: boolean): boolean {
  if (stop_loss <= 0n) return false;
  return is_long ? mark_price <= stop_loss : mark_price >= stop_loss;
}
