# ADR-0008: Exposure PnL and skew carrying fee

## Status

Accepted for testnet redeployment.

## Context

Arithmetic average entry prices do not preserve PnL for positions opened at
different prices because each position's PnL divides by its own entry price.
The former funding pool also recorded obligations before collection, creating
receiver claims that could exceed collectible payer collateral.

## Decision

- Store quote notional (`size`) and additive base exposure per position.
- Store the sums of those quantities per market side.
- Derive PnL from exposure; average entry price is display-only.
- Remove trader-to-trader funding and all funding-pool state.
- Add a time-accruing, utilization-weighted quadratic skew fee charged only to
  the dominant side.
- Route collected skew fees entirely to LP-owned assets.
- Use fee-debt accounting for borrow and skew indices.
- Redeploy on testnet with empty state instead of migrating live positions.

The complete formula and rounding specification is in
`docs/math/pnl-and-skew-fees.md`.

## Consequences

Market PnL is computed in constant time from additive quantities and can be
checked against position sums. There are no funding receivers or unbacked
funding claims. Position and market wire types change, so contracts, bindings,
indexers, keepers, and test fixtures must be updated and redeployed together.
