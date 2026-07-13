// Off-chain MarketTick — parallel to contracts/position-manager/src/tick.rs.
//
// On-chain `MarketTick::refresh` is side-effecting: it updates indices in
// storage, pushes Unrealized PnL to the Vault, emits UpdateIndices. This TS
// version is read-only — it derives a tick from cached state by projecting
// indices forward to `now` using the same accumulation formulas. The result
// matches what an immediate on-chain refresh would produce (modulo the
// inevitable skew between `now` here and ledger time on-chain).

import {
  calcUnrealizedPnl,
  calcFeeFromDebt,
  calcHealth,
  calcUtilizationBps,
  calcBorrowRate,
  calcSkewRate,
  accumulateFeeIndex,
  isTpTriggered,
  isSlTriggered,
} from "./pure.js";
import type {
  MarketState,
  PositionEvaluation,
  PositionState,
  ProjectInput,
  Slice,
} from "./types.js";

export class MarketTick {
  constructor(
    /** Market state with all carrying-fee indices projected to tick time. */
    public readonly market: MarketState,
    public readonly mark_price: bigint,
  ) {}

  /**
   * Project a MarketTick from cached state forward to `now`. Mirrors the
   * index-update arm of the contract's `MarketTick::refresh`, including the
   * pause-fee clamp `effective_start = max(last_index_update, last_unpause_time)`.
   */
  static project(input: ProjectInput): MarketTick {
    const { market, mark_price, vault, rate_config, now, last_unpause_time } = input;

    const effective_start =
      market.last_index_update > last_unpause_time
        ? market.last_index_update
        : last_unpause_time;
    const time_delta = now > effective_start ? now - effective_start : 0n;

    let acc_borrow_index = market.acc_borrow_index;
    let acc_long_skew_index = market.acc_long_skew_index;
    let acc_short_skew_index = market.acc_short_skew_index;

    if (time_delta > 0n) {
      const util_bps = calcUtilizationBps(vault.reserved_usdc, vault.total_assets);
      const borrow_rate = calcBorrowRate(
        util_bps,
        rate_config.base_borrow_rate_bps,
        rate_config.slope1_bps,
        rate_config.slope2_bps,
        rate_config.optimal_utilization_bps,
      );
      acc_borrow_index = accumulateFeeIndex(acc_borrow_index, borrow_rate, time_delta);

      const skew_rate = calcSkewRate(
        market.long_open_interest,
        market.short_open_interest,
        util_bps,
        rate_config.max_skew_rate_bps,
      );
      if (market.long_open_interest > market.short_open_interest) {
        acc_long_skew_index = accumulateFeeIndex(acc_long_skew_index, skew_rate, time_delta);
      } else if (market.short_open_interest > market.long_open_interest) {
        acc_short_skew_index = accumulateFeeIndex(acc_short_skew_index, skew_rate, time_delta);
      }
    }

    const projected: MarketState = {
      ...market,
      acc_borrow_index,
      acc_long_skew_index,
      acc_short_skew_index,
      last_index_update: now,
    };
    return new MarketTick(projected, mark_price);
  }

  /**
   * Mirrors `MarketTick::evaluate` in `contracts/position-manager/src/tick.rs`.
   * Uses additive exposure and fee-debt baselines, matching on-chain rounding.
   */
  evaluate(pos: PositionState, slice?: Slice): PositionEvaluation {
    const size = slice?.size ?? pos.size;
    const collateral = slice?.collateral ?? pos.collateral;
    const base_exposure = slice?.base_exposure ?? pos.base_exposure;
    const borrow_fee_debt = slice?.borrow_fee_debt ?? pos.borrow_fee_debt;
    const skew_fee_debt = slice?.skew_fee_debt ?? pos.skew_fee_debt;

    const pnl = calcUnrealizedPnl(size, base_exposure, this.mark_price, pos.is_long);
    const borrow_fee = calcFeeFromDebt(
      size,
      this.market.acc_borrow_index,
      borrow_fee_debt,
    );
    const skew_index = pos.is_long
      ? this.market.acc_long_skew_index
      : this.market.acc_short_skew_index;
    const skew_fee = calcFeeFromDebt(
      size,
      skew_index,
      skew_fee_debt,
    );
    const effective_health = calcHealth(collateral, pnl, borrow_fee, skew_fee);

    return {
      pnl,
      borrow_fee,
      skew_fee,
      effective_health,
    };
  }

  isTpTriggered(take_profit: bigint, is_long: boolean): boolean {
    return isTpTriggered(take_profit, this.mark_price, is_long);
  }

  isSlTriggered(stop_loss: bigint, is_long: boolean): boolean {
    return isSlTriggered(stop_loss, this.mark_price, is_long);
  }
}
