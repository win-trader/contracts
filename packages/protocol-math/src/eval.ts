// Position-row evaluation helpers. Twin of quote.ts (Increase intent → quote);
// here we go row → PositionEvaluation against a MarketTick. Two operations,
// each with a single fixed shape — callers pick the one matching what data
// they hold, instead of branching on optional tick inputs.

import { BPS } from "./constants.js";
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
  entry_price: Stringy;
}

/**
 * Full PositionEvaluation for an existing Position row against a MarketTick.
 * Used wherever the projection inputs (vault, market, config) are loaded so
 * the row can surface fee-adjusted health / accrued borrow + funding.
 */
export function evaluatePositionRow(
  row: PositionRowShape,
  tick: MarketTick,
  funding_cut_bps: bigint = 0n,
): PositionEvaluation {
  return tick.evaluate(toPositionState(row), undefined, funding_cut_bps);
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
      toBigInt(row.entry_price),
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
 * current accrued borrow / effective_funding so the line on the chart
 * reflects "where the position liquidates *right now*", not "where it
 * would have liquidated at t=0 with no fees."
 *
 * Returns `null` for degenerate positions (zero size, collateral, or entry).
 */
export function liquidationPriceForPosition(
  row: PositionRowShape,
  tick: MarketTick,
  liquidation_threshold_bps: bigint,
  funding_cut_bps: bigint = 0n,
): bigint | null {
  const state = toPositionState(row);
  if (state.size === 0n || state.collateral === 0n || state.entry_price === 0n) {
    return null;
  }
  const evald = tick.evaluate(state, undefined, funding_cut_bps);
  const threshold_value = (state.collateral * liquidation_threshold_bps) / BPS;
  // At liq: collateral + pnl - borrow_fee + effective_funding == threshold_value
  // ⇒ pnl_at_liq = threshold_value - collateral + borrow_fee - effective_funding
  const pnl_at_liq =
    threshold_value - state.collateral + evald.borrow_fee - evald.effective_funding;
  // Long:  pnl = size * (liq - entry) / entry  ⇒  liq = entry + entry * pnl / size
  // Short: pnl = size * (entry - liq) / entry  ⇒  liq = entry - entry * pnl / size
  const adjustment = (state.entry_price * pnl_at_liq) / state.size;
  return state.is_long ? state.entry_price + adjustment : state.entry_price - adjustment;
}
