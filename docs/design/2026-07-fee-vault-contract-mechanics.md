# Vault, Fee, PnL, and LP Share Mechanics

## 1. Goal

This document derives a minimal contract design for a perpetual trading
vault.

The contract must:

- Hold all collateral in one token balance.
- Accrue borrow and funding over time.
- Preserve each claimant's ownership.
- Calculate trader PnL and LP share value.
- Prevent withdrawals from escaping known trader profit or consuming
  required risk backing.
- Keep normal trading operations independent of the number of
  positions.

The design stores only values that cannot be recovered safely from
other state. Physical cash, LP equity, free capital, pending position
fees, PnL, NAV, and share price are derived when needed.

## 2. Numerical model

```text
BPS = 10,000
SECONDS_PER_DAY = 86,400

PRICE_PRECISION = 10^30
INDEX_PRECISION = 10^30
RATE_PRECISION = 10^30
FACTOR_PRECISION = 10^30
SHARE_PRECISION = 10^18
```

The collateral asset is a non-rebasing USD token without transfer fees.
Cash amounts use its native decimals.

```text
ASSET_PRECISION = 10^collateral_decimals
ASSET_TO_SHARE_SCALE = SHARE_PRECISION / ASSET_PRECISION

VIRTUAL_ASSETS = 1
VIRTUAL_SHARES = ASSET_TO_SHARE_SCALE
```

Configured rates are stored as basis points per day. Intermediate rate
calculations preserve `RATE_PRECISION`; they must not round a fractional
rate to a whole basis point before accrual.

Multiplication precedes division through full-precision `mulDiv`.
Signed PnL is kept as a high-precision numerator until the final cash
conversion.

Every cumulative division that repeats over time carries its remainder.
This makes borrow accrual independent of checkpoint frequency. Funding
integrates a continuously decaying rate in closed form (§6.1); its decay
table quantizes at roughly 1e-13 relative, so checkpoint frequency
cannot move accrued funding beyond that tolerance. The equality is
exact only at `instant_weight_bps = BPS`, where the rate is again
piecewise constant.

## 3. Sources of truth

### 3.1 Physical cash

```text
physical_cash = collateral_token.balanceOf(vault)
```

This is the only authoritative cash balance.

The contract does not store an authoritative `available_cash`,
`lp_assets`, or `vault_balance`. Such counters drift after rounding,
donations, or unusual token transfers.

LP share supply is likewise read from the share token:

```text
share_supply = lp_share_token.totalSupply()
```

The vault does not maintain a second supply counter.

### 3.2 Explicit non-LP claims

The vault stores only ownership labels that cannot be derived from the
token balance:

```text
stored_position_collateral_total
pending_receiver_funding_total
execution_budget_total
protocol_claimable_total
risk_keeper_reserve_total
```

Their sum is:

```text
non_lp_claims =
    stored_position_collateral_total
    + pending_receiver_funding_total
    + execution_budget_total
    + protocol_claimable_total
    + risk_keeper_reserve_total
```

Each term represents a claim on the same physical tokens. None requires
a separate pool.

### 3.3 LP cash equity

```text
cash_lp_equity =
    if physical_cash >= non_lp_claims:
        physical_cash - non_lp_claims
    else:
        0
```

The uncompensated difference when `non_lp_claims > physical_cash` is an
explicit vault shortfall.

Because LP equity is residual, token donations automatically belong to
LPs and accounting dust cannot strand in a second authoritative
balance.

### 3.4 Risk backing and free capital

Risk units lock LP cash but do not reduce LP ownership:

```text
required_risk_backing =
    ceil(
        total_risk_units
        × BPS
        / risk_capacity_limit_bps
    )

free_lp_capital =
    max(cash_lp_equity - required_risk_backing, 0)
```

`cash_lp_equity`, marked NAV, and `free_lp_capital` are different
quantities and are never substituted for one another.

## 4. Minimal stored state

### 4.1 Global state

```text
stored_position_collateral_total
pending_receiver_funding_total
execution_budget_total
protocol_claimable_total
risk_keeper_reserve_total

total_risk_units
open_position_count

borrow_index
borrow_index_remainder
current_borrow_rate
last_global_checkpoint

next_lp_request_id
next_lp_request_to_resolve

active_market_registry
max_active_markets

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

The active market count has a governance hard bound. The registry is
used only when an LP action needs a synchronized marked NAV.

There is no global receiver-flow scalar. The guaranteed receiver
liability accrues per market inside the market checkpoint (§6.3).

`funding_half_life_seconds` is the one memory horizon shared by every
market's funding EMA, bounded to [60, 31,536,000] seconds.

### 4.2 Per-market state

For each side:

```text
size_open_interest
base_exposure
stored_collateral_total
risk_units
```

Each market also stores:

```text
receiver_backed_payer_index_long
receiver_backed_payer_index_short

lp_backed_payer_index_long
lp_backed_payer_index_short

receiver_index_long
receiver_index_short

