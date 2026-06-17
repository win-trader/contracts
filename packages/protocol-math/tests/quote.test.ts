import { describe, it, expect } from "bun:test";
import {
  BPS,
  MarketTick,
  PRECISION,
  evaluateIncrease,
  liquidationPriceAtOpen,
  type BorrowRateConfig,
  type IncreaseQuoteInput,
} from "../src/index.js";

const rateConfig: BorrowRateConfig = {
  base_borrow_rate_bps: 100n,    // 1% base
  slope1_bps: 500n,
  slope2_bps: 5000n,
  optimal_utilization_bps: 8000n,
  base_funding_rate_bps: 100n,
};

function tick(opts: {
  mark?: bigint;
  long_oi?: bigint;
  short_oi?: bigint;
  acc_borrow?: bigint;
  acc_funding?: bigint;
} = {}): MarketTick {
  return new MarketTick(
    {
      acc_borrow_index: opts.acc_borrow ?? 0n,
      acc_funding_index: opts.acc_funding ?? 0n,
      last_index_update: 1_000n,
      long_open_interest: opts.long_oi ?? 1_000_000n * PRECISION,
      short_open_interest: opts.short_oi ?? 1_000_000n * PRECISION,
    },
    opts.mark ?? 100n * PRECISION,
  );
}

function baseInput(
  overrides: Partial<IncreaseQuoteInput> & {
    intent?: Partial<IncreaseQuoteInput["intent"]>;
    vault?: Partial<IncreaseQuoteInput["vault"]>;
    protocol_limits?: Partial<IncreaseQuoteInput["protocol_limits"]>;
  } = {},
): IncreaseQuoteInput {
  return {
    intent: {
      collateral: 1_000n * PRECISION,
      size: 10_000n * PRECISION,
      is_long: true,
      slippage_bps: 0n,
      ...(overrides.intent ?? {}),
    },
    tick: overrides.tick ?? tick(),
    fee_config: { open_fee_bps: 30n, ...(overrides.fee_config ?? {}) },
    vault: {
      reserved_usdc: 100_000n * PRECISION,
      total_assets: 1_000_000n * PRECISION,
      unclaimed_fees: 0n,
      ...(overrides.vault ?? {}),
    },
    protocol_limits: {
      max_utilization_ratio_bps: 8000n,
      liquidation_threshold_bps: 0n,
      ...(overrides.protocol_limits ?? {}),
    },
    rate_config: overrides.rate_config ?? rateConfig,
  };
}

describe("evaluateIncrease — open fee", () => {
  it("computes size * open_fee_bps / BPS", () => {
    const q = evaluateIncrease(baseInput());
    // 10_000 * 30 / 10_000 = 30 (scaled)
    expect(q.open_fee).toBe(30n * PRECISION);
  });

  it("is zero when size is zero", () => {
    const q = evaluateIncrease(baseInput({ intent: { collateral: 0n, size: 0n, is_long: true } }));
    expect(q.open_fee).toBe(0n);
  });
});

describe("evaluateIncrease — borrow / funding daily", () => {
  it("daily_borrow uses safe_basis-based utilization", () => {
    // 10% util (100k / 1M), under-optimal kink: base + util*slope1/BPS = 100 + 1000*500/10000 = 150 bps
    // daily = size * 150 * 86400 / (BPS * SECONDS_PER_YEAR)
    // = 10000e7 * 150 * 86400 / (10000 * 31536000) = 4.109e7  ≈ 4.10 (scaled)
    const q = evaluateIncrease(baseInput());
    // assert positive + within an order of magnitude rather than exact: bigint truncation
    expect(q.daily_borrow > 0n).toBe(true);
    expect(q.daily_borrow).toBe(
      (10_000n * PRECISION * 150n * 86_400n) / (10_000n * 31_536_000n),
    );
  });

  it("subtracts unclaimed_fees from safe_basis", () => {
    // Same reserved but unclaimed_fees eats 50% of total_assets → safe_basis halves → util doubles → rate higher
    const high = evaluateIncrease(
      baseInput({
        vault: {
          reserved_usdc: 100_000n * PRECISION,
          total_assets: 1_000_000n * PRECISION,
          unclaimed_fees: 500_000n * PRECISION,
        },
      }),
    );
    const low = evaluateIncrease(baseInput());
    expect(high.daily_borrow > low.daily_borrow).toBe(true);
  });

  it("daily_funding is negative for the dominant side (longs pay when long_oi > short_oi)", () => {
    const q = evaluateIncrease(
      baseInput({
        tick: tick({
          long_oi: 2_000_000n * PRECISION,
          short_oi: 1_000_000n * PRECISION,
        }),
        intent: { is_long: true, collateral: 1_000n * PRECISION, size: 10_000n * PRECISION },
      }),
    );
    expect(q.daily_funding < 0n).toBe(true);
  });

  it("daily_funding flips sign with side", () => {
    const long = evaluateIncrease(
      baseInput({
        tick: tick({ long_oi: 2_000_000n * PRECISION, short_oi: 1_000_000n * PRECISION }),
        intent: { is_long: true, collateral: 1_000n * PRECISION, size: 10_000n * PRECISION },
      }),
    );
    const short = evaluateIncrease(
      baseInput({
        tick: tick({ long_oi: 2_000_000n * PRECISION, short_oi: 1_000_000n * PRECISION }),
        intent: { is_long: false, collateral: 1_000n * PRECISION, size: 10_000n * PRECISION },
      }),
    );
    expect(long.daily_funding).toBe(-short.daily_funding);
  });

  it("daily_funding is zero when long_oi == short_oi", () => {
    const q = evaluateIncrease(baseInput());
    expect(q.daily_funding).toBe(0n);
  });
});

