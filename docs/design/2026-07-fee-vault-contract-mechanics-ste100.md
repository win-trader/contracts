# Vault, Fee, PnL, and LP Share Mechanics

## 1. Purpose

This document uses ASD-STE100 Simplified Technical English.

This document specifies the logic for a perpetual trading vault.
It does not specify contract code.

The contract must do these functions:

1. Hold all collateral in one token balance.
2. Accrue borrow fees with time.
3. Accrue funding fees with time.
4. Preserve the ownership of each claim.
5. Calculate trader PnL.
6. Calculate the LP share value.
7. Prevent unsafe withdrawals.
8. Keep normal trading independent of the position count.

Store a value only when the contract cannot derive it safely.
Derive physical cash from the collateral-token balance.
Derive LP equity, free capital, pending fees, PnL, NAV, and share price.

## 2. Technical terms

The following terms have one meaning in this document.

| Term | Meaning |
|---|---|
| active market | A market that has state that is necessary for NAV. |
| base exposure | The quantity of the underlying asset in a position. |
| cash LP equity | LP cash before unrealized trader PnL. |
| checkpoint | An update that accrues fees to a specified time. |
| claim | An accounting label that identifies the owner of vault cash. |
| clean terminal state | A state with no position, risk unit, receiver claim, or execution budget. |
| debt baseline | The index-derived amount already applied to a position. |
| effective collateral | Stored collateral after accrued fees and funding credits. |
| half-life | The time in which the skew EMA closes half of its distance to the current skew. |
| instant weight | The weight of the current skew in the integral skew blend. |
| integral skew | The blend of the current signed skew and the skew EMA. |
| LP | A liquidity provider. |
| marked vault NAV | LP value after recognized unrealized trader PnL. |
| oracle round | One authenticated set of market prices with one identifier and timestamp. |
| payer side | The side that pays funding for a checkpoint window. The sign of the integrated integral skew selects this side. |
| physical cash | The collateral-token balance of the vault. |
| position size | The USD notional value at entry. |
| receiver side | The side opposite the payer side. |
| risk unit | A fixed measure of gross vault capacity that a position uses. |
| side | The long side or the short side of a market. |
| skew | The normalized signed difference between long and short base exposure. |
| skew EMA | The stored exponential moving average of the signed skew. |
| stored collateral | Position collateral recorded in contract state. |

`PnL` means profit and loss.
`NAV` means net asset value.
`ADL` means auto-deleveraging.
`FIFO` means first in, first out.
`BPS` means basis points.

The word `must` identifies a requirement.
The word `can` identifies an ability.

## 3. Numerical model

Use these constants:

```text
BPS = 10,000
SECONDS_PER_DAY = 86,400

PRICE_PRECISION = 10^30
INDEX_PRECISION = 10^30
RATE_PRECISION = 10^30
FACTOR_PRECISION = 10^30
SHARE_PRECISION = 10^18
```

Use a non-rebasing USD token as the collateral asset.
The collateral token must not have a transfer fee.
Use the native token decimals for cash amounts.

```text
ASSET_PRECISION = 10^collateral_decimals
ASSET_TO_SHARE_SCALE = SHARE_PRECISION / ASSET_PRECISION

VIRTUAL_ASSETS = 1
VIRTUAL_SHARES = ASSET_TO_SHARE_SCALE
```

Store configured rates as basis points per day.
Keep `RATE_PRECISION` during intermediate rate calculations.
Do not round a fractional rate to one basis point before accrual.

Multiply before division.
Use a full-precision `mulDiv` operation.

Keep signed PnL as a high-precision numerator.
Convert the numerator to cash only at the final step.

Carry the remainder of each repeated cumulative division.
The checkpoint frequency must not change accrued borrow value.

Funding integrates a continuously decaying rate in closed form.
The funding decay table quantizes at approximately 1e-13 relative error.
The checkpoint frequency must not change accrued funding value beyond
that tolerance.
Only an instant weight of `BPS` makes the funding equality exact.

## 4. Sources of truth

### 4.1 Physical cash

Calculate physical cash as follows:

```text
physical_cash = collateral_token.balanceOf(vault)
```

Use physical cash as the only authoritative cash balance.

Do not store an authoritative `available_cash` value.
Do not store an authoritative `lp_assets` value.
Do not store an authoritative `vault_balance` value.

Stored balance counters can drift after rounding or a token donation.

Read the LP share supply from the share token:

```text
share_supply = lp_share_token.totalSupply()
```

Do not store a second share-supply counter.

### 4.2 Non-LP claims

Store these explicit non-LP claims:

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

Each value is a claim on the same physical tokens.
Do not create a separate token pool for a claim.

### 4.3 LP cash equity

Calculate cash LP equity:

```text
cash_lp_equity =
    max(physical_cash - non_lp_claims, 0)
```

If claims are more than cash, calculate the difference as a shortfall.

Do not store cash LP equity.
Derive cash LP equity when the value is necessary.

An unsolicited collateral transfer increases LP cash equity.
This rule prevents drift in a second balance counter.

### 4.4 Risk backing

Calculate required risk backing:

```text
required_risk_backing =
    ceil(
        total_risk_units
        × BPS
        / risk_capacity_limit_bps
    )
```

Calculate free LP capital:

```text
free_lp_capital =
    max(cash_lp_equity - required_risk_backing, 0)
```

Risk units lock LP cash.
Risk units do not reduce LP ownership.

Do not substitute these quantities for each other:

```text
cash_lp_equity
vault_nav
free_lp_capital
```

## 5. Stored state

### 5.1 Global state

Store these global claims:

```text
stored_position_collateral_total
pending_receiver_funding_total
execution_budget_total
protocol_claimable_total
risk_keeper_reserve_total
```

Store these global risk values:

```text
total_risk_units
open_position_count
lp_blocked_side_count
```

Store these global borrow values:

```text
borrow_index
borrow_index_remainder
current_borrow_rate
```

Store this global checkpoint value:

```text
last_global_checkpoint
```

Do not store a global receiver-flow rate.
Do not store a global receiver-accrual remainder.
The market checkpoint accrues the receiver liability for each market.