skew_ema
funding_index_remainders
pending_receiver_remainder
last_funding_checkpoint

current_payer_side
current_payer_rate

close_fee_low_bps
close_fee_high_bps
max_funding_rate_bps_day
instant_weight_bps
market_risk_factor_bps

size_caps
base_exposure_caps

warning_pnl_factor_bps
adl_pnl_factor_bps
recovery_pnl_factor_bps
hard_cap_pnl_factor_bps
initial_margin_bps
maintenance_margin_bps
liquidation_reward_bps
adl_reward_bps
```

The aggregate size and base-exposure fields are sufficient for funding,
raw market-side PnL, and directional limits.

`skew_ema` is the market's stored funding history and
`pending_receiver_remainder` carries the sub-unit receiver-liability
accrual (§6.3). `current_payer_side` and `current_payer_rate` are
display state refreshed from the blended integral skew after every
checkpoint and mutation; accrual never reads them. There is no stored
receiver or LP flow rate.

The size caps must not exceed 10^16 and the base-exposure caps must
not exceed 10^18 — the i128 headroom the §6.2 window integral needs.

### 4.3 Per-position state

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

A position does not store a current fee, personal rate, current PnL, or
effective collateral. Those values are derived.

### 4.4 LP request state

```text
request_id
owner
kind
amount
request_time
execute_after
status
```

For a deposit, `amount` is collateral held in a request router outside
the vault. For a withdrawal, `amount` is LP shares held by the router.
Escrowed withdrawal shares remain in total supply until settlement.

No aggregate request ledger or withdrawal cash claim is stored.

### 4.5 Universal cash transition rule

Every token transfer and every ownership-label change must explain its
effect on residual LP equity:

| Event | Physical cash | Non-LP claims | LP cash equity |
|---|---:|---:|---:|
| Trader collateral deposit | increases | increases equally | unchanged |
| Execution-budget deposit | increases | increases equally | unchanged |
| Receiver funding accrual | unchanged | increases | decreases |
| Receiver claim becomes position collateral | unchanged | unchanged in total | unchanged |
| LP-backed fee is collected | unchanged | position collateral decreases | increases |
| Protocol or keeper fee is collected | unchanged | label moves between claims | unchanged |
| Non-LP claim is withdrawn | decreases | decreases equally | unchanged |
| LP deposit | increases | unchanged | increases |
| LP withdrawal | decreases | unchanged | decreases |
| Trader realizes profit | decreases beyond removed collateral | decreases by collateral | decreases by profit |
| Trader realizes loss | decreases by less than removed collateral | decreases by collateral | increases by loss |

This table is the accounting test for every state transition. A label
must never create cash, and a cash transfer must never be left without
an ownership effect.

## 5. Aggregate price exposure

### 5.1 Creating base exposure

For an increase at execution price:

```text
base_added =
    floor(
        size_added
        × PRICE_PRECISION
        / execution_price
    )
```

The position and its market side increase by the same `size_added` and
`base_added`.

On a partial close, reductions telescope:

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

The last close removes the complete remainder. Repeated partial closes
therefore cannot strand base exposure.

Risk units use the same remaining-value pattern.

### 5.2 Raw PnL

At mark price `p`:

```text
long_raw_pnl_numerator =
    long_base_exposure × p
    - long_size_open_interest × PRICE_PRECISION

short_raw_pnl_numerator =
    short_size_open_interest × PRICE_PRECISION
    - short_base_exposure × p
```

These equal the sum of raw position PnL on the side, apart from the
explicit aggregate-to-cash rounding step.

### 5.3 Recognized PnL for LP shares

For each side:

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

Trader profit is recognized in full because the vault may owe it.
Trader loss is not recognized beyond aggregate stored collateral
because that excess is not collectible.

For one synchronized price snapshot:

```text
aggregate_recognized_trader_pnl_numerator =
    sum(recognized_pnl_numerator for every active side)

vault_nav_numerator =
    cash_lp_equity × PRICE_PRECISION
    - aggregate_recognized_trader_pnl_numerator

vault_nav =
    floor(max(vault_nav_numerator, 0) / PRICE_PRECISION)
```

The sum loops over the bounded active-market registry, never positions.

### 5.4 What NAV does and does not recognize

Marked NAV includes:

- Residual LP cash.
- Unrealized and realized trader price PnL.
- Collected LP fee revenue.
- Collected LP-backed funding.
- Receiver claims already deducted through `non_lp_claims`.
- Recognized bad debt and direct token donations.

It excludes:

- Uncollected borrow fees.
- Uncollected payer funding.
- Hypothetical future fees.

This is intentionally asymmetric. Recognizing a receivable before
collection could let an LP withdraw cash that an insolvent trader never
pays.

The aggregate loss cap is not an exact proof of every position's
solvency. Individual collateral limits are nonlinear and cannot be
reconstructed from a few market sums. Timely liquidation remains
necessary.

## 6. Funding mechanics

### 6.1 Integral skew and rate

Funding prices net directional imbalance blended with its own history,
so a book flip is charged gradually rather than instantly repricing the
payer side.

For nonzero total base exposure, the signed skew is a fraction of one
at `INDEX_PRECISION` scale (zero on an empty book):

```text
S =
    (long_base - short_base)
    × INDEX_PRECISION
    / (long_base + short_base)
