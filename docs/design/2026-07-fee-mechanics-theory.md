# Fee Mechanics — Economic Theory

## 1. Purpose

A perpetual market gives traders leveraged price exposure. A vault
supplies the cash used to pay profitable traders, and liquidity
providers own the vault's residual value.

The fee system has four jobs:

1. Charge realized trading profit when exposure closes.
2. Charge over time for consuming vault capacity.
3. Make the payer side of a market — the side the time-blended skew
   points at — pay for directional imbalance.
4. Keep execution and safety rewards separate from LP-owned cash.

The design favors a small number of explicit rules. It uses one physical
vault balance, cumulative indices for time-based fees, and aggregate
market state. It does not require separate cash pools or iteration over
positions.

## 2. Basic quantities

```text
BPS = 10,000
SECONDS_PER_DAY = 86,400
```

All configured rates are basis points per day. For a fee base `x`:

```text
fee =
    x
    × rate_bps_day
    × elapsed_seconds
    / (BPS × SECONDS_PER_DAY)
```

Each position stores two exposure quantities:

```text
size
    USD notional at entry

base_exposure
    quantity of the underlying asset represented by the position
```

USD size is stable and is therefore a simple funding fee base. Base
exposure is the directional quantity. At one common mark price, it
determines the market's current long and short exposure.

For each market and side, the system aggregates:

```text
size_open_interest
base_exposure
stored_collateral
```

The aggregates make market-level accounting independent of the number
of positions.

## 3. Closing fee

A closing fee applies only when exposure is removed — a trader decrease
or close, a liquidation, an ADL action, or a triggered order. Opens and
increases carry no position fee; the full amount a trader supplies
becomes stored collateral.

The fee has two tiers, judged on the book the close leaves behind. A
removal receives the low tier when it improves or preserves normalized
directional balance, and the high tier when it makes the balance worse.

```text
skew(base_long, base_short) =
    if base_long + base_short = 0:
        0
    else:
        |base_long - base_short|
        × BPS
        / (base_long + base_short)

skew_after = skew of the book minus the removed exposure

if skew_after <= skew_before:
    fee_bps = close_fee_low_bps
else:
    fee_bps = close_fee_high_bps

closing_fee =
    min(size_removed × fee_bps / BPS, payable_price_pnl)
```

The fee only ever comes out of realized positive price PnL, after any
emergency payout cap. A close without realized profit pays nothing, so
the fee can never create a shortfall or deepen bad debt.

Base exposure, rather than entry-time USD open interest, is used for the
comparison. This makes the closing incentive point in the same direction
as funding.

The tier boundary is intentionally discrete. A continuous impact curve
could be more precise, but it would introduce another curve and more
path-dependent quoting. The two-tier rule is the simpler policy.

## 4. Funding

### 4.1 Purpose and payer

Funding prices directional imbalance, blended with its own history so a
book flip is charged gradually rather than instantly repricing the
payer side.

The signed skew is a fraction of one:

```text
S =
    (long_base_exposure - short_base_exposure)
    / (long_base_exposure + short_base_exposure)
```

It is zero on an empty or balanced book and ±1 for a one-sided market.

Each market keeps an exponential moving average `E` of the signed skew.
It starts at zero and decays toward `S` with a global half-life `H`:

```text
E(t) = S + (E₀ - S) × 2^(−t/H)
```

The blended integral skew mixes the two with a per-market instant
weight `w` (in bps):

```text
I = (w × S + (BPS − w) × E) / BPS
```

`w = BPS` reproduces the pure instant skew. The payer over a checkpoint
window is the side the sign of `∫ I dt` points at. After a book flip
the payer can be the *lighter* side for a while: a trader who balances
the book keeps receiving while the history fades, which is exactly the
incentive the blend is meant to create.

### 4.2 Quadratic rate

The payer side pays:

```text
payer_rate_bps_day =
    max_funding_rate_bps_day × I²
```

with `I` read as a signed fraction of one; the sign selects the payer
and never changes the magnitude of the charge.

