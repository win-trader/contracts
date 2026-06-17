import { describe, it, expect } from "bun:test";
import {
  toBigInt,
  toMarketState,
  toPositionState,
  toVaultLiquidity,
  toBorrowRateConfig,
} from "../src/coerce.js";

describe("toBigInt", () => {
  it("treats null, undefined, and empty string as zero", () => {
    expect(toBigInt(null)).toBe(0n);
    expect(toBigInt(undefined)).toBe(0n);
    expect(toBigInt("")).toBe(0n);
  });

  it("parses numeric strings", () => {
    expect(toBigInt("123")).toBe(123n);
    expect(toBigInt("-9876543210")).toBe(-9876543210n);
  });
});

describe("toMarketState", () => {
  it("coerces strings to bigints", () => {
    const state = toMarketState({
      acc_borrow_index: "100",
      acc_funding_index: "200",
      last_index_update: "1700000000",
      long_open_interest: "5000",
      short_open_interest: "3000",
    });
    expect(state.acc_borrow_index).toBe(100n);
    expect(state.acc_funding_index).toBe(200n);
    expect(state.last_index_update).toBe(1700000000n);
    expect(state.long_open_interest).toBe(5000n);
    expect(state.short_open_interest).toBe(3000n);
  });

  it("defaults null columns to zero", () => {
    const state = toMarketState({
      acc_borrow_index: null,
      acc_funding_index: null,
      last_index_update: null,
      long_open_interest: null,
      short_open_interest: null,
    });
    expect(state.acc_borrow_index).toBe(0n);
    expect(state.long_open_interest).toBe(0n);
  });
});

describe("toPositionState", () => {
  it("preserves is_long booleans verbatim", () => {
    const long = toPositionState({
      is_long: true,
      size: "1",
      collateral: "2",
      entry_price: "3",
      entry_borrow_index: "4",
      entry_funding_index: "5",
    });
    expect(long.is_long).toBe(true);

    const short = toPositionState({
      is_long: false,
      size: null,
      collateral: null,
      entry_price: null,
      entry_borrow_index: null,
      entry_funding_index: null,
    });
    expect(short.is_long).toBe(false);
    expect(short.size).toBe(0n);
  });
});

describe("toVaultLiquidity", () => {
  it("handles undefined row by defaulting all fields to zero", () => {
    const v = toVaultLiquidity(undefined);
    expect(v.reserved_usdc).toBe(0n);
    expect(v.total_assets).toBe(0n);
  });

  it("coerces present fields", () => {
    const v = toVaultLiquidity({
      reserved_usdc: "1000",
      total_assets: "10000",
    });
    expect(v.reserved_usdc).toBe(1000n);
    expect(v.total_assets).toBe(10000n);
  });
});

describe("toBorrowRateConfig", () => {
  it("handles undefined row by defaulting all fields to zero", () => {
    const c = toBorrowRateConfig(undefined);
    expect(c.base_borrow_rate_bps).toBe(0n);
    expect(c.slope1_bps).toBe(0n);
    expect(c.slope2_bps).toBe(0n);
    expect(c.optimal_utilization_bps).toBe(0n);
    expect(c.base_funding_rate_bps).toBe(0n);
  });

  it("coerces present fields", () => {
    const c = toBorrowRateConfig({
      base_borrow_rate_bps: "100",
      slope1_bps: "500",
      slope2_bps: "5000",
      optimal_utilization_bps: "8000",
      base_funding_rate_bps: "100",
    });
    expect(c.base_borrow_rate_bps).toBe(100n);
    expect(c.slope1_bps).toBe(500n);
    expect(c.slope2_bps).toBe(5000n);
    expect(c.optimal_utilization_bps).toBe(8000n);
    expect(c.base_funding_rate_bps).toBe(100n);
  });
});