Store these LP request values:

```text
next_lp_request_id
next_lp_request_to_resolve
```

Store this active-market state:

```text
active_market_registry
max_active_markets
```

Governance must set a hard maximum for the active-market count.
Use the registry only during an LP settlement.
Use it to calculate NAV and to evaluate market-side risk states.

Store these global control parameters:

```text
risk_capacity_limit_bps
max_withdraw_utilization_bps
min_deposit_nav_factor_bps
lp_request_delay

funding_half_life_seconds

base_borrow_rate_bps_day
max_variable_borrow_rate_bps_day

lp_revenue_share_bps
risk_keeper_revenue_share_bps
global_hard_cap_factor_limit_bps
max_adl_reward
max_insolvent_touch_reward
```

`funding_half_life_seconds` is one memory horizon for every market.
Keep `funding_half_life_seconds` from 60 through 31,536,000 seconds.

### 5.2 Market state

Store these values for each market side:

```text
size_open_interest
base_exposure
stored_collateral_total
risk_units
risk_state
```

Store these funding indices for each market:

```text
receiver_backed_payer_index_long
receiver_backed_payer_index_short

lp_backed_payer_index_long
lp_backed_payer_index_short

receiver_index_long
receiver_index_short
```

Store these funding control values:

```text
skew_ema
funding_index_remainders
pending_receiver_remainder
last_funding_checkpoint

current_payer_side
current_payer_rate
```

`skew_ema` is the stored funding history for the market.
`pending_receiver_remainder` carries the sub-unit receiver-liability
accrual.
`current_payer_side` and `current_payer_rate` are display state.
Refresh them from the integral skew after each checkpoint and mutation.
Do not store a receiver-flow rate.
Do not store an LP-flow rate.

Store these market parameters:

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

warning_pnl_factor_bps
adl_pnl_factor_bps
recovery_pnl_factor_bps
hard_cap_pnl_factor_bps
initial_margin_bps
maintenance_margin_bps
liquidation_reward_bps
adl_reward_bps
```

Keep each size cap at or below 10^16.
Keep each base-exposure cap at or below 10^18.
The two ceilings keep the funding window integral inside i128
arithmetic.

Use the market aggregates for funding.
Use the market aggregates for market-side PnL.
Use the market aggregates for directional limits.

### 5.3 Position state

Store these values for each position:

```text
owner
market
direction

size
base_exposure
stored_collateral
risk_units

borrow_debt
funding_paid_to_receivers_debt
funding_paid_to_lps_debt
funding_received_debt

execution_budget
conditional_order_terms
```

Do not store a current fee.
Do not store a personal rate.
Do not store current PnL.
Do not store effective collateral.
Derive these values when they are necessary.

### 5.4 LP request state

Store these values for each LP request:

```text
request_id
owner
kind
amount
request_time
execute_after
status
```

For a deposit, `amount` is collateral in a request router.
For a withdrawal, `amount` is LP shares in the request router.

Keep escrowed withdrawal shares in total share supply until settlement.
Do not store an aggregate request ledger.
Do not store a withdrawal cash claim.

## 6. Cash transitions

Each transfer or claim change must explain the LP equity change.

| Event | Physical cash | Non-LP claims | LP cash equity |
|---|---:|---:|---:|
| Trader collateral deposit | Increase | Increase by the same amount | No change |
| Execution-budget deposit | Increase | Increase by the same amount | No change |
| Receiver funding accrual | No change | Increase | Decrease |
| Receiver claim becomes collateral | No change | No total change | No change |
| Receiver-backed payer collection | No change | Position collateral decreases | Increase |
| LP-backed fee collection | No change | Position collateral decreases | Increase |
| Protocol-fee collection | No change | Move value between claims | No change |
| Keeper-fee collection | No change | Move value between claims | No change |
| Non-LP claim withdrawal | Decrease | Decrease by the same amount | No change |
| LP deposit | Increase | No change | Increase |
| LP withdrawal | Decrease | No change | Decrease |
| Trader-profit settlement | Decrease beyond removed collateral | Decrease by collateral | Decrease by profit |
| Trader-loss settlement | Decrease by less than removed collateral | Decrease by collateral | Increase by loss |

An accounting label must not create cash.
A cash transfer must have an ownership effect.
Use this table to test each state transition.

Each settlement entry point must emit one event with the amounts it moved.
Each keeper checkpoint must emit the updated indices, the skew EMA, and
the current rates.
Each revenue split must emit the collected amount and each share.
Each LP settlement event must carry the post-settlement supply and NAV.
Each canonical round publication must emit the round identifier.
Each risk-state transition must emit the market, the side, and the new
state.

An off-chain consumer must be able to rebuild each row of this table from
the events.
Index and rate values are exact at each checkpoint event.
A position action can change the displayed payer side and rate between
checkpoint events without an event.
The funding rate also decays continuously between events.
A consumer must treat accrual projections between checkpoint events as
estimates with keeper-cadence staleness.

## 7. Price exposure and PnL

### 7.1 Base exposure

For an increase, calculate new base exposure:

```text
base_added =
    floor(
        size_added
        × PRICE_PRECISION
        / execution_price
    )
```

Increase the position size by `size_added`.
Increase the side-size aggregate by the same amount.
Increase the position base exposure by `base_added`.
Increase the side-base aggregate by the same amount.

For a partial close, calculate the remaining exposure:

```text
size_after = size_before - size_removed

base_after =
    floor(
        base_before
        × size_after
        / size_before
    )

base_removed = base_before - base_after
```

Remove the complete remainder on the final close.
Use the same remaining-value method for risk units.
The method must not strand base exposure or risk units.

### 7.2 Raw PnL

At mark price `p`, calculate side PnL:

```text
long_raw_pnl_numerator =
    long_base_exposure × p
    - long_size_open_interest × PRICE_PRECISION

short_raw_pnl_numerator =
    short_size_open_interest × PRICE_PRECISION
    - short_base_exposure × p
```

The side value equals the sum of raw position PnL.
Only the specified cash-conversion rounding can cause a difference.

### 7.3 Recognized PnL

For each side, calculate recognized PnL:

```text
if raw_pnl_numerator >= 0:
    recognized_pnl_numerator = raw_pnl_numerator