The quadratic curve is deliberate:

- Small imbalances remain inexpensive.
- The charge accelerates as imbalance grows.
- A persistently one-sided market converges to the configured ceiling.

Every existing position on the payer side pays the current rate. The
rate is not piecewise constant: between actions it moves continuously
as the EMA converges, and accrual integrates that motion exactly.

### 4.3 Dividing the payer flow

Receiver-side traders offset part of the payer exposure. LPs bear the
unmatched remainder. The funding flow is divided by that
counter-exposure:

```text
payer_flow_per_day =
    payer_size_open_interest
    × payer_rate_bps_day
    / BPS

counterparty_share =
    min(1, receiver_base_exposure / payer_base_exposure)

trader_receiver_flow_per_day =
    payer_flow_per_day × counterparty_share

lp_funding_flow_per_day =
    payer_flow_per_day - trader_receiver_flow_per_day
```

Under the EMA the payer can be the lighter side, so the raw ratio can
exceed one. The cap at one hands receivers at most the whole flow; it
is what keeps the LP slice non-negative, and while receivers over-match
the payer the LP slice is zero.

This rule is continuous:

- With no receiver side, all collected funding belongs to LPs.
- A small receiver side receives a small share.
- As the market approaches balance and its history fades, receiver-side
  traders receive a larger share while the payer rate itself approaches
  zero.
- A dust position cannot redirect the complete funding flow away from
  LPs.

The receiver rate per unit of receiver-side USD size is derived from
the allocated receiver flow:

```text
receiver_rate =
    trader_receiver_flow
    / receiver_size_open_interest
```

This rate may differ from the payer rate when positions were opened at
different prices. The conserved quantity is total flow, not equality of
per-position rates.

### 4.4 Recognition rule

Receiver funding becomes a vault-backed claim as it accrues. It does
not depend on a payer settling first.

Payer funding becomes LP cash revenue only when it is collected from
position collateral or an external payment.

This asymmetry is intentional:

- Receivers receive the funding promised by the index.
- Uncollected payer fees are not treated as cash.
- If a payer becomes insolvent, LPs absorb the receiver-backed
  shortfall.

The guaranteed receiver amount must therefore be included among
non-LP claims as soon as it accrues.

## 5. Borrow fee

### 5.1 Purpose

Borrow prices the gross settlement and liquidation capacity consumed by
open leveraged positions. Funding already prices net directional
imbalance, so borrow does not try to duplicate that job.

An increase creates fixed risk units:

```text
risk_units_added =
    size_added
    × market_risk_factor_bps
    / BPS
```

Risk units:

- Are the position's borrow fee base.
- Increase only when risk is opened.
- Decrease proportionally when risk is closed.
- Do not change merely because the oracle price moves.
- Lock capacity but are not a cash expense or a profit entitlement.

This deliberately uses gross exposure. Gross positions still create
liquidation, settlement, collateral-loss, and operational load even
when long and short directional exposure is balanced.

### 5.2 Utilization and rate

All markets backed by the vault share one capacity domain.

```text
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

The rate is low when the vault has ample capacity and rises
quadratically as capacity becomes scarce.

New risk must separately satisfy:

```text
total_risk_units_after
    <= cash_lp_equity_after
       × risk_capacity_limit_bps
       / BPS
```

Capping the rate input at `BPS` does not authorize overcommitment. The
capacity gate rejects it.

Every market also has hard per-side size and base-exposure limits.
These bound exposure without an onchain volatility estimator or a
cross-market correlation model.

## 6. Accrual through indices

Time-based fees use cumulative indices. An index is cumulative fee per
unit of fee base.

For a rate that applies over `dt`:

```text
index_delta =
    rate_bps_day
    × INDEX_PRECISION
    × dt
    / (BPS × SECONDS_PER_DAY)
```

A position stores the index-derived amount already accounted for,
called its debt baseline:

```text
accrued =
    current_index_value_for_position
    - stored_debt_baseline
