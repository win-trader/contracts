import { describe, it, expect } from "bun:test";
import {
  BPS,
  MarketTick,
  PRECISION,
  evaluatePositionMarkOnly,
  evaluatePositionRow,
  liquidationPriceForPosition,
} from "../src/index.js";

function tickAt(mark: bigint): MarketTick {
  return new MarketTick(
    {
      acc_borrow_index: 0n,
      acc_funding_index: 0n,
      last_index_update: 0n,
      long_open_interest: 1_000_000n * PRECISION,
      short_open_interest: 1_000_000n * PRECISION,
    },
    mark,
  );
}

const baseRow = {
  is_long: true,
  size: "10000000000000",          // 1_000_000 (scaled × 1e7)
  collateral: "1000000000000",     // 100_000 (scaled × 1e7)
  entry_price: "1000000000",       // 100 (scaled × 1e7)
  entry_borrow_index: "0",
  entry_funding_index: "0",
};

describe("evaluatePositionMarkOnly", () => {
  it("computes pnl as size * (mark - entry) / entry for longs", () => {
    const { pnl } = evaluatePositionMarkOnly(
      { is_long: true, size: "10000", entry_price: "100" },
      110n,
    );
    // pnl = 10000 * (110 - 100) / 100 = 1000
    expect(pnl).toBe(1000n);
  });

  it("flips sign for shorts", () => {
    const { pnl } = evaluatePositionMarkOnly(
      { is_long: false, size: "10000", entry_price: "100" },
      110n,
    );
    // pnl = 10000 * (100 - 110) / 100 = -1000
    expect(pnl).toBe(-1000n);
  });

  it("returns zero for zero size", () => {
    const { pnl } = evaluatePositionMarkOnly(
      { is_long: true, size: "0", entry_price: "100" },
      110n,
    );
    expect(pnl).toBe(0n);
  });

  it("coerces null fields as zero", () => {
    const { pnl } = evaluatePositionMarkOnly(
      { is_long: true, size: null, entry_price: null },
      110n,
    );
    expect(pnl).toBe(0n);
  });
});

describe("evaluatePositionRow", () => {
  it("delegates to MarketTick.evaluate via toPositionState", () => {
    const tick = tickAt(110n * PRECISION);
    const evald = evaluatePositionRow(baseRow, tick);
    // size 1M, entry 100, mark 110 → pnl = 1M * 10 / 100 = 100k (scaled)
    expect(evald.pnl).toBe(100_000n * PRECISION);
    // No indices accrued ⇒ no fees
    expect(evald.borrow_fee).toBe(0n);
    expect(evald.funding_fee).toBe(0n);
  });

  it("passes funding_cut_bps through to evaluate", () => {
    // Set acc_funding_index = INDEX_PRECISION so for size=1M (scaled),
    // calcFundingFee returns size-magnitude. Set long_oi > short_oi so the
    // short trader receives funding without zero-sum scaling (payer_oi >=
    // receiver_oi). That gives us a positive zero_sum_funding the cut bites
    // into; without those conditions the cut is multiplied by zero.
    const tick = new MarketTick(
      {
        acc_borrow_index: 0n,
        acc_funding_index: 100_000_000_000_000n, // = INDEX_PRECISION (1e14)
        last_index_update: 0n,
        long_open_interest: 2_000_000n * PRECISION,
        short_open_interest: 1_000_000n * PRECISION,
      },
      100n * PRECISION,
    );
    const noCut = evaluatePositionRow(
      { ...baseRow, is_long: false },
      tick,
      0n,
    );
    const withCut = evaluatePositionRow(
      { ...baseRow, is_long: false },
      tick,
      1000n, // 10% cut
    );
    expect(noCut.funding_protocol_cut).toBe(0n);
    expect(withCut.funding_protocol_cut > 0n).toBe(true);
  });
});

describe("liquidationPriceForPosition", () => {
  it("returns null on degenerate rows", () => {
    const tick = tickAt(100n * PRECISION);
    expect(
      liquidationPriceForPosition({ ...baseRow, size: "0" }, tick, 0n),
    ).toBeNull();
    expect(
      liquidationPriceForPosition({ ...baseRow, collateral: "0" }, tick, 0n),
    ).toBeNull();
    expect(
      liquidationPriceForPosition({ ...baseRow, entry_price: "0" }, tick, 0n),
    ).toBeNull();
  });

  it("at t=0 (no fees), long liq = entry - entry * collateral / size", () => {
    const tick = tickAt(100n * PRECISION); // no accruals
    const liq = liquidationPriceForPosition(baseRow, tick, 0n);
    // entry 100, collateral 100k, size 1M → liq = 100 * (1 - 100k/1M) = 90 (scaled)
    expect(liq).toBe(90n * PRECISION);
  });

  it("short: liq = entry + entry * collateral / size", () => {
    const tick = tickAt(100n * PRECISION);
    const liq = liquidationPriceForPosition(
      { ...baseRow, is_long: false },
      tick,
      0n,
    );
    expect(liq).toBe(110n * PRECISION);
  });

  it("non-zero liq threshold pulls long liq closer to entry (higher)", () => {
    const tick = tickAt(100n * PRECISION);
    const lowerLiq = liquidationPriceForPosition(baseRow, tick, 0n);
    const higherLiq = liquidationPriceForPosition(baseRow, tick, 500n); // 5% threshold
    expect(higherLiq! > lowerLiq!).toBe(true);
  });

  it("accrued borrow fee raises long liq (less downside cushion)", () => {
    // borrow_fee > 0 makes pnl_at_liq more positive → liq closer to entry (higher)
    // Equivalent: as borrow eats into collateral, you liquidate sooner on a long.
    const noFee = tickAt(100n * PRECISION);
    const noFeeLiq = liquidationPriceForPosition(baseRow, noFee, 0n);

    const accruedBorrow = new MarketTick(
      {
        ...noFee.market,
        acc_borrow_index: BigInt("100000000000"), // some positive accrual
      },
      100n * PRECISION,
    );
    const withFeeLiq = liquidationPriceForPosition(baseRow, accruedBorrow, 0n);
    expect(withFeeLiq! > noFeeLiq!).toBe(true);
  });
});
