import { describe, expect, it } from "bun:test";
import {
  EXPOSURE_PRECISION,
  INDEX_PRECISION,
  MarketTick,
  SECONDS_PER_YEAR,
  calcBaseExposure,
  type CarryingFeeConfig,
  type MarketState,
  type PositionState,
} from "../src/index.js";

const market: MarketState = {
  acc_borrow_index: 0n,
  acc_long_skew_index: 0n,
  acc_short_skew_index: 0n,
  last_index_update: 100n,
  long_open_interest: 80_000n,
  short_open_interest: 20_000n,
};
const rate_config: CarryingFeeConfig = {
  base_borrow_rate_bps: 100n,
  slope1_bps: 0n,
  slope2_bps: 0n,
  optimal_utilization_bps: 8000n,
  max_skew_rate_bps: 5000n,
};

describe("MarketTick.project", () => {
  it("accrues borrow and only the dominant-side skew index", () => {
    const tick = MarketTick.project({
      market,
      mark_price: 100n,
      vault: { reserved_usdc: 100_000n, total_assets: 200_000n },
      rate_config,
      now: 100n + SECONDS_PER_YEAR,
      last_unpause_time: 0n,
    });
    expect(tick.market.acc_borrow_index).toBe(INDEX_PRECISION / 100n);
    expect(tick.market.acc_long_skew_index > 0n).toBe(true);
    expect(tick.market.acc_short_skew_index).toBe(0n);
  });

  it("balanced OI accrues no skew", () => {
    const tick = MarketTick.project({
      market: { ...market, long_open_interest: 50_000n, short_open_interest: 50_000n },
      mark_price: 100n,
      vault: { reserved_usdc: 100_000n, total_assets: 200_000n },
      rate_config,
      now: 100n + SECONDS_PER_YEAR,
      last_unpause_time: 0n,
    });
    expect(tick.market.acc_long_skew_index).toBe(0n);
    expect(tick.market.acc_short_skew_index).toBe(0n);
  });
});

describe("MarketTick.evaluate", () => {
  it("uses exposure PnL and subtracts both carrying fees", () => {
    const size = 1_000n;
    const pos: PositionState = {
      is_long: true,
      size,
      collateral: 200n,
      entry_price: 100n,
      base_exposure: calcBaseExposure(size, 100n),
      borrow_fee_debt: 0n,
      skew_fee_debt: 0n,
    };
    const tick = new MarketTick({
      ...market,
      acc_borrow_index: INDEX_PRECISION / 100n,
      acc_long_skew_index: INDEX_PRECISION / 50n,
    }, 110n);
    expect(pos.base_exposure).toBe(10n * EXPOSURE_PRECISION);
    expect(tick.evaluate(pos)).toEqual({
      pnl: 100n,
      borrow_fee: 10n,
      skew_fee: 20n,
      effective_health: 270n,
    });
  });
});