```

The required position baselines are:

```text
borrow_debt
funding_paid_to_receivers_debt
funding_paid_to_lps_debt
funding_received_debt
```

Funding uses separate payer indices for receiver-backed and LP-backed
flow. This preserves their different accounting treatment.

The system stores:

- One global borrow index.
- Per-market funding indices and the per-market skew EMA.
- One global pending receiver-funding claim, fed per market.

The borrow index advances with a piecewise-constant rate. The funding
indices advance with the exact closed-form integral of the continuously
decaying funding rate over each checkpoint window.

The guaranteed receiver claim accrues inside each market's checkpoint,
atomically with that market's indices, so a position's credit can never
outrun its market's contribution. LP pricing checkpoints every active
market to see the exact liability; other actions tolerate
keeper-cadence staleness in the total.

## 7. Checkpoint order

The borrow rate is constant between checkpoints; the funding rate
follows a known decay curve between them. In both cases a state change
cannot retroactively alter the accrual for elapsed time.

Every action that can change a fee base, rate, cash ownership, or risk
state follows this order:

```text
1. Accrue the global borrow index to now using the old rate.
2. Accrue the affected market's funding indices, its skew EMA, and its
   share of the guaranteed receiver claim over the elapsed window,
   using the pre-mutation book.
3. Calculate and capitalize the affected position's pending fees.
4. Apply the position, cash, claim, or configuration mutation.
5. Refresh the market's displayed payer side and rate from the
   post-mutation book and EMA.
6. Recompute the global borrow rate for future time.
```

Repeated checkpointing at the same timestamp changes nothing.

If governance changes a time-based parameter, the old parameter applies
through the checkpoint and the new parameter applies only afterward.
An operational pause does not stop the accrual clock; only closing or
settling the position stops its time-based fees.

The borrow rate is piecewise constant. Receiver liabilities may reduce
cash LP equity during an interval, but the resulting utilization change
is reflected at the next checkpoint. The funding rate is the opposite:
it decays continuously as the EMA converges, and each market checkpoint
advances its indices with the exact closed-form integral of that decay,
so checkpoint frequency moves accrued funding only within the decay
table's quantization.

## 8. Position collateral and settlement

Before a position changes, its effective collateral is:

```text
effective_collateral =
    stored_collateral
    + pending_funding_received
    - pending_funding_paid_to_receivers
    - pending_funding_paid_to_lps
    - pending_borrow
```

Accrued fees enter margin and liquidation calculations immediately.

Two margin rates govern position health. Leverage-increasing actions —
opening, adding size, or withdrawing collateral without closing — must
leave the position at or above the initial margin. De-risking actions —
a pure collateral top-up or a partial close — need only clear the
maintenance margin, which is also the liquidation threshold. The gap
between the two is a guaranteed entry buffer: a position opened at
maximum leverage does not sit on the liquidation boundary.

On any increase, decrease, close, or liquidation:

1. Checkpoint the relevant indices.
2. Calculate all pending fee amounts.
3. Apply them to stored collateral and their destination ledgers.
4. Reset every debt baseline to the current indices.
5. Apply the requested size change.

Capitalizing all old accrual before a partial close avoids historical
pro-rata debt bookkeeping. The remaining position starts from the
current indices.

The closing fee is collected at close, out of realized positive price
PnL. Collected closing and borrow fees are divided among:

```text
LP-owned revenue
risk-keeper reserve
protocol claimable revenue
```

LP-backed funding belongs entirely to LPs when collected. Receiver
funding moves between the guaranteed receiver claim and position
collateral; it is not recognized twice.

At full settlement:

```text
settlement_equity =
    stored_collateral
    + payable_price_pnl
    + pending_funding_received
    - pending_funding_paid
    - pending_borrow

closing_fee_collected =
    min(closing_fee, max(settlement_equity, 0))

