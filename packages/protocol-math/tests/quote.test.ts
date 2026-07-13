import { describe, expect, it } from "bun:test";
import {
  BPS,
  MarketTick,
  SECONDS_PER_YEAR,
  evaluateIncrease,
  liquidationPriceAtOpen,
  type CarryingFeeConfig,
  type IncreaseQuoteInput,
} from "../src/index.js";

const rates: CarryingFeeConfig = {
  base_borrow_rate_bps: 100n,
  slope1_bps: 500n,
  slope2_bps: 5000n,
  optimal_utilization_bps: 8000n,
  max_skew_rate_bps: 5000n,
};

function input(is_long = true): IncreaseQuoteInput {
  return {
    intent: { collateral: 1_000n, size: 10_000n, is_long, slippage_bps: 100n },
    tick: new MarketTick({
      acc_borrow_index: 0n,
      acc_long_skew_index: 0n,
      acc_short_skew_index: 0n,
      last_index_update: 0n,
      long_open_interest: 80_000n,
      short_open_interest: 20_000n,
    }, 100n),
    fee_config: { open_fee_bps: 30n },
    vault: { reserved_usdc: 50_000n, total_assets: 100_000n, unclaimed_fees: 0n },
    protocol_limits: { max_utilization_ratio_bps: 8000n, liquidation_threshold_bps: 200n },
    rate_config: rates,
  };
}

describe("evaluateIncrease", () => {
  it("quotes the open fee and future carrying fees separately", () => {
    const q = evaluateIncrease(input());
    expect(q.open_fee).toBe(30n);
    expect(q.daily_borrow).toBe(
      (10_000n * 350n * 86_400n) / (BPS * SECONDS_PER_YEAR),
    );
    expect(q.daily_skew > 0n).toBe(true);
  });

  it("does not give the minority side a skew credit", () => {
    expect(evaluateIncrease(input(false)).daily_skew).toBe(0n);
  });

  it("quotes direction-aware acceptable prices", () => {
    expect(evaluateIncrease(input(true)).acceptable_price).toBe(101n);
    expect(evaluateIncrease(input(false)).acceptable_price).toBe(99n);
  });

  it("reports liquidity headroom", () => {
    const q = evaluateIncrease(input());
    expect(q.liquidity_headroom).toBe(30_000n);
    expect(q.exceeds_liquidity).toBe(false);
  });
});

describe("liquidationPriceAtOpen", () => {
  it("inverts initial long and short health", () => {
    expect(liquidationPriceAtOpen(100n, 1_000n, 10_000n, true, 0n)).toBe(90n);
    expect(liquidationPriceAtOpen(100n, 1_000n, 10_000n, false, 0n)).toBe(110n);
  });
});
