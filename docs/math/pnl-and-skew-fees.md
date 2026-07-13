# Exposure PnL and skew carrying fees

Status: implementation specification

This document defines the financial model used by the contracts. It is written
as a review surface for quantitative and security reviewers: every stored
quantity, unit, formula, rounding rule, and state transition is named here.

## 1. Units

- `size`, collateral, fees, and PnL use the collateral token's integer units.
- Oracle prices use `PRICE_DECIMALS = 7`.
- Base exposure uses `EXPOSURE_PRECISION = 10^18`.
- Borrow and skew fee indices use `INDEX_PRECISION = 10^14`.
- Rates and ratios use `BPS = 10_000`.
- Multiplication followed by division is evaluated with widened `I256`
  intermediates. Contract state remains `i128`.

## 2. Authoritative position state

`size` is the position's quote notional (and therefore its quote cost).
`base_exposure` is the corresponding base quantity at exposure precision:

```text
base_delta = floor(size_delta * EXPOSURE_PRECISION / execution_price)
```

The authoritative cost basis is not an average price. A display-only entry
price is stored/derived for clients:

```text
display_entry = floor(size * EXPOSURE_PRECISION / base_exposure)
```

The derived price is not fed back into settlement math.

## 3. Position PnL

```text
mark_value = floor(base_exposure * mark_price / EXPOSURE_PRECISION)
long_pnl  = mark_value - size
short_pnl = size - mark_value
```

This representation is additive. Market PnL is computed from the sums of the
same position quantities rather than from an arithmetic average entry price.

## 4. Market PnL

```text
long_pnl = floor(long_base_exposure * mark / EXPOSURE_PRECISION)
           - long_open_interest

short_pnl = short_open_interest
            - floor(short_base_exposure * mark / EXPOSURE_PRECISION)

market_pnl = long_pnl + short_pnl
```

The accounting invariant is:

```text
market.open_interest == sum(position.size)
market.base_exposure == sum(position.base_exposure)
```

Market valuation floors after summing exposure, while a diagnostic sum of
individual positions floors once per position. For `n` positions on one side,
the difference between those two mark-value calculations is in `[0, n - 1]`
collateral-token atoms. Across both sides, the absolute PnL comparison is
therefore bounded by `(n_long - 1) + (n_short - 1)` atoms when both sides are
non-empty. This is valuation dust, not lost exposure; full closes consume the
exact stored exposure remainder.

## 5. Partial close

```text
closed_base = floor(position.base_exposure * size_delta / position.size)
```

PnL is calculated from `closed_base` and `size_delta`. The exact same
`closed_base` is removed from the position and market. A full close consumes
the complete stored remainder, preventing permanent rounding dust.

## 6. Fee-debt accounting

Positions store borrow and skew fee debt rather than averaged entry indices.
For fee index `I`:

```text
accrued_fee = max(floor(size * I / INDEX_PRECISION) - fee_debt, 0)
```

On increase by `d` at the current index:

```text
fee_debt += floor(d * I / INDEX_PRECISION)
```

Partial closes remove debt proportionally; full closes consume the exact
remainder.

## 7. Skew carrying fee

There is no trader-to-trader funding rate, funding index, funding pool, or
funding receivable. Instead, the dominant side pays LPs for the time during
which LP capital carries directional exposure.

At each checkpoint, using the state that existed during the elapsed interval:

```text
total_oi = long_oi + short_oi
skew = abs(long_oi - short_oi)
concentration_bps = floor(skew * BPS / total_oi)
utilization_bps = floor(reserved_usdc * BPS / vault_safe_basis)

quadratic_bps = floor(concentration_bps * concentration_bps / BPS)
concentrated_rate = floor(max_skew_rate_bps * quadratic_bps / BPS)
skew_rate_bps = floor(concentrated_rate * min(utilization_bps, BPS) / BPS)
```

Only the dominant side's cumulative skew index advances. Balanced OI produces
zero. Utilization scales the charge so a tiny one-sided market does not pay as
if it consumed the entire Vault. The minority side receives no payment and no
claim against LP assets.

### Calibration examples

With `max_skew_rate_bps = 5_000` (50% APR):

| Long / short OI | Concentration | Utilization | Dominant-side APR |
|---:|---:|---:|---:|
| 50 / 50 | 0% | any | 0% |
| 60 / 40 | 20% | 50% | 1% |
| 75 / 25 | 50% | 80% | 10% |
| 100 / 0 | 100% | 50% | 25% |
| 100 / 0 | 100% | 100% | 50% |

These are ideal real-number values before integer flooring. The quadratic term
makes mild imbalance cheap while making concentrated directional exposure
progressively more expensive. The utilization term ties revenue to the amount
of LP capacity actually at risk.

## 8. Checkpoint ordering

Every OI-changing operation follows this order:

1. Load the old market and Vault risk basis.
2. Accrue borrow and skew indices for the elapsed interval using old OI.
3. Persist the checkpoint timestamp.
4. Evaluate the position against refreshed indices and the current mark.
5. Apply position and market exposure deltas.

This prevents a transaction from retroactively changing who paid for an
earlier interval. A keeper may checkpoint quiet markets, but the next trade or
close performs the same catch-up, so correctness does not depend on a keeper.

## 9. Settlement and LP revenue

```text
position_health = collateral + price_pnl - borrow_fee - skew_fee
trader_payout = max(position_health, 0)
```

Collected skew fees belong entirely to LPs because they compensate directional
risk. They are not included in the dev/staker revenue split. A close computes
the fee, reduces the trader's payout, and only then affects LP equity. Accrued
but uncollectible skew fees are not booked as Vault assets or receivables.

Fee collection priority for a closed slice is price PnL, borrow fee, then skew
fee within the slice's realizable equity. This affects fee attribution only;
the trader payout remains `max(position_health, 0)`.

## 10. Review invariants

1. Aggregate exposure equals the sum of position exposure after every action.
2. Market PnL equals summed position PnL within documented valuation dust.
3. Operation ordering does not change final exposure or PnL.
4. Splitting time into checkpoints changes fees only by bounded integer dust.
5. Balanced markets accrue no skew fee.
6. Only the side dominant during an interval accrues a skew fee.
7. No fee expectation increases LP NAV before tokens are collected.
8. A full close returns exposure and fee-debt counters to zero.

## 11. Explicit non-goals and residual risks

- The fee is a risk charge, not a mechanism that forces OI back to balance.
- `max_skew_rate_bps` is governance-controlled within a hard 200% APR ceiling;
  parameter changes affect future intervals after the next checkpoint.
- Oracle correctness, Vault solvency, ADL selection, and collectible bad debt
  remain separate risk controls. Exposure accounting makes PnL additive but
  does not make an insolvent trader's losses collectible.
- Index and exposure arithmetic floors toward zero at the documented stages.
  Reviewers should test long horizons, frequent checkpoints, and many partial
  closes for economically material cumulative dust.