```

Each market stores a skew EMA `E`, initialized to zero, decaying toward
`S` with the global half-life `H = funding_half_life_seconds`:

```text
E(t) = S + (E₀ - S) × 2^(−t/H)
```

The blended integral skew mixes the instant skew and the EMA with the
per-market `instant_weight_bps` `w`:

```text
I = (w × S + (BPS − w) × E) / BPS
```

`w = BPS` reproduces the pure instant skew. The rate is quadratic in
the blend:

```text
payer_rate_bps_day =
    max_funding_rate_bps_day × I²
```

with `I` read as a signed fraction of one; the sign never changes the
rate. The payer for a checkpoint window is the side the sign of
`∫ I dt` over that window points at — after a book flip this can be the
*lighter* side for a while, and a trader who balances the book keeps
receiving while the history fades.

### 6.2 Window integral and funding indices

Between two checkpoints the book is constant, so the blend has the
closed form `I(t) = A + B·d(t)` with:

```text
d(t) = 2^(−t/H)
A = S
B = (BPS − w) × (E₀ − S) / BPS
```

The window weight — the exact integral of the rate over the elapsed
`Δt` — is:

```text
d  = d(Δt)
J₁ = (H / ln 2) × (1 − d)
J₂ = (H / (2 ln 2)) × (1 − d²)

W = ∫ rate dt =
    max_funding_rate_bps_day
    × (A²·Δt + 2AB·J₁ + B²·J₂)
```

`d` is computed by a 47-entry square-and-multiply table of
`2^(−2^(−i))` constants that quantizes at roughly 1e-13 relative.
Dividing `W` by `BPS × SECONDS_PER_DAY` yields the index delta per unit
of payer-side size.

The receiver side absorbs the share of the payer flow its
counter-exposure matches, capped at the whole flow:

```text
receiver_share = min(1, receiver_base / payer_base)

W_receiver = W × receiver_share
W_lp       = W − W_receiver
```

Because the payer can be the lighter side, `receiver_share` would
otherwise exceed one; the cap is what keeps `W_lp` non-negative and the
LP-backed index monotone. If the receiver side has zero size or zero
base exposure, `W_receiver` is zero and the complete collected payer
flow is LP-backed.

The payer side has two payer indices:

```text
receiver_backed_payer_index_delta =
    W_receiver / (BPS × SECONDS_PER_DAY)

lp_backed_payer_index_delta =
    W_lp / (BPS × SECONDS_PER_DAY)
```

The receiver side has one receiver index, derived from the exact amount
the payer index will collect:

```text
receiver_cash =
    payer_size × receiver_backed_payer_index_delta

receiver_index_delta =
    receiver_cash / receiver_size
```

All divisions carry remainders.

Payer index increments round up at the position-obligation boundary.
Receiver credits round down. Aggregate receiver credit must never
exceed aggregate receiver-backed payer accrual.

### 6.3 Guaranteed receiver liability

The liability accrues per market, inside the market checkpoint and
atomically with the indices:

```text
pending_receiver_funding_total +=
    floor(
        payer_size
        × receiver_backed_payer_index_delta
        / INDEX_PRECISION
    )
```

The sub-unit part is carried in the market's
`pending_receiver_remainder`. This is the authoritative vault liability
and reduces cash LP equity immediately. Because the liability and the
receiver credits derive from the same exact payer collection, a
position's credit can never outrun its market's contribution to the
liability.

There is no global receiver-flow scalar and the global checkpoint does
not touch this total.

When receiver funding is capitalized into a position:

```text
pending_receiver_funding_total -= receiver_credit
stored_position_collateral_total += receiver_credit
position.stored_collateral += receiver_credit
```

Total non-LP claims do not change. Ownership merely moves from an
unassigned funding claim to position collateral.

When a payer's receiver-backed fee is collected:

```text
position collateral decreases
stored_position_collateral_total decreases
```

No new receiver claim is created because it was already recorded during
accrual. The reduction restores the LP cash that provisionally backed
the receiver.

If the payer cannot pay, the uncollected difference is LP bad debt; the
receiver claim is not reversed.

After the final position closes, `open_position_count` alone decides
the release: aggregate conservation makes every market size zero, so no
receiver can remain and any unassigned receiver-rounding residue is
released to LP residual cash without a market loop. A market whose book
empties also drops its carried funding remainders.

Because the liability accrues per market, LP pricing checkpoints every
active market (bounded by `max_active_markets`) so it sees exact
liabilities; entry points that touch no market tolerate keeper-cadence
staleness in the total.

## 7. Borrow mechanics

### 7.1 Creating risk units

```text
risk_units_added =
    floor(
        size_added
        × market_risk_factor_bps
        / BPS
    )
