// IncreaseQuote — staged-order twin of PositionEvaluation.
//
// Derived values for a hypothetical Increase against a MarketTick: the
// trader's open fee, daily borrow / skew accruals at the current rates,
// the liquidation price for the brand-new Position (no fees accrued yet),
// the acceptable-price slippage cap, and the vault-side liquidity headroom.
//
// All arithmetic mirrors the contract's pre-trade snapshot (open-fee bps,
// kink borrow rate, utilization-weighted quadratic skew, safe_basis-based util,
// liquidation_threshold-aware liq price) so the UI preview matches what the
// contract would charge if the tx fired right now.

import { BPS, SECONDS_PER_YEAR } from "./constants.js";
import { calcBorrowRate, calcSkewRate, calcUtilizationBps } from "./pure.js";
import type { MarketTick } from "./tick.js";
import type { CarryingFeeConfig } from "./types.js";

const SECONDS_PER_DAY = 86_400n;

export interface IncreaseIntent {
  collateral: bigint;
  size: bigint;
  is_long: boolean;
  /** Slippage tolerance in bps for the contract's `acceptable_price` cap.
   *  0n disables the cap (matches the on-chain "opt out" convention). */
  slippage_bps?: bigint;
}

export interface IncreaseQuoteInput {
  intent: IncreaseIntent;
  tick: MarketTick;
  fee_config: { open_fee_bps: bigint };
  /** Vault snapshot at quote time. `safe_basis = total_assets - unclaimed_fees`
   *  matches the contract's `VaultView.safe_basis`, the basis the on-chain
   *  utilization and liquidity-cap checks use. */
  vault: {
    reserved_usdc: bigint;
    total_assets: bigint;
    unclaimed_fees: bigint;
  };
  protocol_limits: {
    max_utilization_ratio_bps: bigint;
    liquidation_threshold_bps: bigint;
  };
  rate_config: CarryingFeeConfig;
}

export interface IncreaseQuote {
  /** Trader-paid open fee: `size * open_fee_bps / BPS`. */
  open_fee: bigint;
  /** Daily borrow accrual on the new size at the pre-trade kink rate. */
  daily_borrow: bigint;
  /** Daily surcharge if this position would be on the post-trade dominant side. */
  daily_skew: bigint;
  /** Liquidation price at t=0 (no fees accrued). Honors
   *  `liquidation_threshold_bps`. `null` when inputs are degenerate
   *  (collateral or size zero, mark unavailable). */
  liquidation_price: bigint | null;
  /** Worst-case fill price the trader is willing to accept. `0n` means
   *  "no cap" — the contract treats 0 as opt-out. */
  acceptable_price: bigint;
  /** Remaining notional the vault will reserve before
   *  `max_utilization_ratio_bps` would be exceeded. */
  liquidity_headroom: bigint;
  /** `true` when the staged size would push reserved past the cap. */
  exceeds_liquidity: boolean;
}

/**
 * Liquidation price for a freshly-opened position before any fees accrue.
 * Inverts the on-chain liquidation gate
 *   `effective_health < collateral * liquidation_threshold_bps / BPS`
 * under the assumption that borrow and skew fees are still zero, so
 *   collateral + pnl == collateral * liquidation_threshold_bps / BPS
 * ⇒ pnl == -(BPS - liquidation_threshold_bps) * collateral / BPS  (`-loss_buffer`)
 *   pnl_long  = size * (liq - mark) / mark  ⇒ liq = mark - mark * loss_buffer / size
 *   pnl_short = size * (mark - liq) / mark  ⇒ liq = mark + mark * loss_buffer / size
 *
 * Returns `null` for degenerate inputs (zero size, collateral, or mark).
 *
 * For an existing position with accrued indices, use
 * {@link liquidationPriceForPosition} in `eval.ts` instead — that variant
 * accounts for the position's current borrow and skew fees.
 */
export function liquidationPriceAtOpen(
  mark: bigint,
  collateral: bigint,
  size: bigint,
  is_long: boolean,
  liquidation_threshold_bps: bigint,
): bigint | null {
  if (collateral <= 0n || size <= 0n || mark <= 0n) return null;
  const loss_buffer = (collateral * (BPS - liquidation_threshold_bps)) / BPS;
  const adjustment = (mark * loss_buffer) / size;
  return is_long ? mark - adjustment : mark + adjustment;
}

export function evaluateIncrease(input: IncreaseQuoteInput): IncreaseQuote {
  const { intent, tick, fee_config, vault, protocol_limits, rate_config } = input;
  const { collateral, size, is_long } = intent;
  const slippage_bps = intent.slippage_bps ?? 0n;
  const mark = tick.mark_price;

  const open_fee = size > 0n ? (size * fee_config.open_fee_bps) / BPS : 0n;

  const safe_basis =
    vault.total_assets > vault.unclaimed_fees
      ? vault.total_assets - vault.unclaimed_fees
      : 0n;

  const util_bps =
    safe_basis > 0n ? calcUtilizationBps(vault.reserved_usdc, safe_basis) : 0n;
  const borrow_rate = calcBorrowRate(
    util_bps,
    rate_config.base_borrow_rate_bps,
    rate_config.slope1_bps,
    rate_config.slope2_bps,
    rate_config.optimal_utilization_bps,
  );
  const daily_borrow =
    size > 0n ? (size * borrow_rate * SECONDS_PER_DAY) / (BPS * SECONDS_PER_YEAR) : 0n;

  // Opening itself is not charged. This projects the next day's carrying cost
  // using post-trade OI, which is the state future checkpoints observe.
  const post_long_oi = tick.market.long_open_interest + (is_long ? size : 0n);
  const post_short_oi = tick.market.short_open_interest + (is_long ? 0n : size);
  const skew_rate = calcSkewRate(
    post_long_oi,
    post_short_oi,
    util_bps,
    rate_config.max_skew_rate_bps,
  );
  const is_dominant = is_long ? post_long_oi > post_short_oi : post_short_oi > post_long_oi;
  const daily_skew =
    size > 0n && is_dominant
      ? (size * skew_rate * SECONDS_PER_DAY) / (BPS * SECONDS_PER_YEAR)
      : 0n;

  const liquidation_price = liquidationPriceAtOpen(
    mark,
    collateral,
    size,
    is_long,
    protocol_limits.liquidation_threshold_bps,
  );

  const acceptable_price =
    slippage_bps > 0n && mark > 0n
      ? is_long
        ? mark + (mark * slippage_bps) / BPS
        : mark - (mark * slippage_bps) / BPS
      : 0n;

  const max_reserved =
    (safe_basis * protocol_limits.max_utilization_ratio_bps) / BPS;
  const liquidity_headroom =
    max_reserved > vault.reserved_usdc ? max_reserved - vault.reserved_usdc : 0n;
  const exceeds_liquidity = size > 0n && size > liquidity_headroom;

  return {
    open_fee,
    daily_borrow,
    daily_skew,
    liquidation_price,
    acceptable_price,
    liquidity_headroom,
    exceeds_liquidity,
  };
}