else:
    recognized_pnl_numerator =
        -min(
            abs(raw_pnl_numerator),
            stored_collateral_total × PRICE_PRECISION
        )
```

Recognize trader profit in full.
The vault can owe the complete trader profit.

Do not recognize a trader loss above the side collateral aggregate.
The vault cannot collect the excess loss.

For one synchronized price snapshot, calculate:

```text
aggregate_recognized_trader_pnl_numerator =
    sum(recognized_pnl_numerator for every active side)

vault_nav_numerator =
    cash_lp_equity × PRICE_PRECISION
    - aggregate_recognized_trader_pnl_numerator

vault_nav =
    floor(max(vault_nav_numerator, 0) / PRICE_PRECISION)
```

Loop through the bounded active-market registry.
Do not loop through positions.

### 7.4 NAV recognition

Include these values in marked NAV:

- Residual LP cash.
- Unrealized trader price PnL.
- Realized trader price PnL.
- Collected LP fee revenue.
- Collected LP-backed funding.
- Receiver claims through the non-LP claim deduction.
- Recognized bad debt.
- Direct token donations.

Exclude these values from marked NAV:

- Uncollected borrow fees.
- Uncollected payer funding.
- Future fees.

Do not recognize a receivable before collection.
An insolvent trader can fail to pay the receivable.

The aggregate loss limit cannot prove the solvency of each position.
Individual collateral limits are nonlinear.
A small set of market sums cannot reconstruct those limits.
The system must use timely liquidation.

## 8. Funding mechanics

### 8.1 Integral skew and rate

Calculate the signed skew as a fraction of one at `INDEX_PRECISION`
scale:

```text
skew_frac(long_base, short_base) =
    if long_base + short_base = 0:
        0
    else:
        (long_base - short_base)
        × INDEX_PRECISION
        / (long_base + short_base)
```

Write `S` for the current signed skew.
`S` is positive when longs dominate.
`S` is zero for an empty book.

Store one skew EMA for each market.
Write `E` for the stored EMA.
Seed `E` with the post-open skew when an open finds an empty book.
Zero is not a neutral start.
A zero seed would discount a one-sided launch until the history fills.
Set `E` to zero when the book empties.
An empty market keeps no funding memory.
Zero the displayed payer side and rate at the same moment.
`E` decays toward `S` with the global half-life `H`:

```text
H = funding_half_life_seconds

E(t) = S + (E₀ - S) × 2^(−t/H)
```

Blend the current skew and the EMA with the market instant weight `w`:

```text
w = instant_weight_bps

I = (w × S + (BPS − w) × E) / BPS
```

An instant weight of `BPS` reproduces the pure instant skew.

Calculate the unsigned payer rate from the integral skew:

```text
payer_rate_bps_day =
    max_funding_rate_bps_day
    × I²
```

Treat `I` as a signed fraction of one in this formula.
The sign of `I` does not change the rate.

The payer side for a checkpoint window is the sign of the integrated
integral skew:

```text
payer_sign = sign(∫ I dt over the window)

payer_sign > 0: longs pay
payer_sign < 0: shorts pay
payer_sign = 0: no side pays
```

The payer side can be the side with less base exposure.
This condition occurs after the book flips against its history.
A trader who balances the book keeps the receiving position while the
history decays.

### 8.2 Window integral and indices

The book is constant between two checkpoints.
Thus the integral skew has a closed form on a window:

```text
d(t) = 2^(−t/H)

A = S
B = (BPS − w) × (E₀ − S) / BPS

I(t) = A + B × d(t)
```

Integrate the rate exactly over the window length `Δt`:

```text
d = d(Δt)

J₁ = (H / ln 2) × (1 − d)
J₂ = (H / (2 × ln 2)) × (1 − d²)

W = ∫ rate dt =
    max_funding_rate_bps_day
    × (A² × Δt + 2 × A × B × J₁ + B² × J₂)
```

Calculate `d` with a 47-entry square-and-multiply table of
`2^(−2^(−i))` constants.
The table quantizes at approximately 1e-13 relative error.

Write `W` for the window weight `∫ rate dt`.
Divide `W` by `BPS × SECONDS_PER_DAY` for the index delta per unit of
payer-side size.

Split `W` between the receiver-backed part and the LP-backed part:

```text
receiver_share = min(1, receiver_base / payer_base)

W_receiver = W × receiver_share
W_lp       = W − W_receiver
```

If the receiver side has zero size, set `W_receiver` to zero.
If the receiver side has zero base exposure, set `W_receiver` to zero.

The payer side can be the lighter side.
The cap at one keeps `W_lp` at or above zero.
The cap keeps the LP-backed index monotone.

The payer side has two payer indices.

Calculate the payer-index changes:

```text
receiver_backed_payer_index_delta =
    W_receiver
    / (BPS × SECONDS_PER_DAY)

lp_backed_payer_index_delta =
    W_lp
    / (BPS × SECONDS_PER_DAY)
```

The receiver side has one receiver index.
Derive its change from the exact payer collection:

```text
receiver_cash =
    payer_size × receiver_backed_payer_index_delta

receiver_index_delta =
    receiver_cash
    / receiver_size
```

Carry each division remainder.

Round payer obligations up at the position boundary.
Round receiver credits down.
Do not let complete receiver credit exceed receiver-backed payer accrual.

### 8.3 Receiver liability

Accrue the receiver liability inside the market checkpoint:

```text
pending_receiver_funding_total +=
    floor(
        payer_size
        × receiver_backed_payer_index_delta
        / INDEX_PRECISION
    )