```

The position, market, and global risk-unit totals change by the same
amount.

Before accepting new risk:

```text
total_risk_units_after
    <= cash_lp_equity_after
       × risk_capacity_limit_bps
       / BPS
```

The action must also remain within its market-side size and
base-exposure caps and below its warning PnL factor.

### 7.2 Global borrow rate

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

borrow_rate_bps_day =
    base_borrow_rate_bps_day
    + max_variable_borrow_rate_bps_day
      × utilization_bps²
      / BPS²
```

At a checkpoint:

```text
borrow_index_delta =
    borrow_rate_bps_day
    × INDEX_PRECISION
    × elapsed_time
    / (BPS × SECONDS_PER_DAY)
```

The stored rate applies to the elapsed interval. The newly calculated
rate applies only to future time.

For a position:

```text
borrow_index_value =
    ceil(position.risk_units × borrow_index / INDEX_PRECISION)

pending_borrow =
    borrow_index_value - position.borrow_debt
```

## 8. Checkpointing

### 8.1 Global checkpoint

For `now > last_global_checkpoint`:

1. Advance the borrow index with the stored borrow rate.
2. Carry its division remainder.
3. Set `last_global_checkpoint = now`.

For the same timestamp, it is a no-op. The global checkpoint advances
borrow only; every funding quantity, including the guaranteed receiver
liability, accrues in the market checkpoint.

### 8.2 Market checkpoint

For `now > market.last_funding_checkpoint`:

1. Resolve the funding window with the §6.2 closed-form integral.
2. Select the window's payer side from the sign of `∫ I dt`.
3. Split the window weight into its receiver-backed and LP-backed
   parts.
4. Advance the payer side's receiver-backed and LP-backed indices.
5. Advance the receiver side's receiver index.
6. Accrue the guaranteed receiver liability from the same payer
   collection.
7. Carry all remainders.
8. Advance the skew EMA to the window end.
9. Refresh the displayed payer side and rate from the integral skew.
10. Set the market checkpoint timestamp to `now`.

### 8.3 Mutation order

Every action that changes exposure, claims, risk units, or a rate input
uses:

```text
1. checkpoint global
2. checkpoint affected market
3. capitalize affected position fees
4. apply the requested mutation
5. refresh the market's displayed payer side and rate
6. re-evaluate the market's risk states
7. derive the new global borrow rate
```

An LP settlement checkpoints global accrual and every active market —
LP pricing must see exact receiver liabilities (§6.3) — then calculates
marked NAV, applies the LP cash/share mutation, and recomputes the
borrow rate. Entry points that touch no market checkpoint only global
state and tolerate keeper-cadence staleness in the liability total.

An operational pause does not change either accrual clock. Positions
continue to pay or receive until they are settled.

A parameter update first checkpoints every index directly affected by
that parameter. A market funding parameter update touches that market;
a global borrow parameter update touches the global index. New values
never reprice past time.

## 9. Position fee accounting

### 9.1 Pending amounts

For the position's direction:

```text
pending_paid_to_receivers =
    ceil(size × receiver_backed_payer_index / INDEX_PRECISION)
    - funding_paid_to_receivers_debt

pending_paid_to_lps =
    ceil(size × lp_backed_payer_index / INDEX_PRECISION)
    - funding_paid_to_lps_debt

pending_received =
    floor(size × receiver_index / INDEX_PRECISION)
    - funding_received_debt
```

Borrow uses risk units as shown earlier.

Negative results are forbidden; they indicate broken baseline or index
monotonicity.

### 9.2 Effective collateral

```text
effective_collateral =
    stored_collateral
    + pending_received
    - pending_paid_to_receivers
    - pending_paid_to_lps
    - pending_borrow
```

Margin checks use effective collateral, not the stale stored amount.

### 9.3 Capitalization

Before leaving a changed position open:

1. Add `pending_received` to stored collateral and move the same amount
   from pending receiver claims.
2. Collect receiver-backed payer funding from stored collateral.
3. Collect LP-backed payer funding from any remaining collateral.
4. Collect borrow from any remaining collateral and split it.
5. Reset all four debt baselines to current index-derived values.

Collateral supplied by the trader in the same action is available for
this waterfall. Positive PnL realized by a decrease is also available
before any remainder is paid to the trader.

Amounts collectible from a position are limited by its available
value. This fixed priority minimizes the shortfall against guaranteed
receiver claims. Any unpaid receiver-backed amount is bad debt. No
uncollected borrow or LP-backed funding is recorded as revenue.

A mutation that would leave any accrued obligation unpaid cannot leave
the position open. It must add collateral, reduce enough profitable
exposure to pay the obligation, or use the insolvent full-close path.
This permits every surviving position to reset its baselines without
forgiving debt or storing another unpaid-fee ledger.

Collected closing and borrow fees are split:

```text
lp_amount =
    floor(collected × lp_revenue_share_bps / BPS)

keeper_amount =
    floor(collected × risk_keeper_revenue_share_bps / BPS)

protocol_amount =
    collected - lp_amount - keeper_amount
```

Only `keeper_amount` and `protocol_amount` increase explicit non-LP
claims. The LP amount remains in residual cash equity.

### 9.4 Partial close

After checkpointing, derive the remaining quantities:

```text
remaining_size = old_size - size_removed

remaining_base =
    floor(old_base × remaining_size / old_size)

remaining_risk =
    floor(old_risk × remaining_size / old_size)

base_removed = old_base - remaining_base
risk_removed = old_risk - remaining_risk
```

Settle pending fees and realized PnL through the close waterfall. If the
position remains open, every accrued obligation must be paid and its
debts are reset from `remaining_size`, `remaining_risk`, and current
indices. The closed portion carries no historical debt forward.

## 10. Position actions

### 10.1 Open or increase

The action order is:

1. Authenticate a current canonical market price.
2. Run global and market checkpoints.
3. Authenticate and transfer any collateral supplied with the action.
4. Calculate the existing position's pending fees and funding credit.
5. Derive `base_added` and `risk_units_added`.
6. Settle old obligations from old collateral, funding credit, and
   newly supplied collateral.
7. Check resulting collateral and the applicable margin.
8. Check global risk capacity, market-side caps, and the warning factor.
9. Increase position and market aggregates.
10. Set debt baselines so new size starts at current indices.
11. Refresh the displayed payer side/rate and the borrow rate.

Nothing is charged at open or increase. The complete supplied
collateral becomes stored position collateral.

The margin gate depends on what the action does to leverage: an open,
and an increase that adds size, must satisfy the initial margin; an
increase that only adds collateral de-risks and must clear just the
maintenance floor (§10.3).

The transfer, claim-label changes, and residual LP revenue must reconcile
through the universal cash transition rule.

### 10.2 Decrease or close

The action order is:

1. Authenticate the current canonical market price.
2. Checkpoint global and market accrual.
3. Derive the removed size, base exposure, and risk units.
4. Calculate raw PnL for the removed exposure.
5. Apply any emergency side payout factor to positive PnL.
6. Calculate every pending fee and funding credit.
7. Settle credits, obligations, and realized PnL through the fixed
   waterfall.
8. Collect the closing fee from realized positive price PnL.
9. Transfer the remaining realized profit on a partial close.
10. Apply any explicit collateral withdrawal.
11. Require a remaining position to satisfy the applicable margin.
12. Reduce position and market aggregates.
13. Reset the remaining position's debts, or delete a complete close.
14. Refresh the displayed payer side/rate and the borrow rate.

For removed exposure:

```text
if long:
    raw_pnl_numerator =
        base_removed × mark_price
        - size_removed × PRICE_PRECISION

if short:
    raw_pnl_numerator =
        size_removed × PRICE_PRECISION
        - base_removed × mark_price
```

Convert to cash once with the directional rounding rule. If an
emergency factor applies:

```text
if raw_price_pnl > 0:
    payable_price_pnl =
        floor(raw_price_pnl × payout_factor)
else:
    payable_price_pnl = raw_price_pnl
```

For a partial close:

- Funding received and positive payable PnL first provide value from
  which accrued obligations can be collected.
- The closing fee is collected after every obligation.
- The realized profit transferred to the trader is
  `min(payable_price_pnl - closing_fee, stored_collateral)`; the
  transfer reduces LP cash equity.
- After guaranteed receiver-backed obligations are paid, negative PnL
  reduces remaining stored collateral and increases LP cash equity by
  the amount collected.
- If the loss exceeds complete position collateral, the action becomes
  an insolvent full close rather than leaving an undercollateralized
  remainder.
- `collateral_withdrawn` is optional, occurs after PnL, and cannot make
  the remaining position unhealthy.
- A decrease that removes size de-risks and is held to the maintenance
  margin; a pure collateral withdrawal raises leverage and must leave
  the position back above the initial margin (§10.3).

For a full close:

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

trader_payout = max(close_equity - collected_closing_fee, 0)
bad_debt      = max(-close_equity, 0)
```

Available value is distributed in this order:

1. Receiver-backed funding.
2. Negative price PnL owed to LPs, when present.
3. LP-backed funding.
4. Borrow.
5. The closing fee.
6. The trader's remaining equity.

Positive price PnL adds to the value available for the waterfall.
Unpaid LP-backed funding and borrow are not booked as revenue.

Removing the stored collateral claim and transferring
`trader_payout` makes the difference flow automatically to or from LP
residual equity. Bad debt is emitted for risk accounting; it is not
stored as a fictitious receivable.

Every close path — trader decrease or close, liquidation, ADL, and
triggered order — charges the closing fee through this same
settlement:

```text
closing_fee =
    min(
        ceil(size_removed × fee_bps / BPS),
        payable_price_pnl
    )
