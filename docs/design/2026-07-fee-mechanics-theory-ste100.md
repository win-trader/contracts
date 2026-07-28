# Fee Mechanics: Economic Theory

## 1. Purpose

This document uses ASD-STE100 Simplified Technical English.

This document explains the economic rules for a perpetual market.
It does not specify contract code.

A perpetual market gives leveraged price exposure to a trader.
A vault supplies cash to profitable traders.
A liquidity provider owns the residual value of the vault.

The fee system has four functions:

1. It charges a fee when a trader opens exposure.
2. It charges a fee while a position uses vault capacity.
3. It charges the dominant market side for directional imbalance.
4. It keeps safety rewards separate from LP cash.

The system uses one physical vault balance.
The system uses cumulative indices for time-based fees.
The system uses aggregate market state.
The system does not use a separate cash pool for each claim.
The system does not iterate through positions.

## 2. Technical terms

The following terms have one meaning in this document.

| Term | Meaning |
|---|---|
| base exposure | The quantity of the underlying asset in a position. |
| cash LP equity | The cash that belongs to liquidity providers before unrealized price PnL. |
| checkpoint | An update that accrues fees to a specified time. |
| claim | An accounting label that identifies the owner of vault cash. |
| clean terminal state | A state with no position, risk unit, receiver claim, or execution budget. |
| dominant side | The side with more base exposure. |
| effective collateral | Stored collateral after all accrued fees and funding credits. |
| light side | The side with less base exposure. |
| LP | A liquidity provider. |
| marked vault NAV | LP value after recognized unrealized trader PnL. |
| position size | The USD notional value at entry. |
| risk unit | A fixed measure of gross capacity that a position uses. |
| side | The long side or the short side of a market. |
| skew | The normalized difference between long and short base exposure. |
| stored collateral | Position collateral recorded in contract state. |

`PnL` means profit and loss.
`NAV` means net asset value.
`BPS` means basis points.

The word `must` identifies a requirement.
The word `can` identifies an ability.

## 3. Basic quantities

Use these constants:

```text
BPS = 10,000
SECONDS_PER_DAY = 86,400
```

Store each configured rate as basis points per day.

Calculate a fee on a fee base `x` as follows:

```text
fee =
    x
    × rate_bps_day
    × elapsed_seconds
    / (BPS × SECONDS_PER_DAY)
```

Store two exposure quantities for each position:

```text
size
    USD notional value at entry

base_exposure
    quantity of the underlying asset in the position
```

Use position size as the funding fee base.
Position size does not change when the mark price changes.

Use base exposure as the directional quantity.
At one mark price, base exposure gives the current directional exposure.

Store these aggregates for each market side:

```text
size_open_interest
base_exposure
stored_collateral_total
```

The aggregates must make market accounting independent of the position count.

## 4. Opening fee

Charge an opening fee for an open or increase action.
Do not charge a position fee for a decrease or close action.

Use two opening-fee tiers.
Use the low tier when an action improves or preserves the balance.
Use the high tier when an action makes the balance worse.

Calculate skew as follows:

```text
skew(long_base, short_base) =
    if long_base + short_base = 0:
        0
    else:
        |long_base - short_base|
        × BPS
        / (long_base + short_base)
```

Select the fee tier as follows:

```text
if skew_after <= skew_before:
    fee_bps = open_fee_low_bps
else:
    fee_bps = open_fee_high_bps

opening_fee = ceil(size_added × fee_bps / BPS)
```

Use base exposure for the skew comparison.
Do not use entry-time USD open interest for the comparison.

The two-tier boundary is intentionally discrete.
A continuous curve can give more precision.
A continuous curve also adds more rules and makes quotes path-dependent.
The two-tier rule is the selected simple policy.

## 5. Funding

### 5.1 Purpose

Funding sets a price for directional imbalance.

Select the payer side as follows:

```text
if long_base_exposure > short_base_exposure:
    longs pay

if short_base_exposure > long_base_exposure:
    shorts pay

if long_base_exposure = short_base_exposure:
    no side pays
```

Calculate normalized skew as follows:

```text
skew_bps =
    |long_base_exposure - short_base_exposure|
    × BPS
    / (long_base_exposure + short_base_exposure)
```

Skew is zero for a balanced market.
Skew is `BPS` for a one-sided market.

### 5.2 Funding rate

Calculate the dominant-side rate as follows:

```text
payer_rate_bps_day =
    max_funding_rate_bps_day
    × skew_bps²
    / BPS²
```

The quadratic curve has three effects:

- A small imbalance has a small fee.
- The fee increases faster when the imbalance increases.
- A one-sided market reaches the configured maximum rate.

Charge each existing position on the dominant side at the current rate.
Apply a new rate only after the action that changes the imbalance.

### 5.3 Funding flow

Light-side traders offset part of the dominant exposure.
LPs offset the unmatched part.
Divide the payer flow in proportion to this counter-exposure.

Calculate the complete payer flow as follows:

```text
payer_flow_per_day =
    dominant_size_open_interest
    × payer_rate_bps_day
    / BPS
```

Calculate the light-side share as follows:

```text
counterparty_share =
    light_base_exposure
    / dominant_base_exposure
```

Calculate the receiver flows as follows:

```text
trader_receiver_flow_per_day =
    payer_flow_per_day × counterparty_share

lp_funding_flow_per_day =
    payer_flow_per_day - trader_receiver_flow_per_day
```

The `counterparty_share` value must be from zero through one.

Apply these results:

- If the light side is zero, LPs receive all collected funding.
- If the light side is small, light-side traders receive a small share.
- If the market approaches balance, the trader share increases.
- If the market approaches balance, the payer rate approaches zero.
- A dust position cannot redirect all funding from LPs.

Derive the light-side receiver rate from the allocated flow:

```text
receiver_rate =
    trader_receiver_flow
    / light_size_open_interest
```

The receiver rate can differ from the payer rate.
Different entry prices can cause this difference.
The calculations must conserve the complete flow.
The per-position rates do not have to be equal.

### 5.4 Funding recognition

Record receiver funding as a vault-backed claim when the funding accrues.
Do not wait for a payer to settle.

Do not include uncollected payer funding in LP cash.
Collect payer funding from position collateral or from an external payment.

Collected receiver-backed funding restores the LP cash that backs the receiver claim.
Collected LP-backed funding is LP revenue.

This rule gives these results:

- The receiver gets the amount that the receiver index specifies.
- The vault does not record an uncollected payer fee as cash.
- LPs absorb a shortfall if a payer becomes insolvent.

Include guaranteed receiver funding in non-LP claims when it accrues.

## 6. Borrow fee

### 6.1 Purpose

The borrow fee prices gross settlement and liquidation capacity.
Funding prices net directional imbalance.
The borrow fee must not replace the funding fee.

Calculate new risk units as follows:

```text
risk_units_added =
    floor(
        size_added
        × market_risk_factor_bps
        / BPS
    )
```

Apply these rules to risk units:

- Use risk units as the borrow fee base.
- Add risk units only when a position opens risk.
- Remove risk units in proportion to closed risk.
- Do not change risk units only because the oracle price changes.
- Use risk units to lock vault capacity.
- Do not treat risk units as a cash expense.
- Do not treat risk units as a profit claim.

Use gross exposure for risk units.
Balanced gross positions still cause settlement and liquidation work.
Balanced gross positions can also cause collateral loss.

### 6.2 Utilization

Use one capacity domain for all markets in the vault.

Calculate utilization as follows:

```text
utilization_bps =
    min(
        total_risk_units × BPS / cash_lp_equity,
        BPS
    )
```

Calculate the borrow rate as follows:

```text
borrow_rate_bps_day =
    base_borrow_rate_bps_day
    + max_variable_borrow_rate_bps_day
      × utilization_bps²
      / BPS²
```

A low utilization produces a low rate.
A high utilization produces a higher rate.

Apply this capacity gate to new risk:

```text
total_risk_units_after
    <= cash_lp_equity_after
       × risk_capacity_limit_bps
       / BPS
```

The utilization cap does not permit excess risk.
Reject an action that fails the capacity gate.