```

Carry the sub-unit part in the market `pending_receiver_remainder`.

Accrue the liability and the funding indices in the same market
checkpoint.
Both values derive from the same exact payer collection.
Thus a position credit cannot exceed its market's liability
contribution.

Do not accrue the receiver liability at the global checkpoint.
Do not store a global receiver-flow rate.

Use `pending_receiver_funding_total` as the authoritative receiver liability.
The liability reduces cash LP equity when it accrues.

The sum of position receiver credits must not exceed the liability.

When receiver funding becomes position collateral, apply:

```text
pending_receiver_funding_total -= receiver_credit
stored_position_collateral_total += receiver_credit
position.stored_collateral += receiver_credit
```

The total non-LP claim does not change.
The operation changes only the ownership label.

When the vault collects a receiver-backed payer fee, apply:

```text
position collateral decreases
stored_position_collateral_total decreases
```

Do not create a new receiver claim.
The accrual step already recorded the receiver claim.
The collection restores LP cash that backed the receiver claim.

If the payer cannot pay, LPs absorb the difference.
Do not reverse the receiver claim.

The final close checkpoints global state and the affected market.
Use the final close time.

Release an unassigned rounding residue only when this value is zero:

```text
open_position_count
```

Aggregate conservation then requires every market size to be zero.
Do not loop through markets during the final close.

The released amount becomes LP residual cash.
Drop a market's carried funding remainders when both of its sides have
zero size open interest.

LP pricing must see exact receiver liabilities.
Thus an LP settlement checkpoints every active market.
The `max_active_markets` bound limits this loop.
An entry point without a market accepts keeper-cadence staleness in the
liability total.

## 9. Borrow mechanics

### 9.1 Risk units

Calculate risk units for new exposure:

```text
risk_units_added =
    floor(
        size_added
        × market_risk_factor_bps
        / BPS
    )
```

Increase the position, market, and global totals by the same amount.

Before new risk, apply the capacity gate:

```text
total_risk_units_after
    <= cash_lp_equity_after
       × risk_capacity_limit_bps
       / BPS
```

Also apply the market-side size cap.
Also apply the market-side base-exposure cap.
Reject new risk at or above the warning PnL factor.

### 9.2 Borrow rate

Calculate utilization:

```text
if total_risk_units = 0:
    utilization_bps = 0
else if cash_lp_equity = 0:
    utilization_bps = BPS
else:
    utilization_bps =
        min(
            total_risk_units × BPS / cash_lp_equity,
            BPS
        )
```

Calculate the borrow rate:

```text
borrow_rate_bps_day =
    base_borrow_rate_bps_day
    + max_variable_borrow_rate_bps_day
      × utilization_bps²
      / BPS²
```

At a checkpoint, calculate the borrow-index change:

```text
borrow_index_delta =
    borrow_rate_bps_day
    × INDEX_PRECISION
    × elapsed_time
    / (BPS × SECONDS_PER_DAY)
```

Use the stored rate for the elapsed interval.
Use the new rate only for future time.

For a position, calculate pending borrow:

```text
borrow_index_value =
    ceil(position.risk_units × borrow_index / INDEX_PRECISION)

pending_borrow =
    borrow_index_value - position.borrow_debt
```

## 10. Checkpoints

### 10.1 Global checkpoint

If `now` is after `last_global_checkpoint`, do this procedure:

1. Advance the borrow index with the stored rate.
2. Carry the division remainder.
3. Set `last_global_checkpoint` to `now`.

If the timestamps are equal, do not change state.

The global checkpoint advances borrow only.
The market checkpoint accrues all funding value.

### 10.2 Market checkpoint

If `now` is after the market checkpoint, do this procedure:

1. Resolve the funding window with the closed-form integral.
2. Select the window payer side from the sign of the integrated skew.
3. Split the window weight into the receiver-backed part and the
   LP-backed part.
4. Advance the receiver-backed payer index for the payer side.
5. Advance the LP-backed payer index for the payer side.
6. Advance the receiver index for the receiver side.
7. Accrue the receiver liability from the same payer collection.
8. Carry all remainders.
9. Advance the skew EMA to the window end.
10. Refresh the displayed payer side and payer rate from the integral
    skew.
11. Set the market checkpoint to `now`.

### 10.3 Mutation order

Use this order for an exposure, claim, risk, or rate-input change:

1. Checkpoint global state.
2. Checkpoint the affected market.
3. Capitalize the affected position fees.
4. Apply the requested mutation.
5. Refresh the displayed payer side and payer rate.
6. Evaluate the market risk states.
7. Calculate the new global borrow rate.

Skip a step if the action has no applicable market or position.

For an LP settlement, checkpoint global accrual first.
Then checkpoint each active market.
LP pricing must see exact receiver liabilities.
Then calculate marked NAV.
Then apply the cash and share changes.
Then calculate the new borrow rate.

An entry point without a market checkpoints global state only.
Its receiver-liability view accepts keeper-cadence staleness.

An operational pause must not stop an accrual clock.
A position pays or receives fees until settlement.

Checkpoint an affected index before a parameter update.
A market funding update must checkpoint global state and that market.
A global borrow update must checkpoint global state.
Do not use a new parameter for past time.

## 11. Position fee accounting

### 11.1 Closing fee

Do not charge a fee for an open or increase action.
The complete supplied collateral becomes stored collateral.

Charge a closing fee when a close path removes size.
The close paths are the trader decrease, the trader close, the
liquidation, the ADL action, and the triggered order.
All close paths use one settlement procedure.

Collect the fee immediately after capitalization.
Collect the fee before the partial-close profit payout.

Calculate normalized base-exposure skew with this function:

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
skew_before = skew(long_base, short_base)

for a long close:
    skew_after = skew(long_base − base_removed, short_base)

for a short close:
    skew_after = skew(long_base, short_base − base_removed)
```

Select the closing-fee tier:

```text
if skew_after <= skew_before:
    fee_bps = close_fee_low_bps
else:
    fee_bps = close_fee_high_bps
```

Use the low tier when the removal improves or preserves the balance.
Use the high tier when the removal makes the balance worse.

Use base exposure for the skew comparison.
Do not use entry-time USD open interest for the comparison.

Calculate the closing fee as a share of the realized profit:

```text
closing_fee = ceil(payable_price_pnl × fee_bps / BPS)
```

`payable_price_pnl` is the realized positive price PnL after the
emergency payout factor.
Collect the fee only out of realized positive price PnL.
A close without realized profit pays zero.
Thus the closing fee has no shortfall path.
The tier validation bounds `fee_bps` at BPS, so the fee never
exceeds the payable profit and a winner always keeps the
complement share of the realized profit.