```

The tier is computed on the book the close leaves behind: the low
`close_fee_low_bps` when removing the exposure improves or preserves
normalized base-exposure skew, the high `close_fee_high_bps` when it
worsens it. The fee only ever comes out of realized positive price PnL
after the hard cap, ranked below every accrued obligation — losers pay
zero and no shortfall path exists. The collected fee is split through
the standard revenue split under its own closing-fee source, and every
close event carries the amount.

### 10.3 Margins and liquidation

Each market configures two margin rates, validated as
`0 < maintenance_margin_bps <= initial_margin_bps <= BPS`, with the
requirement for a position of `size`:

```text
margin_requirement =
    ceil(size × margin_bps / BPS)
```

The initial margin gates every leverage-increasing action: an open, an
increase that adds size, and a pure collateral withdrawal. The
maintenance margin gates de-risking checks and liquidation: a pure
collateral top-up, a partial close that removes size, and the
liquidation test itself.

The gap between the two is the trader's guaranteed entry buffer. Max
leverage (displayed as `floor(BPS / initial_margin_bps)`) no longer
sits on the liquidation boundary: a freshly opened position must lose
the buffer before it can be liquidated.

Liquidation uses the same preparation and settlement accounting as a
close. It is permitted when effective collateral plus current payable
price PnL fails the maintenance requirement.

The liquidation reward:

- Is capped by configuration.
- Cannot exceed value actually available for it.
- Comes from the liquidated position after guaranteed fees and the
  closing fee, and before any residual trader payout.
- Is accounted separately from trader payout and LP PnL.

An insolvent-position touch may receive a capped reward from the
risk-keeper reserve because revealing bad debt improves the vault's
accounting state.

### 10.4 Execution budgets

Execution budget is transferred in separately and recorded in both the
position and `execution_budget_total`.

When paid:

```text
physical cash decreases
position.execution_budget decreases
execution_budget_total decreases
```

LP cash equity is unchanged. Insufficient execution budget may prevent
the optional order execution, but it must not corrupt the position
settlement.

## 11. LP share mechanics

### 11.1 Price source

The vault uses the canonical oracle prices already used for:

- Opening and closing positions.
- Trader PnL.
- Margin and liquidation.

There is no separately governed NAV price.

For an LP settlement, the oracle adapter supplies one authenticated,
synchronized snapshot containing every active market price and a unique
monotonic round identifier. The vault derives NAV onchain from those
prices and its own aggregates.

### 11.2 Why requests are delayed

An immediate withdrawal would let an LP act on price information before
the oracle marks the vault's trader liability.

Every LP action therefore begins as an escrowed request:

```text
execute_after = request_time + lp_request_delay

settlement_round =
    first canonical synchronized oracle round
    with timestamp >= execute_after
```

This mapping is unique and verifiable from oracle round identifiers.
The assigned round satisfies:

```text
settlement_round.timestamp >= execute_after
previous_round.timestamp < execute_after
```

Neither the requester nor executor supplies an arbitrary round. The
oracle adapter must expose enough authenticated round history to verify
the predecessor condition.

The request is valid only while `settlement_round` is the current
canonical round accepted by the vault. If a later round is accepted
first, the request expires and its full escrow becomes returnable. It
cannot fall forward to a more favorable round.

Any account may execute or expire a mature request. A production
deployment must operate a keeper for liveness.

### 11.3 FIFO resolution

Only `next_lp_request_to_resolve` may settle or expire. After resolution,
the pointer advances by one.

This gives deterministic priority when two withdrawals compete for the
same free capital. It also prevents a requester or block builder from
choosing the economically favorable member of a queue.

Each request is full-or-zero:

- A passing deposit transfers all escrowed assets and mints all shares.
- A passing withdrawal burns all escrowed shares and transfers all
  assets atomically.
- A failed or expired request returns its complete escrow.

There are no partial fills or persistent withdrawal cash claims.

Resolution marks the request complete and advances the FIFO pointer
before performing an external token transfer.

FIFO can cause head-of-line delay. That is the accepted cost of simple,
deterministic scarcity allocation. An expired or failing head can be
resolved permissionlessly, so it cannot block the queue permanently.

### 11.4 Share conversion

Using the pre-transfer state:

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

The share price shown to users is:

```text
vault_nav / share_supply
```

The virtual quantities are used for executable conversion, not reported
as owned assets or shares.

### 11.5 Deposit settlement

At the assigned settlement round:

1. Checkpoint global accrual to the current settlement time.
2. Validate a synchronized price for every active market.
3. Derive physical cash, non-LP claims, cash LP equity, and marked NAV.
4. Reject a shortfall, warning/ADL state, or invalid oracle snapshot.
5. Check deposit eligibility.
6. Calculate shares from the pre-deposit NAV and supply.
7. Transfer escrowed collateral into the vault.
8. Mint shares to the owner.
9. Recompute the borrow rate for future time.
10. Mark the request settled and advance FIFO.

A clean first deposit requires:

```text
physical_cash = 0
non_lp_claims = 0
total_risk_units = 0
open_position_count = 0
share_supply = 0
```

Every later ordinary deposit requires:

```text
cash_lp_equity > 0
vault_nav > 0

