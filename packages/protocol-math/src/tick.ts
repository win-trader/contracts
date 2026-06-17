// Off-chain MarketTick — parallel to contracts/position-manager/src/tick.rs.
//
// On-chain `MarketTick::refresh` is side-effecting: it updates indices in
// storage, pushes Unrealized PnL to the Vault, emits UpdateIndices. This TS
// version is read-only — it derives a tick from cached state by projecting
// indices forward to `now` using the same accumulation formulas. The result
// matches what an immediate on-chain refresh would produce (modulo the
// inevitable skew between `now` here and ledger time on-chain).

import { BPS } from "./constants.js";
import {
  calcUnrealizedPnl,
  calcBorrowFee,
  calcFundingFee,
  calcHealth,
  calcUtilizationBps,
  calcBorrowRate,
  calcFundingRate,
  accumulateBorrowIndex,
  accumulateFundingIndex,
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
    /** Market state with `acc_borrow_index` and `acc_funding_index` projected to tick time. */
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
    let acc_funding_index = market.acc_funding_index;

    if (time_delta > 0n) {
      const util_bps = calcUtilizationBps(vault.reserved_usdc, vault.total_assets);
      const borrow_rate = calcBorrowRate(
        util_bps,
        rate_config.base_borrow_rate_bps,
        rate_config.slope1_bps,
        rate_config.slope2_bps,
        rate_config.optimal_utilization_bps,
      );
      acc_borrow_index = accumulateBorrowIndex(acc_borrow_index, borrow_rate, time_delta);

      const funding_rate = calcFundingRate(
        market.long_open_interest,
        market.short_open_interest,
        rate_config.base_funding_rate_bps,
      );
      acc_funding_index = accumulateFundingIndex(acc_funding_index, funding_rate, time_delta);
    }

    const projected: MarketState = {
      ...market,
      acc_borrow_index,
      acc_funding_index,
      last_index_update: now,
    };
    return new MarketTick(projected, mark_price);
  }

  /**
   * Mirrors `MarketTick::evaluate` in `contracts/position-manager/src/tick.rs`.
   * `funding_cut_bps` defaults to 0n; callers without access to the protocol
   * config still get correct raw fields and a zero-sum-scaled
   * `effective_funding`, but `funding_protocol_cut` and `effective_health`
   * will under-cut a positive-funding receiver.
   */
  evaluate(pos: PositionState, slice?: Slice, funding_cut_bps: bigint = 0n): PositionEvaluation {
    const size = slice?.size ?? pos.size;
    const collateral = slice?.collateral ?? pos.collateral;

    const pnl = calcUnrealizedPnl(size, pos.entry_price, this.mark_price, pos.is_long);
    const borrow_fee = calcBorrowFee(
      size,
      pos.entry_borrow_index,
      this.market.acc_borrow_index,
    );
    const funding_fee = calcFundingFee(
      size,
      pos.entry_funding_index,
      this.market.acc_funding_index,
      pos.is_long,
    );

    // Zero-sum cap: receivers (funding_fee > 0) are scaled by
    // payer_oi / receiver_oi so total received never exceeds total paid.
    let zero_sum_funding = funding_fee;
    if (funding_fee > 0n) {
      const payer_oi = pos.is_long
        ? this.market.short_open_interest
        : this.market.long_open_interest;
      const receiver_oi = pos.is_long
        ? this.market.long_open_interest
        : this.market.short_open_interest;
      if (receiver_oi <= 0n) {
        zero_sum_funding = 0n;
      } else if (payer_oi < receiver_oi) {
        zero_sum_funding = (funding_fee * payer_oi) / receiver_oi;
      }
    }

    const funding_protocol_cut =
      zero_sum_funding > 0n ? (zero_sum_funding * funding_cut_bps) / BPS : 0n;
    const effective_funding = zero_sum_funding - funding_protocol_cut;

    const health = calcHealth(collateral, pnl, borrow_fee, funding_fee);
    const effective_health = calcHealth(collateral, pnl, borrow_fee, effective_funding);

    return {
      pnl,
      borrow_fee,
      funding_fee,
      effective_funding,
      funding_protocol_cut,
      health,
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