trader_payout = max(settlement_equity - closing_fee_collected, 0)
bad_debt      = max(-settlement_equity, 0)
```

The closing fee ranks below every accrued obligation and never exceeds
payable price PnL, so it cannot create bad debt.

## 9. Vault ownership quantities

All tokens are held in one physical vault balance:

```text
physical_cash = collateral_token.balanceOf(vault)
```

Non-LP claims are explicit accounting labels:

```text
non_lp_claims =
    stored_position_collateral_total
    + pending_receiver_funding_total
    + execution_budget_total
    + protocol_claimable_total
    + risk_keeper_reserve_total
```

LP cash equity is the residual:

```text
cash_lp_equity =
    max(physical_cash - non_lp_claims, 0)
```

It is derived, not stored. Unexpected transfers therefore become LP
cash rather than creating accounting drift.

Three quantities must remain distinct:

```text
cash_lp_equity
    LP-owned cash before unrealized price PnL

marked_vault_nav
    LP value after recognized unrealized trader PnL

free_lp_capital
    cash LP equity not required to back current risk units
```

```text
required_risk_backing =
    ceil(total_risk_units × BPS / risk_capacity_limit_bps)

free_lp_capital =
    max(cash_lp_equity - required_risk_backing, 0)
```

Risk backing is a withdrawal lock. It is not subtracted from NAV.

## 10. Marked NAV

LP shares must reflect open trader PnL before traders close. Otherwise
an LP could observe an impending trader profit and withdraw at an
inflated price.

For each market:

```text
long_raw_pnl =
    long_base_exposure × mark_price
    - long_size_open_interest

short_raw_pnl =
    short_size_open_interest
    - short_base_exposure × mark_price
```

The formulas are evaluated at high precision.

Trader profit is recognized in full. Aggregate trader loss on one side
is recognized only up to that side's stored collateral:

```text
if raw_side_pnl >= 0:
    recognized_side_pnl = raw_side_pnl
else:
    recognized_side_pnl =
        -min(abs(raw_side_pnl), side_stored_collateral_total)
```

The vault's marked value is:

```text
marked_vault_nav =
    max(
        cash_lp_equity
        - sum(recognized_side_pnl),
        0
    )
```

The vault derives this value from the same canonical market-price
oracle used for trading, liquidation, and trader PnL. There is no
separate NAV oracle.

Aggregate base exposure makes each market-side calculation constant
time. A vault with multiple active markets needs one governance-bounded
loop over those markets when pricing an LP action. It never loops over
positions.

Marked NAV removes the normal realization-timing attack: open trader
profit already lowers the LP price. It cannot exactly discover an
individually insolvent position from aggregates. Timely liquidation,
hard exposure caps, and LP freezes during emergency states bound that
remaining limitation.

## 11. Fair LP participation

LP shares represent proportional ownership of marked vault NAV.

Deposits and withdrawals are delayed requests. A request:

1. Locks its assets or shares.
2. Becomes eligible after `lp_request_delay`.
3. Is assigned by rule to the first canonical synchronized oracle round
   at or after eligibility.
4. Executes fully against that round or fails and unlocks.

The requester and executor cannot choose a later favorable price.
Requests resolve in request-ID order when scarce free capital makes
ordering economically relevant.

The oracle round contains the market prices used by trading. The vault
calculates NAV from those prices and its stored aggregates. It never
accepts an externally supplied NAV.

Deposits require:

- A solvent vault.
- Positive NAV and cash LP equity, except for a clean first deposit.
- NAV above a configured floor relative to cash LP equity.

Withdrawals require:

- Enough free LP capital.
- Post-withdraw utilization no greater than
  `max_withdraw_utilization_bps`.
- No warning, ADL, insolvency, or oracle failure.

A request either settles completely or returns its complete escrow.
There are no partial fills or delayed withdrawal cash claims.

Virtual assets and shares protect the initial exchange rate from
donation-based inflation:

```text
shares_minted =
    deposit_assets
    × (share_supply + VIRTUAL_SHARES)
    / (marked_vault_nav + VIRTUAL_ASSETS)

assets_paid =
    shares_burned
    × (marked_vault_nav + VIRTUAL_ASSETS)
    / (share_supply + VIRTUAL_SHARES)