vault_nav × BPS / cash_lp_equity
    >= min_deposit_nav_factor_bps
```

If NAV is too low, additional capital must enter through an explicit
recapitalization path that mints no shares. This prevents share
hyperinflation and dilution of the existing cohort.

### 11.6 Withdrawal settlement

Using pre-withdraw NAV:

1. Calculate `withdrawal_assets`.
2. Require `withdrawal_assets <= free_lp_capital`.
3. Calculate post-withdraw cash LP equity.
4. Require post-withdraw utilization not above its configured maximum.
5. Require no warning, ADL, shortfall, or invalid oracle state.
6. Burn the escrowed shares.
7. Transfer collateral directly to the owner.
8. Recompute the borrow rate.
9. Mark the request settled and advance FIFO.

```text
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

If the checks fail, the complete share escrow is returned. The request
does not wait as a cash claim.

The complete share supply cannot be burned while any open position,
risk unit, receiver claim, or execution budget remains.

In a clean terminal state, the final LP may receive all
`cash_lp_equity`. This removes residual dust that virtual quantities
would otherwise leave ownerless.

### 11.7 Effect of open trader profit

Suppose a trader has an unrealized profit and has not closed. That
profit is already positive `recognized_trader_pnl`, so it is subtracted
from marked vault NAV.

An LP request made before the trader closes settles from a future oracle
round. At that round:

- If the profit still exists, the LP receives the lower marked value.
- If the trader already closed, physical cash has fallen by the payout
  and the corresponding unrealized PnL has disappeared.

The two paths produce the same economic result apart from defined
rounding. Closing time therefore does not let an LP escape a known
trader win.

The remaining risk is undiscovered individual insolvency, not ordinary
unrealized profit. LP settlement freezes during recognized emergency
states, and keepers are paid to liquidate or reveal insolvency.

## 12. Emergency payout and ADL

For each profitable market side:

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

The contract enforces:

```text
recovery_factor
    < warning_factor
    < adl_factor
    < hard_cap_factor
```

At warning:

- New risk on the affected side stops.
- New LP requests and LP settlement stop.

At ADL:

- A permissionless or keeper action may reduce a profitable position.
- The action must demonstrably reduce the affected side's positive PnL.
- A capped reward is paid from `risk_keeper_reserve_total`.

At the hard cap, profitable settlement uses a side-level payout factor:

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

Every profitable position on that side uses the same factor at the same
snapshot. Losses remain fully payable up to position value.

Ordinary LP operations resume only when all sides fall below their
recovery factors.

The configured sum of all side hard-cap factors must not exceed a global
limit. This bounds simultaneous promises against one vault without
pretending that markets have separate cash pools.

## 13. Failure states

### 13.1 Zero LP equity

If:

```text
physical_cash = non_lp_claims
```

then cash LP equity is zero. The claims are still fully backed and their
normal withdrawal paths remain valid. LP deposits and withdrawals stop.

### 13.2 Cash shortfall

If:

```text
physical_cash < non_lp_claims
```

then:

```text
cash_shortfall = non_lp_claims - physical_cash
```

The vault freezes ordinary outgoing claims and LP actions. Risk-reducing
position actions remain available when they cannot worsen the
shortfall.

The shortfall is derived rather than stored as an independent balance.
Recapitalization transfers cash to the vault and mints no shares. Normal
claim withdrawals resume only after the derived shortfall is zero.

### 13.3 Risk-capacity breach

Price movement does not change risk units, but cash losses or new
receiver liabilities can reduce their backing.

During a breach:

- New risk is rejected.
- LP withdrawals are rejected.
- Risk-reducing closes and liquidations remain available.
- Deposits may proceed only if ordinary deposit rules pass and the
  completed deposit cures the breach.

### 13.4 Oracle failure

A position action fails when its required market price is unavailable
or invalid.

An LP settlement fails unless one synchronized canonical snapshot
contains every active market. The request cannot substitute a separate
NAV value or a different price round.

If the request's assigned round is no longer current, it expires and
returns escrow. Safety actions must not wait behind an LP request.

### 13.5 Unexpected transfers

An unsolicited collateral transfer increases `physical_cash` without
increasing a non-LP claim. It therefore becomes LP residual equity.

Unexpected token loss may create a shortfall. The contract does not hide
either event behind a stored available-balance counter.

## 14. Rounding

Rounding follows ownership and solvency:

| Quantity | Direction |
|---|---|
| Closing fee charged | Up, capped at payable price PnL |
| Borrow obligation | Up |
| Funding payer obligation | Up |
| Aggregate receiver liability | Down with remainder carry |
| Funding receiver credit | Down |
| LP deposit shares | Down |
| LP withdrawal assets | Down |
| Required risk backing | Up |
| Post-withdraw utilization | Up |
| Trader profit paid | Down |
| Trader loss collected | Up, limited by available value |
| Protocol and keeper split shares | Down; protocol receives exact remainder |

