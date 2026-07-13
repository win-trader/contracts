import { describe, expect, it } from "bun:test";
import {
  toBigInt,
  toCarryingFeeConfig,
  toMarketState,
  toPositionState,
} from "../src/index.js";

describe("row coercion", () => {
  it("normalizes nullable numeric values", () => {
    expect(toBigInt(undefined)).toBe(0n);
    expect(toBigInt("42")).toBe(42n);
  });

  it("coerces the exposure market shape", () => {
    expect(toMarketState({
      acc_borrow_index: "1",
      acc_long_skew_index: "2",
      acc_short_skew_index: "3",
      last_index_update: "4",
      long_open_interest: "5",
      short_open_interest: "6",
    })).toEqual({
      acc_borrow_index: 1n,
      acc_long_skew_index: 2n,
      acc_short_skew_index: 3n,
      last_index_update: 4n,
      long_open_interest: 5n,
      short_open_interest: 6n,
    });
  });

  it("coerces exposure and fee debts", () => {
    const state = toPositionState({
      is_long: true,
      size: "10",
      collateral: "2",
      entry_price: "5",
      base_exposure: "20",
      borrow_fee_debt: "3",
      skew_fee_debt: "4",
    });
    expect(state.base_exposure).toBe(20n);
    expect(state.borrow_fee_debt).toBe(3n);
    expect(state.skew_fee_debt).toBe(4n);
  });

  it("coerces carrying-fee configuration", () => {
    expect(toCarryingFeeConfig({
      base_borrow_rate_bps: "100",
      slope1_bps: "500",
      slope2_bps: "5000",
      optimal_utilization_bps: "8000",
      max_skew_rate_bps: "3000",
    }).max_skew_rate_bps).toBe(3000n);
  });
});