Apply hard size and base-exposure limits to each market side.
The limits bound exposure without an onchain volatility estimator.
The limits also avoid an onchain correlation model.

## 7. Time-based accrual

Use cumulative indices for time-based fees.
An index is the cumulative fee for one unit of the fee base.

For a rate that applies during `dt`, calculate the index change:

```text
index_delta =
    rate_bps_day
    × INDEX_PRECISION
    × dt
    / (BPS × SECONDS_PER_DAY)
```

Store the accounted index amount as a debt baseline.

Calculate new accrual as follows:

```text
accrued =
    current_index_value_for_position
    - stored_debt_baseline
```

Store these debt baselines for each position:

```text
borrow_debt
funding_paid_to_receivers_debt
funding_paid_to_lps_debt
funding_received_debt
```

Use separate payer indices for receiver-backed flow and LP-backed flow.
The two flows have different accounting treatment.

Store these system indices and totals:

- One global borrow index.
- Per-market funding indices.
- One global receiver-flow rate.
- One global pending receiver-funding claim.

Use the global receiver-flow rate to accrue claims without a market loop.

## 8. Checkpoint order

A rate is constant between two checkpoints.
A state change must not change a rate for past time.

Use this procedure for each applicable action:

1. Accrue global indices with the old rates.
2. Accrue guaranteed claims with the old rates.
3. Accrue the affected market indices with the old flows.
4. Capitalize the affected position fees.
5. Apply the requested mutation.
6. Calculate the new market funding flows.
7. Update the global receiver-flow sum.
8. Calculate the new global borrow rate.

Skip a step if the action has no applicable market or position.

A second checkpoint at the same time must have no effect.

Checkpoint an affected index before a parameter change.
Apply the old parameter through the checkpoint time.
Apply the new parameter after the checkpoint time.

An operational pause must not stop fee accrual.
A close or settlement stops accrual for the removed position.

Keep the borrow rate constant between checkpoints.
A receiver claim can reduce cash LP equity during an interval.
Use the reduced equity at the next checkpoint.

A continuous quadratic repricing needs a time-dependent integral.
The design does not use that additional mechanism.

## 9. Position collateral and settlement

Calculate effective collateral before a position change:

```text
effective_collateral =
    stored_collateral
    + pending_funding_received
    - pending_funding_paid_to_receivers
    - pending_funding_paid_to_lps
    - pending_borrow
```

Include accrued fees in margin and liquidation calculations.

Use this procedure for an increase, decrease, close, or liquidation:

1. Checkpoint the applicable indices.
2. Calculate all pending fee amounts.
3. Apply the amounts to stored collateral.
4. Apply the amounts to their destination claims.
5. Reset each debt baseline to the current index value.
6. Apply the position-size change.

Capitalize all old accrual before a partial close.
The remaining position starts at the current indices.
This rule removes the need for historical pro-rata debt.

Collect each opening fee immediately.
Split each collected opening fee and borrow fee between these owners:

```text
LP-owned revenue
risk-keeper reserve
protocol claimable revenue
```

Give all collected LP-backed funding to LPs.

Move receiver funding between the guaranteed claim and position collateral.
Do not record the same receiver funding two times.

At a full settlement, calculate:

```text
close_equity =
    stored_collateral
    + pending_funding_received
    + payable_price_pnl
    - pending_funding_paid_to_receivers
    - pending_funding_paid_to_lps
    - pending_borrow

trader_payout = max(close_equity, 0)
bad_debt      = max(-close_equity, 0)
```

Distribute available value in this order:

1. Pay receiver-backed funding.
2. Pay negative price PnL to LPs.
3. Pay LP-backed funding.
4. Pay borrow.
5. Pay the remaining equity to the trader.

Positive price PnL adds value to the distribution.
For a liquidation, pay its reward after borrow.
Pay the liquidation reward before residual trader equity.

Do not charge a closing fee.

## 10. Vault ownership

Hold all collateral tokens in one vault balance.

Derive physical cash as follows:

```text
physical_cash = collateral_token.balanceOf(vault)
```

Store these non-LP claims:

```text
stored_position_collateral_total
pending_receiver_funding_total
execution_budget_total
protocol_claimable_total
risk_keeper_reserve_total
```

Calculate complete non-LP claims:

```text
non_lp_claims =
    stored_position_collateral_total
    + pending_receiver_funding_total
    + execution_budget_total
    + protocol_claimable_total
    + risk_keeper_reserve_total
```

Calculate cash LP equity:

```text
cash_lp_equity =
    max(physical_cash - non_lp_claims, 0)
```

Derive cash LP equity when the value is necessary.
Do not store cash LP equity as an authoritative counter.
An unexpected token transfer becomes LP cash.

Keep these quantities separate:

```text
cash_lp_equity
    LP cash before unrealized price PnL

vault_nav
    LP value after recognized unrealized trader PnL

free_lp_capital
    LP cash that is not required as risk backing
```

Calculate required risk backing and free capital:

```text
required_risk_backing =
    ceil(total_risk_units × BPS / risk_capacity_limit_bps)

free_lp_capital =
    max(cash_lp_equity - required_risk_backing, 0)
```

Risk backing is a withdrawal lock.
Do not subtract risk backing from NAV.

## 11. Marked NAV

Include open trader PnL in the LP share price.
This rule prevents an LP from leaving before a known trader profit settles.

For each market, calculate raw side PnL:

```text
long_raw_pnl =
    long_base_exposure × mark_price
    - long_size_open_interest

short_raw_pnl =
    short_size_open_interest
    - short_base_exposure × mark_price
```

Use high precision for the formulas.

Recognize trader profit in full.
Limit recognized trader loss to the stored collateral for that side.

```text
if raw_side_pnl >= 0:
    recognized_side_pnl = raw_side_pnl
else:
    recognized_side_pnl =
        -min(abs(raw_side_pnl), stored_collateral_total)
```

Calculate marked vault NAV:

```text
vault_nav =
    max(
        cash_lp_equity
        - sum(recognized_side_pnl),
        0
    )
```

Use the canonical trading-price oracle.
Use the same oracle for trading, liquidation, trader PnL, and NAV.
Do not use a separate NAV oracle.

Use aggregate base exposure for each market-side calculation.
An LP settlement can loop through the active markets.
Governance must set a maximum active-market count.
An LP action must not loop through positions.

Marked NAV includes ordinary unrealized trader profit.
Marked NAV cannot identify every insolvent position from aggregates.

Use timely liquidation to reduce this limitation.
Use hard exposure caps to reduce this limitation.
Stop LP settlement during an emergency state.

## 12. LP participation

LP shares give proportional ownership of marked vault NAV.

Use a delayed request for each deposit and withdrawal.
A request has these steps:

1. Lock the assets or shares.
2. Wait for `lp_request_delay`.
3. Assign the first valid synchronized oracle round.
4. Execute the complete request or return the complete escrow.

Assign the first canonical synchronized round at or after eligibility.
Do not let the requester select a later round.
Do not let the executor select a later round.

Resolve requests in request-ID order.
Use this order for deposits and withdrawals.

Use market prices from the canonical oracle round.
Calculate NAV in the vault from stored aggregates.
Do not accept an externally supplied NAV.

Permit a deposit only when all these conditions are true:

- The vault is solvent.
- NAV is positive.
- Cash LP equity is positive.
- NAV is above the configured cash-equity floor.

A clean first deposit is an exception to the positive-value conditions.

Permit a withdrawal only when all these conditions are true:

- Free LP capital is sufficient.
- Post-withdraw utilization is not above the configured maximum.
- No warning state exists.
- No ADL state exists.
- No insolvency exists.
- No oracle failure exists.

Settle a request completely or return its complete escrow.
Do not make a partial fill.
Do not create a delayed withdrawal cash claim.

Use virtual assets and virtual shares:

```text
deposit_shares =
    floor(
        deposit_assets
        × (share_supply + VIRTUAL_SHARES)
        / (vault_nav + VIRTUAL_ASSETS)
    )

withdrawal_assets =
    floor(
        withdrawal_shares
        × (vault_nav + VIRTUAL_ASSETS)
        / (share_supply + VIRTUAL_SHARES)
    )
```

