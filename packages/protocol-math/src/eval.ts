// Position-row evaluation helpers. Twin of quote.ts (Increase intent → quote);
// here we go row → PositionEvaluation against a MarketTick. Two operations,
// each with a single fixed shape — callers pick the one matching what data
// they hold, instead of branching on optional tick inputs.

import { BPS, EXPOSURE_PRECISION } from "./constants.js";
import { calcUnrealizedPnl } from "./pure.js";
import {
  toBigInt,
  toPositionState,
  type PositionRowShape,
  type Stringy,
} from "./coerce.js";
import type { MarketTick } from "./tick.js";
import type { PositionEvaluation } from "./types.js";

/** Subset of PositionRowShape sufficient for mark-only PnL. */
export interface PositionMarkInput {
  is_long: boolean;
  size: Stringy;
  base_exposure: Stringy;
}

/**
 * Full PositionEvaluation for an existing Position row against a MarketTick.
 * Used wherever the projection inputs (vault, market, config) are loaded so
 * the row can surface fee-adjusted health / accrued borrow + skew fees.
 */
export function evaluatePositionRow(
  row: PositionRowShape,
  tick: MarketTick,
): PositionEvaluation {
  return tick.evaluate(toPositionState(row));
}

/**
 * Mark-to-market PnL only. Used where the row doesn't carry enough state
 * for a full evaluation (e.g. LeaderboardOpenPosition has no collateral or
 * entry indices) or where the caller only needs `pnl` for display.
 */
export function evaluatePositionMarkOnly(
  row: PositionMarkInput,
  mark_price: bigint,
): { pnl: bigint } {
  return {
    pnl: calcUnrealizedPnl(
      toBigInt(row.size),
      toBigInt(row.base_exposure),
      mark_price,
      row.is_long,
    ),
  };
}

/**
 * Liquidation price for an existing Position projected against the given
 * MarketTick. Inverts the on-chain liquidation gate
 *   `effective_health < collateral * liquidation_threshold_bps / BPS`
 * to solve for the mark price that would trip it, using the position's
 * current accrued borrow and skew fees so the line on the chart
 * reflects "where the position liquidates *right now*", not "where it
 * would have liquidated at t=0 with no fees."
 *
 * Returns `null` for degenerate positions (zero size, collateral, or entry).
 */
export function liquidationPriceForPosition(
  row: PositionRowShape,
  tick: MarketTick,
  liquidation_threshold_bps: bigint,
): bigint | null {
  const state = toPositionState(row);
  if (state.size === 0n || state.collateral === 0n || state.base_exposure === 0n) {
    return null;
  }
  const evald = tick.evaluate(state);
  const threshold_value = (state.collateral * liquidation_threshold_bps) / BPS;
  // At liq: collateral + pnl - borrow_fee - skew_fee == threshold_value.
  const pnl_at_liq =
    threshold_value - state.collateral + evald.borrow_fee + evald.skew_fee;
  const mark_value = state.is_long
    ? state.size + pnl_at_liq
    : state.size - pnl_at_liq;
  if (mark_value <= 0n) return 0n;
  return (mark_value * EXPOSURE_PRECISION) / state.base_exposure;
}