```

The final share cannot be burned while trading risk remains. In a clean
terminal state, the final LP may redeem all residual LP cash so virtual
quantities do not strand dust.

## 12. Solvency controls

Each market side has warning, ADL, recovery, and hard-cap PnL factors:

```text
recovery < warning < ADL < hard cap
```

Their purpose is to stop new risk and reduce profitable exposure before
aggregate trader profit consumes unsafe vault capacity.

- New risk stops at the warning factor.
- Ordinary LP actions stop at the warning factor.
- Funded auto-deleveraging begins at the ADL factor.
- The hard cap is a last-resort payout limit.
- Normal operation resumes below the recovery factor.

ADL and insolvent-position discovery receive capped rewards from the
risk-keeper reserve. A safety mechanism that depends on external action
must fund that action.

If:

```text
non_lp_claims = physical_cash
```

LP cash equity is zero, but non-LP claims remain fully backed.

If:

```text
non_lp_claims > physical_cash
```

the vault has a cash shortfall. Ordinary outgoing claims and LP actions
freeze until explicit recapitalization. Recapitalization adds cash
without minting LP shares.

## 13. Required properties

The completed mechanism must preserve:

1. An empty book, or a balanced book whose funding history has
   decayed, produces zero funding.
2. Funding rises quadratically with the blended integral skew.
3. Receiver-side traders receive only the flow justified by their
   counter-exposure, capped at the whole payer flow; LPs receive the
   unmatched collected flow.
4. A dust receiver-side position cannot redirect all funding.
5. Receiver claims never exceed receiver-backed payer accrual.
6. Receiver funding is guaranteed at accrual; payer and borrow revenue
   are recognized only when collected.
7. Borrow rises quadratically with vault-wide utilization.
8. Risk units price gross action-created capacity and do not mark with
   price.
9. New risk cannot exceed global or per-market capacity limits.
10. Checkpoints settle elapsed time on the pre-mutation state before
    every relevant mutation.
11. New size never pays fees from before it existed.
12. Partial closes capitalize old accrual before resetting baselines.
13. Accrued fees affect margin and liquidation.
14. One physical balance reconciles to explicit non-LP claims plus LP
    residual cash.
15. Marked NAV includes open trader PnL and uses the trading price
    oracle.
16. Trader losses recognized in NAV do not exceed aggregate stored
    collateral for that market side.
17. No LP action loops over positions.
18. A delayed LP request cannot choose its settlement round.
19. A withdrawal cannot consume locked risk backing.
20. Emergency risk blocks ordinary LP entry and exit.
21. Rounding cannot create value or over-credit a receiver.

## 14. Parameters

The model requires:

```text
close_fee_low_bps per market
close_fee_high_bps per market
max_funding_rate_bps_day per market
instant_weight_bps per market
funding_half_life_seconds

market_risk_factor_bps per market
max_long_size_open_interest per market
max_short_size_open_interest per market
max_long_base_exposure per market
max_short_base_exposure per market

base_borrow_rate_bps_day
max_variable_borrow_rate_bps_day
risk_capacity_limit_bps
max_withdraw_utilization_bps

lp_request_delay
min_deposit_nav_factor_bps

initial_margin_bps per market
maintenance_margin_bps per market
liquidation_reward_bps per market
warning_pnl_factor_bps per market
adl_pnl_factor_bps per market
recovery_pnl_factor_bps per market
hard_cap_pnl_factor_bps per market
global_hard_cap_factor_limit_bps

lp_revenue_share_bps
risk_keeper_revenue_share_bps
adl_reward_bps
max_adl_reward
max_insolvent_touch_reward
```

The funding and borrow exponents are fixed at two. The model uses
governed conservative ceilings and exposure caps rather than an onchain
volatility estimator. Calibration must cover one-sided markets, book
flips against the funding history, sudden price moves, high
utilization, delayed liquidation, minimum-size positions, rounding
boundaries, and simultaneous stress across markets.