Do not burn the complete share supply while an open position remains.
Do not burn the complete share supply while a risk unit remains.
Do not burn the complete share supply while a receiver claim remains.
Do not burn the complete share supply while an execution budget remains.

A final LP can redeem all LP cash in a clean terminal state.
This terminal rule prevents stranded dust.

## 13. Solvency controls

Set four PnL factors for each market side:

```text
recovery < warning < ADL < hard cap
```

Keep one emergency state for each market side.
Enter a restricted state when the applicable factor reaches its threshold.
Keep the restricted state until the factor is below the recovery threshold.

Use the factors as follows:

- Stop new risk at the warning factor.
- Stop ordinary LP actions at the warning factor.
- Start funded auto-deleveraging at the ADL factor.
- Limit payouts at the hard-cap factor.
- Resume normal operation below the recovery factor.

Pay a capped reward for an ADL action.
Pay a capped reward for the discovery of an insolvent position.
Pay the rewards from the risk-keeper reserve.

If non-LP claims equal physical cash, cash LP equity is zero.
The non-LP claims remain fully backed.

If non-LP claims are more than physical cash, a shortfall exists.
Stop ordinary outgoing claims during a shortfall.
Stop LP actions during a shortfall.

Use recapitalization to add cash during a shortfall.
Do not mint LP shares for recapitalization.

## 14. Required properties

The completed system must preserve these properties:

1. Balanced base exposure produces zero funding.
2. Funding increases quadratically with normalized skew.
3. Light-side traders receive funding in proportion to counter-exposure.
4. LPs receive the unmatched collected funding.
5. A dust light-side position cannot redirect all funding.
6. Receiver claims do not exceed receiver-backed payer accrual.
7. Receiver funding becomes guaranteed when it accrues.
8. Uncollected payer funding is not LP cash.
9. Borrow revenue becomes revenue only when the vault collects it.
10. Borrow increases quadratically with vault-wide utilization.
11. Risk units measure gross risk that an action creates.
12. Oracle price changes do not change risk units.
13. New risk stays within global capacity limits.
14. New risk stays within market-side limits.
15. A checkpoint applies the old rate before a mutation.
16. New size does not pay a fee for time before the size existed.
17. A partial close capitalizes old accrual before it resets baselines.
18. Accrued fees affect margin and liquidation.
19. One physical balance reconciles with all claims and LP cash.
20. Marked NAV includes open trader PnL.
21. Marked NAV uses the canonical trading-price oracle.
22. Recognized trader loss does not exceed the side collateral aggregate.
23. An LP action does not loop through positions.
24. A delayed LP request cannot select its settlement round.
25. A withdrawal does not consume required risk backing.
26. An emergency state stops ordinary LP actions.
27. Rounding does not create value.
28. Rounding does not over-credit a receiver.

## 15. Parameters

Configure these market parameters:

```text
open_fee_low_bps
open_fee_high_bps
max_funding_rate_bps_day

market_risk_factor_bps
max_long_size_open_interest
max_short_size_open_interest
max_long_base_exposure
max_short_base_exposure

maintenance_margin_bps
liquidation_reward_bps
warning_pnl_factor_bps
adl_pnl_factor_bps
recovery_pnl_factor_bps
hard_cap_pnl_factor_bps
adl_reward_bps
```

Configure these vault parameters:

```text
base_borrow_rate_bps_day
max_variable_borrow_rate_bps_day
risk_capacity_limit_bps
max_withdraw_utilization_bps

lp_request_delay
min_deposit_nav_factor_bps

global_hard_cap_factor_limit_bps

lp_revenue_share_bps
risk_keeper_revenue_share_bps
max_adl_reward
max_insolvent_touch_reward
```

Fix the funding exponent at two.
Fix the borrow exponent at two.

Use conservative governance limits.
Do not use an onchain volatility estimator.

Test the parameters with these conditions:

- A one-sided market.
- A sudden price change.
- High utilization.
- A delayed liquidation.
- A minimum-size position.
- A rounding boundary.
- Stress in multiple markets at the same time.