Split the collected fee as specified in Section 11.4.
Use the closing-fee revenue source for the split event.

### 11.2 Pending amounts

For the position direction, calculate:

```text
pending_funding_paid_to_receivers =
    ceil(size × receiver_backed_payer_index / INDEX_PRECISION)
    - funding_paid_to_receivers_debt

pending_funding_paid_to_lps =
    ceil(size × lp_backed_payer_index / INDEX_PRECISION)
    - funding_paid_to_lps_debt

pending_funding_received =
    floor(size × receiver_index / INDEX_PRECISION)
    - funding_received_debt
```

Calculate pending borrow as specified in Section 9.2.

A pending amount must not be negative.
A negative value identifies an invalid baseline or a decreasing index.

### 11.3 Effective collateral

Calculate effective collateral:

```text
effective_collateral =
    stored_collateral
    + pending_funding_received
    - pending_funding_paid_to_receivers
    - pending_funding_paid_to_lps
    - pending_borrow
```

Use effective collateral for margin checks.
Do not use stale stored collateral for a margin check.

### 11.4 Capitalization

Before a changed position remains open, do this procedure:

1. Add `pending_funding_received` to stored collateral.
2. Remove the same amount from pending receiver claims.
3. Collect receiver-backed funding from stored collateral.
4. Collect LP-backed funding from the remaining collateral.
5. Collect borrow from the remaining collateral.
6. Split the collected borrow fee.
7. Reset all debt baselines to current index values.

Collateral from the current action is available to this procedure.
Positive PnL from a decrease is also available to this procedure.

Do not collect more than the available position value.
Use the specified collection order.
The order reduces shortfall against guaranteed receiver claims.

Record unpaid receiver-backed funding as bad debt.
Do not record uncollected borrow as revenue.
Do not record uncollected LP-backed funding as revenue.

Do not leave an open position with an unpaid accrued obligation.
The trader can add collateral.
The trader can realize sufficient positive PnL.
Otherwise, use the insolvent full-close procedure.

This rule permits a complete baseline reset.
The system does not need an unpaid-fee ledger.

Split collected closing and borrow fees:

```text
lp_amount =
    floor(collected × lp_revenue_share_bps / BPS)

keeper_amount =
    floor(collected × risk_keeper_revenue_share_bps / BPS)

protocol_amount =
    collected - lp_amount - keeper_amount
```

Increase explicit claims only for `keeper_amount` and `protocol_amount`.
Keep `lp_amount` in residual LP cash.

### 11.5 Partial close

After checkpoints, calculate remaining values:

```text
remaining_size = old_size - size_removed

remaining_base =
    floor(old_base × remaining_size / old_size)

remaining_risk =
    floor(old_risk × remaining_size / old_size)

base_removed = old_base - remaining_base
risk_removed = old_risk - remaining_risk
```

Settle pending fees and realized PnL.
Use the close waterfall.

If the position remains open, pay all accrued obligations.
Reset debts from the remaining size and risk units.
Use the current indices.
Do not carry historical debt for the closed part.

## 12. Position actions

### 12.1 Open or increase

Use this procedure:

1. Authenticate a current canonical market price.
2. Checkpoint global state.
3. Checkpoint the affected market.
4. Transfer supplied collateral.
5. Calculate pending fees for the existing position.
6. Calculate pending funding credit for the existing position.
7. Calculate `base_added`.
8. Calculate `risk_units_added`.
9. Settle old obligations from the available collateral.
10. Check the resulting collateral.
11. Check the applicable margin requirement.
12. Check global risk capacity.
13. Check market-side limits.
14. Check the warning factor.
15. Increase the position and market aggregates.
16. Set the debt baselines at the current indices.
17. Refresh the displayed payer side and payer rate.
18. Calculate the new borrow rate.

Do not charge a fee for this action.
The complete supplied collateral becomes stored position collateral.

Select the margin requirement as follows:

- An open action must satisfy the initial margin.
- An increase with added size must satisfy the initial margin.
- An increase with only added collateral must satisfy the maintenance
  margin.

All cash and claim changes must satisfy the cash-transition rules.

### 12.2 Decrease or close

Use this procedure:

1. Authenticate a current canonical market price.
2. Checkpoint global state.
3. Checkpoint the affected market.
4. Calculate removed size, base exposure, and risk units.
5. Calculate raw PnL for the removed exposure.
6. Apply an emergency payout factor to positive PnL.
7. Calculate pending fees.
8. Calculate pending funding credit.
9. Settle credits, obligations, and realized PnL.
10. Collect the closing fee from realized positive price PnL.
11. Transfer the remaining realized profit for a partial close.
12. Apply an explicit collateral withdrawal.
13. Check the applicable margin for a remaining position.
14. Reduce the position and market aggregates.
15. Reset remaining debt baselines.
16. Delete the position after a complete close.
17. Refresh the displayed payer side and payer rate.
18. Calculate the new borrow rate.

For removed long exposure, calculate:

```text
raw_pnl_numerator =
    base_removed × mark_price
    - size_removed × PRICE_PRECISION
```

For removed short exposure, calculate:

```text
raw_pnl_numerator =
    size_removed × PRICE_PRECISION
    - base_removed × mark_price
```

Convert the numerator to cash one time.
Use the specified directional rounding.

If an emergency factor applies, calculate:

```text
if raw_price_pnl > 0:
    payable_price_pnl =
        floor(raw_price_pnl × payout_factor)
else:
    payable_price_pnl = raw_price_pnl
```

For a partial close, apply these rules:

- Received funding supplies value for accrued obligations.
- Positive payable PnL supplies value for accrued obligations.
- Collect the closing fee after the obligations.
- Transfer the remaining realized profit to the trader.
- The transfer reduces LP cash equity.
- Apply negative PnL after guaranteed receiver obligations.
- Collected negative PnL increases LP cash equity.
- Use a full insolvent close if loss exceeds complete collateral.
- Apply `collateral_withdrawn` after PnL.
- Do not permit a withdrawal that makes the position unhealthy.
- A decrease with removed size must satisfy the maintenance margin.
- A withdrawal without removed size must satisfy the initial margin.

