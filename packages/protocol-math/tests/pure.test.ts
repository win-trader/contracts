import { describe, expect, test } from "bun:test";
import {
  BPS,
  INDEX_PRECISION,
  PRECISION,
  baseExposure,
  borrowIndexDelta,
  borrowRate,
  calcUnrealizedPnl,
  liquidationPriceAtOpen,
  liquidationThreshold,
  pendingBorrow,
  pnl,
  riskUnits,
  utilizationBps,
} from "../src/index.js";

const usd = (n: bigint) => n * PRECISION;

describe("PnL mirrors math::pnl", () => {
  test("base rounds against the trader: floor for longs, ceil for shorts", () => {
    expect(baseExposure(usd(1_000n), usd(3n), true)).toBe(3_333_333_333n);
    expect(baseExposure(usd(1_000n), usd(3n), false)).toBe(3_333_333_334n);
  });

  test("a 10% move on $1,000 is $100 either way", () => {
    expect(calcUnrealizedPnl(usd(1_000n), usd(50_000n), usd(55_000n), true)).toBe(usd(100n));
    expect(calcUnrealizedPnl(usd(1_000n), usd(50_000n), usd(45_000n), false)).toBe(usd(100n));
  });

  test("rounding never favours the trader", () => {
    // Opened and marked at the same price: zero or a unit's loss, never a gain.
    for (const is_long of [true, false]) {
      expect(calcUnrealizedPnl(usd(1_000n), usd(3n), usd(3n), is_long)).toBeLessThanOrEqual(0n);
    }
  });
});

describe("liquidation price matches the contract's liquidation test", () => {
  const mm = 250n; // maintenance margin bps
  const reward = 2_500_000n; // $0.25
  const liquidatable = (size: bigint, collateral: bigint, entry: bigint, price: bigint, is_long: boolean) =>
    collateral + pnl(size, baseExposure(size, entry, is_long), price, is_long) <=
    liquidationThreshold(size, mm, reward);

  for (const is_long of [true, false]) {
    test(`${is_long ? "long" : "short"}: the boundary price liquidates, one tick safer does not`, () => {
      const size = usd(10_000n);
      const collateral = usd(1_000n);
      const entry = usd(60_000n);
      const p = liquidationPriceAtOpen(entry, collateral, size, is_long, mm, reward)!;
      expect(liquidatable(size, collateral, entry, p, is_long)).toBe(true);
      expect(liquidatable(size, collateral, entry, is_long ? p + 1n : p - 1n, is_long)).toBe(false);
    });
  }

  test("a long whose collateral exceeds its size has no liquidation price", () => {
    expect(liquidationPriceAtOpen(usd(100n), usd(2_000n), usd(1_000n), true, mm, reward)).toBeNull();
  });

  test("the threshold is the larger of maintenance margin and the liquidation reward", () => {
    expect(liquidationThreshold(usd(1_000n), mm, reward)).toBe(usd(25n));
    expect(liquidationThreshold(5n, mm, reward)).toBe(reward);
  });
});

describe("borrow mirrors borrow::rate_at and calculate_pending", () => {
  test("utilization is risk over equity, capped at 100%", () => {
    expect(utilizationBps(0n, 0n)).toBe(0n);
    expect(utilizationBps(usd(50n), usd(100n))).toBe(5_000n);
    expect(utilizationBps(usd(500n), usd(100n))).toBe(BPS);
    expect(utilizationBps(1n, 0n)).toBe(BPS);
  });

  test("the rate is base plus max_variable times utilization squared", () => {
    // 50% utilization: 25 + 250 · 0.25 = 87.5 bps/day.
    expect(borrowRate(5_000n, 25n, 250n)).toBe(87n * INDEX_PRECISION + INDEX_PRECISION / 2n);
    expect(borrowRate(0n, 25n, 250n)).toBe(25n * INDEX_PRECISION);
  });

  test("matches the contract's BORROW_CURVE_VECTORS (shared/src/fixed.rs) at 25 + 250 bps/day", () => {
    const vectors: [bigint, bigint][] = [
      [0n, 2500000000000000n],
      [2500n, 4062500000000000n],
      [5000n, 8750000000000000n],
      [7500n, 16562500000000000n],
      [8500n, 20562500000000000n],
      [10000n, 27500000000000000n],
    ];
    for (const [util, rate] of vectors) expect(borrowRate(util, 25n, 250n)).toBe(rate);
  });

  test("one day at 100 bps/day moves the index by 1% of INDEX_PRECISION", () => {
    expect(borrowIndexDelta(100n * INDEX_PRECISION, 86_400n)).toBe(INDEX_PRECISION / 100n);
  });

  test("pending borrow rounds up and never undercuts the window minimum", () => {
    const units = riskUnits(usd(1_000n), 1_000n); // 10% of $1,000
    expect(units).toBe(usd(100n));
    expect(pendingBorrow(units, INDEX_PRECISION / 100n, 0n)).toBe(usd(1n));
    expect(pendingBorrow(units, 1n, 0n)).toBe(1n);
    expect(pendingBorrow(units, 1n, usd(2n))).toBe(usd(2n));
  });
});
