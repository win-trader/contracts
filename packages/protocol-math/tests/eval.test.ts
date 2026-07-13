import { describe, expect, it } from "bun:test";
import {
  EXPOSURE_PRECISION,
  INDEX_PRECISION,
  MarketTick,
  calcBaseExposure,
  evaluatePositionMarkOnly,
  evaluatePositionRow,
  liquidationPriceForPosition,
  type PositionRowShape,
} from "../src/index.js";

const row: PositionRowShape = {
  is_long: true,
  size: "1000",
  collateral: "100",
  entry_price: "100",
  base_exposure: calcBaseExposure(1000n, 100n).toString(),
  borrow_fee_debt: "0",
  skew_fee_debt: "0",
};

function tick(mark = 110n): MarketTick {
  return new MarketTick({
    acc_borrow_index: 0n,
    acc_long_skew_index: 0n,
    acc_short_skew_index: 0n,
    last_index_update: 0n,
    long_open_interest: 1_000n,
    short_open_interest: 0n,
  }, mark);
}

describe("position evaluation", () => {
  it("computes mark-only PnL from exposure", () => {
    expect(evaluatePositionMarkOnly({
      is_long: true,
      size: "1000",
      base_exposure: (10n * EXPOSURE_PRECISION).toString(),
    }, 110n).pnl).toBe(100n);
  });

  it("coerces a row into the full evaluation", () => {
    expect(evaluatePositionRow(row, tick()).effective_health).toBe(200n);
  });

  it("moves liquidation closer after carrying fees accrue", () => {
    const clean = liquidationPriceForPosition(row, tick(100n), 0n)!;
    const charged = liquidationPriceForPosition(row, new MarketTick({
      ...tick(100n).market,
      acc_borrow_index: INDEX_PRECISION / 100n,
      acc_long_skew_index: INDEX_PRECISION / 100n,
    }, 100n), 0n)!;
    expect(clean).toBe(90n);
    expect(charged > clean).toBe(true);
  });
});