Calculate the realized-profit transfer:

```text
realized_payout =
    min(
        payable_price_pnl − closing_fee,
        stored_collateral
    )
```

For a full close, calculate:

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

Positive price PnL adds value to this waterfall.
The closing fee ranks below every accrued obligation.
Do not record unpaid LP-backed funding as revenue.
Do not record unpaid borrow as revenue.

Remove the stored collateral claim.
Transfer `trader_payout`.
These operations move the difference to or from LP residual equity.

Emit bad debt for risk accounting.
Do not store bad debt as a receivable.
Each close event must include the closing fee.

### 12.3 Margins and liquidation

Configure two margin rates for each market:

```text
0 < maintenance_margin_bps
    <= initial_margin_bps
    <= BPS
```

Calculate a margin requirement as follows:

```text
margin_requirement =
    ceil(size × margin_bps / BPS)
```

Use the initial margin for each action that increases leverage:

- An open action.
- An increase with added size.
- A collateral withdrawal without removed size.

Use the maintenance margin for each de-risking check and for
liquidation:

- An increase with only added collateral.
- A partial close with removed size.
- The liquidation test.

The gap between the two margins is the trader's guaranteed entry
buffer.
The maximum leverage shown to a trader is
`floor(BPS / initial_margin_bps)`.
A new position at maximum leverage does not sit on the liquidation
boundary.

Use the close preparation and settlement rules for a liquidation.

Permit liquidation when the position fails the maintenance requirement.
Use effective collateral and current payable PnL for this test.

Apply these liquidation-reward rules:

- Apply the configured reward limit.
- Do not pay more reward than the available value.
- Pay the reward from the liquidated position.
- Account for the reward separately from PnL.

For a liquidation, insert the reward after the closing fee in the close
waterfall.
Pay the reward before residual trader equity.

Pay a capped insolvent-position reward from the risk-keeper reserve.
The reward pays for the discovery of bad debt.

### 12.4 Execution budget

Transfer the execution budget separately.
Record the budget in the position.
Increase `execution_budget_total` by the same amount.

When the vault pays an execution budget, apply:

```text
physical cash decreases
position.execution_budget decreases
execution_budget_total decreases
```

The payment must not change LP cash equity.
An insufficient budget can stop an optional order.
An insufficient budget must not corrupt position settlement.

## 13. LP share mechanics

### 13.1 Price source

Use the canonical oracle prices for these functions:

- Position opens.
- Position closes.
- Trader PnL.
- Margin.
- Liquidation.
- LP NAV.

Do not use a separate NAV price.

For LP settlement, get one synchronized oracle snapshot.
The snapshot must contain each active-market price.
The snapshot must have a unique monotonic round identifier.
Each round price must be a fresh source aggregation at the round timestamp.
A round must not contain a cached observation.
A cached observation can predate the request cutoff while the round
timestamp passes it.

Calculate NAV in the vault.
Use the authenticated prices and stored aggregates.
Do not accept a supplied NAV.

The read-only accounting snapshot is a quote function.
The snapshot validates the round structure: the price count, the symbol
order, and positive prices.
The snapshot does not validate the round provenance.
The settlement path enforces provenance: only the request router starts a
settlement, and the request router reads the round from the oracle router.
An off-chain consumer must read rounds from the oracle router and must not
trust a snapshot computed from any other round.

### 13.2 Delayed requests

An immediate LP action can use price information before an oracle update.
Use a delayed request to prevent this action.

Calculate request eligibility:

```text
execute_after = request_time + lp_request_delay
```

Assign the settlement round:

```text
settlement_round =
    first canonical synchronized oracle round
    with timestamp >= execute_after
```

The assigned round must satisfy:

```text
settlement_round.timestamp >= execute_after
previous_round.timestamp < execute_after
```

The oracle adapter must authenticate the predecessor round.
The requester must not supply an arbitrary round.
The executor must not supply an arbitrary round.

Accept the request only while the assigned round is current.
If a later round becomes current, expire the request.
Return the complete escrow after expiry.
Do not move the request to a later round.

Permit any account to execute or expire a mature request.
Use a production keeper to provide liveness.

### 13.3 FIFO resolution

Resolve only `next_lp_request_to_resolve`.
Advance the pointer after a resolution.
Resolve requests in request-ID order.

FIFO gives deterministic priority for scarce free capital.
FIFO prevents selection of a favorable request from the queue.

Each request is full or zero.

- A deposit transfers all assets and mints all shares.
- A withdrawal burns all shares and transfers all assets.
- A failed request returns all escrow.
- An expired request returns all escrow.

Do not make a partial fill.
Do not create a persistent withdrawal cash claim.

Before an external token transfer, complete these actions:

1. Mark the request as complete.
2. Advance the FIFO pointer.

FIFO can cause a delay behind the first request.
Any account can resolve a failed or expired first request.
Thus, the first request cannot stop the queue permanently.

### 13.4 Share conversion

Use state from before the transfer.

```text
conversion_assets = vault_nav + VIRTUAL_ASSETS
conversion_shares = share_supply + VIRTUAL_SHARES

deposit_shares =
    floor(
        deposit_assets
        × conversion_shares
        / conversion_assets
    )

withdrawal_assets =
    floor(
        withdrawal_shares
        × conversion_assets
        / conversion_shares
    )
```

Report the user share price as:

```text
vault_nav / share_supply
```

Use virtual quantities only for executable conversion.
Do not report virtual quantities as owned assets or shares.

### 13.5 Deposit settlement

At the assigned round, use this procedure:

1. Checkpoint global accrual to the settlement time.
2. Checkpoint each active market to the settlement time.
3. Validate a synchronized price for each active market.
4. Evaluate each market-side risk state.
5. Calculate physical cash.
6. Calculate non-LP claims.
7. Calculate cash LP equity.
8. Calculate marked NAV.
9. Reject a cash shortfall.
10. Resolve the request as failed if a risk state blocks settlement.
11. Check deposit eligibility.
12. Calculate shares from pre-deposit NAV and supply.
13. Mark the request as settled.
14. Advance the FIFO pointer.
15. Transfer all escrowed collateral into the vault.
16. Mint all shares to the owner.
17. Calculate the new borrow rate.