Index and flow divisions carry remainders. Position-level receiver
rounding may never exceed its backing payer allocation. Final position
close, final market exposure removal, and clean final LP redemption
consume stored remainders through explicit telescoping rules.

Residual token dust belongs to LP cash equity.

## 15. Complexity boundary

| Operation | Complexity |
|---|---:|
| Global fee checkpoint | O(1) |
| Market funding checkpoint | O(1) |
| Open, increase, decrease, close | O(1) |
| Liquidation or one ADL action | O(1) |
| Create or resolve one LP request | O(active markets) for settlement NAV |
| Calculate one market's aggregate PnL | O(1) |

Request creation itself is O(1); request settlement performs the bounded
market loop.

There is:

- No loop over positions.
- No loop over LP requests.
- No stored or trusted aggregate NAV.
- No separate NAV oracle.
- No onchain cross-market correlation matrix.
- No onchain volatility estimator.

A fresh multi-market marked NAV cannot be strict O(1) without trusting
an externally supplied aggregate because every market price may change
without a contract mutation. The bounded market loop is the explicit,
minimal exception.

## 16. Required invariants

### 16.1 Cash ownership

In normal operation:

```text
physical_cash
    = cash_lp_equity
      + stored_position_collateral_total
      + pending_receiver_funding_total
      + execution_budget_total
      + protocol_claimable_total
      + risk_keeper_reserve_total
```

When claims exceed cash, the derived cash shortfall equals the
difference and LP cash equity is zero.

### 16.2 Aggregate conservation

For every position mutation, its size, base exposure, collateral, risk
units, and execution budget change by exactly the same amount as the
corresponding market or global aggregate.

Closing the final position on a side leaves every side aggregate at
zero.

### 16.3 Funding conservation

For every interval:

```text
receiver_backed_payer_flow
    + lp_backed_payer_flow
    = total_payer_flow

receiver_credit
    <= receiver_backed_payer_accrual
```

A receiver capitalization moves ownership between two non-LP labels and
does not change total claims.

### 16.4 Index correctness

- Indices never decrease.
- A same-timestamp checkpoint is idempotent.
- New size and risk units begin at current baselines.
- A mutation never changes the accrual for already elapsed time.
- Splitting a borrow interval into checkpoints produces the same
  accrual, subject only to the stored remainder.
- Splitting a funding interval agrees within the decay table's
  quantization (roughly 1e-13 relative); it is exact only at
  `instant_weight_bps = BPS`.

### 16.5 Risk capacity

Every risk-increasing action satisfies the global capacity gate and its
market-side size and base-exposure caps.

Every successful withdrawal satisfies free-capital and post-withdraw
utilization gates.

Risk units reduce free capital but not cash LP equity or NAV.

### 16.6 Marked NAV

For one synchronized oracle round:

```text
vault_nav =
    max(
        cash_lp_equity
        - aggregate_recognized_trader_pnl,
        0
    )
```

Positive trader PnL is recognized in full. Negative side PnL is not
recognized beyond that side's stored collateral.

The vault accepts market prices, never a supplied NAV.

### 16.7 LP settlement

For every request:

```text
execute_after = request_time + lp_request_delay

settlement_round =
    first canonical round
    with timestamp >= execute_after
```

Only the FIFO head may resolve. It either settles completely against its
assigned current round or returns complete escrow. A later round cannot
be selected.

Share conversion uses pre-transfer NAV and supply. No successful
withdrawal creates a persistent cash claim.

### 16.8 Safety

- Accrued fees enter effective collateral.
- New LP actions stop at warning or worse.
- ADL rewards come only from the keeper reserve and only after a
  qualifying risk reduction.
- The final LP share cannot burn while trading risk remains.
- Recapitalization never mints shares.
- Revenue-share parameters sum to no more than `BPS`.
- `lp_request_delay` is nonzero.
- The sum of side hard-cap factors does not exceed
  `global_hard_cap_factor_limit_bps`, which does not exceed `BPS`.

## 17. Implementation sequence

The implementation can proceed in seven independently testable layers:

1. Residual cash ownership from `balanceOf` and explicit claims.
2. Position and market aggregates for size, base exposure, collateral,
   and risk units.
3. Global borrow checkpointing and capacity gates.
4. Per-market funding windows, indices, and guaranteed receiver claims.
5. Fee capitalization, partial closes, and complete settlement.
6. Marked NAV plus deterministic FIFO LP requests.
7. Warning, liquidation, ADL, cash-shortfall, and recapitalization
   states.

Each layer must add its invariants before the next layer depends on it.
The final system should be tested with long time gaps, repeated
same-block checkpoints, dust receiver-side exposure, book flips against
the funding EMA, partial closes, one-sided markets, simultaneous market
stress, receiver settlement before payer settlement, delayed
liquidation, LP requests around price moves, missed oracle rounds,
capacity-bound withdrawals, and terminal dust cleanup.
