import { describe, it, expect } from "bun:test";
import {
  BPS,
  INDEX_PRECISION,
  MarketTick,
  PRECISION,
  SECONDS_PER_YEAR,
} from "../src/index.js";
import type {
  BorrowRateConfig,
  MarketState,
  PositionState,
  ProjectInput,
  VaultLiquidity,
} from "../src/types.js";

const baseMarket: MarketState = {
  acc_borrow_index: 0n,
  acc_funding_index: 0n,
  last_index_update: 1_000n,
  long_open_interest: 1_000_000n * PRECISION,
  short_open_interest: 500_000n * PRECISION,
};

const baseVault: VaultLiquidity = {
  reserved_usdc: 100_000n * PRECISION,
  total_assets: 1_000_000n * PRECISION,
};

const baseRateConfig: BorrowRateConfig = {
  base_borrow_rate_bps: 100n,
  slope1_bps: 500n,
  slope2_bps: 5000n,
  optimal_utilization_bps: 8000n,
  base_funding_rate_bps: 100n,
};

function input(overrides: Partial<ProjectInput> = {}): ProjectInput {
  return {
    market: baseMarket,
    mark_price: 100n * PRECISION,
    vault: baseVault,
    rate_config: baseRateConfig,
    now: 1_000n,
    last_unpause_time: 0n,
    ...overrides,
  };
}

describe("MarketTick.project", () => {
  it("zero time_delta leaves indices unchanged", () => {
    const tick = MarketTick.project(input({ now: 1_000n }));
    expect(tick.market.acc_borrow_index).toBe(0n);
    expect(tick.market.acc_funding_index).toBe(0n);
  });

  it("advances borrow index forward in time", () => {
    const tick = MarketTick.project(input({ now: 1_000n + SECONDS_PER_YEAR }));
    // 10% utilization (100k/1M), base=100, slope1=500 → rate = 100 + 1000*500/10000 = 150 bps
    // index_delta = 150 * 1e14 * SECONDS_PER_YEAR / (1e4 * SECONDS_PER_YEAR) = 1.5e12
    expect(tick.market.acc_borrow_index).toBe(150n * (INDEX_PRECISION / BPS));
  });

  it("advances funding index forward in time", () => {
    const tick = MarketTick.project(input({ now: 1_000n + SECONDS_PER_YEAR }));
    // long=1M, short=0.5M, total=1.5M, base=100 → rate = 0.5M*100/1.5M = 33
    // index_delta = 33 * 1e14 / 1e4 = 3.3e11
    expect(tick.market.acc_funding_index).toBe(33n * (INDEX_PRECISION / BPS));
  });

  it("clamps effective_start to last_unpause_time when newer", () => {
    // Pause clamp: indices accumulate only from the unpause moment forward.
    const last_index_update = 1_000n;
    const last_unpause_time = 1_500n;
    const now = 2_000n;
    const market = { ...baseMarket, last_index_update };
    const projected = MarketTick.project(input({ market, now, last_unpause_time }));
    const noPause = MarketTick.project(
      input({
        market: { ...baseMarket, last_index_update: last_unpause_time },
        now,
        last_unpause_time: 0n,
      }),
    );
    // Both should accumulate over the same effective window (now - 1_500).
    expect(projected.market.acc_borrow_index).toBe(noPause.market.acc_borrow_index);
  });

  it("ignores last_unpause_time when older than last_index_update", () => {
    const last_index_update = 2_000n;
    const last_unpause_time = 1_000n; // older — irrelevant
    const market = { ...baseMarket, last_index_update };
    const tick = MarketTick.project(
      input({ market, now: 2_000n + SECONDS_PER_YEAR, last_unpause_time }),
    );
    // Should match the no-pause case anchored at last_index_update.
    const noPause = MarketTick.project(
      input({ market, now: 2_000n + SECONDS_PER_YEAR, last_unpause_time: 0n }),
    );
    expect(tick.market.acc_borrow_index).toBe(noPause.market.acc_borrow_index);
  });

  it("sets projected last_index_update to now", () => {
    const tick = MarketTick.project(input({ now: 9_999n }));
    expect(tick.market.last_index_update).toBe(9_999n);
  });
});