A clean first deposit requires:

```text
physical_cash = 0
non_lp_claims = 0
total_risk_units = 0
open_position_count = 0
share_supply = 0
```

A later deposit requires:

```text
cash_lp_equity > 0
vault_nav > 0

vault_nav × BPS / cash_lp_equity
    >= min_deposit_nav_factor_bps
```

If NAV is too low, use recapitalization.
Do not mint shares for recapitalization.
The NAV floor prevents share hyperinflation.
The NAV floor also prevents dilution of existing LPs.

### 13.6 Withdrawal settlement

Use pre-withdraw NAV.

Use this procedure:

1. Checkpoint global accrual to the settlement time.
2. Checkpoint each active market to the settlement time.
3. Validate a synchronized price for each active market.
4. Evaluate each market-side risk state.
5. Calculate physical cash and non-LP claims.
6. Calculate cash LP equity and marked NAV.
7. Calculate `withdrawal_assets`.
8. Compare the amount with free LP capital.
9. Calculate post-withdraw cash LP equity.
10. Calculate post-withdraw utilization.
11. Check the configured utilization maximum.
12. Resolve the request as failed if a risk state blocks settlement.
13. Reject a shortfall.
14. Mark the request as settled.
15. Advance the FIFO pointer.
16. Burn all escrowed shares.
17. Transfer all collateral to the owner.
18. Calculate the new borrow rate.

Apply these formulas:

```text
require withdrawal_assets <= free_lp_capital

post_cash_lp_equity =
    cash_lp_equity - withdrawal_assets

if total_risk_units = 0:
    post_utilization_bps = 0
else:
    require post_cash_lp_equity > 0

    post_utilization_bps =
        ceil(
            total_risk_units
            × BPS
            / post_cash_lp_equity
        )

require post_utilization_bps
    <= max_withdraw_utilization_bps
```

If a check fails, return all escrowed shares.
Do not keep the request as a cash claim.

Do not burn the complete share supply while one of these values remains:

```text
open position
risk unit
receiver claim
execution budget
```

In a clean terminal state, pay all cash LP equity to the final LP.
This action removes residual dust from the virtual quantities.

### 13.7 Open trader profit

Include unrealized trader profit in recognized trader PnL.
Subtract recognized trader PnL from marked NAV.

An LP request settles at a future oracle round.

If the trader profit still exists, use the lower marked NAV.
If the trader closed, physical cash includes the trader payout.
The unrealized PnL then no longer exists.

The two paths have the same economic result.
Only specified rounding can cause a difference.

An LP cannot escape an ordinary known trader profit.
Undiscovered individual insolvency remains a risk.
Stop LP settlement during a recognized emergency.
Pay keepers to liquidate or reveal insolvency.

## 14. Emergency payout and ADL

For each profitable market side, calculate:

```text
positive_pnl_factor_bps =
    if positive_raw_side_pnl = 0:
        0
    else if cash_lp_equity = 0:
        BPS
    else:
        positive_raw_side_pnl
        × BPS
        / cash_lp_equity
```

Configure the factors in this order:

```text
recovery_factor
    < warning_factor
    < adl_factor
    < hard_cap_factor
```

Use these values for `risk_state`:

```text
NORMAL
WARNING
ADL
HARD_CAP
```

Evaluate risk state during each price-authenticated action.
Set the state to the highest threshold that the current factor reaches.
Keep `WARNING` when a restricted factor is between recovery and warning.
Set the state to `NORMAL` only below the recovery factor.

Update `lp_blocked_side_count` when a side enters or leaves `NORMAL`.
Reject new LP requests when `lp_blocked_side_count` is not zero.

An LP settlement evaluates all sides in its synchronized snapshot.
Apply each current factor before the request resolves.
Resolve the request as failed if a factor blocks LP settlement.
Keep the risk-state update when the request fails.

At the warning factor, apply these restrictions:

- Stop new risk on the affected side.
- Stop new LP requests.
- Stop LP settlement.

At the ADL factor, permit a risk-reduction action.
The action must reduce positive PnL on the affected side.
Pay a capped reward from `risk_keeper_reserve_total`.

At the hard-cap factor, calculate:

```text
hard_cap_value =
    cash_lp_equity
    × hard_cap_pnl_factor_bps
    / BPS

payout_factor =
    min(
        1,
        hard_cap_value / positive_raw_side_pnl
    )
```

Use the same payout factor for each profitable position on that side.
Use one price snapshot for all positions that use the factor.
Collect losses up to the available position value.

Resume ordinary LP operations only below all recovery factors.

Limit the sum of all side hard-cap factors.
Use `global_hard_cap_factor_limit_bps` as the limit.
The limit bounds simultaneous claims against one vault.

## 15. Failure states

### 15.1 Zero LP equity

If physical cash equals non-LP claims, cash LP equity is zero.
The non-LP claims remain fully backed.
Keep normal non-LP withdrawal paths available.
Stop LP deposits and LP withdrawals.

### 15.2 Cash shortfall

If physical cash is less than non-LP claims, calculate:

```text
cash_shortfall = non_lp_claims - physical_cash
```

Stop ordinary outgoing claims.
Stop LP actions.

Permit a risk-reduction action if it cannot increase the shortfall.

Derive the shortfall.
Do not store an independent shortfall balance.

For recapitalization, transfer cash into the vault.
Do not mint shares.
Resume claim withdrawals only when the shortfall is zero.

### 15.3 Risk-capacity breach

An oracle price change does not change risk units.
A cash loss can reduce risk backing.
A new receiver liability can also reduce risk backing.

During a capacity breach, apply these restrictions:

- Reject new risk.
- Reject LP withdrawals.
- Permit risk-reduction closes.
- Permit liquidations.

Permit a deposit only if normal deposit rules pass.
The completed deposit must correct the capacity breach.

### 15.4 Oracle failure

Reject a position action if its market price is invalid or unavailable.