describe("evaluateIncrease — liquidation price", () => {
  it("returns null on degenerate inputs", () => {
    const noSize = evaluateIncrease(baseInput({ intent: { collateral: 1n, size: 0n, is_long: true } }));
    expect(noSize.liquidation_price).toBeNull();

    const noColl = evaluateIncrease(baseInput({ intent: { collateral: 0n, size: 1n, is_long: true } }));
    expect(noColl.liquidation_price).toBeNull();
  });

  it("long: liq = mark - mark * collateral / size when threshold = 0", () => {
    // collateral/size = 1/10 → adjustment = mark/10 → liq = mark * 9/10
    const q = evaluateIncrease(baseInput());
    const mark = 100n * PRECISION;
    expect(q.liquidation_price).toBe(mark - mark / 10n);
  });

  it("short: liq = mark + mark * collateral / size", () => {
    const q = evaluateIncrease(
      baseInput({ intent: { collateral: 1_000n * PRECISION, size: 10_000n * PRECISION, is_long: false } }),
    );
    const mark = 100n * PRECISION;
    expect(q.liquidation_price).toBe(mark + mark / 10n);
  });

  it("non-zero liq_threshold buys the trader more headroom (further liq price)", () => {
    const noThresh = evaluateIncrease(baseInput());
    const withThresh = evaluateIncrease(
      baseInput({ protocol_limits: { max_utilization_ratio_bps: 8000n, liquidation_threshold_bps: 500n } }),
    );
    // For longs: higher threshold → smaller loss_buffer → smaller adjustment → liq closer to mark (higher)
    expect(withThresh.liquidation_price! > noThresh.liquidation_price!).toBe(true);
  });
});

describe("liquidationPriceAtOpen", () => {
  const ENTRY = 100n * PRECISION;

  it("returns null on degenerate inputs", () => {
    expect(liquidationPriceAtOpen(ENTRY, 1n, 0n, true, 0n)).toBeNull();
    expect(liquidationPriceAtOpen(ENTRY, 0n, 1n, true, 0n)).toBeNull();
    expect(liquidationPriceAtOpen(0n, 1n, 1n, true, 0n)).toBeNull();
  });

  it("long with threshold=0: liq = entry × (1 − collateral/size) — 10× → entry × 0.9", () => {
    const collateral = 10n * PRECISION;
    const size = 100n * PRECISION;
    expect(liquidationPriceAtOpen(ENTRY, collateral, size, true, 0n)).toBe(90n * PRECISION);
  });

  it("short with threshold=0: liq = entry × (1 + collateral/size) — 10× → entry × 1.1", () => {
    const collateral = 10n * PRECISION;
    const size = 100n * PRECISION;
    expect(liquidationPriceAtOpen(ENTRY, collateral, size, false, 0n)).toBe(110n * PRECISION);
  });

  it("2× long with threshold=0 liquidates at half entry", () => {
    const collateral = 50n * PRECISION;
    const size = 100n * PRECISION;
    expect(liquidationPriceAtOpen(ENTRY, collateral, size, true, 0n)).toBe(50n * PRECISION);
  });

  it("non-zero threshold buys headroom (long liq is higher than at threshold=0)", () => {
    const collateral = 10n * PRECISION;
    const size = 100n * PRECISION;
    const at0 = liquidationPriceAtOpen(ENTRY, collateral, size, true, 0n)!;
    const at500 = liquidationPriceAtOpen(ENTRY, collateral, size, true, 500n)!;
    expect(at500 > at0).toBe(true);
  });
});

describe("evaluateIncrease — acceptable price (slippage cap)", () => {
  it("returns 0 when slippage_bps is 0 (opt out)", () => {
    const q = evaluateIncrease(baseInput());
    expect(q.acceptable_price).toBe(0n);
  });

  it("long: acceptable = mark * (1 + slippage)", () => {
    const q = evaluateIncrease(baseInput({ intent: { collateral: 1n, size: 1n, is_long: true, slippage_bps: 100n } }));
    const mark = 100n * PRECISION;
    expect(q.acceptable_price).toBe(mark + (mark * 100n) / BPS);
  });

  it("short: acceptable = mark * (1 - slippage)", () => {
    const q = evaluateIncrease(baseInput({ intent: { collateral: 1n, size: 1n, is_long: false, slippage_bps: 100n } }));
    const mark = 100n * PRECISION;
    expect(q.acceptable_price).toBe(mark - (mark * 100n) / BPS);
  });
});

describe("evaluateIncrease — liquidity headroom", () => {
  it("headroom = safe_basis * max_util_ratio / BPS - reserved", () => {
    // safe_basis = 1M, max_util = 80% → max_reserved = 800k; reserved = 100k → headroom = 700k
    const q = evaluateIncrease(baseInput());
    expect(q.liquidity_headroom).toBe(700_000n * PRECISION);
    expect(q.exceeds_liquidity).toBe(false);
  });

  it("exceeds_liquidity true when staged size > headroom", () => {
    const q = evaluateIncrease(
      baseInput({ intent: { collateral: 100_000n * PRECISION, size: 800_000n * PRECISION, is_long: true } }),
    );
    expect(q.exceeds_liquidity).toBe(true);
  });

  it("headroom is zero when reserved already exceeds cap", () => {
    const q = evaluateIncrease(
      baseInput({
        vault: {
          reserved_usdc: 900_000n * PRECISION,
          total_assets: 1_000_000n * PRECISION,
          unclaimed_fees: 0n,
        },
      }),
    );
    expect(q.liquidity_headroom).toBe(0n);
  });
});