describe("MarketTick.evaluate", () => {
  const pos: PositionState = {
    is_long: true,
    size: 1_000n * PRECISION,
    collateral: 100n * PRECISION,
    entry_price: 100n * PRECISION,
    entry_borrow_index: 0n,
    entry_funding_index: 0n,
  };

  it("whole position by default", () => {
    const tick = new MarketTick(
      { ...baseMarket, acc_borrow_index: 0n, acc_funding_index: 0n },
      110n * PRECISION,
    );
    const e = tick.evaluate(pos);
    // PnL = 1000 * 10/100 = 100 USDC; borrow=0; funding=0; health = 100 + 100 = 200
    expect(e.pnl).toBe(100n * PRECISION);
    expect(e.borrow_fee).toBe(0n);
    expect(e.funding_fee).toBe(0n);
    expect(e.health).toBe(200n * PRECISION);
  });

  it("slice scales linearly", () => {
    const tick = new MarketTick(
      { ...baseMarket, acc_borrow_index: 0n, acc_funding_index: 0n },
      110n * PRECISION,
    );
    const half = tick.evaluate(pos, { size: pos.size / 2n, collateral: pos.collateral / 2n });
    const whole = tick.evaluate(pos);
    expect(half.pnl).toBe(whole.pnl / 2n);
    expect(half.health).toBe(whole.health / 2n);
  });

  it("longs pay funding when funding index rose", () => {
    const tick = new MarketTick(
      { ...baseMarket, acc_funding_index: INDEX_PRECISION / 100n }, // +1%
      100n * PRECISION,
    );
    const e = tick.evaluate(pos);
    expect(e.funding_fee).toBeLessThan(0n);
  });

  it("payer-side funding passes through to effective_funding", () => {
    const market = {
      ...baseMarket,
      acc_funding_index: INDEX_PRECISION / 100n,
      long_open_interest: pos.size,
      short_open_interest: 0n,
    };
    const tick = new MarketTick(market, 100n * PRECISION);
    const e = tick.evaluate(pos, undefined, 500n);
    expect(e.funding_fee).toBeLessThan(0n);
    expect(e.funding_protocol_cut).toBe(0n);
    expect(e.effective_funding).toBe(e.funding_fee);
    expect(e.effective_health).toBe(e.health);
  });

  it("receiver-side funding is zero-sum-capped when payer_oi < receiver_oi", () => {
    const shortPos: PositionState = { ...pos, is_long: false };
    const market = {
      ...baseMarket,
      acc_funding_index: INDEX_PRECISION / 100n,
      long_open_interest: pos.size / 4n,
      short_open_interest: pos.size,
    };
    const tick = new MarketTick(market, 100n * PRECISION);
    const e = tick.evaluate(shortPos, undefined, 0n);
    expect(e.funding_fee).toBeGreaterThan(0n);
    expect(e.effective_funding).toBe(e.funding_fee / 4n);
  });

  it("receiver-side funding takes the protocol cut after zero-sum scaling", () => {
    const shortPos: PositionState = { ...pos, is_long: false };
    const market = {
      ...baseMarket,
      acc_funding_index: INDEX_PRECISION / 100n,
      long_open_interest: pos.size,
      short_open_interest: pos.size,
    };
    const tick = new MarketTick(market, 100n * PRECISION);
    const e = tick.evaluate(shortPos, undefined, 1000n);
    expect(e.funding_protocol_cut).toBe(e.funding_fee / 10n);
    expect(e.effective_funding).toBe(e.funding_fee - e.funding_protocol_cut);
  });

  it("receiver with zero opposing OI gets nothing", () => {
    const shortPos: PositionState = { ...pos, is_long: false };
    const market = {
      ...baseMarket,
      acc_funding_index: INDEX_PRECISION / 100n,
      long_open_interest: 0n,
      short_open_interest: pos.size,
    };
    const tick = new MarketTick(market, 100n * PRECISION);
    const e = tick.evaluate(shortPos);
    expect(e.funding_fee).toBeGreaterThan(0n);
    expect(e.effective_funding).toBe(0n);
  });
});

describe("MarketTick.isTpTriggered / isSlTriggered", () => {
  it("delegates mark price from the bound tick", () => {
    const tick = new MarketTick(baseMarket, 110n * PRECISION);
    expect(tick.isTpTriggered(110n * PRECISION, true)).toBe(true);
    expect(tick.isTpTriggered(111n * PRECISION, true)).toBe(false);
    expect(tick.isSlTriggered(110n * PRECISION, true)).toBe(true);
    expect(tick.isSlTriggered(109n * PRECISION, true)).toBe(false);
  });
});