For LP settlement, require one synchronized canonical snapshot.
The snapshot must contain each active-market price.
Do not accept a separate NAV value.
Do not accept prices from different rounds.

If the assigned round is not current, expire the request.
Return the complete escrow.
A safety action must not wait behind an LP request.

### 15.5 Unexpected transfer

An unsolicited collateral transfer increases physical cash.
The transfer does not increase a non-LP claim.
Thus, the transfer increases LP residual equity.

An unexpected token loss can cause a shortfall.
Do not hide either event with a stored available-balance counter.

## 16. Rounding

Use these rounding directions:

| Quantity | Direction |
|---|---|
| Closing fee charged | Up, but not above payable price PnL |
| Borrow obligation | Up |
| Funding payer obligation | Up |
| Aggregate receiver liability | Down, with a carried remainder |
| Funding receiver credit | Down |
| LP deposit shares | Down |
| LP withdrawal assets | Down |
| Required risk backing | Up |
| Post-withdraw utilization | Up |
| Trader profit paid | Down |
| Trader loss collected | Up, but not above available value |
| Protocol share | Exact remainder after the other fee shares |
| Keeper share | Down |

Carry remainders for index divisions and flow divisions.
Position receiver credit must not exceed its payer allocation.

Use telescoping rules for these final operations:

- Final position close.
- Final market exposure removal.
- Clean final LP redemption.

Residual token dust belongs to LP cash equity.

## 17. Complexity limits

| Operation | Complexity |
|---|---:|
| Global fee checkpoint | O(1) |
| Market funding checkpoint | O(1) |
| Open, increase, decrease, or close | O(1) |
| Liquidation | O(1) |
| One ADL action | O(1) |
| LP request creation | O(1) |
| LP request settlement | O(active markets) |
| One market aggregate-PnL calculation | O(1) |

Do not loop through positions.
Do not loop through LP requests.
Do not store or trust an aggregate NAV.
Do not use a separate NAV oracle.
Do not use an onchain cross-market correlation matrix.
Do not use an onchain volatility estimator.

Each active-market price can change without a contract mutation.
Thus, a fresh multi-market NAV needs an active-market loop.
The governance bound makes this loop safe.

## 18. Required invariants

### 18.1 Cash ownership

In normal operation, preserve:

```text
physical_cash
    = cash_lp_equity
      + stored_position_collateral_total
      + pending_receiver_funding_total
      + execution_budget_total
      + protocol_claimable_total
      + risk_keeper_reserve_total
```

If claims exceed cash, cash LP equity is zero.
The derived shortfall equals the difference.

### 18.2 Aggregate conservation

For each position mutation, preserve equal aggregate changes.
Apply this rule to size, base exposure, collateral, and risk units.
Apply this rule to the execution budget.

After the final position closes, each related side aggregate must be zero.

### 18.3 Funding conservation

For each interval, preserve:

```text
receiver_backed_payer_flow
    + lp_backed_payer_flow
    = total_payer_flow

receiver_credit
    <= receiver_backed_payer_accrual
```

Receiver capitalization changes two non-LP labels.
It does not change complete non-LP claims.

### 18.4 Index correctness

Preserve these index properties:

- An index does not decrease.
- A same-time checkpoint has no effect.
- New size starts at the current baseline.
- New risk units start at the current baseline.
- A mutation does not change the accrual for past time.
- Stored remainders make split borrow intervals exact.
- The funding decay table bounds split-interval error to approximately
  1e-13 relative.
- An instant weight of `BPS` makes split funding intervals exact.

### 18.5 Risk capacity

Each risk increase must pass the global capacity gate.
Each risk increase must pass market-side size limits.
Each risk increase must pass market-side base-exposure limits.

Each withdrawal must pass the free-capital gate.
Each withdrawal must pass the post-withdraw utilization gate.

Risk units reduce free capital.
Risk units do not reduce cash LP equity.
Risk units do not reduce NAV.

### 18.6 Marked NAV

For one synchronized oracle round, preserve:

```text
vault_nav =
    max(
        cash_lp_equity
        - aggregate_recognized_trader_pnl,
        0
    )
```

Recognize positive trader PnL in full.
Do not recognize negative side PnL above side collateral.
Accept market prices only.
Do not accept a supplied NAV.

### 18.7 LP settlement

For each LP request, preserve:

```text
execute_after = request_time + lp_request_delay

settlement_round =
    first canonical round
    with timestamp >= execute_after
```

Only the FIFO head can resolve.
Settle the complete request against its assigned current round.
Otherwise, return the complete escrow.
Do not select a later round.

Use pre-transfer NAV and share supply.
A successful withdrawal must not create a persistent cash claim.

### 18.8 Safety

Preserve these safety properties:

- Accrued fees enter effective collateral.
- Stop new LP actions at a warning state.
- Pay ADL rewards only after a qualifying risk reduction.
- Pay ADL rewards only from the risk-keeper reserve.
- Do not burn the final LP share while trading risk exists.
- Do not mint shares during recapitalization.
- Keep `lp_blocked_side_count` equal to the number of restricted sides.
- Keep complete revenue shares at or below `BPS`.
- Set `lp_request_delay` to a nonzero value.
- Keep the sum of side hard-cap factors within the global limit.
- Keep the global hard-cap limit at or below `BPS`.

## 19. Implementation sequence

Implement and test the system in this order:

1. Implement residual cash ownership and explicit claims.
2. Implement position and market aggregates.
3. Implement the global borrow checkpoint and capacity gates.
4. Implement market funding indices and receiver claims.
5. Implement fee capitalization and position settlement.
6. Implement marked NAV and FIFO LP requests.
7. Implement warnings, liquidation, ADL, shortfall, and recapitalization.

Add the invariants for each layer before the next layer.

Test these conditions:

- A long time between checkpoints.
- Repeated checkpoints in one block.
- A dust receiver-side position.
- A partial close.
- A one-sided market.
- A book flip against the funding history.
- Stress in multiple markets.
- Receiver settlement before payer settlement.
- A delayed liquidation.
- An LP request near a large price change.
- A missed oracle round.
- A withdrawal at the capacity limit.
- Final dust removal.
