# Fee Mechanics: Economic Theory

## 1. Purpose

This document uses ASD-STE100 Simplified Technical English.

This document explains the economic rules for a perpetual market.
It does not specify contract code.

A perpetual market gives leveraged price exposure to a trader.
A vault supplies cash to profitable traders.
A liquidity provider owns the residual value of the vault.

The fee system has four functions:

1. It charges a fee on realized profit when a trader closes exposure.
2. It charges a fee while a position uses vault capacity.
3. It charges the funding payer side for directional imbalance.
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
| effective collateral | Stored collateral after all accrued fees and funding credits. |
| half-life | The time in which the skew EMA closes half of its distance to the current skew. |
| instant weight | The weight of the current skew in the integral skew blend. |
| integral skew | The blend of the current signed skew and the skew EMA. |
| LP | A liquidity provider. |
| marked vault NAV | LP value after recognized unrealized trader PnL. |
| payer side | The side that pays funding for a checkpoint window. The sign of the integrated integral skew selects this side. |
| position size | The USD notional value at entry. |
| receiver side | The side opposite the payer side. |
| risk unit | A fixed measure of gross capacity that a position uses. |
| side | The long side or the short side of a market. |
| skew | The normalized signed difference between long and short base exposure. |
| skew EMA | The stored exponential moving average of the signed skew. |
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

## 4. Closing fee

Do not charge a position fee for an open or increase action.
The complete supplied collateral becomes stored collateral.

Charge a closing fee when an action removes exposure.
These actions are the trader decrease, the trader close, the
liquidation, the ADL action, and the triggered order.

Use two closing-fee tiers.
Judge the tier on the book that the close leaves behind.
Use the low tier when the removal improves or preserves the balance.
Use the high tier when the removal makes the balance worse.

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

Compare the book before the removal with the book that remains:

```text
skew_after = skew of the book minus the removed exposure
```

Select the fee tier as follows:

```text
if skew_after <= skew_before:
    fee_bps = close_fee_low_bps
else:
    fee_bps = close_fee_high_bps

closing_fee = ceil(payable_price_pnl × fee_bps / BPS)
```

The fee is a share of the realized profit, not of the removed size.
Validate `fee_bps` at most BPS, so a winner always keeps the
complement share of the realized profit.
Collect the fee only out of realized positive price PnL.
A close without realized profit pays zero.
Thus the closing fee cannot cause a shortfall.
The closing fee cannot deepen bad debt.

Use base exposure for the skew comparison.
Do not use entry-time USD open interest for the comparison.

The two-tier boundary is intentionally discrete.
A continuous curve can give more precision.
A continuous curve also adds more rules and makes quotes path-dependent.
The two-tier rule is the selected simple policy.

## 5. Funding

### 5.1 Purpose

Funding sets a price for directional imbalance.
Funding blends the imbalance with its own history.
A book flip is charged gradually, not instantly.

Calculate the signed skew as a fraction of one:

```text
S =
    (long_base_exposure - short_base_exposure)
    / (long_base_exposure + short_base_exposure)
```

`S` is zero for an empty or balanced market.
`S` is +1 or −1 for a one-sided market.

Keep one skew EMA `E` for each market.
Initialize `E` to zero.
`E` decays toward `S` with the global half-life `H`:

```text
E(t) = S + (E₀ - S) × 2^(−t/H)
```

Blend the current skew and the EMA with the market instant weight `w`:

```text
I = (w × S + (BPS − w) × E) / BPS
```

An instant weight of `BPS` reproduces the pure instant skew.

Select the payer side for a checkpoint window as follows:

```text
if ∫ I dt > 0 over the window:
    longs pay

if ∫ I dt < 0 over the window:
    shorts pay

if ∫ I dt = 0 over the window:
    no side pays
```

The payer side can be the side with less base exposure.
This condition occurs after the book flips against its history.
A trader who balances the book keeps the receiving position while the
history decays.

### 5.2 Funding rate

Calculate the payer rate as follows:

```text
payer_rate_bps_day =
    max_funding_rate_bps_day
    × I²
```

Treat `I` as a signed fraction of one in this formula.
The sign of `I` selects the payer side.
The sign of `I` does not change the rate.

The quadratic curve has three effects:

- A small imbalance has a small fee.
- The fee increases faster when the imbalance increases.
- A persistently one-sided market converges to the configured maximum
  rate.

Charge each existing position on the payer side at the current rate.
The rate decays continuously between actions.
Accrual integrates the decay exactly.

### 5.3 Funding flow

Receiver-side traders offset part of the payer exposure.
LPs offset the unmatched part.
Divide the payer flow in proportion to this counter-exposure.

Calculate the complete payer flow as follows:

```text
payer_flow_per_day =
    payer_size_open_interest
    × payer_rate_bps_day
    / BPS
```

Calculate the receiver-side share as follows:

```text
counterparty_share =
    min(
        1,
        receiver_base_exposure / payer_base_exposure
    )
```

Calculate the receiver flows as follows:

```text
trader_receiver_flow_per_day =
    payer_flow_per_day × counterparty_share

lp_funding_flow_per_day =
    payer_flow_per_day - trader_receiver_flow_per_day
```

The payer side can be the lighter side.
Then the raw exposure ratio can be more than one.
The cap at one gives receivers at most the complete payer flow.
The cap keeps the LP flow at or above zero.
The LP flow is zero while receivers over-match the payer.

Apply these results:

- If the receiver side is zero, LPs receive all collected funding.
- If the receiver side is small, receiver-side traders receive a small
  share.
- If the market approaches balance and its history decays, the trader
  share increases.
- If the market approaches balance and its history decays, the payer
  rate approaches zero.
- A dust position cannot redirect all funding from LPs.

Derive the receiver-side rate from the allocated flow:

```text
receiver_rate =
    trader_receiver_flow
    / receiver_size_open_interest
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
- One skew EMA for each market.
- One global pending receiver-funding claim.

Advance the borrow index with a piecewise-constant rate.
Advance the funding indices with the exact closed-form integral of the
decaying funding rate.

Accrue the receiver claim in each market checkpoint.
Accrue the claim atomically with that market's indices.
A position credit cannot exceed its market's claim contribution.
Do not store a global receiver-flow rate.

## 8. Checkpoint order

The borrow rate is constant between two checkpoints.
The funding rate follows a known decay curve between two checkpoints.
A state change must not change accrual for past time.

Use this procedure for each applicable action:

1. Accrue the global borrow index with the old rate.
2. Accrue the affected market indices with the pre-mutation book.
3. Accrue the market's receiver claim in the same market checkpoint.
4. Advance the market's skew EMA to the checkpoint time.
5. Capitalize the affected position fees.
6. Apply the requested mutation.
7. Refresh the displayed payer side and payer rate.
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

Funding is different.
The funding rate decays continuously as the EMA converges.
Each market checkpoint integrates that decay in closed form.
The checkpoint frequency changes accrued funding only within the
decay-table tolerance.

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

Use two margin rates for each market.
Keep the maintenance margin above zero.
Keep the maintenance margin at or below the initial margin.

Use the initial margin for an action that increases leverage.
These actions are an open, an increase with added size, and a
collateral withdrawal without removed size.

Use the maintenance margin for a de-risking check and for liquidation.
These checks are an increase with only added collateral, a partial
close with removed size, and the liquidation test.

The gap between the two margins is the trader's guaranteed entry
buffer.
A position at maximum leverage does not start on the liquidation
boundary.

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

Collect the closing fee at each close.
Collect it only from realized positive price PnL.
Split each collected closing fee and borrow fee between these owners:

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

collected_closing_fee =
    min(closing_fee, max(close_equity, 0))

trader_payout = max(close_equity − collected_closing_fee, 0)
bad_debt      = max(-close_equity, 0)
```

Distribute available value in this order:

1. Pay receiver-backed funding.
2. Pay negative price PnL to LPs.
3. Pay LP-backed funding.
4. Pay borrow.
5. Pay the closing fee.
6. Pay the remaining equity to the trader.

Positive price PnL adds value to the distribution.
The closing fee ranks below each accrued obligation.
For a liquidation, pay its reward after the closing fee.
Pay the liquidation reward before residual trader equity.

A close without realized profit pays zero closing fee.
The closing fee cannot create bad debt.

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

1. An empty book, or a balanced book with a decayed funding history,
   produces zero funding.
2. Funding increases quadratically with the blended integral skew.
3. Receiver-side traders receive funding in proportion to
   counter-exposure, up to the complete payer flow.
4. LPs receive the unmatched collected funding.
5. A dust receiver-side position cannot redirect all funding.
6. Receiver claims do not exceed receiver-backed payer accrual.
7. Receiver funding becomes guaranteed when it accrues.
8. Uncollected payer funding is not LP cash.
9. Borrow revenue becomes revenue only when the vault collects it.
10. Borrow increases quadratically with vault-wide utilization.
11. Risk units measure gross risk that an action creates.
12. Oracle price changes do not change risk units.
13. New risk stays within global capacity limits.
14. New risk stays within market-side limits.
15. A checkpoint settles elapsed time with the pre-mutation state.
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
close_fee_low_bps
close_fee_high_bps
max_funding_rate_bps_day
instant_weight_bps

market_risk_factor_bps
max_long_size_open_interest
max_short_size_open_interest
max_long_base_exposure
max_short_base_exposure

initial_margin_bps
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
funding_half_life_seconds

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
- A book flip against the funding history.
- A sudden price change.
- High utilization.
- A delayed liquidation.
- A minimum-size position.
- A rounding boundary.
- Stress in multiple markets at the same time.
