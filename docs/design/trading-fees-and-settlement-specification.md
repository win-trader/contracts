# WinTrader Trading, Fees, and Settlement Specification

## 1. The system in plain language

WinTrader is a perpetual trading vault. Traders use collateral to take long or
short exposure to a market, while liquidity providers supply the cash that
backs profitable positions. The system tracks each position's gains, losses,
fees, and risk until its exposure is removed.

Trader-requested position changes are not priced at the moment they are
created. A trader first commits to an action, and the action settles later
against a qualifying price observation that did not exist when the commitment
was made. This prevents a trader from knowingly trading against a stale vault
price.

### 1.1 The vault and liquidity providers

Liquidity providers deposit assets into a shared vault. The vault is the
trader's counterparty: it pays profitable traders and receives traders' losses
and the LP share of collected fees.

LPs earn revenue for supplying this backing, but they also bear the protocol's
residual market risk. This includes profitable trader payouts, price gaps that
make a position insolvent before it can be liquidated, and other shortfalls
that remain after the position's own collateral has been consumed.

### 1.2 Positions, collateral, size, leverage, and PnL

A position represents a trader's exposure to one market in one direction. A
long position gains value when the market price rises. A short position gains
value when the market price falls.

The trader supplies collateral, but chooses a position size that can be larger
than that collateral. Leverage describes this relationship. For example, a
position with five times as much size as collateral uses 5x leverage. Greater
leverage causes the same market move to have a larger effect on the trader's
collateral and brings the position closer to liquidation.

PnL is the position's profit or loss from the difference between its entry
price and its settlement price. Profit is not guaranteed to be paid in full:
system-wide risk controls can limit a payout when aggregate trader profit
threatens the vault. Fees, funding, and keeper rewards are accounted for
separately from price PnL.

### 1.3 Opening a position

Opening creates new market exposure. The trader chooses the market, direction,
size, collateral, and acceptable execution conditions, then commits the
collateral to an order.

A position is created only when a later settlement attempt obtains an eligible
fresh price and all safety checks pass. The opening fee and keeper reward are
deducted at settlement, and only the remaining amount becomes position
collateral. If settlement fails, no position is created and no opening fee is
charged.

### 1.4 Increasing a position

An increase adds size to an existing position and can add collateral at the
same time. Adding size is economically the same as opening that additional
exposure, so the added size can incur an opening fee. A collateral-only deposit
is a separate action and does not change the position's exposure.

Before the increase is applied, obligations accumulated by the existing
position are settled. The enlarged position then begins a new borrow period
using its resulting total exposure. The completed position must still satisfy
the applicable collateral, margin, and capacity requirements.

### 1.5 Decreasing a position

A decrease removes part of a position without closing it completely. The
removed portion realizes its share of price PnL, and its associated exposure
is removed from the market.

The action also settles the position's accumulated borrow and funding. A
profitable decrease can pay a closing fee on the portion removed. The remaining
position keeps its updated collateral and exposure and begins a new borrow
period.

### 1.6 Closing a position

A close removes all remaining exposure and ends the position. The system
realizes payable price PnL, settles funding and borrow, pays the applicable
keeper reward, and charges a closing fee only when settlement profit remains
available to pay it.

Any residual equity is returned to the trader. If the position cannot satisfy
all senior obligations, its available value is distributed in the defined
settlement order and the remaining shortfall becomes a vault loss.

### 1.7 Market and limit orders

A market order commits to execute at the authenticated current price used by
the first settlement call that reaches a terminal outcome, subject to the
trader's execution bounds. The price must come from an observation newer than
the commitment. The protocol does not claim that this is the first oracle
observation produced after the delay; enforcing a particular oracle round
would require a separate round-assignment mechanism.

A limit order waits for the market to reach a specified trigger. Reaching the
trigger makes the order eligible for execution; it does not guarantee an exact
fill price. A limit order can remain pending through unsuccessful trigger
checks and can be cancelled by its owner before execution.

Both kinds of order can expire. An order that expires can no longer execute.

### 1.8 Take-profit and stop-loss

A take-profit instruction requests an exit after the market reaches a
favourable trigger. A stop-loss instruction requests an exit after the market
moves far enough against the position. These instructions let a position be
managed without requiring its owner to submit the eventual settlement
transaction.

A keeper executes the instruction when its trigger and execution requirements
are satisfied. The trigger is a condition for attempting the action, not a
guarantee of settlement at the trigger price. Both actions settle the normal
borrow and funding obligations, and either can pay a closing fee when the
executed decrease or close realizes eligible profit.

### 1.9 Liquidation

Liquidation forcibly closes a position whose effective collateral has fallen
to or below its required safety threshold. The threshold is deliberately above
zero so a keeper has time and economic reason to remove the risk before all
position value disappears.

Liquidation is a safety action, not a voluntary trade. It pays a fixed keeper
reward but no closing fee. The position's remaining collateral pays its losses,
accrued obligations, and keeper reward as far as possible. LP equity covers a
shortfall if a violent price movement jumps past the liquidation buffer.

### 1.10 Automatic deleveraging

Automatic deleveraging, or ADL, forcibly reduces or closes exposure when
profitable positions on one side of a market create too much liability for the
vault. It exists to restore solvency and reduce directional risk when ordinary
liquidation is not the relevant remedy.

ADL can affect a healthy trader who did nothing wrong. It therefore does not
charge a closing fee. The action pays its configured fixed keeper reward from
the affected position's payable value or collateral and settles the position's
other obligations normally.

### 1.11 Keepers

Keepers are independent accounts that settle pending orders, execute triggered
instructions, clean up expired orders, liquidate unsafe positions, and perform
ADL. Anyone satisfying the action's requirements can act as a keeper.

Each completed keeper action pays a configured fixed reward for that specific
action. The reward comes from committed order collateral or position value,
not from a percentage share of protocol fee revenue. One settlement call
handles one action and pays at most one action reward.

### 1.12 Order creation and settlement

Trader-requested actions use two phases. During creation, the trader records a
binding instruction, its execution constraints, and the price-observation
cursor that existed at that moment. An entry order also transfers its submitted
collateral into escrow so later execution does not depend on the trader still
being present or funded.

During settlement, a keeper supplies a newer qualifying observation. The
system checks the action again against current prices, vault capacity, position
health, and the trader's bounds before changing exposure. Creation records
intent; settlement performs the economic action.

Liquidation and ADL are forced safety actions and do not require a prior trader
commitment.

### 1.13 Fresh-price execution

Every trader-requested action must wait for its market's configured execution
delay and for a qualifying oracle observation created strictly after the
action was committed. Waiting alone is not sufficient: if no newer observation
exists, the action is not ready to settle.

This rule prevents the trader from committing after seeing a market move and
then receiving the older vault price. The later observation, rather than the
passage of time by itself, is what makes the execution price independent of the
trader's decision.

### 1.14 Failed and expired orders

An eligible market-order attempt is terminal. It either succeeds or ends the
order as failed; it cannot wait and retry until the market eventually gives the
trader a favourable result. Expected failures such as slippage or unavailable
vault capacity pay the keeper from escrow, refund the remainder, create no
position, and charge no opening fee.

A limit order behaves differently while its trigger has not been reached: it
remains pending, and an unsuccessful trigger check does not consume it. Before
expiry, its owner can cancel it and receive the entire escrow without a fee or
keeper reward.

At expiry, an order is no longer executable. A keeper can remove it, receive
the configured expiry reward from escrow, and return the remainder to the
owner. An unexpected internal failure reverts atomically, so it changes no
state and pays no reward.

## 2. Accounting model and numerical conventions

This section defines the quantities used throughout the specification. A name
has one meaning and one unit everywhere it appears.

### 2.1 Numerical precision and units

All arithmetic uses integers. Decimal values are represented by fixed scales:

```text
BPS               = 10,000
SECONDS_PER_DAY   = 86,400
PRICE_PRECISION   = 10^7
INDEX_PRECISION   = 10^14
```

`BPS` represents one whole as basis points. For example, `100` is 1% and
`10,000` is 100%. Configured daily rates are expressed in basis points per
day.

Prices, collateral cash amounts, USD notionals, and base exposure use
`PRICE_PRECISION`. With seven decimal places, one dollar or one whole unit is
represented by `10,000,000`. Borrow and funding rates, cumulative indices,
skew fractions, and decay factors use `INDEX_PRECISION` when fractional
precision beyond basis points is required.

LP shares use six more decimal places than the collateral asset:

```text
SHARE_SCALE = 10^6 shares per collateral unit at the initial conversion rate
```

Time is measured in whole seconds. No fee uses ledger count or an assumed
ledger duration as its time source.

Every formula multiplies before it divides and retains the widest safe
intermediate value. Repeated fractional accrual carries its remainder instead
of discarding it. Floating-point arithmetic is never used.

### 2.2 Physical vault cash

Physical vault cash is the collateral-token balance actually held by the
vault:

```text
physical_cash = collateral_token_balance_of_vault
```

This token balance is the sole authority for how much cash exists. The system
does not maintain a second authoritative vault-balance counter because such a
counter could drift after rounding, direct token transfers, or unusual but
valid balance changes.

Accounting records describe who owns portions of this cash; they do not create
additional cash. A transfer into or out of the vault must change the physical
balance and the corresponding ownership label atomically.

An unexpected direct transfer into the vault has no non-LP ownership label and
therefore belongs to LPs. If explicit claims exceed physical cash, the
difference is an observable vault shortfall rather than a hidden negative
balance.

### 2.3 LP equity

LPs own the residual cash after every explicit non-LP claim has been removed:

```text
cash_lp_equity = max(physical_cash - non_lp_claims, 0)
```

Cash LP equity is not the same as marked vault value. Open trader PnL must also
be recognized when valuing LP shares. For each market side, trader profit is
recognized in full while trader loss is recognized only up to the collateral
stored by that side:

```text
if raw_side_pnl >= 0:
    recognized_side_pnl = raw_side_pnl
else:
    recognized_side_pnl = -min(abs(raw_side_pnl), side_stored_collateral)

marked_vault_nav = max(
    cash_lp_equity - sum(recognized_side_pnl),
    0
)
```

Positive trader PnL reduces LP value because it is a liability of the vault.
Collectible trader loss increases LP value. Loss beyond stored collateral is
not treated as an LP receivable because the trader cannot be forced to pay it.
Uncollected future borrow and funding are likewise excluded from marked value.

A third quantity, free LP capital, measures cash that is not locked as risk
backing:

```text
required_risk_backing = ceil(
    total_risk_units * BPS / risk_capacity_limit_bps
)

free_lp_capital = max(cash_lp_equity - required_risk_backing, 0)
```

Risk backing restricts withdrawals but remains LP property. It is not deducted
from marked vault NAV.

### 2.4 Position collateral

Stored position collateral is the portion of vault cash currently owned by an
open position. It begins with the collateral left after entry charges and can
then change as the position is touched.

Position collateral increases when the trader adds collateral, when payable
profit is credited, or when receiver funding is moved into the position. It
decreases when the position realizes a loss, pays funding or borrow, pays a
keeper reward, or returns value to the trader.

Most of these changes are ownership-label movements inside one vault balance.
For example, realizing trader profit moves value from LP residual equity to the
position's collateral label; it does not require a token transfer within the
vault. A trader payout removes both the position label and the same amount of
physical cash.

Stored collateral is not a complete live health measure because time-based
obligations and unrealized PnL can change without rewriting the position. Live
health uses effective collateral instead.

### 2.5 Explicit claims and liabilities

Every non-LP ownership claim on vault cash is represented explicitly:

```text
non_lp_claims =
      stored_position_collateral_total
    + pending_receiver_funding_total
    + action_escrow_total
    + protocol_claimable_total
    + referral_claimable_total
```

- `stored_position_collateral_total` is the sum of collateral owned by all
  open positions.
- `pending_receiver_funding_total` is funding already guaranteed to receiving
  traders but not yet moved into their positions.
- `action_escrow_total` is collateral committed to pending trader actions but
  not yet converted into position collateral or refunded.
- `protocol_claimable_total` is collected protocol revenue awaiting claim.
- `referral_claimable_total` is collected referral revenue awaiting claim and
  equals the sum of individual referral balances.

Keeper rewards are paid directly from order escrow or position value and do
not accumulate in a keeper reserve. LP revenue has no explicit claim bucket;
leaving collected LP revenue in the residual is what credits LPs.

Receiver-backed funding becomes an explicit claim when it accrues, before its
eventual receiver touches the position. This guarantee can temporarily reduce
cash LP equity before payer funding is collected. If claims exceed cash, the
shortfall is:

```text
vault_shortfall = max(non_lp_claims - physical_cash, 0)
```

### 2.6 Position size and base exposure

Position size is stable USD notional. Base exposure is the quantity of the
underlying asset represented by that notional at execution. Both are stored
because they answer different questions: size is the fee and margin base,
while base exposure determines directional price PnL and market skew.

For execution price `price`:

```text
long_base_exposure  = floor(size * PRICE_PRECISION / price)
short_base_exposure = ceil(size * PRICE_PRECISION / price)
```

Long exposure rounds down and short exposure rounds up so conversion rounding
cannot give the trader free PnL. A market side aggregates both size and base
exposure, allowing market-level PnL and skew to be calculated without reading
individual positions.

When exposure is increased, the added base is calculated from the added size
and its own execution price, then added to the existing base. The resulting
position therefore naturally represents a size-weighted entry price without
needing to store that average as an accounting authority.

### 2.7 Risk units

Risk units measure how much shared vault capacity a position consumes:

```text
risk_units = floor(
    position_size * market_risk_factor_bps / BPS
)
```

They are the fee base for borrow and the capacity base for the vault. Risk
units depend on position size and the market's configured risk factor. They do
not depend on collateral, leverage, direction, entry price, or later price
movement.

Risk units are not cash and do not represent ownership. They remain fixed while
position size is unchanged and are re-derived from complete resulting position
size after every size mutation. They are not accumulated from independently
rounded size tranches. Long and short risk units are both counted because
offsetting directional exposure still creates liquidation, settlement, and
operational risk.

### 2.8 Raw and payable PnL

Raw PnL is the position's price result before emergency payout limits:

```text
long_raw_pnl  = floor(base_exposure * mark_price / PRICE_PRECISION) - size
short_raw_pnl = size - ceil(base_exposure * mark_price / PRICE_PRECISION)
```

A positive value is trader profit and a negative value is trader loss.
Negative PnL always passes through without a payout reduction.

Payable PnL is the amount the settlement system is permitted to recognize for
the position. For a positive raw PnL, the active market-side payout factor is
applied first and the result is then limited by available LP equity:

```text
if raw_pnl <= 0:
    payable_pnl = raw_pnl
else:
    factor_adjusted_pnl = floor(
        raw_pnl * active_payout_factor / INDEX_PRECISION
    )

    payable_pnl = min(factor_adjusted_pnl, cash_lp_equity)
```

The normal payout factor is one whole. A hard-cap risk state can lower it
uniformly for every profitable position on the affected market side. The LP
equity clamp also applies outside the hard-cap state so no individual profit
credit can exceed the cash backing available when it is credited.

Fees that use profit as their base use positive payable price PnL, not raw PnL,
funding credits, or stored collateral.

### 2.9 Effective collateral

Effective collateral is the live value used for margin and liquidation checks:

```text
effective_collateral =
      stored_collateral
    + payable_pnl
    + pending_funding_received
    - pending_receiver_backed_funding_owed
    - pending_lp_backed_funding_owed
    - pending_borrow
```

Pending borrow includes the active minimum-borrow floor. Each pending amount is
calculated from current indices without first changing stored collateral.

Effective collateral can therefore fall even when the stored collateral field
has not changed. Every health-sensitive action must use this live value after
bringing the relevant indices current. Action-specific charges, such as a
keeper reward, are also included when checking the state that would remain
after that action.

### 2.10 Utilization

Utilization compares the vault's total risk units with its cash LP equity:

```text
if total_risk_units == 0:
    utilization_bps = 0
else if cash_lp_equity == 0:
    utilization_bps = BPS
else:
    utilization_bps = min(
        floor(total_risk_units * BPS / cash_lp_equity),
        BPS
    )
```

This capped value is an input to the borrow-rate curve. The cap prevents the
rate calculation from exceeding its defined domain; it does not authorize the
vault to accept excessive risk.

Every action that adds exposure must separately satisfy the capacity gate:

```text
total_risk_units_after * BPS
    <= cash_lp_equity_after * risk_capacity_limit_bps
```

Per-market size and base-exposure limits can impose additional bounds even when
global capacity remains available.

### 2.11 Rounding rules

Rounding never favours the trader. The consistent direction is:

| Quantity | Direction |
|---|---|
| Long base exposure | Down |
| Short base exposure | Up |
| Long value used for PnL | Down |
| Short buyback value used for PnL | Up |
| Borrow owed | Up |
| Funding owed | Up |
| Funding received | Down |
| Opening fee | Up |
| Closing-fee components | Up before their caps |
| LP and referral revenue shares | Down |
| LP shares minted for a deposit | Down |
| Assets paid for an LP withdrawal | Down |

When a fee distribution contains rounding residue, explicitly calculated
shares round down and the protocol receives the remainder. This makes the
distribution sum exactly to the amount collected.

For a partial position reduction, remaining base exposure rounds down and the
removed portion receives the difference. Remaining risk units are freshly
derived from resulting size, and the removed risk is the difference from the
old canonical value. A final close removes every residual unit, so repeated
partial decreases cannot strand exposure or risk.

This one rule is a conservation rule rather than a direction rule, and the
table above does not apply to it. The removed and remaining base always sum to
exactly the pre-reduction base, so the sub-unit allocated by the rounding is
not created or destroyed; it only moves between the portion realized now and
the portion still open. For a long, rounding the removed base up realizes a
sub-unit of exposure earlier and leaves the survivor with exactly that much
less; for a short the sign is reversed. Neither direction leaks value, and
repeated decreases cannot accumulate an advantage because the total is
conserved at every step and a final close removes the remainder exactly.

What does protect the vault here is the split itself. Because `floor(x) +
floor(y) <= floor(x + y)`, valuing two portions separately can only produce
less trader value than valuing the whole position once. A partial reduction is
therefore weakly conservative regardless of how the sub-unit is allocated.

Any negative pending fee obtained by subtracting a stored debt baseline from a
monotonic index is an invariant violation. A floor or cap must never hide it.

### 2.12 Stored and derived values

The system stores a value only when it cannot be reconstructed safely and
efficiently from authoritative state. Stored values include ownership claims,
position and market exposure, cumulative indices and their remainders,
position debt baselines, active borrow minima, order state, risk-state latches,
and configuration.

The following values are derived when needed and are not independent sources
of truth:

- physical cash;
- total non-LP claims;
- cash LP equity and vault shortfall;
- marked vault NAV and LP share price;
- required risk backing and free LP capital;
- utilization and the next borrow rate;
- raw and payable PnL;
- effective collateral;
- pending borrow and funding; and
- opening, closing, and keeper charges for the current action.

Aggregate counters are stored only to make bounded accounting possible. Each
aggregate must equal the sum of the records it represents, and every mutation
must update the individual record and its aggregate in the same atomic
operation.
## 3. Fees and rewards

The protocol has four economic mechanisms: opening and closing fees collect
revenue, borrow charges for vault capacity over time, funding rewards exposure
that balances the market, and fixed keeper rewards pay for execution work.
These mechanisms remain separate because they price different things.

From the trader's perspective, the signed funding term is funding owed minus
funding received:

```text
entry_deductions = opening_fee + keeper_entry_reward

settlement_deductions =
      borrow_fee
    + funding_owed
    - funding_received
    + keeper_action_reward
    + closing_fee
```

The actual settlement result also includes positive or negative payable price
PnL. A funding credit can exceed the other deductions and make the net fee for
a position negative.

### 3.1 Opening fee

The opening fee charges for newly added position size:

```text
opening_fee = ceil(added_size * open_fee_bps / BPS)
```

The rate is configured per market and has an initial default of zero:

```text
open_fee_bps = 0
```

An initial open applies the fee to the complete new size. An increase applies
it only to the size added by that action. Adding collateral without adding size
does not pay an opening fee. The fee has no skew tier and no separate leverage
multiplier.

For an initial entry, the trader submits one collateral amount. The fee is
deducted from that amount rather than transferred on top:

```text
net_collateral_added = submitted_collateral - opening_fee
```

For an increase, any added collateral first joins the position and the opening
fee is then deducted from the resulting stored position collateral. A
size-only increase therefore records the fee as a small collateral reduction;
it does not require a separate wallet transfer at settlement.

For a delayed entry, the opening fee is calculated during settlement but is
collected only if the position is successfully created. The post-fee,
post-keeper-reward collateral must pass every minimum-collateral, initial
margin, health, and capacity check. A failed or expired entry charges no
opening fee.

Opening-fee revenue is shared by LPs, the protocol, and an eligible referrer as
defined below. Keepers receive no share of it.

### 3.2 Closing fee

The closing fee applies only when an action removes exposure and realizes
positive payable price PnL. It has no market-skew component. Its target is the
larger of a size-based component and a PnL-based component:

```text
target_closing_fee = max(size_fee, pnl_fee)
```

The initial per-market rates are:

```text
close_size_fee_bps = 5       # 0.05% of size removed
close_pnl_fee_bps  = 1,000   # 10% of payable realized price PnL
```

A voluntary full close, partial decrease, take-profit, or stop-loss can pay the
fee. Liquidation and ADL never pay it. Funding, borrow, and keeper rewards are
separate obligations whether or not a closing fee is collected.

#### 3.2.1 Size-based component

The size component is based on the notional exposure removed by the action:

```text
size_fee = ceil(size_removed * close_size_fee_bps / BPS)
```

This component establishes the profit threshold a trade must clear before the
trader retains price profit. It is calculated even for a small winning close,
but it can only be collected from eligible settlement profit. It is zero as a
charged fee when payable price PnL is zero or negative.

#### 3.2.2 PnL-based component

The PnL component shares in the position's payable realized price profit:

```text
pnl_fee = ceil(
    positive_payable_price_pnl * close_pnl_fee_bps / BPS
)
```

The base is price PnL after any hard-cap payout factor and LP-equity clamp.
Funding received does not create eligibility for a closing fee, and neither
stored collateral nor added collateral is part of this fee base.

The PnL component lets protocol revenue grow with a large win even after the
size threshold has been cleared.

#### 3.2.3 Profit cap

The nominal fee can consume positive price PnL but cannot turn that price PnL
negative:

```text
if payable_price_pnl <= 0:
    nominal_closing_fee = 0
else:
    nominal_closing_fee = min(
        payable_price_pnl,
        max(size_fee, pnl_fee)
    )
```

Funding, borrow, and the applicable keeper reward are senior to the closing
fee. The closing fee may consume only profit remaining from the current
settlement after those items:

```text
profit_after_senior_items = max(
    0,
      payable_price_pnl
    + funding_received
    - receiver_backed_funding_owed
    - lp_backed_funding_owed
    - borrow_fee
    - keeper_reward
)

closing_fee = min(
    nominal_closing_fee,
    profit_after_senior_items
)
```

Funding received offsets senior obligations before those obligations reduce
eligible price profit; it never increases the fee above `nominal_closing_fee`,
which is itself capped by payable price PnL. Under this source-attribution rule,
the fee remains a debit from price profit rather than from the funding credit.
This second cap protects the position's pre-existing collateral. If the
nominal closing fee is `$50` but senior settlement items leave only `$20` of
current settlement profit, the collected closing fee is `$20`. The unpaid
`$30` is waived. It is not a receivable and never becomes bad debt.

#### 3.2.4 Partial decreases

A partial decrease uses only the size and payable price PnL realized by the
removed portion:

```text
size_fee = ceil(size_removed * close_size_fee_bps / BPS)
pnl_fee  = ceil(positive_payable_pnl_removed * close_pnl_fee_bps / BPS)
```

The same profit and senior-item caps then apply. No closing fee is calculated
from the unrealized PnL of the exposure that remains open. Collected revenue is
distributed immediately; no part of the fee remains attached to the surviving
position.

### 3.3 Borrow fee

Borrow is rent for consuming LP-backed risk capacity. Every open position pays
it on risk units, regardless of direction, leverage, collateral, entry price,
or whether the position is profitable:

```text
actual_borrow = ceil(
    risk_units * current_borrow_index / INDEX_PRECISION
) - borrow_debt

pending_borrow = max(actual_borrow, stored_minimum_borrow_fee)
```

`borrow_debt` is the position's index baseline for its current borrow window.
A negative `actual_borrow` is an invariant violation and must be rejected
before the minimum is applied.

Borrow revenue is collected only to the extent the position can pay it. The
collected amount is distributed 90% to LPs and 10% to the protocol by default.
It never pays a referrer or keeper.

#### 3.3.1 Utilization-based rate

The vault has one global borrow rate shared by all markets. Its rate increases
as aggregate risk consumes LP cash equity:

```text
u = utilization_bps / BPS
e = borrow_exponent_bps / BPS

borrow_rate_bps_day =
      base_borrow_rate_bps_day
    + max_variable_borrow_bps_day * u^e
```

Initial global defaults are:

```text
base_borrow_rate_bps_day  = 25
max_variable_borrow_bps_day = 250
borrow_exponent_bps       = 20,000  # exponent 2, a square curve
```

The configured rate is expressed on risk units. With a 10% market risk factor,
a rate of 25 bps per day on risk units feels like 2.5 bps per day on position
notional.

The rate is piecewise constant between state changes. Time already elapsed is
always accrued with the previously stored rate. A mutation that changes risk
units or LP cash equity refreshes the rate only for future time.

#### 3.3.2 Minimum borrow duration

Each borrow window quotes a monetary minimum equivalent to holding the
resulting exposure for a configured number of seconds at the rate present when
the window begins:

```text
min_borrow_fee_seconds = 900  # initial global default: 15 minutes

stored_minimum_borrow_fee = ceil(
    resulting_risk_units
    * borrow_rate_at_reset_bps_day
    * min_borrow_fee_seconds
    / (BPS * SECONDS_PER_DAY)
)
```

When the stored borrow rate is represented at `INDEX_PRECISION`, the equivalent
integer formula includes `INDEX_PRECISION` in the denominator.

The minimum is a floor, not an additional charge. Actual accrual satisfies it:

```text
pending_borrow = max(actual_borrow, stored_minimum_borrow_fee)
```

The monetary quote is fixed for that borrow window. Later changes to
utilization do not retroactively change its minimum, while actual borrow
continues to accrue through the global index at each rate that applied over
time.

#### 3.3.3 Borrow-window resets

An initial open starts the first borrow window. Every size increase or decrease
settles the old window for the complete position and starts a new one:

```text
1. Accrue the global borrow index using the old rate.
2. Calculate max(actual borrow, stored minimum).
3. Deduct that full amount from existing stored position collateral.
4. Apply the size mutation and update total risk units.
5. Refresh the global borrow rate from the resulting vault state.
6. Reset the position's borrow debt to the current index.
7. Quote a new monetary minimum for the full resulting risk units.
```

The previous window does not carry a tranche, proportional remainder, or
minimum into the new one. A decrease deliberately begins a fresh minimum for
the entire position that remains.

A collateral-only addition does not settle borrow, reset the debt baseline, or
quote a new minimum because it does not change exposure. A full close,
liquidation, or ADL settles the current window but creates no replacement
window.

A voluntary size mutation cannot leave an underfunded position open. If its
existing collateral cannot cover the completed borrow window and other senior
obligations, the action is rejected and the position must be liquidated.

### 3.4 Funding

Funding prices directional imbalance. It transfers value from the side whose
exposure follows the market's accumulated skew to the traders who offset that
exposure and to LPs who remain exposed to the unmatched part.

Funding is not protocol revenue. It uses position size as its fee base, can be
paid or received, and is the only signed fee component from the trader's point
of view.

#### 3.4.1 Signed skew

The live skew of a market is calculated from long and short base exposure:

```text
if long_base + short_base == 0:
    signed_skew = 0
else:
    signed_skew =
        (long_base - short_base)
        * INDEX_PRECISION
        / (long_base + short_base)
```

Positive skew means long exposure dominates; negative skew means short
exposure dominates. Funding uses the sign. Any unsigned skew quantity used for
monitoring is distinct and does not select a fee tier.

#### 3.4.2 EMA memory

Funding blends live skew with an exponentially decaying history so a brief book
change cannot immediately erase or reverse the economic incentive.

For one interval during which live skew `S` is constant:

```text
H = funding_half_life_seconds
w = instant_weight_bps / BPS
d(t) = 2^(-t / H)

E(t) = S + (E0 - S) * d(t)
I(t) = w * S + (1 - w) * E(t)
```

`E0` is the skew EMA at the beginning of the interval and `I(t)` is the blended
integral skew that drives funding. Initial defaults are a 12-hour half-life and
30% instant weight:

```text
funding_half_life_seconds = 43,200
instant_weight_bps        = 3,000
```

When the first position opens into an empty market, the EMA is seeded from the
new live skew rather than zero. When the whole market becomes empty, the EMA
and its unassigned remainders are reset to zero.

#### 3.4.3 Payer determination

The payer rate is quadratic in the blended skew:

```text
funding_rate_bps_day(t) =
    max_funding_rate_bps_day
    * (I(t) / INDEX_PRECISION)^2
```

The initial per-market maximum is 80 bps per day. Mild imbalance is therefore
cheap, while a severely one-sided book becomes expensive quickly.

The payer side at each instant is determined by the sign of blended skew:

```text
I(t) > 0  => longs pay at time t
I(t) < 0  => shorts pay at time t
I(t) = 0  => no funding accrues at time t
```

The current larger side alone does not decide who pays. After a trader balances
the book, the previous dominant side can continue paying while the EMA decays,
which rewards the trader who supplied the balancing exposure.

If `I(t)` crosses zero during a checkpoint interval, the interval is split at
the crossing. The positive and negative subintervals accrue to their respective
payer-side indices. The protocol must never select one payer for the complete
interval from the sign of an integrated linear average, because that would
assign funding generated on one side of the crossing to the other side.

Because the funding rate changes continuously as the EMA decays, the interval
is integrated in closed form. With:

```text
A  = S
B  = (1 - w) * (E0 - S)
d  = 2^(-elapsed / H)
J1 = H / ln(2)     * (1 - d)
J2 = H / (2 ln(2)) * (1 - d^2)
```

the quadratic weight is:

```text
integral of I(t)^2 =
    A^2 * elapsed + 2*A*B*J1 + B^2*J2
```

This makes the economic result depend on the actual interval rather than how
often an unchanged market happens to be checkpointed.

#### 3.4.4 Receiver-backed and LP-backed funding

The payer flow is divided according to how much opposing base exposure exists:

```text
receiver_fraction = min(1, receiver_base / payer_base)
lp_fraction       = 1 - receiver_fraction
```

The receiver-backed portion goes to positions on the opposing side. The
LP-backed portion belongs to LP residual equity because LPs remain the
counterparty for payer exposure that no trader offsets.

Separate indices track payer obligations backed by receivers, payer
obligations backed by LPs, and receiver credits. At the position boundary:

```text
funding_owed_to_receivers = ceil(
    size * payer_receiver_index_delta / INDEX_PRECISION
)

funding_owed_to_lps = ceil(
    size * payer_lp_index_delta / INDEX_PRECISION
)

funding_received = floor(
    size * receiver_index_delta / INDEX_PRECISION
)
```

Receiver credits round down and payer obligations round up.

Receiver-backed funding is guaranteed when it accrues. It immediately becomes
an explicit vault liability and is collected before LP-backed funding and
borrow. LP-backed funding is recognized as LP revenue only when collected.
Funding never creates protocol, referral, or keeper revenue.

### 3.5 Fixed keeper rewards

Keeper rewards pay for discrete execution work. Each action has an independent
fixed cash amount:

```text
keeper_open_reward
keeper_increase_reward
keeper_decrease_reward
keeper_close_reward
keeper_tp_reward
keeper_sl_reward
keeper_limit_order_reward
keeper_expiry_reward
keeper_liquidation_reward
keeper_adl_reward
keeper_lp_resolve_reward
```

Every initial value is:

```text
2,500,000  # $0.25 at PRICE_PRECISION
```

Only the reward corresponding to the executed action is paid. Rewards do not
stack because an action also happened to originate from an order. A successful
settlement call processes one action and pays at most one keeper reward.

The reward is an execution cost, not protocol revenue. It is transferred to
the keeper rather than accumulated in a reserve or shared with LPs, the
protocol, or a referrer.

Where a reward's source can be smaller than the reward itself, the payment is
capped at what that source holds and the action still completes. This applies
to the ordinary failure reward on a position action (§7.0) and to
`keeper_lp_resolve_reward` on a request whose escrow or released assets are
worth less than the reward. A capped reward never draws on LP equity;
`keeper_liquidation_reward` remains the single exception with an explicit LP
backstop.

#### 3.5.1 Open execution

A successful market-open execution pays `keeper_open_reward` from entry-order
escrow. The opening fee and reward are both removed before the remainder
becomes position collateral.

An eligible market-open attempt that terminates because of an expected failure
still pays this reward from escrow, refunds the remainder, and charges no
opening fee. An ineligible call or reverted transaction pays nothing.

#### 3.5.2 Increase execution

A successful size increase pays `keeper_increase_reward` from stored position
collateral. The reward is included in the post-action health check.
It is separate from the opening fee on added size.

#### 3.5.3 Decrease execution

A successful voluntary partial decrease pays `keeper_decrease_reward` from
position collateral. The reward is senior to any closing fee and reduces the
current settlement profit available to that fee.

#### 3.5.4 Close execution

A successful voluntary full close pays `keeper_close_reward` from position
value before the closing fee.

#### 3.5.5 Take-profit execution

A take-profit execution pays `keeper_tp_reward` from position collateral. It
does not also pay the decrease or close reward. Its profitable exposure removal
can pay the normal closing fee after the keeper reward and other senior items.

#### 3.5.6 Stop-loss execution

A stop-loss execution pays `keeper_sl_reward` from position collateral. It
does not also pay the decrease or close reward. A stop-loss that realizes no
positive payable price PnL pays no closing fee.

#### 3.5.7 Limit-order execution

A successful limit-entry execution pays `keeper_limit_order_reward` from order
escrow. An eligible triggered attempt that reaches a terminal expected failure
also pays it, refunds the remaining escrow, and charges no opening fee.

A trigger check that has not crossed the limit is not an execution attempt. The
order remains pending and no reward is paid. Owner cancellation pays no keeper
reward.

#### 3.5.8 Expiry cleanup

Permissionless cleanup of an expired order pays `keeper_expiry_reward` from
the order's escrow and refunds the remainder to the owner. Entry-order creation
must ensure the committed collateral can fund this reward.

#### 3.5.9 Liquidation

Liquidation pays `keeper_liquidation_reward` from the position's remaining
value. The liquidation threshold preserves at least the larger of maintenance
margin and the fixed reward during ordinary price movement:

```text
maintenance_margin = ceil(
    position_size * maintenance_margin_bps / BPS
)

liquidation_threshold = max(
    maintenance_margin,
    keeper_liquidation_reward
)

liquidatable = effective_collateral <= liquidation_threshold
```

The global minimum collateral must be greater than the configured liquidation
reward. With the initial defaults, minimum collateral is `$1.00` and the reward
is `$0.25`.

A price gap can jump past the reserved value. The keeper still receives the
full fixed reward and LP equity covers its shortfall. Liquidation pays no
closing fee.

#### 3.5.10 Automatic deleveraging

ADL pays `keeper_adl_reward` from the affected position's payable profit or
stored collateral. It does not also pay a close or liquidation reward. ADL
pays no closing fee because it is a forced vault-safety action rather than a
voluntary trade.

### 3.6 Referral rewards

A trader can associate a referrer with the account. A valid referrer receives
a share of opening and closing fees actually collected from that trader:

```text
referral_reward = floor(
    collected_fee * referral_fee_share_bps / BPS
)
```

The initial share is 250 bps, or 2.5% of the collected fee. It is carved
entirely from the protocol portion and never reduces the LP share.

Opening referral rewards apply to successful opens and size increases.
Closing referral rewards apply to profitable voluntary closes, decreases,
take-profits, and stop-losses. There is no referral reward when the applicable
fee is zero, waived, or uncollected. Borrow, funding, keeper rewards,
liquidation, and ADL do not generate referral revenue.

Referral codes are first-come and their owner is immutable. A trader may change
the account's referrer; the latest valid selection applies to future fees.
Self-referral is rejected. Earned rewards accumulate as an explicit claim until
the referrer withdraws them.

### 3.7 Fee revenue distribution

Opening and closing fees use the same distribution. With an eligible referrer:

```text
lp_revenue = floor(
    collected_fee * fee_lp_revenue_share_bps / BPS
)

referral_revenue = floor(
    collected_fee * referral_fee_share_bps / BPS
)

protocol_revenue =
    collected_fee - lp_revenue - referral_revenue
```

Without a referrer, `referral_revenue` is zero and the protocol receives the
remainder. Initial defaults produce:

| Recipient | No referrer | With referrer |
|---|---:|---:|
| LPs | 90% | 90% |
| Protocol | 10% | 7.5% plus rounding residue |
| Referrer | 0% | 2.5% |
| Keepers | 0% | 0% |

Borrow revenue uses a separate split:

```text
borrow_lp_revenue = floor(
    collected_borrow * borrow_lp_revenue_share_bps / BPS
)

borrow_protocol_revenue =
    collected_borrow - borrow_lp_revenue
```

Its initial values are 90% to LPs and 10% to the protocol. It has no referral
or keeper share.

LP revenue remains in residual cash LP equity. Protocol and referral revenue
increase their explicit claim totals. All shares are calculated from the
amount actually collected, never from a nominal fee that the position could
not pay.

### 3.8 Settlement waterfall

Settlement uses a fixed priority so a shortfall always reaches the least
protected claim first. Positive funding and payable profit are credited before
charges are collected. Receiver-backed funding has the highest payment
priority because it was guaranteed when accrued.

For a decrease, close, take-profit, or stop-loss, the economic order is:

```text
0. Credit funding received.
1. Credit positive payable price PnL, if any.
2. Collect receiver-backed funding owed.
3. Apply negative payable price PnL, if any.
4. Collect LP-backed funding owed.
5. Collect the completed borrow window.
6. Pay the fixed keeper reward.
7. Collect the closing fee from remaining current settlement profit only.
8. Leave residual value in the surviving position or pay it to the trader.
```

On an increase or decrease that leaves a position open, previously accrued
funding and the completed borrow window are capitalized against existing stored
collateral before the new exposure state begins. The closing-fee profit cap
still subtracts those senior items when determining how much current realized
profit is eligible, so the closing fee cannot indirectly replace collateral
used to pay them.

A partial action must satisfy every senior obligation and leave the surviving
position healthy. It cannot leave unpaid debt attached to a reset index
baseline. A full forced settlement may exhaust all position value and report a
shortfall.

Entry settlement uses a separate escrow distribution:

```text
successful entry:
    escrow
    - keeper action reward
    - opening fee
    = initial position collateral

terminal expected failure:
    escrow
    - keeper action reward
    = owner refund
```

The opening fee is distributed only on success.

### 3.9 Bad debt and the LP backstop

Bad debt exists when a full forced settlement has insufficient position value
to satisfy losses and protected obligations. The system reports the shortfall
explicitly and removes the position; it does not leave a negative-collateral
position open.

LPs are the residual backstop. In particular, LP equity absorbs:

- trader profit paid by the vault;
- negative position equity that remains after collateral is exhausted;
- unpaid guaranteed receiver-backed funding;
- a guaranteed liquidation reward after a price gap; and
- accounting residue assigned to the vault residual.

Uncollected LP-backed funding and borrow are not recorded as earned revenue.
They are forgone revenue rather than additional cash debt. An uncollectible
closing fee is waived by definition. Protocol and referral claims are created
only from fees actually collected.

This backstop does not let an ordinary action bypass liquidation. Once a
position is liquidatable, voluntary increase, decrease, close, TP, and SL
settlement yield to the forced liquidation path. The LP backstop exists for
forced settlement and unavoidable price gaps, not as optional financing for a
voluntary action.

### 3.10 Fee examples

The examples use seven-decimal cash precision and initial parameter values.

#### Example A: successful open

A trader submits `$5,100` collateral for a `$100,000` market position. The
opening-fee rate is initially zero and the open keeper reward is `$0.25`:

```text
opening_fee                = $0.00
keeper_open_reward         = $0.25
initial_position_collateral = $5,099.75
```

If the opening-fee rate were later configured to 5 bps, the same open would
pay `$50` and begin with `$5,049.75` of position collateral.

#### Example B: size fee consumes a small win

A `$100,000` position closes with `$30` of payable price profit and no senior
items for this simplified example:

```text
size_fee             = $100,000 * 0.05% = $50
pnl_fee              = $30 * 10%        = $3
target_closing_fee   = max($50, $3)     = $50
nominal_closing_fee  = min($30, $50)    = $30
price profit retained                     $0
```

The fee consumes the win but does not touch original collateral.

#### Example C: profit exceeds the size threshold

The same position closes with `$60` of payable price profit:

```text
size_fee             = $50
pnl_fee              = $6
closing_fee          = $50
price profit retained = $10
```

If the position closes at a loss, the closing fee is zero.

#### Example D: the PnL component dominates

The same position closes with `$2,000` of payable price profit:

```text
size_fee             = $50
pnl_fee              = $200
closing_fee          = $200
price profit retained = $1,800
```

#### Example E: senior items reduce the closing fee

A position has `$60` of payable price profit and a `$50` nominal closing fee.
Funding, borrow, and the keeper reward consume `$40` of the current settlement
profit:

```text
profit_after_senior_items = $20
closing_fee = min($50, $20) = $20
profit retained after all items            $0
```

The remaining `$30` of nominal closing fee is waived; stored collateral is not
used to collect it.

#### Example F: minimum borrow duration

A `$100,000` position in a market with a 10% risk factor has `$10,000` of risk
units. At 87.5 bps per day on risk units and a 15-minute minimum:

```text
minimum_borrow_fee =
    $10,000 * 0.875% * 15 / 1,440
  = $0.9114584 after rounding up
```

If actual index accrual for the window is `$0.20`, the position pays
`$0.9114584`. If actual accrual is `$2.00`, it pays `$2.00`.

#### Example G: funding split

A settled market has long and short base exposures in a 4:1 ratio, equivalent
at the common price to `$400,000` long and `$100,000` short. Its blended skew
magnitude is `0.6`. With `$400,000` of payer size and an 80-bps maximum, the
quadratic payer rate is 28.8 bps per day and longs pay `$1,152` per day:

```text
receiver_fraction = $100,000 / $400,000 = 25%
to short receivers = $288
to LPs             = $864
to protocol        = $0
to keepers         = $0
```

## 4. Index and checkpoint system

Borrow and funding accrue continuously in economic terms, but the protocol
cannot update every open position every second. Cumulative indices convert that
unbounded task into constant work: time advances one global borrow accumulator
and a bounded set of accumulators for the affected market, while each position
stores only the accumulator values from its last accounting boundary.

### 4.1 Why indices are necessary

Without indices, collecting one hour of borrow or funding would require finding
and updating every position that was open during that hour. That would make the
cost of an action grow with the number of traders and would eventually make
settlement impossible.

An index instead stores cumulative fee per unit of a stable fee base:

```text
borrow index base   = position risk units
funding index base  = position size
```

A position's pending amount is the current cumulative index value for its fee
base minus the debt value recorded at its previous boundary. Opening,
increasing, decreasing, and settling one position therefore remain independent
of the total number of positions.

Indices also separate time from mutation. First, the index advances for the
elapsed interval using the state and rate that existed during that interval.
Only afterward can a position action change exposure, utilization, skew, or a
future rate.

### 4.2 Global borrow index

The vault stores one global borrow index shared by every market:

```text
borrow_index
borrow_index_remainder
current_borrow_rate
last_global_checkpoint
```

`borrow_index` is cumulative borrow per risk unit at `INDEX_PRECISION`.
`current_borrow_rate` is the utilization-derived bps-per-day rate multiplied by
`INDEX_PRECISION`. The rate remains constant until the next state change that
refreshes it.

For elapsed time `dt`:

```text
denominator = BPS * SECONDS_PER_DAY

numerator =
    current_borrow_rate * dt
    + borrow_index_remainder

borrow_index_delta     = floor(numerator / denominator)
borrow_index_remainder = numerator % denominator
borrow_index          += borrow_index_delta
```

The index contains the complete sequence of historical rates. A position does
not need to know when utilization changed; subtracting its baseline captures
all intervals since that baseline was recorded.

The global index and timestamp are never reset, including when no positions
are open. Their monotonic lifetime avoids special cases for positions opened
after an empty period.

### 4.3 Per-market funding indices

Funding depends on one market's directional exposure, so each market has its
own indices. Each side can be a payer in one interval and a receiver in another.
The market therefore stores three cumulative indices for each side:

| Index | Meaning |
|---|---|
| `receiver_backed_payer_index_long` | Receiver-guaranteed funding owed by longs |
| `receiver_backed_payer_index_short` | Receiver-guaranteed funding owed by shorts |
| `lp_backed_payer_index_long` | LP-backed funding owed by longs |
| `lp_backed_payer_index_short` | LP-backed funding owed by shorts |
| `receiver_index_long` | Funding credit earned by longs |
| `receiver_index_short` | Funding credit earned by shorts |

All six are cumulative fee per unit of position size at `INDEX_PRECISION`.
They only increase. A change in payer direction advances a different set of
indices; it never decreases or rewinds an earlier one.

The market also stores the state required to advance them:

```text
skew_ema
long_payer_remainders
short_payer_remainders
pending_receiver_funding
last_funding_checkpoint
current_payer_side
current_payer_rate
```

Each payer-side remainder group contains independent
`receiver_payer_remainder`, `lp_payer_remainder`,
`receiver_liability_remainder`, and `receiver_distribution_remainder` fields.
Long-payer and short-payer carries are never reused by one another.

The displayed payer side and rate describe the state after the latest
checkpoint. Accrued obligations come from cumulative indices, never from these
display fields.

### 4.4 Position debt baselines

Each position stores four monetary debt baselines:

```text
funding_paid_to_receivers_debt
funding_paid_to_lps_debt
funding_received_debt
borrow_debt
```

For the position's direction, the baseline values are recorded with the same
rounding used when the current cumulative value is later read:

```text
funding_paid_to_receivers_debt = ceil(
    position_size * receiver_backed_payer_index / INDEX_PRECISION
)

funding_paid_to_lps_debt = ceil(
    position_size * lp_backed_payer_index / INDEX_PRECISION
)

funding_received_debt = floor(
    position_size * receiver_index / INDEX_PRECISION
)

borrow_debt = ceil(
    position_risk_units * borrow_index / INDEX_PRECISION
)
```

Pending amounts subtract these baselines from current cumulative values:

```text
pending_receiver_funding_owed =
    ceil(position_size * current_receiver_backed_payer_index
         / INDEX_PRECISION)
    - funding_paid_to_receivers_debt

pending_lp_funding_owed =
    ceil(position_size * current_lp_backed_payer_index
         / INDEX_PRECISION)
    - funding_paid_to_lps_debt

pending_funding_received =
    floor(position_size * current_receiver_index
          / INDEX_PRECISION)
    - funding_received_debt

actual_borrow =
    ceil(position_risk_units * current_borrow_index
         / INDEX_PRECISION)
    - borrow_debt

pending_borrow = max(actual_borrow, stored_minimum_borrow_fee)
```

The raw differences are checked before any floor or cap is applied. A negative
difference means an index decreased or a baseline was corrupted and is always
an invariant violation.

### 4.5 Accrual remainders

Repeated integer division must not discard sub-unit accrual. Every cumulative
division uses the same carried-division pattern:

```text
total     = new_numerator + previous_remainder
quotient  = floor(total / divisor)
remainder = total % divisor
```

The quotient advances the relevant index or claim, and the remainder is stored
for the next checkpoint. This makes many short checkpoints converge to the
same result as one long checkpoint.

Borrow needs one remainder for the global rate-to-index division. Funding needs
separate remainders because its divisions represent different ownership
boundaries. The following four remainders exist independently for each payer
direction:

- receiver-backed payer weight into its payer index;
- LP-backed payer weight into its payer index;
- aggregate receiver-backed payer accrual into the guaranteed receiver claim;
  and
- aggregate payer backing into the receiver-side credit index.

Remainders from different divisions or payer directions are never combined.
Each has its own unit and divisor and can only be reused by the exact
calculation and directional stream that produced it.

#### 4.5.1 A carried remainder is only valid for a constant divisor

A carried remainder encodes an undistributed fraction of one divisor. Reusing
it under a different divisor changes the value it represents and is therefore
forbidden.

Three of the four funding remainders divide by a constant: both payer-index
divisions use `BPS * SECONDS_PER_DAY`, and the guaranteed-liability division
uses `INDEX_PRECISION`. Their carries may persist for the lifetime of the
payer stream.

The receiver-distribution division is the exception. It divides by
`receiver.size_open_interest`, which changes whenever a position on the
receiving side is opened, increased, decreased, or removed. A remainder
produced modulo a large receiver size represents a small fraction of a large
base; carried into a division by a smaller receiver size it is credited as a
much larger fraction of a smaller base, and receivers can then be credited
more than the receiver-backed accrual that justifies it.

The rule is therefore:

```text
when a market side's size_open_interest changes, reset the
receiver_distribution_remainder of the opposite side's payer stream to zero
```

The reset runs after the checkpoint and before the exposure mutation, so every
carry lives entirely inside a window of constant receiver size. Because the
checkpoint always precedes the mutation (§4.9), no accrual is lost by the
reset itself.

The discarded fraction is strictly less than one whole cash unit of receiver
credit. It is never re-credited to receivers, so the reset can only
under-distribute. Its guaranteed-liability counterpart remains in
`market.pending_receiver_funding` and is released to LPs by the empty-book
rule in §4.13.

With a constant divisor the cumulative identity is exact: the value
distributed through the receiver index over any sequence of checkpoints equals
the accrued backing minus the final retained remainder, so the aggregate
receiver credit can never exceed the recognized liability. §9.4's guarantee
depends on this reset and does not hold without it.

### 4.6 Funding EMA and exact window integration

Between market checkpoints, long and short exposure are constant. The funding
EMA still moves continuously toward the live skew, so funding cannot be
calculated as the end-of-window rate multiplied by elapsed time.

For live skew `S`, starting EMA `E0`, half-life `H`, and instant weight `w`:

```text
d(t) = 2^(-t / H)
E(t) = S + (E0 - S) * d(t)

A = S
B = (1 - w) * (E0 - S)
I(t) = A + B * d(t)
```

At elapsed time `dt`:

```text
d  = 2^(-dt / H)
J1 = H / ln(2)     * (1 - d)
J2 = H / (2 ln(2)) * (1 - d^2)

quadratic_integral =
    A^2 * dt + 2*A*B*J1 + B^2*J2

funding_weight =
    max_funding_rate_bps_day * quadratic_integral

linear_integral = A * dt + B * J1
```

`linear_integral` is useful for locating and validating a zero crossing, but
its sign does not select one payer for an interval containing both signs.

`funding_weight` is the `INDEX_PRECISION`-scaled integral of bps times seconds.
For an interval on which `I(t)` keeps one sign, that sign selects the payer and
the non-negative quadratic integral determines the amount. Because `I(t)` is
monotonic while live skew is constant, it has at most one zero crossing in a
checkpoint interval. When a crossing exists, deterministically solve for its
fixed-point elapsed duration, integrate the two subintervals independently,
and carry each result to the corresponding payer stream. The crossing uses the
same declared decay quantization as the EMA calculation; it is not rounded to
an arbitrary whole-second boundary before integration.

The decay function is evaluated deterministically in fixed point. Splitting an
unchanged interval into multiple checkpoints must reproduce the one-window
result within the declared decay-quantization tolerance. When the instant
weight is 100%, the EMA contributes nothing and the result is exactly
checkpoint-frequency independent apart from carried integer division.

### 4.7 Global checkpoint

The global checkpoint advances borrow to `now`:

```text
function checkpoint_global(now):
    require now >= last_global_checkpoint

    if now == last_global_checkpoint:
        return

    elapsed = now - last_global_checkpoint

    numerator =
        current_borrow_rate * elapsed
        + borrow_index_remainder

    borrow_index += floor(
        numerator / (BPS * SECONDS_PER_DAY)
    )

    borrow_index_remainder =
        numerator % (BPS * SECONDS_PER_DAY)

    last_global_checkpoint = now
```

This operation never derives a new rate. It accounts for past time using the
rate already stored for that interval. Rate refresh is a separate final step
after all mutations that can change utilization.

A pause does not stop this clock. The next checkpoint includes all elapsed
wall-clock time while positions remained open.

### 4.8 Market checkpoint

The market checkpoint advances one market's funding state to `now`:

```text
function checkpoint_market(market, now):
    require now >= market.last_funding_checkpoint

    if now == market.last_funding_checkpoint:
        return

    elapsed = now - market.last_funding_checkpoint

    segments = integrate_funding_window_by_sign(
        long_base,
        short_base,
        skew_ema,
        instant_weight,
        half_life,
        max_funding_rate,
        elapsed
    )
```

`segments` contains one segment if blended skew keeps one sign and two segments
if it crosses zero. Each segment contains its payer side, non-negative funding
weight, elapsed bounds, and EMA after the segment. Process the segments in time
order. If a segment's selected payer side has nonzero size, its funding weight
is split:

```text
receiver_weight =
    if receiver_size == 0 or receiver_base == 0:
        0
    else if receiver_base >= payer_base:
        segment.funding_weight
    else:
        floor(segment.funding_weight * receiver_base / payer_base)

lp_weight = segment.funding_weight - receiver_weight
```

The carried payer-index divisions are:

```text
(receiver_payer_index_delta, payer_stream.receiver_payer_remainder) = carried_div(
    receiver_weight,
    BPS * SECONDS_PER_DAY,
    payer_stream.receiver_payer_remainder
)

(lp_payer_index_delta, payer_stream.lp_payer_remainder) = carried_div(
    lp_weight,
    BPS * SECONDS_PER_DAY,
    payer_stream.lp_payer_remainder
)
```

The corresponding payer-side indices increase by these deltas. The guaranteed
receiver liability and receiver credit index both derive from the exact scaled
backing represented by the receiver-backed payer-index delta:

```text
receiver_backing_scaled =
    payer_size * receiver_payer_index_delta

(receiver_liability_delta, payer_stream.receiver_liability_remainder) = carried_div(
    receiver_backing_scaled,
    INDEX_PRECISION,
    payer_stream.receiver_liability_remainder
)

pending_receiver_funding_total += receiver_liability_delta
market.pending_receiver_funding += receiver_liability_delta

if receiver_size > 0:
    (receiver_index_delta, payer_stream.receiver_distribution_remainder) = carried_div(
        receiver_backing_scaled,
        receiver_size,
        payer_stream.receiver_distribution_remainder
    )
else:
    receiver_index_delta = 0
```

The receiver-side index increases by `receiver_index_delta`. If there is no
selected payer exposure, no funding index advances, but the EMA and checkpoint
time still advance so historical skew continues to decay correctly.

Finally:

```text
market.skew_ema = segments.ema_after
market.current_payer_side = payer_side_after_checkpoint
market.current_payer_rate = displayed_rate_after_checkpoint
market.last_funding_checkpoint = now
```

Recognizing `receiver_liability_delta` changes non-LP claims and can therefore
change cash LP equity. The global borrow rate is refreshed after the complete
action, not inside the market checkpoint.

### 4.9 Checkpoint and mutation order

Every state-changing action follows one time boundary:

```text
1. Read one authoritative action timestamp.
2. Checkpoint the global borrow index to that timestamp.
3. Checkpoint every affected market to the same timestamp.
4. Calculate the affected position's old pending obligations.
5. Apply the action's settlement waterfall, exposure changes, and claim changes.
6. Update position, market-side, market, and global aggregates together.
7. Reset the receiver-distribution remainder of the opposite payer stream for
   every market side whose size changed (§4.5.1).
8. Refresh market display and risk state from the resulting book.
9. Refresh the global borrow rate from resulting risk units and LP cash equity.
10. Store the completed state atomically.
```

The order prevents two forms of retroactive accounting. New exposure never
earns or owes funding from before it existed, and a utilization change never
reprices borrow time that elapsed before the mutation. Step 7 keeps every
carried division inside a window of constant divisor.

An action touching one market checkpoints only that market. An LP action that
calculates marked vault NAV checkpoints the global index and every active
market from one synchronized price snapshot, because receiver liabilities and
unrealized PnL from every market affect LP value. The active-market set must
remain governance-bounded so this operation has a predictable maximum cost.

Changing a parameter also respects the boundary. Accrual affected by that
parameter is checkpointed under the old value first; the new value applies only
afterward. A global funding-memory update checkpoints every active market. A
per-market funding update checkpoints that market. A borrow-curve update
checkpoints the global borrow index before refreshing its rate.

### 4.10 Opening a borrow window

A position begins a borrow window only after its exposure has been accepted
and the resulting global borrow rate has been derived. The new position stores:

```text
borrow_debt = ceil(
    resulting_risk_units * current_borrow_index / INDEX_PRECISION
)

stored_minimum_borrow_fee = ceil(
    resulting_risk_units
    * current_borrow_rate
    * min_borrow_fee_seconds
    / (INDEX_PRECISION * BPS * SECONDS_PER_DAY)
)
```

`current_borrow_rate` in this formula is the scaled rate after the exposure and
claim mutations. The baseline prevents the new exposure from paying historical
index growth. The monetary minimum fixes the minimum obligation for this one
window without storing a rate history on the position.

The position's three funding debt baselines are initialized from the current
indices in the same step. A position opened into an existing market therefore
begins only at the post-checkpoint boundary.

### 4.11 Settling and resetting a borrow window

An increase or partial decrease closes the old borrow window for the full old
position before changing size:

```text
function settle_and_reset_borrow_window(position, size_after):
    raw_actual =
        ceil(position.risk_units * borrow_index / INDEX_PRECISION)
        - position.borrow_debt

    require raw_actual >= 0

    borrow_due = max(
        raw_actual,
        position.stored_minimum_borrow_fee
    )

    collect borrow_due from old stored collateral
    apply the size and risk-unit mutation
    refresh the global borrow rate

    if size_after > 0:
        position.borrow_debt = ceil(
            risk_units_after * borrow_index / INDEX_PRECISION
        )

        position.stored_minimum_borrow_fee = quote_minimum(
            risk_units_after,
            refreshed_borrow_rate,
            min_borrow_fee_seconds
        )
```

Funding accrued on the old size is settled at the same boundary. After the
mutation, all three funding debt baselines are reset using the resulting size
and the current funding indices.

No old minimum is apportioned to the removed or added size. No old actual
borrow is carried forward. The surviving position starts at zero actual
borrow with one new monetary minimum for its full resulting exposure.

A collateral-only addition changes no index baseline and starts no new borrow
window. A full close, liquidation, or full ADL settles the existing window and
then removes the position, so no new minimum or debt baseline is created.

### 4.12 Reading pending fees without mutation

A read-only quote must report fees as of the requested timestamp without
requiring a state-changing checkpoint. It performs the same calculations on an
in-memory copy:

```text
1. Virtually advance the global borrow index to the quote timestamp.
2. Virtually advance the position's market funding indices to that timestamp.
3. Calculate cumulative index values for the position's size and risk units.
4. Subtract the stored debt baselines.
5. Validate every raw pending amount is non-negative.
6. Apply the stored monetary minimum to raw borrow.
7. Return the pending amounts without saving any state or remainder.
```

The preview and the state-changing checkpoint must use identical arithmetic.
Given the same starting state, timestamp, and market book, a quote immediately
followed by settlement must produce the same pending amounts unless another
transaction changes the state first.

Reading only the last stored indices would understate fees after time has
elapsed. Writing during a quote would make a view operation alter economics.
Virtual advancement avoids both errors.

### 4.13 Empty-book initialization and reset

An empty market has no meaningful funding history. When the first position
opens:

```text
1. Checkpoint the empty market to the action timestamp.
2. Add the first exposure.
3. Set skew_ema to the new live skew.
4. Refresh the displayed payer side and rate.
5. Initialize the position's funding debts from current indices.
```

Seeding the EMA from zero would describe a balanced history that never existed
and would give a one-sided launch a temporary funding discount.

When the final position in a market is removed, the market clears:

```text
skew_ema = 0
current_payer_side = none
current_payer_rate = 0
all market funding remainders = 0
release market.pending_receiver_funding from the market and global claim totals
```

The cumulative funding indices and checkpoint timestamp remain monotonic and
are not reset. A future position records the then-current indices as its debt
baselines.

Because receiver liabilities are attributed per market, empty-market cleanup
does not wait for every other market in the vault to become empty. The global
`pending_receiver_funding_total` must equal the sum of the per-market amounts.
When the final position across the complete vault is removed, that total must
already be zero.

### 4.14 Time and rounding invariants

The index system must always satisfy these properties:

1. Global and market checkpoint timestamps never decrease.
2. Calling a checkpoint twice at the same timestamp is a no-op the second
   time.
3. Borrow and all six funding indices are monotonic.
4. Raw pending index amounts are non-negative before a minimum or cap is
   applied.
5. Past time uses the rate, exposure, skew, and parameters that were active
   during that time.
6. New size begins at current debt baselines and never inherits historical
   accrual.
7. Payer obligations round up at the position boundary; receiver credits
   round down.
8. Guaranteed aggregate receiver credit never exceeds its receiver-backed
   payer accrual.
9. Every repeated division carries its own remainder until its defined reset.
10. Checkpoint frequency cannot materially change economic accrual.
11. A read-only preview and a mutating checkpoint use identical mathematics.
12. An unexpected failure reverts the index, remainder, claim, and timestamp
    changes together.

## 5. Storage model

The storage model contains only authoritative ownership labels, cumulative
accounting state, active economic commitments, and bounded aggregates. Values
that can be reconstructed from these sources are derived instead of stored as
independent authorities.

All cash amounts are non-negative integers at `PRICE_PRECISION` unless a field
is explicitly signed. Rates expressed as bps use `BPS`; precise rates and
indices use `INDEX_PRECISION`. Timestamps are whole seconds and identifiers are
monotonic unsigned integers that are never reused.

### 5.1 Global configuration

Global configuration applies to the complete vault.

| Field | Unit | Meaning and invariant |
|---|---:|---|
| `min_collateral` | Cash | Minimum stored collateral for a surviving position; must exceed `keeper_liquidation_reward` |
| `min_position_lifetime` | Seconds | Minimum time after an open or size increase before voluntary exposure removal |
| `min_borrow_fee_seconds` | Seconds | Duration used to quote each monetary minimum-borrow obligation; initial value `900` |
| `funding_half_life_seconds` | Seconds | Shared half-life of every market's funding EMA; initial value `43,200` and never zero |
| `risk_capacity_limit_bps` | Bps | Maximum share of cash LP equity assignable to risk units; initial value `8,500` |
| `base_borrow_rate_bps_day` | Bps/day | Borrow rate when utilization is zero; initial value `25` |
| `max_variable_borrow_bps_day` | Bps/day | Maximum utilization-dependent addition; initial value `250` |
| `borrow_exponent_bps` | Bps exponent | Shape of the utilization curve; `20,000` means a square |
| `fee_lp_revenue_share_bps` | Bps | LP share of collected opening and closing fees; initial value `9,000` |
| `borrow_lp_revenue_share_bps` | Bps | LP share of collected borrow; initial value `9,000` |
| `referral_fee_share_bps` | Bps | Referral share of collected opening and closing fees; initial value `250` and carved from protocol revenue |
| `max_active_markets` | Count | Hard bound on the active-market registry and any synchronized LP-accounting loop |
| `global_hard_cap_factor_limit_bps` | Bps | Bound on aggregate configured hard-cap exposure across market sides |

The protocol share is not stored as a separate percentage. It is the exact
remainder after the configured LP and applicable referral shares. Configuration
validation therefore requires:

```text
fee_lp_revenue_share_bps + referral_fee_share_bps <= BPS
borrow_lp_revenue_share_bps <= BPS
```

LP request policy is global:

| Field | Unit | Meaning |
|---|---:|---|
| `lp_request_delay_seconds` | Seconds | Delay before an LP request can use its assigned synchronized price snapshot |
| `max_withdraw_utilization_bps` | Bps | Maximum utilization permitted after an LP withdrawal |
| `min_deposit_nav_factor_bps` | Bps | Minimum marked-NAV-to-cash-equity factor for an ordinary share-minting deposit |

Fixed keeper rewards are also global and independently configurable. They are
listed separately below because they form one coherent configuration group.

The removed global fields are equally important: there is no minimum borrow
index delta, user-selected execution budget, keeper revenue share, keeper
reserve, percentage liquidation reward, percentage ADL reward, maximum ADL
reward, or insolvency-touch reward.

### 5.2 Global accounting state

One global accounting record holds the complete non-LP claim totals, global
risk counters, and borrow clock:

| Field | Type and unit | Meaning |
|---|---|---|
| `position_collateral_total` | Cash | Sum of stored collateral across every open position |
| `pending_receiver_funding_total` | Cash | Sum of guaranteed receiver funding attributed to every market but not yet relabelled into receiver positions |
| `action_escrow_total` | Cash | Sum of collateral committed to pending trader actions and held inside the vault |
| `protocol_claimable_total` | Cash | Collected protocol revenue not yet withdrawn |
| `referral_claimable_total` | Cash | Sum of all unclaimed referrer balances |
| `total_risk_units` | Cash-scaled risk units | Sum of risk units across every open position |
| `open_position_count` | Count | Number of stored open positions |
| `restricted_market_side_count` | Count | Number of market sides latched in `Warning`, `ADL`, or `HardCap` |
| `borrow_index` | Index | Global cumulative borrow per risk unit |
| `borrow_index_remainder` | Scaled remainder | Carry for the rate-to-index division |
| `current_borrow_rate` | Index-scaled bps/day | Rate applying from the last global checkpoint forward |
| `last_global_checkpoint` | Timestamp | Time through which the global borrow index has accrued |

The accounting record is loaded, changed as one in-memory unit, and saved once
at the end of an atomic action. Ownership totals must never be updated through
independent storage paths.

Global operational state also stores:

```text
next_position_id
next_action_id
active_market_ids
initialized
paused
configuration_authority
oracle_authority
vault_asset
state_version
```

The monotonically increasing ID counters prevent identifier reuse. The active
market registry contains each market with live accounting state exactly once
and never exceeds `max_active_markets`. Pausing blocks the configured mutation
paths but does not alter either accrual timestamp.

### 5.3 Market configuration

Each market stores its own trading, funding, fee, risk, and execution policy:

| Field | Unit | Meaning and initial value |
|---|---:|---|
| `open_fee_bps` | Bps | Opening fee on added size; `0` |
| `close_size_fee_bps` | Bps | Closing size component; `5` |
| `close_pnl_fee_bps` | Bps | Closing PnL component; `1,000` |
| `max_funding_rate_bps_day` | Bps/day | Quadratic funding rate at full blended skew; `80` |
| `instant_weight_bps` | Bps | Live-skew weight in the funding blend; `3,000` |
| `market_risk_factor_bps` | Bps | Share of position size converted into risk units; `1,000` |
| `initial_margin_bps` | Bps | Margin required after opening or adding risk; `500` |
| `maintenance_margin_bps` | Bps | Ordinary liquidation and surviving-position floor; `250` |
| `recovery_pnl_factor_bps` | Bps | Restricted-side recovery threshold; `250` |
| `warning_pnl_factor_bps` | Bps | Warning-state threshold; `400` |
| `adl_pnl_factor_bps` | Bps | ADL-state threshold; `500` |
| `hard_cap_pnl_factor_bps` | Bps | Hard-cap-state threshold and payout cap; `600` |
| `max_long_size_open_interest` | Notional | Hard long-side size cap |
| `max_short_size_open_interest` | Notional | Hard short-side size cap |
| `max_long_base_exposure` | Base units | Hard long-side base cap |
| `max_short_base_exposure` | Base units | Hard short-side base cap |
| `order_execution_delay_seconds` | Seconds | Minimum delay for trader-requested actions; initial value `5`, validated from `1` through `30` |

Configuration must preserve these orderings:

```text
maintenance_margin_bps <= initial_margin_bps

recovery_pnl_factor_bps
    < warning_pnl_factor_bps
    < adl_pnl_factor_bps
    < hard_cap_pnl_factor_bps
```

Fee rates are independent of market skew. Liquidation and ADL rewards are
global fixed amounts rather than market percentages.

A configuration change first checkpoints every accumulator whose future result
depends on the changed field. The updated value applies only after that
checkpoint.

`market_risk_factor_bps` may change only while the market has no open
positions. This keeps every live position's canonical risk units equal to the
value derived from its complete current size without requiring an unbounded
market-wide position rewrite.

### 5.4 Market accounting state

Each market has a long side and a short side. Each side stores:

| Field | Unit | Invariant |
|---|---:|---|
| `size_open_interest` | Notional | Sum of position size on the side |
| `base_exposure` | Base units | Sum of position base exposure on the side |
| `stored_collateral_total` | Cash | Sum of stored collateral owned by positions on the side |
| `risk_units` | Risk units | Sum of position risk units on the side |
| `risk_state` | Enum | One of `Normal`, `Warning`, `ADL`, or `HardCap` |

The market-level funding record stores:

| Field | Unit | Meaning |
|---|---:|---|
| `receiver_backed_payer_index_long` | Index | Guaranteed-receiver funding owed per unit of long size |
| `receiver_backed_payer_index_short` | Index | Guaranteed-receiver funding owed per unit of short size |
| `lp_backed_payer_index_long` | Index | LP-backed funding owed per unit of long size |
| `lp_backed_payer_index_short` | Index | LP-backed funding owed per unit of short size |
| `receiver_index_long` | Index | Funding credit per unit of long size |
| `receiver_index_short` | Index | Funding credit per unit of short size |
| `skew_ema` | Signed index fraction | Historical skew after the last funding checkpoint |
| `last_funding_checkpoint` | Timestamp | Time through which market funding has accrued |
| `long_payer_remainders` | Remainder group | Four independent carries for receiver-payer, LP-payer, liability, and receiver-distribution divisions while longs pay |
| `short_payer_remainders` | Remainder group | The corresponding four independent carries while shorts pay |
| `pending_receiver_funding` | Cash | Guaranteed receiver funding attributable to this market and included in the global total |
| `current_payer_side` | Enum | Display value: `None`, `Long`, or `Short` after the latest checkpoint |
| `current_payer_rate` | Index-scaled bps/day | Display rate after the latest checkpoint |

The six cumulative funding indices and checkpoint timestamp never decrease.
Display fields do not create obligations and are never used instead of index
deltas. Market configuration is stored with or keyed by this accounting state
but remains logically distinct from it.

### 5.5 Position state

One open position stores:

| Field | Type and unit | Meaning and invariant |
|---|---|---|
| `position_id` | Identifier | Unique and never reused |
| `owner` | Address | Account authorized to create voluntary actions for the position |
| `market_id` | Market identifier | Market whose aggregates include this position |
| `direction` | Enum | `Long` or `Short`; immutable for the position lifetime |
| `size` | Notional | Current positive USD exposure |
| `base_exposure` | Base units | Current positive underlying exposure |
| `stored_collateral` | Cash | Trader-owned cash label currently attached to the position |
| `risk_units` | Risk units | Canonical capacity usage derived from complete current size and the market risk factor |
| `borrow_debt` | Cash | Rounded cumulative borrow value at the active window's baseline |
| `stored_minimum_borrow_fee` | Cash | Fixed monetary minimum for the active borrow window |
| `funding_paid_to_receivers_debt` | Cash | Rounded receiver-backed payer value at the last funding boundary |
| `funding_paid_to_lps_debt` | Cash | Rounded LP-backed payer value at the last funding boundary |
| `funding_received_debt` | Cash | Rounded receiver-credit value at the last funding boundary |
| `opened_at` | Timestamp | Time the first exposure was successfully created |
| `last_size_increase_at` | Timestamp | Start of the current minimum-position-lifetime restriction |
| `pending_mutation_action_id` | Optional identifier | The one pending voluntary increase, decrease, or close for this position |
| `take_profit` | Optional trigger | Attached take-profit instruction |
| `stop_loss` | Optional trigger | Attached stop-loss instruction |

There is no separately authoritative entry price. Size and base exposure
together preserve the complete price exposure, including multiple increases.
There is also no execution budget, keeper reserve allocation, accrued-fee
counter, cached PnL, cached effective collateral, or cached health value.

An attached trigger stores the minimum information needed to execute safely:

```text
TriggerInstruction {
    trigger_price
    acceptable_price
    committed_at
    execute_after
    commit_observed_at
}
```

Take-profit and stop-loss may coexist because they represent opposite exit
conditions. They are position instructions, not prepaid keeper budgets. Their
keeper rewards come from position value when one executes.

The position record, its market-side aggregates, and
`position_collateral_total` must change together. A surviving position always
has positive size and base exposure, satisfies minimum collateral and the
required health floor, and has non-negative pending index deltas.

### 5.6 Pending-action state

Trader-requested price-sensitive mutations are represented by a unified
pending-action record. Only pending actions are stored as executable objects;
terminal outcomes remove the record and are preserved by their emitted result.

Every action has a common header:

```text
PendingActionHeader {
    action_id
    owner
    market_id
    kind
    created_at
    execute_after
    commit_observed_at
}
```

`kind` selects one immutable payload:

```text
MarketOpen {
    direction
    size
    submitted_collateral
    acceptable_price
    expires_at
    take_profit
    stop_loss
}

LimitOpen {
    direction
    size
    submitted_collateral
    acceptable_price
    trigger_price
    trigger_above
    expires_at
    take_profit
    stop_loss
}

Increase {
    position_id
    size_added
    collateral_added
    acceptable_price
}

Decrease {
    position_id
    size_removed
    acceptable_price
}

Close {
    position_id
    acceptable_price
}
```

The header freezes the execution delay at creation:

```text
execute_after = created_at + order_execution_delay_seconds_at_creation
```

A later configuration change does not alter an existing commitment.
`commit_observed_at` is the qualifying oracle cursor recorded at creation; an
execution observation must be strictly newer.

The payload freezes every trader-controlled economic input. Settlement may
read current vault state and current position state, but it must not replace a
committed size, trigger, acceptable price, direction, or collateral amount with
new caller input.

`pending_mutation_action_id` provides an O(1) reverse reference from a position
to its one ordinary pending mutation. It prevents conflicting increases,
decreases, or closes from being committed against the same pre-action state.
Liquidation and ADL remain able to invalidate this reference as forced safety
actions.

An action ID is consumed permanently when its record is removed. A later call
with that ID fails as nonexistent and cannot replay its transfer or keeper
payment.

### 5.7 Entry escrow

Collateral that must be available for future execution is transferred at
action creation and stored as action escrow. The pending action stores its
individual amount and the global accounting record stores the aggregate:

```text
action_escrow_total =
    sum(escrowed_collateral for every pending collateral-bearing action)
```

For a market or limit entry:

```text
escrowed_collateral = submitted_collateral
```

For an increase that adds collateral, the added amount is likewise committed
at creation. An increase without added collateral and every decrease or close
has zero escrow.

Escrow is an explicit non-LP claim. It is physical cash in the vault but is not
LP equity, position collateral, fee revenue, or available risk backing. It does
not enter market aggregates before successful settlement.

Terminal distribution must reduce both the action's escrow and
`action_escrow_total` by exactly the same amount:

```text
successful entry:
    opening fee       -> LP/protocol/referral ownership
    keeper reward     -> keeper transfer
    remainder         -> position collateral

successful increase with added collateral:
    complete escrow   -> position collateral
    opening fee       -> deducted from resulting position collateral
                         before final health checks

expected entry failure or expiry:
    keeper reward     -> keeper transfer
    remainder         -> owner refund

owner limit cancellation:
    complete escrow   -> owner refund
```

No terminal action can leave positive escrow attached to a removed record.

### 5.8 Referral state

Referral accounting uses three persistent maps and one global aggregate:

| Mapping | Value | Rule |
|---|---|---|
| `referral_code_owner[code]` | Address | First valid registration wins; owner is immutable |
| `trader_referrer[trader]` | Optional address | Trader can replace it; self-referral is invalid |
| `referral_balance[referrer]` | Cash | Accumulated unclaimed rewards |
| `referral_claimable_total` | Cash aggregate | Must equal the sum of all referral balances |

A fee uses the trader's valid referrer at the moment the fee is collected.
Accrual increases the individual balance and aggregate in the same operation.
A claim decreases both by the exact transferred amount.

Referral ownership and positive balances are durable economic state. Storage
maintenance or archival must never silently erase a code owner or unclaimed
balance.

### 5.9 LP claim state

LP deposits and withdrawals use delayed FIFO requests. Each request stores:

```text
LpRequest {
    request_id
    owner
    kind              # Deposit or Withdrawal
    escrowed_amount   # Collateral for a deposit, LP shares for a withdrawal
    requested_at
    execute_after
    status            # Pending, Settled, or Failed
}
```

Global LP request state stores:

```text
next_lp_request_id
next_lp_request_to_resolve
```

Only the FIFO head can resolve, and every terminal resolution advances the
pointer exactly once. A deposit's collateral remains outside vault physical
cash until successful settlement. A withdrawal's escrowed shares remain in
total share supply until they are burned on success. Failed or expired requests
return their complete escrow.

There is no persistent pending-withdrawal cash claim and no partial LP fill. A
request settles fully or refunds fully, so LP request escrow is not included in
the vault's non-LP cash claims.

### 5.10 Keeper reward configuration

The global keeper-reward group contains:

| Field | Payment source |
|---|---|
| `keeper_open_reward` | Entry escrow |
| `keeper_limit_order_reward` | Entry escrow |
| `keeper_increase_reward` | Stored position collateral |
| `keeper_decrease_reward` | Stored position collateral |
| `keeper_close_reward` | Position value |
| `keeper_tp_reward` | Position value |
| `keeper_sl_reward` | Position value |
| `keeper_expiry_reward` | Action escrow |
| `keeper_liquidation_reward` | Position value, with LP gap backstop |
| `keeper_adl_reward` | Position payable profit or collateral |
| `keeper_lp_resolve_reward` | LP request escrow or released assets |

Each field is a fixed cash amount with an initial value of `2,500,000`, or
`$0.25`. The fields are independent even though their initial values match.

The configuration stores no generic execution reward and no user override.
Every settlement kind maps to exactly one field. Validation requires:

```text
min_collateral > keeper_liquidation_reward
keeper_open_reward <= min_collateral
keeper_limit_order_reward <= min_collateral
keeper_expiry_reward <= min_collateral
```

Entry-order creation must also guarantee that its actual escrow can pay the
applicable open, limit, or expiry reward.

### 5.11 Aggregate exposure and risk state

The storage layout deliberately duplicates bounded aggregates so actions never
loop over positions. That duplication is valid only while these equalities
hold:

```text
global.total_risk_units
    = sum(position.risk_units)
    = sum(all market-side risk_units)

global.open_position_count
    = count(stored open positions)

global.position_collateral_total
    = sum(position.stored_collateral)
    = sum(all market-side stored_collateral_total)

global.pending_receiver_funding_total
    = sum(market.pending_receiver_funding)

market.long.size_open_interest
    = sum(size of long positions in that market)

market.short.size_open_interest
    = sum(size of short positions in that market)

market.long.base_exposure
    = sum(base exposure of long positions in that market)

market.short.base_exposure
    = sum(base exposure of short positions in that market)

global.restricted_market_side_count
    = count(market sides whose risk state is not Normal)
```

An exposure mutation updates the position, its side, its market, and the global
record in one atomic transition. The active-market registry contains a market
while its state can affect synchronized accounting. A market can leave the
registry only after its open interest, claims, remainders requiring settlement,
and restricted states are cleared.

Risk-state latches are authoritative stored state. Current positive-PnL factors
are derived from a synchronized price snapshot, but a restricted side remains
restricted until the recovery rule explicitly transitions its stored state.

### 5.12 Liability totals

The cash ownership equation is:

```text
non_lp_claims =
      position_collateral_total
    + pending_receiver_funding_total
    + action_escrow_total
    + protocol_claimable_total
    + referral_claimable_total

if physical_cash >= non_lp_claims:
    physical_cash = cash_lp_equity + non_lp_claims
else:
    vault_shortfall = non_lp_claims - physical_cash
```

`pending_receiver_funding_total` is not the sum of current position credits.
It is the sum of `market.pending_receiver_funding` across active markets. Each
market amount is accrued from receiver-backed payer indices, reduced when
credits are moved into that market's receiver positions, and released when
that market becomes empty and no receiver in it can remain.

`protocol_claimable_total` increases only from actually collected protocol
revenue and decreases only with a protocol claim. It has no per-fee receivable
for amounts a position could not pay.

Keeper rewards do not appear in the liability equation because they are paid
atomically. LP revenue does not appear because LPs own the residual. There is
no keeper-reserve or execution-budget liability.

### 5.13 Derived values that are not stored

The following values must be recalculated from authoritative state:

| Derived value | Authoritative inputs |
|---|---|
| Physical cash | Vault collateral-token balance |
| Non-LP claims | Five global claim totals |
| Cash LP equity and shortfall | Physical cash and non-LP claims |
| Marked vault NAV | Cash LP equity, market-side aggregates, and synchronized prices |
| LP share price | Marked NAV and actual share-token supply |
| Required risk backing and free LP capital | Total risk units, cash LP equity, and capacity limit |
| Utilization | Total risk units and cash LP equity |
| Next borrow rate | Utilization and global curve parameters |
| Position entry price | Position size and base exposure |
| Raw and payable PnL | Position exposure, price, market risk state, and LP equity |
| Pending funding | Position size, market indices, and funding debts |
| Pending borrow | Position risk units, global index, borrow debt, and stored minimum |
| Effective collateral | Stored collateral, payable PnL, and pending fees |
| Liquidation eligibility | Effective collateral, size, margin config, and fixed reward |
| Opening and closing fees | Action values and market fee configuration |

Snapshots of these values may be returned or emitted, but they are not stored
as competing accounting authorities. A cache used only for display must never
be accepted by a settlement path in place of a fresh derivation.

### 5.14 State cleanup

Cleanup is part of every terminal transition:

- A full close, liquidation, or full ADL removes the position record, clears
  its pending-action reverse reference and attached triggers, decrements the
  open-position count, and removes its complete exposure and collateral from
  every aggregate.
- A forced position removal invalidates any ordinary pending mutation that
  references the position. Any collateral escrowed by that mutation is
  refunded according to its terminal cleanup rule.
- Successful, failed, cancelled, expired, and superseded trader actions remove
  their executable pending record. Their IDs are never reused, and the
  terminal result is emitted before completion.
- Removing a collateral-bearing action sets its individual escrow to zero and
  reduces `action_escrow_total` by exactly the distributed amount.
- When a market becomes empty, its EMA, payer display, risk state, and funding
  remainders reset as specified by the empty-book rule. Its cumulative indices
  and checkpoint timestamp remain monotonic.
- When a market has no open positions, its unassigned guaranteed-receiver
  residue is released to LP equity and deducted from the global aggregate.
- A claimed protocol or referral balance is reduced before its cash transfer;
  a failed transfer reverts both changes.
- A terminal LP request retains its status for FIFO history but holds no
  escrow. The FIFO pointer cannot remain on a terminal request.

No live position, pending action, escrow, referral balance, or protocol claim
may disappear because of storage expiry. Storage lifetime must be extended or
the state must remain restorable for as long as the economic obligation exists.

## 6. Core accounting algorithms

The pseudocode in this section defines reusable accounting primitives. Every
user-facing operation later composes these primitives in the stated order.

The following conventions apply:

```text
mul_div_floor(a, b, d) = floor(a * b / d)
mul_div_ceil(a, b, d)  = ceil(a * b / d)
carried_div(n, d, r)   = (floor((n + r) / d), (n + r) % d)
```

All additions, subtractions, multiplications, and conversions are checked.
Unless a function explicitly permits partial collection, insufficient value is
an error. Any error reverts every state change and transfer made by the action.

Position-collateral mutations always update three authorities together:

```text
function add_position_collateral(position, market_side, ledger, amount):
    require amount >= 0
    position.stored_collateral += amount
    market_side.stored_collateral_total += amount
    ledger.position_collateral_total += amount

function remove_position_collateral(position, market_side, ledger, amount):
    require 0 <= amount <= position.stored_collateral
    position.stored_collateral -= amount
    market_side.stored_collateral_total -= amount
    ledger.position_collateral_total -= amount
```

LP equity is the residual and has no counter to update. Reducing a position or
escrow claim without creating another claim credits LPs automatically.

### 6.1 Accrue global borrow

```text
function accrue_global_borrow(ledger, now):
    require now >= ledger.last_global_checkpoint

    if now == ledger.last_global_checkpoint:
        return

    elapsed = now - ledger.last_global_checkpoint
    denominator = BPS * SECONDS_PER_DAY

    numerator =
        ledger.current_borrow_rate * elapsed
        + ledger.borrow_index_remainder

    index_delta = floor(numerator / denominator)
    next_remainder = numerator % denominator

    ledger.borrow_index += index_delta
    ledger.borrow_index_remainder = next_remainder
    ledger.last_global_checkpoint = now
```

`current_borrow_rate` is already scaled by `INDEX_PRECISION`. The function does
not recalculate it. The stored rate prices the entire elapsed interval, and the
new rate is derived only after the enclosing action finishes its economic
mutation.

### 6.2 Accrue market funding

```text
function accrue_market_funding(ledger, market, now, global_config):
    require now >= market.last_funding_checkpoint

    if now == market.last_funding_checkpoint:
        return

    elapsed = now - market.last_funding_checkpoint

    segments = integrate_funding_window_by_sign(
        market.long.base_exposure,
        market.short.base_exposure,
        market.skew_ema,
        market.config.instant_weight_bps,
        global_config.funding_half_life_seconds,
        market.config.max_funding_rate_bps_day,
        elapsed
    )
```

Process each nonzero-sign segment in chronological order. Select its payer and
receiver aggregates from the segment sign and select the remainder group keyed
by that payer direction. If its funding weight or payer size is zero, skip its
index and liability changes. In all cases, advance the EMA and timestamp for
the complete window.

For each processed segment:

```text
if receiver.size_open_interest == 0 or receiver.base_exposure == 0:
    receiver_weight = 0
else if receiver.base_exposure >= payer.base_exposure:
    receiver_weight = segment.funding_weight
else:
    receiver_weight = mul_div_floor(
        segment.funding_weight,
        receiver.base_exposure,
        payer.base_exposure
    )

lp_weight = segment.funding_weight - receiver_weight

(receiver_payer_delta, payer_stream.receiver_payer_remainder) = carried_div(
    receiver_weight,
    BPS * SECONDS_PER_DAY,
    payer_stream.receiver_payer_remainder
)

(lp_payer_delta, payer_stream.lp_payer_remainder) = carried_div(
    lp_weight,
    BPS * SECONDS_PER_DAY,
    payer_stream.lp_payer_remainder
)

payer.receiver_backed_payer_index += receiver_payer_delta
payer.lp_backed_payer_index += lp_payer_delta
```

Create the guaranteed aggregate receiver liability and receiver credit index
from the same exact backing:

```text
receiver_backing_scaled =
    payer.size_open_interest * receiver_payer_delta

(liability_delta, payer_stream.receiver_liability_remainder) = carried_div(
    receiver_backing_scaled,
    INDEX_PRECISION,
    payer_stream.receiver_liability_remainder
)

ledger.pending_receiver_funding_total += liability_delta
market.pending_receiver_funding += liability_delta

if receiver.size_open_interest > 0:
    (receiver_index_delta, payer_stream.receiver_distribution_remainder) = carried_div(
        receiver_backing_scaled,
        receiver.size_open_interest,
        payer_stream.receiver_distribution_remainder
    )

    receiver.receiver_index += receiver_index_delta
```

Finish the checkpoint in every branch:

```text
market.skew_ema = segments.ema_after
market.current_payer_side = payer_side_at_window_end
market.current_payer_rate = displayed_rate_at_window_end
market.last_funding_checkpoint = now
```

The liability credit changes cash LP equity but not physical cash. The enclosing
action refreshes global borrow only after all such claim changes are complete.

### 6.3 Calculate pending borrow

```text
function calculate_pending_borrow(position, ledger):
    cumulative_value = mul_div_ceil(
        position.risk_units,
        ledger.borrow_index,
        INDEX_PRECISION
    )

    raw_actual = cumulative_value - position.borrow_debt
    require raw_actual >= 0

    return PendingBorrow {
        actual: raw_actual,
        minimum: position.stored_minimum_borrow_fee,
        due: max(raw_actual, position.stored_minimum_borrow_fee)
    }
```

The negative check runs before the minimum is applied. A minimum can increase a
valid obligation but can never conceal a broken baseline.

### 6.4 Calculate pending funding

```text
function calculate_pending_funding(position, market):
    indices = funding_indices_for_direction(
        market,
        position.direction
    )

    owed_to_receivers =
        mul_div_ceil(
            position.size,
            indices.receiver_backed_payer,
            INDEX_PRECISION
        )
        - position.funding_paid_to_receivers_debt

    owed_to_lps =
        mul_div_ceil(
            position.size,
            indices.lp_backed_payer,
            INDEX_PRECISION
        )
        - position.funding_paid_to_lps_debt

    received =
        mul_div_floor(
            position.size,
            indices.receiver,
            INDEX_PRECISION
        )
        - position.funding_received_debt

    require owed_to_receivers >= 0
    require owed_to_lps >= 0
    require received >= 0

    return PendingFunding {
        owed_to_receivers,
        owed_to_lps,
        received
    }
```

Reading pending funding never changes the guaranteed receiver liability. That
liability was created when the market index accrued.

Moving received funding into a position is an ownership relabel:

```text
function credit_received_funding(position, side, market, ledger, received):
    require market.pending_receiver_funding >= received
    require ledger.pending_receiver_funding_total >= received
    market.pending_receiver_funding -= received
    ledger.pending_receiver_funding_total -= received
    add_position_collateral(position, side, ledger, received)
```

Total non-LP claims remain unchanged.

### 6.5 Calculate raw and payable PnL

```text
function calculate_raw_pnl(direction, size, base_exposure, price):
    if direction == Long:
        marked_value = mul_div_floor(
            base_exposure,
            price,
            PRICE_PRECISION
        )
        return marked_value - size

    buyback_value = mul_div_ceil(
        base_exposure,
        price,
        PRICE_PRECISION
    )
    return size - buyback_value
```

Payable PnL applies the side-wide hard cap only to positive raw PnL, then
applies the independent LP-equity clamp:

```text
function calculate_payable_pnl(
    position_raw_pnl,
    market_side,
    market_config,
    mark_price,
    cash_lp_equity
):
    if position_raw_pnl <= 0:
        return position_raw_pnl

    capped_pnl = position_raw_pnl

    if market_side.risk_state == HardCap:
        side_raw_pnl = calculate_raw_pnl(
            market_side.direction,
            market_side.size_open_interest,
            market_side.base_exposure,
            mark_price
        )

        side_positive_pnl = max(side_raw_pnl, 0)

        if side_positive_pnl > 0:
            hard_cap_value = mul_div_floor(
                cash_lp_equity,
                market_config.hard_cap_pnl_factor_bps,
                BPS
            )

            capped_pnl = mul_div_floor(
                position_raw_pnl,
                min(hard_cap_value, side_positive_pnl),
                side_positive_pnl
            )

    return min(capped_pnl, cash_lp_equity)
```

A latched `HardCap` side with no aggregate positive PnL applies no side factor.
This is a normal state, not an invariant violation, and must not revert.

The risk state is a stored latch while `side_positive_pnl` is derived from the
current price and the current book, so the two disagree routinely. A single
profitable position on a side whose net aggregate PnL is zero or negative is
the ordinary case, and the latch also survives a price move that removes the
aggregate liability entirely until a recovery transition clears it. Rejecting
that combination would make every settlement path that prices a position on
the latched side revert, including liquidation, voluntary close, TP, SL, ADL,
and the read-only quote in §4.12 — that is, the protocol would lose the
ability to reduce risk exactly while a side is flagged as its riskiest.

The position-level LP-equity clamp still applies in this branch, so a payout
can never exceed the cash available to back it.

For a partial decrease, first derive `size_removed` and `base_removed`, then run
the same raw and payable calculations on those removed quantities. The
surviving exposure is not realized.

Applying payable PnL changes ownership:

```text
function apply_payable_pnl(
    position,
    side,
    ledger,
    payable_pnl,
    available_cash_lp_equity
):
    if payable_pnl > 0:
        require available_cash_lp_equity >= payable_pnl
        add_position_collateral(position, side, ledger, payable_pnl)
        return PnlResult { credited: payable_pnl, uncollectible_loss: 0 }

    loss = abs(payable_pnl)
    collected_loss = min(loss, position.stored_collateral)
    remove_position_collateral(position, side, ledger, collected_loss)

    return PnlResult {
        credited: 0,
        uncollectible_loss: loss - collected_loss
    }
```

Removing collateral for a trader loss credits LP residual equity. An
uncollectible remainder is reported during terminal settlement and cannot be
left attached to a surviving position. A caller on a surviving path must
therefore assert `uncollectible_loss == 0` and revert otherwise; only terminal
settlement may consume a nonzero remainder and report it as bad debt.

### 6.6 Calculate effective collateral

```text
function calculate_effective_collateral(
    position,
    payable_pnl,
    pending_funding,
    pending_borrow
):
    return
          position.stored_collateral
        + payable_pnl
        + pending_funding.received
        - pending_funding.owed_to_receivers
        - pending_funding.owed_to_lps
        - pending_borrow.due
```

For a proposed action, subtract its fixed keeper reward and any other charge
that must be paid before the post-action health check. The closing fee is not
included in liquidation health because liquidation pays no closing fee and a
voluntary closing fee cannot consume original collateral.

All components are calculated from indices and one authenticated price snapshot
at the same action timestamp.

### 6.7 Settle a borrow window

The surviving-position path requires full payment from stored collateral:

```text
function settle_borrow_window_for_survivor(
    position,
    side,
    ledger,
    global_config
):
    pending = calculate_pending_borrow(position, ledger)
    require position.stored_collateral >= pending.due

    remove_position_collateral(
        position,
        side,
        ledger,
        pending.due
    )

    distribute_borrow_revenue(
        ledger,
        pending.due,
        global_config
    )

    return pending.due
```

After the size and risk-unit mutation and after the global rate is refreshed,
start the replacement window:

```text
function initialize_borrow_window(position, ledger, global_config):
    position.borrow_debt = mul_div_ceil(
        position.risk_units,
        ledger.borrow_index,
        INDEX_PRECISION
    )

    position.stored_minimum_borrow_fee = mul_div_ceil(
        position.risk_units * ledger.current_borrow_rate,
        global_config.min_borrow_fee_seconds,
        INDEX_PRECISION * BPS * SECONDS_PER_DAY
    )
```

The multiplication is evaluated through a full-precision checked operation;
the grouped expression does not authorize an overflowing intermediate.

Terminal settlement calculates the same `pending.due` but can collect less if
the complete position value is exhausted. Only the collected amount becomes
borrow revenue, and no new window is initialized.

### 6.8 Calculate the opening fee

```text
function calculate_opening_fee(added_size, market_config):
    require added_size > 0

    return mul_div_ceil(
        added_size,
        market_config.open_fee_bps,
        BPS
    )
```

For an entry action, quote before state mutation and validate the hypothetical
position using:

```text
initial_position_collateral =
      action.escrowed_collateral
    - opening_fee
    - selected_keeper_reward
```

The fee is collected and distributed only after every successful-entry check
passes. A size increase uses `size_added` and deducts the fee from the
position's available value as part of successful settlement. A collateral-only
addition never calls this function.

### 6.9 Calculate the closing fee

```text
function calculate_closing_fee(
    size_removed,
    payable_price_pnl,
    pending_funding,
    borrow_due,
    keeper_reward,
    market_config
):
    if payable_price_pnl <= 0:
        return ClosingFee {
            size_component: 0,
            pnl_component: 0,
            nominal: 0,
            collectible: 0
        }

    size_component = mul_div_ceil(
        size_removed,
        market_config.close_size_fee_bps,
        BPS
    )

    pnl_component = mul_div_ceil(
        payable_price_pnl,
        market_config.close_pnl_fee_bps,
        BPS
    )

    nominal = min(
        payable_price_pnl,
        max(size_component, pnl_component)
    )

    profit_after_senior_items = max(
        0,
          payable_price_pnl
        + pending_funding.received
        - pending_funding.owed_to_receivers
        - pending_funding.owed_to_lps
        - borrow_due
        - keeper_reward
    )

    collectible = min(nominal, profit_after_senior_items)

    return ClosingFee {
        size_component,
        pnl_component,
        nominal,
        collectible
    }
```

Only `collectible` is debited and distributed. `nominal - collectible` is
waived immediately and is never stored.

### 6.10 Apply the settlement waterfall

There are two capitalization paths because a surviving mutation must settle
its old index window before changing exposure, while a terminal settlement can
use the position's complete final value.

For an increase or partial decrease that will leave the position open:

```text
function capitalize_for_surviving_mutation(
    position,
    side,
    ledger,
    market,
    pending_funding,
    pending_borrow,
    global_config
):
    credit_received_funding(
        position,
        side,
        market,
        ledger,
        pending_funding.received
    )

    require position.stored_collateral
        >= pending_funding.owed_to_receivers
         + pending_funding.owed_to_lps
         + pending_borrow.due

    remove_position_collateral(
        position,
        side,
        ledger,
        pending_funding.owed_to_receivers
    )

    remove_position_collateral(
        position,
        side,
        ledger,
        pending_funding.owed_to_lps
    )

    remove_position_collateral(
        position,
        side,
        ledger,
        pending_borrow.due
    )

    distribute_borrow_revenue(
        ledger,
        pending_borrow.due,
        global_config
    )

    return CapitalizedSeniorItems {
        funding_received: pending_funding.received,
        receiver_funding_paid: pending_funding.owed_to_receivers,
        lp_funding_paid: pending_funding.owed_to_lps,
        borrow_paid: pending_borrow.due
    }
```

Receiver-backed collection restores residual cash that provisionally backed
the guaranteed claim. LP-backed collection belongs entirely to LP residual
cash. Neither creates a new explicit claim. Borrow alone invokes its revenue
split.

After capitalization, the action applies its size mutation and any realized
PnL, pays its selected keeper reward, and collects a closing fee when the
action is a profitable decrease. Every debt baseline is then reset from the
resulting size and current indices.

For a terminal close, liquidation, or ADL:

```text
function collect_up_to_position_value(
    position,
    side,
    ledger,
    requested
):
    collected = min(requested, position.stored_collateral)
    remove_position_collateral(position, side, ledger, collected)
    return collected

function settle_terminal_position(inputs):
    credit_received_funding(
        position,
        side,
        market,
        ledger,
        inputs.pending_funding.received
    )

    if inputs.payable_pnl > 0:
        apply_payable_pnl(
            position,
            side,
            ledger,
            inputs.payable_pnl,
            inputs.available_cash_lp_equity
        )

    receiver_collected = collect_up_to_position_value(
        position,
        side,
        ledger,
        inputs.pending_funding.owed_to_receivers
    )

    if inputs.payable_pnl < 0:
        pnl_loss_collected = collect_up_to_position_value(
            position,
            side,
            ledger,
            abs(inputs.payable_pnl)
        )

    lp_funding_collected = collect_up_to_position_value(
        position,
        side,
        ledger,
        inputs.pending_funding.owed_to_lps
    )

    borrow_collected = collect_up_to_position_value(
        position,
        side,
        ledger,
        inputs.pending_borrow.due
    )
    distribute_borrow_revenue(
        ledger,
        borrow_collected,
        inputs.global_config
    )

    pay_selected_keeper_reward(inputs.keeper_policy)

    if inputs.close_reason permits closing fee:
        closing_fee = calculate_closing_fee(
            inputs.size,
            max(inputs.payable_pnl, 0),
            inputs.pending_funding,
            inputs.pending_borrow.due,
            inputs.keeper_reward,
            inputs.market_config
        )

        closing_collected = min(
            closing_fee.collectible,
            position.stored_collateral
        )

        remove_position_collateral(
            position,
            side,
            ledger,
            closing_collected
        )
        distribute_open_close_revenue(
            ledger,
            closing_collected,
            inputs.referrer,
            inputs.global_config
        )

    trader_payout = position.stored_collateral
    remove_position_collateral(
        position,
        side,
        ledger,
        trader_payout
    )
    transfer_cash(owner, trader_payout)

    report every unpaid senior amount and uncollectible PnL
    remove complete position exposure and state
```

`collect_up_to_position_value` reduces position collateral by the smaller of
the requested amount and the value available at that priority. A voluntary
partial action never uses this capped helper: it either pays senior items in
full or reverts. Liquidation's keeper reward can use LP backing after position
value is exhausted; ordinary keeper payments follow their action-specific
eligibility rules.

### 6.11 Distribute collected revenue

Opening and closing fee distribution:

```text
function distribute_open_close_revenue(
    ledger,
    collected_fee,
    referrer,
    global_config
):
    require collected_fee >= 0

    lp_amount = mul_div_floor(
        collected_fee,
        global_config.fee_lp_revenue_share_bps,
        BPS
    )

    referral_amount =
        if referrer exists:
            mul_div_floor(
                collected_fee,
                global_config.referral_fee_share_bps,
                BPS
            )
        else:
            0

    protocol_amount =
        collected_fee - lp_amount - referral_amount

    ledger.protocol_claimable_total += protocol_amount

    if referral_amount > 0:
        referral_balance[referrer] += referral_amount
        ledger.referral_claimable_total += referral_amount

    return RevenueSplit {
        lp_amount,
        protocol_amount,
        referral_amount
    }
```

The fee must already have been removed from position collateral or action
escrow. The LP amount receives no stored credit because it is the remaining
residual.

Borrow distribution:

```text
function distribute_borrow_revenue(ledger, collected_borrow, global_config):
    lp_amount = mul_div_floor(
        collected_borrow,
        global_config.borrow_lp_revenue_share_bps,
        BPS
    )

    protocol_amount = collected_borrow - lp_amount
    ledger.protocol_claimable_total += protocol_amount

    return RevenueSplit {
        lp_amount,
        protocol_amount,
        referral_amount: 0
    }
```

Funding never calls either revenue-distribution function.

### 6.12 Pay a keeper reward

```text
function keeper_reward_for(action_kind, global_config):
    match action_kind:
        MarketOpen  -> keeper_open_reward
        LimitOpen   -> keeper_limit_order_reward
        Increase    -> keeper_increase_reward
        Decrease    -> keeper_decrease_reward
        Close       -> keeper_close_reward
        TakeProfit  -> keeper_tp_reward
        StopLoss    -> keeper_sl_reward
        Expiry      -> keeper_expiry_reward
        Liquidation -> keeper_liquidation_reward
        ADL         -> keeper_adl_reward
        LpResolve   -> keeper_lp_resolve_reward
```

Pay from action escrow:

```text
function pay_keeper_from_escrow(action, ledger, keeper, reward):
    require action.escrowed_collateral >= reward

    action.escrowed_collateral -= reward
    ledger.action_escrow_total -= reward
    transfer_cash(keeper, reward)
```

Pay from position collateral:

```text
function pay_keeper_from_position(position, side, ledger, keeper, reward):
    require position.stored_collateral >= reward

    remove_position_collateral(position, side, ledger, reward)
    transfer_cash(keeper, reward)
```

Liquidation guarantees the configured reward:

```text
function pay_liquidation_keeper(
    position,
    side,
    ledger,
    physical_cash,
    keeper,
    reward
):
    from_position = min(position.stored_collateral, reward)
    remove_position_collateral(position, side, ledger, from_position)

    lp_backstop = reward - from_position
    require derive_cash_lp_equity(ledger, physical_cash) >= lp_backstop

    transfer_cash(keeper, reward)

    return KeeperPayment {
        from_position,
        from_lp_backstop: lp_backstop
    }
```

No keeper payment creates a keeper claim or reserve entry. A reverted action
reverts the cash transfer and its source debit together.

### 6.13 Update exposure aggregates

Derive new exposure from the execution price and market risk factor:

```text
function derive_added_exposure(
    direction,
    current_size,
    current_risk_units,
    size_added,
    price,
    market_config
):
    require size_added > 0
    require price > 0

    if direction == Long:
        base_added = mul_div_floor(
            size_added,
            PRICE_PRECISION,
            price
        )
    else:
        base_added = mul_div_ceil(
            size_added,
            PRICE_PRECISION,
            price
        )

    resulting_size = current_size + size_added
    risk_after = mul_div_floor(
        resulting_size,
        market_config.market_risk_factor_bps,
        BPS
    )

    require risk_after >= current_risk_units
    risk_added = risk_after - current_risk_units

    require base_added > 0
    require risk_added > 0

    return AddedExposure {
        size_added,
        base_added,
        risk_added
    }
```

Adding exposure:

```text
function add_exposure(position, side, ledger, size_added, base_added, risk_added):
    position.size += size_added
    position.base_exposure += base_added
    position.risk_units += risk_added

    side.size_open_interest += size_added
    side.base_exposure += base_added
    side.risk_units += risk_added

    ledger.total_risk_units += risk_added

    reset_receiver_distribution_remainder(market, side.direction)
```

Opening also increments `open_position_count`. A full removal decrements it.

Any change to a side's `size_open_interest` invalidates the carried divisor of
the opposite payer stream's receiver-distribution division:

```text
function reset_receiver_distribution_remainder(market, changed_direction):
    if changed_direction == Long:
        market.short_payer_remainders.receiver_distribution_remainder = 0
    else:
        market.long_payer_remainders.receiver_distribution_remainder = 0
```

The long-payer stream distributes to short receivers and the short-payer
stream distributes to long receivers, so a change on one side clears the carry
of the stream that divides by that side's size. §4.5.1 states why this is
required and why discarding the carry is safe.

For a partial decrease:

```text
function derive_partial_removal(position, size_removed, market_config):
    require 0 < size_removed < position.size

    size_after = position.size - size_removed

    base_after = mul_div_floor(
        position.base_exposure,
        size_after,
        position.size
    )

    risk_after = mul_div_floor(
        size_after,
        market_config.market_risk_factor_bps,
        BPS
    )

    return Removal {
        size_removed,
        base_removed: position.base_exposure - base_after,
        risk_removed: position.risk_units - risk_after,
        size_after,
        base_after,
        risk_after
    }
```

Apply the same removal to the position, its side, and global risk:

```text
position.size = removal.size_after
position.base_exposure = removal.base_after
position.risk_units = removal.risk_after

side.size_open_interest -= removal.size_removed
side.base_exposure -= removal.base_removed
side.risk_units -= removal.risk_removed

ledger.total_risk_units -= removal.risk_removed

reset_receiver_distribution_remainder(market, side.direction)
```

A full close removes the exact remaining size, base exposure, and risk units;
it never performs proportional rounding. After every mutation, enforce global
capacity and the market side's size and base caps where the action adds risk.

### 6.14 Refresh utilization and the borrow rate

```text
function calculate_utilization_bps(total_risk_units, cash_lp_equity):
    if total_risk_units == 0:
        return 0

    if cash_lp_equity <= 0:
        return BPS

    return min(
        mul_div_floor(total_risk_units, BPS, cash_lp_equity),
        BPS
    )

function refresh_borrow_rate(ledger, physical_cash, global_config):
    equity = derive_cash_lp_equity(ledger, physical_cash)
    utilization = calculate_utilization_bps(
        ledger.total_risk_units,
        equity
    )

    variable_factor = fixed_point_power(
        utilization / BPS,
        global_config.borrow_exponent_bps / BPS
    )

    rate_bps_day =
          global_config.base_borrow_rate_bps_day
        + global_config.max_variable_borrow_bps_day * variable_factor

    ledger.current_borrow_rate =
        rate_bps_day * INDEX_PRECISION
```

The calculation keeps `variable_factor` at fixed precision rather than
performing either displayed division as integer truncation. Refresh runs
after every completed mutation that changes total risk units, physical cash,
or any non-LP claim affecting cash LP equity.

### 6.15 Evaluate liquidation eligibility

```text
function evaluate_liquidation(
    position,
    market,
    ledger,
    price,
    physical_cash,
    global_config
):
    pending_funding = calculate_pending_funding(position, market)
    pending_borrow = calculate_pending_borrow(position, ledger)

    raw_pnl = calculate_raw_pnl(
        position.direction,
        position.size,
        position.base_exposure,
        price
    )

    payable_pnl = calculate_payable_pnl(
        raw_pnl,
        side_for(position),
        market.config,
        price,
        derive_cash_lp_equity(ledger, physical_cash)
    )

    effective = calculate_effective_collateral(
        position,
        payable_pnl,
        pending_funding,
        pending_borrow
    )

    maintenance = mul_div_ceil(
        position.size,
        market.config.maintenance_margin_bps,
        BPS
    )

    threshold = max(
        maintenance,
        global_config.keeper_liquidation_reward
    )

    return LiquidationAssessment {
        liquidatable: effective <= threshold,
        insolvent: effective < 0,
        effective_collateral: effective,
        threshold,
        payable_pnl,
        pending_funding,
        pending_borrow
    }
```

The indices and risk state must be current at the assessment timestamp. The
same assessment object is reused by liquidation settlement so eligibility and
payment cannot use different prices or fee snapshots.

### 6.16 Evaluate ADL state

Evaluate each market side from its aggregate positive PnL relative to cash LP
equity:

```text
function evaluate_side_risk_state(side, market_config, price, cash_lp_equity):
    raw_side_pnl = calculate_raw_pnl(
        side.direction,
        side.size_open_interest,
        side.base_exposure,
        price
    )

    positive_pnl = max(raw_side_pnl, 0)

    if positive_pnl == 0:
        pnl_factor_bps = 0
    else if cash_lp_equity == 0:
        pnl_factor_bps = BPS
    else:
        pnl_factor_bps = mul_div_floor(
            positive_pnl,
            BPS,
            cash_lp_equity
        )

    if pnl_factor_bps >= market_config.hard_cap_pnl_factor_bps:
        next_state = HardCap
    else if pnl_factor_bps >= market_config.adl_pnl_factor_bps:
        next_state = ADL
    else if pnl_factor_bps >= market_config.warning_pnl_factor_bps:
        next_state = Warning
    else if side.risk_state != Normal
         and pnl_factor_bps >= market_config.recovery_pnl_factor_bps:
        next_state = Warning
    else:
        next_state = Normal

    return SideRiskAssessment {
        previous_state: side.risk_state,
        next_state,
        positive_pnl,
        pnl_factor_bps
    }
```

Applying a transition updates the stored side state and adjusts
`restricted_market_side_count` if the side crosses between `Normal` and a
restricted state. ADL execution is permitted only under the applicable
restricted-side policy and uses one assessment snapshot for candidate
eligibility, payable PnL, keeper reward, and exposure removal. Candidate
selection and the external ADL operation are specified with the user-facing
functions rather than hidden inside this pure assessment.

### 6.17 Release residual accounting dust

Dust is assigned by explicit rules rather than accumulated in an ownerless
bucket:

```text
function reset_empty_market_funding(market, ledger):
    require market.long.size_open_interest == 0
    require market.short.size_open_interest == 0

    market.skew_ema = 0
    market.current_payer_side = None
    market.current_payer_rate = 0
    market.long_payer_remainders = zeroed remainder group
    market.short_payer_remainders = zeroed remainder group

    residue = market.pending_receiver_funding
    require ledger.pending_receiver_funding_total >= residue
    market.pending_receiver_funding = 0
    ledger.pending_receiver_funding_total -= residue
    # With no position left in this market, the released residue has no
    # receiver owner and becomes LP residual equity.
```

Cumulative indices and the checkpoint timestamp remain unchanged.

When no position remains anywhere:

```text
function verify_no_final_receiver_residue(ledger):
    require ledger.open_position_count == 0
    require ledger.pending_receiver_funding_total == 0
```

Fee split residue goes to the protocol because protocol revenue is calculated
as the amount collected minus the floored LP and referral shares. Proportional
position reductions assign their rounding difference to the removed exposure,
and a final close removes every remainder. In a clean terminal vault, the final
LP withdrawal may receive all residual cash LP equity so virtual share
quantities cannot strand ownerless cash.

## 7. User-facing operations

Every operation executes atomically and processes one action. A settlement
caller cannot submit an array or combine unrelated opens, closes,
liquidations, or ADL actions in one call.

`require keeper authorization` means the caller authenticates the address that
will receive the reward. It does not require membership in a privileged keeper
allowlist; execution remains permissionless.

Price-sensitive trader actions use these common predicates:

```text
fresh_for_commit = fill_observed_at > commit_observed_at
delay_satisfied  = now >= execute_after

entry_price_allowed(direction, price, acceptable_price):
    acceptable_price == 0
    or (direction == Long  and price <= acceptable_price)
    or (direction == Short and price >= acceptable_price)

exit_price_allowed(direction, price, acceptable_price):
    acceptable_price == 0
    or (direction == Long  and price >= acceptable_price)
    or (direction == Short and price <= acceptable_price)
```

An eligible untriggered limit or position trigger remains pending and pays no
reward. A market-style pending action normally uses its first eligible attempt
as its only attempt: an expected deterministic failure consumes the action,
pays the configured action reward, refunds any action escrow, and returns a
terminal failure result. The explicit `RequiresLiquidation` safety outcome remains non-terminal. An
unexpected invariant failure reverts and leaves the action unchanged.

The terminal entry-failure helper is:

```text
function fail_entry_action(action, ledger, keeper, reward, reason):
    pay_keeper_from_escrow(action, ledger, keeper, reward)

    refund = action.escrowed_collateral
    action.escrowed_collateral = 0
    ledger.action_escrow_total -= refund
    transfer_cash(action.owner, refund)

    remove pending action
    refresh global borrow rate after checkpoint and claim changes
    store affected market and ledger
    emit terminal failure result(reason, reward, refund)

    return Failed
```

It never charges an opening fee or creates exposure.

The corresponding terminal helper for an increase, decrease, or close is:

```text
function fail_position_action(action, position, keeper, reward, reason):
    require position is not liquidatable

    payable_reward = min(
        reward,
        max(0, position.stored_collateral - min_collateral),
        max(0, effective_collateral - liquidation_threshold - 1)
    )

    if payable_reward > 0:
        pay_keeper_from_position(
            position, side, ledger, keeper, payable_reward
        )

    if action.escrowed_collateral > 0:
        refund complete action escrow to owner

    clear position.pending_mutation_action_id
    remove pending action
    refresh market risk state and global borrow rate
    store position, market, and ledger
    emit terminal failure result(reason, payable_reward, refund)

    return Failed
```

If the position is already liquidatable, the attempted voluntary settlement
returns `RequiresLiquidation` without consuming the action or paying a reward.
That outcome is correct and stays non-terminal: the liquidation path will
remove the position and supersede the action.

An eligible attempt on a position that is *not* liquidatable is always
terminal. The reward is variable, the finality is not. The keeper receives
whatever the position can pay without dropping below minimum collateral or
into liquidation, which may be less than the configured reward and may be
zero; the action is consumed either way.

The alternative — refusing to terminate when the full reward is unpayable —
was worse in both directions. It left an eligible action pending forever while
`pending_mutation_action_id` blocked every further increase, decrease, or
close on that position, and since position mutations have neither a cancel
operation nor an expiry, the only exits were `add_collateral` or liquidation.
It also broke the first-attempt finality the order lifecycle depends on: an
action that survives an eligible attempt is a free retry.

The variable cap is not exploitable. Suppressing the reward requires holding a
position at minimum collateral or at its liquidation threshold, which is one
tick from being liquidated and cannot be maintained as a strategy; the saving
is at most one fixed reward per action. Keepers are free to skip an action
whose payable reward is too small, and the owner can always settle it
themselves to clear the slot.

This also resolves §10.3.4 item 4 without escrowing a keeper reward at
position-action creation: finality is unconditional, and only the reward
amount is contingent on the position's ability to pay.

### 7.1 Create a market-open order

```text
function create_market_open_order(owner, request):
    require owner authorization
    require system and market accept new exposure
    require request.size > 0
    require request.submitted_collateral > 0
    validate direction, acceptable price, and optional TP/SL prices

    execute_after =
        now + market.config.order_execution_delay_seconds
    require request.expires_at > execute_after

    opening_fee = calculate_opening_fee(request.size, market.config)
    keeper_reward = global_config.keeper_open_reward

    require request.submitted_collateral
        >= opening_fee + keeper_reward + global_config.min_collateral
    require request.submitted_collateral
        >= global_config.keeper_expiry_reward

    require request.submitted_collateral
        - opening_fee
        - keeper_reward
        >= ceil(request.size * market.config.initial_margin_bps / BPS)

    commit_price = read_authenticated_stamped_price(request.market_id)

    transfer_cash_from(
        owner,
        vault,
        request.submitted_collateral
    )

    action_id = consume_next_action_id()
    store PendingAction {
        action_id,
        owner,
        market_id: request.market_id,
        kind: MarketOpen,
        created_at: now,
        execute_after,
        commit_observed_at: commit_price.observed_at,
        payload: MarketOpen {
            direction: request.direction,
            size: request.size,
            submitted_collateral: request.submitted_collateral,
            acceptable_price: request.acceptable_price,
            expires_at: request.expires_at,
            take_profit: request.take_profit,
            stop_loss: request.stop_loss
        },
        escrowed_collateral: request.submitted_collateral
    }

    ledger.action_escrow_total += request.submitted_collateral
    emit market-open commitment
    return action_id
```

Creation performs structural and collateral checks but does not reserve vault
capacity, choose an execution price, collect an opening fee, pay a keeper, or
create a position.

The market order is binding after creation. Its owner cannot cancel it before
execution or expiry.

### 7.2 Settle a market-open order

```text
function settle_market_open(action_id, keeper):
    require keeper authorization
    action = load pending MarketOpen action
    require now < action.expires_at
    require now >= action.execute_after

    fill = read_fresh_uncached_stamped_price(action.market_id)

    if fill.observed_at <= action.commit_observed_at:
        return NotReady without state change or reward

    accrue_global_borrow(ledger, now)
    accrue_market_funding(ledger, market, now, global_config)
    evaluate and apply current market risk state

    reward = global_config.keeper_open_reward
    opening_fee = calculate_opening_fee(action.size, market.config)
    collateral_after_charges =
        action.escrowed_collateral - reward - opening_fee

    exposure = derive_added_exposure(
        action.direction,
        0,
        0,
        action.size,
        fill.price,
        market.config
    )
```

Run a complete preflight against the hypothetical post-settlement state:

```text
expected checks:
    entry_price_allowed(...)
    market still accepts new exposure
    collateral_after_charges >= min_collateral
    collateral_after_charges
        >= initial_margin(action.size) + projected_minimum_borrow
    resulting global risk is within capacity
    resulting market side is within size and base caps
    resulting position is healthy after every charge
```

If any expected check fails, call `fail_entry_action` with
`keeper_open_reward`. The first eligible attempt is terminal.

`projected_minimum_borrow` is the monetary minimum that
`initialize_borrow_window` will quote for this position. The borrow window is
opened after the health checks, so without this term a position could be
admitted exactly at initial margin and be below it the moment its window is
quoted — pending borrow includes the active minimum-borrow floor (§2.9), and
that floor exists from the first second of the window.

The value is deterministic at preflight because every input is already known
from the projected post-settlement state:

```text
projected_risk_units  = exposure.risk_added
projected_equity      = cash LP equity after this action's claim changes
projected_rate        = borrow rate at (total_risk_units + projected_risk_units,
                                        projected_equity)

projected_minimum_borrow = mul_div_ceil(
    projected_risk_units * projected_rate,
    min_borrow_fee_seconds,
    INDEX_PRECISION * BPS * SECONDS_PER_DAY
)
```

At the initial parameters the term is small — at most about `2.9` bps of size
against an initial margin of `500` bps — but it is not zero, and including it
is what makes "resulting position is healthy after every charge" literally
true. The same term belongs in the increase preflight in §7.8, evaluated
against the full resulting risk units rather than only the added ones.

On success:

```text
pay_keeper_from_escrow(action, ledger, keeper, reward)

action.escrowed_collateral -= opening_fee
ledger.action_escrow_total -= opening_fee
distribute_open_close_revenue(
    ledger,
    opening_fee,
    current_referrer(action.owner),
    global_config
)

position_id = consume_next_position_id()
create zeroed position identity

position_collateral = action.escrowed_collateral
action.escrowed_collateral = 0
ledger.action_escrow_total -= position_collateral
add_position_collateral(position, side, ledger, position_collateral)

add_exposure(
    position,
    side,
    ledger,
    exposure.size_added,
    exposure.base_added,
    exposure.risk_added
)
ledger.open_position_count += 1

position.opened_at = now
position.last_size_increase_at = now
position.pending_mutation_action_id = None

initialize funding debt baselines from current market indices
refresh market display and risk state
refresh_borrow_rate(ledger, physical_cash, global_config)
initialize_borrow_window(position, ledger, global_config)

if take-profit or stop-loss was requested:
    attach each trigger using:
        committed_at = now
        execute_after = now + current market delay
        commit_observed_at = fill.observed_at

remove pending action
store position, market, and ledger atomically
emit position-opened and action-settled results
return position_id
```

The successful fill observation becomes the commitment cursor for attached
exit triggers. They cannot use the same observation that opened the position.

### 7.3 Create a limit-open order

Limit creation performs the same authorization, collateral transfer, static
margin checks, and escrow accounting as market-open creation, substituting
`keeper_limit_order_reward` for `keeper_open_reward` and requiring escrow to
cover `keeper_expiry_reward` as well. It additionally requires a positive
trigger price and freezes the trigger direction relative to the authenticated
commit price:

```text
function create_limit_open_order(owner, request):
    validate and escrow exactly as market-open creation
    require request.trigger_price > 0

    commit_price = read_authenticated_stamped_price(request.market_id)

    trigger_above = request.trigger_price >= commit_price.price

    store PendingAction {
        kind: LimitOpen,
        commit_observed_at: commit_price.observed_at,
        execute_after: now + market delay,
        payload: LimitOpen {
            direction,
            size,
            submitted_collateral,
            acceptable_price,
            trigger_price,
            trigger_above,
            expires_at,
            take_profit,
            stop_loss
        },
        escrowed_collateral: submitted_collateral
    }
```

The owner can cancel this pending order. No capacity is reserved and no fee or
reward is collected at creation.

### 7.4 Settle a limit-open order

```text
function settle_limit_open(action_id, keeper):
    require keeper authorization
    action = load pending LimitOpen action
    require now < action.expires_at
    require now >= action.execute_after

    fill = read_fresh_uncached_stamped_price(action.market_id)

    if fill.observed_at <= action.commit_observed_at:
        return NotReady without state change or reward

    trigger_crossed =
        if action.trigger_above:
            fill.price >= action.trigger_price
        else:
            fill.price <= action.trigger_price

    if not trigger_crossed:
        return Pending without state change or reward
```

Once triggered, checkpoint, preflight, and settle exactly like a market open,
with these substitutions:

```text
keeper reward = keeper_limit_order_reward
action kind    = LimitOpen
```

A triggered attempt that fails slippage, capacity, margin, exposure caps, or
another expected execution check is terminal. It pays the limit reward from
escrow, refunds the remainder, charges no opening fee, and creates no position.
An untriggered observation does not consume the order.

### 7.5 Cancel a limit order

```text
function cancel_limit_open(action_id, owner):
    require owner authorization
    action = load pending LimitOpen action
    require action.owner == owner
    require now < action.expires_at

    refund = action.escrowed_collateral
    action.escrowed_collateral = 0
    ledger.action_escrow_total -= refund

    remove pending action
    transfer_cash(owner, refund)

    emit action-cancelled result
    return refund
```

Cancellation is allowed only before expiry and before successful execution
removes the pending record. It pays no keeper reward and no opening fee.
Market-open orders do not expose this operation.

### 7.6 Clean up an expired order

Expiry uses one exact boundary:

```text
executable = now < expires_at
expired    = now >= expires_at
```

```text
function clean_expired_entry(action_id, keeper):
    require keeper authorization
    action = load pending MarketOpen or LimitOpen action
    require now >= action.expires_at

    reward = global_config.keeper_expiry_reward
    pay_keeper_from_escrow(action, ledger, keeper, reward)

    refund = action.escrowed_collateral
    action.escrowed_collateral = 0
    ledger.action_escrow_total -= refund

    remove pending action
    transfer_cash(action.owner, refund)

    emit action-expired result(reward, refund)
```

Expiry cleanup charges no opening fee and creates no position. Entry creation
must have guaranteed enough escrow for this reward.

### 7.7 Add collateral

A pure collateral addition is immediate because it adds no price exposure and
cannot exploit a stale execution price:

```text
function add_collateral(position_id, owner, amount):
    require owner authorization
    require amount > 0

    position = load position
    require position.owner == owner

    accrue_global_borrow(ledger, now)
    accrue_market_funding(ledger, market, now, global_config)

    transfer_cash_from(owner, vault, amount)
    add_position_collateral(position, side, ledger, amount)

    derive current health for reporting
    refresh_borrow_rate(ledger, physical_cash, global_config)
    store position, market, and ledger atomically
```

Because physical cash and the position claim increase by the same amount, cash
LP equity and the borrow rate ordinarily remain unchanged. The checkpoints
still preserve one consistent timestamp for health reporting.

This action charges no opening fee or keeper reward. It does not settle funding
or borrow, reset any debt baseline, change the stored minimum borrow fee, or
restart the minimum-position-lifetime clock. A liquidatable owner may use this
path to rescue the position before a liquidation transaction succeeds.

### 7.8 Create and settle an increase

Creation:

```text
function create_increase(position_id, owner, request):
    require owner authorization
    require request.size_added > 0
    require request.collateral_added >= 0

    position = load position
    require position.owner == owner
    require position.pending_mutation_action_id is None

    commit_price = read_authenticated_stamped_price(position.market_id)

    if request.collateral_added > 0:
        transfer_cash_from(owner, vault, request.collateral_added)
        ledger.action_escrow_total += request.collateral_added

    action_id = consume_next_action_id()
    store Increase action with immutable size, collateral, acceptable price,
        commit observation, and execute-after timestamp
    position.pending_mutation_action_id = action_id

    store position and ledger atomically
    return action_id
```

Creation does not settle the old borrow window, add exposure, or charge a fee.

Successful settlement:

```text
function settle_increase(action_id, keeper):
    require keeper authorization
    load matching pending action and position
    require position.pending_mutation_action_id == action_id
    require now >= action.execute_after

    fill = read_fresh_uncached_stamped_price(position.market_id)
    if fill.observed_at <= action.commit_observed_at:
        return NotReady without state change or reward

    accrue_global_borrow(ledger, now)
    accrue_market_funding(ledger, market, now, global_config)

    pending_funding = calculate_pending_funding(position, market)
    pending_borrow = calculate_pending_borrow(position, ledger)

    if position is liquidatable at this snapshot:
        return RequiresLiquidation without state change or reward
    if not entry_price_allowed(position.direction, fill.price, acceptable):
        return fail_position_action(
            action,
            position,
            keeper,
            keeper_increase_reward,
            Slippage
        )

    preflight the complete resulting position, capacity, and market caps

    if an expected preflight check fails:
        return fail_position_action(
            action,
            position,
            keeper,
            keeper_increase_reward,
            failed_check
        )
```

An expected first-attempt failure consumes the market-style action. It pays
`keeper_increase_reward` from stored position collateral when the position can
safely pay it, refunds all added-collateral escrow, clears the reverse
reference, and charges no opening fee. A liquidatable position is left for the
liquidation path rather than charged for an ordinary failed attempt.

On success:

```text
senior = capitalize_for_surviving_mutation(...)

if action.collateral_added > 0:
    move complete action escrow into position collateral

opening_fee = calculate_opening_fee(action.size_added, market.config)
remove_position_collateral(position, side, ledger, opening_fee)
distribute_open_close_revenue(
    ledger,
    opening_fee,
    current_referrer(position.owner),
    global_config
)

pay_keeper_from_position(
    position,
    side,
    ledger,
    keeper,
    global_config.keeper_increase_reward
)

exposure = derive_added_exposure(
    position.direction,
    position.size,
    position.risk_units,
    action.size_added,
    fill.price,
    market.config
)
add_exposure(position, side, ledger, exposure...)

require resulting collateral >= min_collateral
require resulting effective collateral
    >= initial_margin(resulting size) + projected_minimum_borrow
enforce global capacity and market exposure caps

position.last_size_increase_at = now
reset all funding debt baselines for resulting size
refresh market display and risk state
refresh_borrow_rate(ledger, physical_cash, global_config)
initialize_borrow_window(position, ledger, global_config)

clear position.pending_mutation_action_id
remove pending action with zero escrow
store all state and emit increase result
```

The completed old borrow window is paid from pre-existing position collateral
before added collateral joins the position. The new minimum uses the full
resulting risk units and post-increase borrow rate.

### 7.9 Create and settle a decrease

Creation records a binding market-style removal:

```text
function create_decrease(position_id, owner, size_removed, acceptable_price):
    require owner authorization
    position = load position
    require position.owner == owner
    require position.pending_mutation_action_id is None
    require 0 < size_removed < position.size

    commit = read_authenticated_stamped_price(position.market_id)
    action_id = consume_next_action_id()

    store Decrease action with size_removed, acceptable_price,
        commit.observed_at, and execute_after
    position.pending_mutation_action_id = action_id
    return action_id
```

Successful settlement:

```text
function settle_decrease(action_id, keeper):
    require keeper authorization
    load matching action and position
    require now >= action.execute_after
    if now < position.last_size_increase_at + min_position_lifetime:
        return NotReady without state change or reward

    fill = read_fresh_uncached_stamped_price(position.market_id)
    if fill.observed_at <= action.commit_observed_at:
        return NotReady without state change or reward

    accrue global borrow and market funding to now
    calculate pending funding and borrow
    if position is liquidatable at this snapshot:
        return RequiresLiquidation without state change or reward

    if not exit_price_allowed(position.direction, fill.price, acceptable):
        return fail_position_action(
            action,
            position,
            keeper,
            keeper_decrease_reward,
            Slippage
        )

    removal = derive_partial_removal(
        position,
        action.size_removed,
        market.config
    )
    raw_pnl = calculate_raw_pnl(
        position.direction,
        removal.size_removed,
        removal.base_removed,
        fill.price
    )
    payable_pnl = calculate payable PnL for the removed exposure

    preflight full capitalization, keeper reward, closing fee,
        remaining health, and minimum collateral

    if an expected preflight check fails:
        return fail_position_action(
            action,
            position,
            keeper,
            keeper_decrease_reward,
            failed_check
        )
```

On success:

```text
senior = capitalize_for_surviving_mutation(...)

pnl_result = apply_payable_pnl(
    position,
    side,
    ledger,
    payable_pnl,
    cash_lp_equity
)
require pnl_result.uncollectible_loss == 0

reward = keeper_decrease_reward
pay_keeper_from_position(position, side, ledger, keeper, reward)

closing = calculate_closing_fee(
    removal.size_removed,
    max(payable_pnl, 0),
    pending_funding,
    pending_borrow.due,
    reward,
    market.config
)

closing_collected = min(closing.collectible, position.stored_collateral)
remove_position_collateral(position, side, ledger, closing_collected)
distribute_open_close_revenue(
    ledger,
    closing_collected,
    current_referrer(position.owner),
    global_config
)

apply removal to position, side, and global risk aggregates

require surviving position collateral >= min_collateral
require surviving effective collateral >= maintenance margin

reset funding debts for resulting size
refresh market display and risk state
refresh_borrow_rate(ledger, physical_cash, global_config)
initialize_borrow_window(position, ledger, global_config)

clear reverse reference
remove pending action
store state and emit decrease result
```

Realized residual profit remains position collateral. A separate immediate
collateral-withdrawal operation is not part of this decrease.

Two guards in this sequence exist to keep the surviving path and the terminal
path from diverging. `require pnl_result.uncollectible_loss == 0` enforces
§6.5: a surviving position may never carry a loss its collateral could not
absorb, and if preflight admitted one, the action must revert rather than
leave the deficit attached. The `min` against stored collateral mirrors the
terminal path in §6.10 so both paths debit the same way; preflight already
guarantees the fee is payable, so the clamp is defence in depth and not a
licence to collect a partial fee where a full one was due. Passing
`max(payable_pnl, 0)` matches the close path and makes the call site agree
with the sign convention `calculate_closing_fee` already applies internally.

### 7.10 Create and settle a voluntary close

Creation is identical to decrease creation except the action kind is `Close`
and no size is stored; it always targets the position's complete remaining
exposure at settlement.

```text
function create_close(position_id, owner, acceptable_price):
    require owner authorization and position ownership
    require position.pending_mutation_action_id is None

    commit = read_authenticated_stamped_price(position.market_id)
    create Close action with commit cursor and execute-after timestamp
    set position.pending_mutation_action_id
```

Settlement:

```text
function settle_close(action_id, keeper):
    require keeper authorization
    load matching action and position
    require now >= action.execute_after
    if now < position.last_size_increase_at + min_position_lifetime:
        return NotReady without state change or reward

    fill = read_fresh_uncached_stamped_price(position.market_id)
    if fill.observed_at <= action.commit_observed_at:
        return NotReady without state change or reward

    accrue global borrow and market funding to now
    calculate pending funding and pending borrow
    calculate raw and payable PnL for the complete position

    assessment = evaluate liquidation from the same snapshot
    if assessment.liquidatable:
        return RequiresLiquidation without state change or reward

    if not exit_price_allowed(position.direction, fill.price, acceptable):
        return fail_position_action(
            action,
            position,
            keeper,
            keeper_close_reward,
            Slippage
        )

    reward = keeper_close_reward
    closing = calculate_closing_fee(
        position.size,
        max(payable_pnl, 0),
        pending_funding,
        pending_borrow.due,
        reward,
        market.config
    )

    settle_terminal_position with:
        reason = VoluntaryClose
        keeper reward = reward
        closing fee permitted = true

    clear pending action and all attached triggers
    refresh empty-market state if necessary
    release final receiver residue if no position remains
    refresh market risk state and global borrow rate
    store state and emit close result
```

A close at zero or negative payable price PnL pays no closing fee. Funding,
borrow, and the close keeper reward still settle. Any closing-fee amount not
collectible from current settlement profit is waived.

### 7.11 Execute take-profit

The owner may attach, replace, or remove a take-profit instruction without a
keeper:

```text
function set_take_profit(position_id, owner, trigger_price, acceptable_price):
    require owner authorization and position ownership
    require trigger_price > 0

    commit = read_authenticated_stamped_price(position.market_id)
    position.take_profit = TriggerInstruction {
        trigger_price,
        acceptable_price,
        committed_at: now,
        execute_after: now + market delay,
        commit_observed_at: commit.observed_at
    }

function clear_take_profit(position_id, owner):
    require owner authorization and position ownership
    position.take_profit = None
```

Execution:

```text
function execute_take_profit(position_id, keeper):
    require keeper authorization
    load position and take-profit instruction
    require now >= instruction.execute_after
    if now < position.last_size_increase_at + min_position_lifetime:
        return NotReady without state change or reward

    fill = read_fresh_uncached_stamped_price(position.market_id)
    if fill.observed_at <= instruction.commit_observed_at:
        return NotReady without state change or reward

    trigger_crossed =
        (position.direction == Long  and fill.price >= trigger_price)
        or
        (position.direction == Short and fill.price <= trigger_price)

    if not trigger_crossed:
        return Pending without state change or reward

    if not exit_price_allowed(position.direction, fill.price, acceptable):
        return Pending without state change or reward

    checkpoint and calculate complete-position funding, borrow, and PnL
    if position is liquidatable:
        return RequiresLiquidation without state change or reward
    calculate closing fee using keeper_tp_reward
    settle_terminal_position with reason TakeProfit and closing fee permitted
    invalidate any pending voluntary mutation and refund its escrow
    remove the position and both attached triggers
    perform market, residue, risk-state, and borrow-rate cleanup
    emit take-profit result
```

The execution pays only `keeper_tp_reward`, not a close reward in addition.

### 7.12 Execute stop-loss

Setting and clearing stop-loss uses the same commitment record as take-profit.
Its trigger direction is reversed:

```text
stop_loss_crossed =
    (position.direction == Long  and fill.price <= trigger_price)
    or
    (position.direction == Short and fill.price >= trigger_price)
```

```text
function execute_stop_loss(position_id, keeper):
    require keeper authorization
    require delay, fresh observation, and trigger, each with the same
        non-terminal outcome as take-profit
    if the minimum position lifetime has not elapsed:
        return NotReady without state change or reward
    if the exit slippage bound is not satisfied:
        return Pending without state change or reward
    checkpoint and calculate complete-position funding, borrow, and PnL
    if position is liquidatable:
        return RequiresLiquidation without state change or reward
    calculate closing fee using keeper_sl_reward
    settle_terminal_position with reason StopLoss and closing fee permitted
    invalidate any pending voluntary mutation and refund its escrow
    remove the position and both attached triggers
    perform market, residue, risk-state, and borrow-rate cleanup
    emit stop-loss result
```

A losing stop-loss pays no closing fee. A stop-loss can close in profit—for
example, after the trigger was moved above entry—and then pays the normal
closing fee. It pays only `keeper_sl_reward`.

### 7.13 Liquidate a position

Liquidation is permissionless to an authorized keeper and needs no prior trader
commitment or execution delay:

```text
function liquidate(position_id, keeper):
    require keeper authorization
    load position, market, ledger, and physical cash

    now = authoritative timestamp
    accrue_global_borrow(ledger, now)
    accrue_market_funding(ledger, market, now, global_config)

    price = read_authenticated_current_price(position.market_id)
    evaluate and apply market risk state from this snapshot

    assessment = evaluate_liquidation(
        position,
        market,
        ledger,
        price,
        physical_cash,
        global_config
    )

    require assessment.liquidatable

    settle terminal funding, PnL, and borrow in priority order
    pay keeper_liquidation_reward from remaining position value,
        using LP residual cash for any gap
    charge no closing fee

    remove complete position exposure and state
    invalidate its pending voluntary mutation and refund any action escrow
    clear attached TP and SL instructions
    reset empty-market state if necessary
    release final receiver residue if no position remains
    refresh market risk state and global borrow rate

    emit liquidation result including:
        effective collateral
        threshold
        keeper reward from position
        keeper reward from LP backstop
        bad debt and unpaid senior amounts
```

The same price, pending-fee snapshot, and liquidation assessment are used for
eligibility and settlement. The function cannot reassess with a later or
different price midway through the action.

### 7.14 Execute automatic deleveraging

ADL is also a forced action with no trader commitment or execution delay:

```text
function execute_adl(position_id, keeper):
    require keeper authorization
    load position, market, ledger, and physical cash

    now = authoritative timestamp
    accrue global borrow and market funding
    price = read_authenticated_current_price(position.market_id)

    side_assessment = evaluate_side_risk_state(
        position.side,
        market.config,
        price,
        cash_lp_equity
    )
    require side_assessment.next_state is ADL or HardCap

    position_raw_pnl = calculate raw PnL using the same snapshot
    require position_raw_pnl > 0

    calculate pending funding and borrow
    liquidation_assessment = evaluate_liquidation using the same snapshot
    if liquidation_assessment.liquidatable:
        return RequiresLiquidation without state change or reward

    apply side risk transition
    calculate position payable PnL using the same snapshot

    settle complete position with:
        reason = ADL
        keeper reward = keeper_adl_reward
        reward source = payable position value or stored collateral
        closing fee permitted = false

    remove complete exposure and position state
    invalidate pending voluntary mutation and refund its escrow
    clear attached triggers
    reset empty-market and receiver residue when applicable
    evaluate resulting side risk state
    refresh global borrow rate
    emit ADL result
```

The caller supplies one candidate position. The action processes only that
position. ADL pays no liquidation or close reward in addition to its own fixed
reward.

### 7.15 Register or change a referrer

Register an immutable code owner:

```text
function register_referral_code(referrer, code):
    require referrer authorization
    require code is valid and unregistered

    referral_code_owner[code] = referrer
    emit code-registered result
```

Select or replace a trader's referrer:

```text
function set_referrer(trader, code):
    require trader authorization
    referrer = referral_code_owner[code]
    require referrer exists
    require referrer != trader

    trader_referrer[trader] = referrer
    emit referrer-updated result
```

Changing the mapping affects only fees collected afterward. It does not move
or recalculate previously accrued referral balances.

### 7.16 Claim referral revenue

```text
function claim_referral_revenue(referrer):
    require referrer authorization

    amount = referral_balance[referrer]
    require amount > 0
    require ledger.referral_claimable_total >= amount

    referral_balance[referrer] = 0
    ledger.referral_claimable_total -= amount
    transfer_cash(referrer, amount)

    emit referral-claimed result
    return amount
```

The debit occurs before the transfer in the logical sequence. Atomic reversion
restores both if the transfer fails.

Protocol revenue follows the same claim pattern:

```text
function claim_protocol_revenue(protocol_recipient, amount):
    require protocol authority
    require 0 < amount <= ledger.protocol_claimable_total

    ledger.protocol_claimable_total -= amount
    transfer_cash(protocol_recipient, amount)
```

### 7.17 Deposit and withdraw LP liquidity

LP actions use a separate delayed FIFO request lifecycle because their share
price depends on a synchronized mark of every active market.

Create a deposit request:

```text
function request_lp_deposit(owner, assets):
    require owner authorization
    require assets > keeper_lp_resolve_reward
    require LP requests are currently allowed

    transfer collateral from owner into LP request escrow outside the vault

    request_id = consume next LP request ID
    store pending Deposit request {
        owner,
        escrowed_amount: assets,
        requested_at: now,
        execute_after: now + lp_request_delay_seconds
    }

    return request_id
```

Create a withdrawal request identically, but escrow LP shares instead of
collateral. Escrowed shares remain in total supply until successful settlement.

Only the FIFO head can resolve:

```text
function resolve_next_lp_request(executor):
    require executor authorization
    request = load request[next_lp_request_to_resolve]
    require request is Pending

    round = latest authenticated synchronized price round

    if round.timestamp < request.execute_after:
        return NotReady without state change or reward

    accrue global borrow and every active market to one timestamp
    derive physical cash, claims, LP equity, synchronized marked NAV,
        free capital, utilization, and risk states
```

Resolution is permissionless and pays `keeper_lp_resolve_reward` on every
terminal outcome, exactly like every other settlement in this protocol. A
deposit pays it from its asset escrow before conversion; a withdrawal pays it
from the assets it releases. The reward is what makes prompt resolution
somebody's job, and the request's owner may act as the executor.

An earlier draft of this operation assigned each request a unique price round
by requiring `round.previous_timestamp < request.execute_after`, and marked the
request `Expired` when that did not hold. That rule was unsound. `round` is
always the *latest* round, so any executor who simply waited two rounds made
the condition impossible to satisfy and killed the request; because only the
FIFO head can resolve, a single passive or hostile executor could stall the
entire queue. It also gave a request exactly one chance at one round, which is
a liveness cliff for an operation that must eventually complete.

The property that rule was protecting is real: whoever picks the settlement
moment picks the NAV, and a withdrawing LP who can choose the moment can exit
at an inflated share price and leave the mark-to-market loss with the LPs who
stay. It is not, however, a property that needs its own mechanism. It is the
same problem as a trader choosing the observation that fills a market order,
and this specification already answers that question the same way everywhere
else: a mandatory delay fixes the earliest possible moment, and a fixed keeper
reward makes a competing party settle at the first opportunity, so the party
with an interest in waiting does not control the timing.

Applying that answer here deletes the round-assignment machinery, the
`previous_timestamp` accessor it needed from the oracle, and the `Expired`
outcome that arose only from it. The residual exposure is the same one stated
in §1.7: the guarantee rests on there being a competing executor, not on a
protocol rule that names one round. The production delay of one day makes the
window to compete a wide one, so this is a weaker assumption here than it is
for a five-second trader action.

Successful deposit settlement uses the pre-deposit state:

```text
resolve_reward = min(
    keeper_lp_resolve_reward,
    request.escrowed_amount
)
deposit_assets = request.escrowed_amount - resolve_reward

conversion_assets = marked_vault_nav + 1
conversion_shares = share_supply + SHARE_SCALE

shares_to_mint = mul_div_floor(
    deposit_assets,
    conversion_shares,
    conversion_assets
)

require deposit eligibility and shares_to_mint > 0
transfer resolve_reward from escrow to the executor
transfer remaining asset escrow into the vault
mint shares_to_mint to owner
mark request Settled and advance FIFO pointer
refresh global borrow rate
```

The reward is deducted before conversion, so the depositor mints shares for
the assets that actually reach the vault and no share is minted against value
paid to the executor.

Successful withdrawal settlement uses the pre-withdrawal state:

```text
assets_to_pay = mul_div_floor(
    withdrawal_shares,
    marked_vault_nav + 1,
    share_supply + SHARE_SCALE
)

require assets_to_pay <= free_lp_capital
require post-withdraw utilization <= max_withdraw_utilization_bps
require no prohibited shortfall or restricted market state

resolve_reward = min(keeper_lp_resolve_reward, assets_to_pay)

burn complete escrowed shares
transfer resolve_reward from vault to the executor
transfer assets_to_pay - resolve_reward from vault to owner
mark request Settled and advance FIFO pointer
refresh global borrow rate
```

The reward comes out of the assets the withdrawal releases, after every
capacity and health check has been satisfied on the full amount. A withdrawal
worth less than the reward pays the executor everything it releases; it is
never topped up from LP equity.

An expected failed deposit or withdrawal pays the resolve reward, marks the
request failed, advances the FIFO pointer, and refunds the remaining escrow.
There is no `Expired` outcome: a request that is not yet resolvable stays
`Pending` and is retried, and a request that becomes resolvable either settles
or fails. There are no partial fills and no persistent withdrawal cash
claims. In a clean terminal vault, the final LP
may withdraw all residual cash LP equity so conversion rounding cannot strand
ownerless assets.

## 8. Order lifecycle and failure behavior

This section defines whether an action remains live, terminates, pays a keeper,
or reverts. These rules are part of the economic design: allowing a trader to
retry only after unfavourable observations would give the trader a free option.

### 8.1 Order states

Trader actions use these logical states:

| State | Stored? | Meaning |
|---|---|---|
| `Pending` | Yes | The commitment can still become executable |
| `NotReady` | No; action remains `Pending` | Execution delay, minimum position lifetime, or fresh-observation requirement is not yet satisfied |
| `WaitingForTrigger` | No; action remains `Pending` | A limit, TP, or SL trigger has not crossed |
| `Executed` | Terminal result | The requested economic mutation completed |
| `Failed` | Terminal result | An eligible market-style attempt failed an expected deterministic check |
| `Cancelled` | Terminal result | The owner cancelled a cancellable pending limit entry |
| `Expired` | Terminal result | A keeper cleaned an entry after its expiry boundary |
| `Superseded` | Terminal result | Liquidation or ADL removed the referenced position first |

Only `Pending` is stored as an executable action record. A terminal transition
removes that record, consumes its ID permanently, clears any reverse reference,
distributes all escrow, and emits the terminal result. Historical terminal
states do not remain executable storage objects.

Eligibility is a predicate, not a stored state:

```text
eligible =
      now >= execute_after
  and fill_observed_at > commit_observed_at
  and, for expiring entries, now < expires_at
  and, for actions that remove exposure,
      now >= position.last_size_increase_at + min_position_lifetime
```

Every clause of this predicate is a timing gate, so failing any of them yields
`NotReady`. None of them is an execution attempt, and none consumes the action
or pays a reward. In particular, calling a decrease, close, take-profit, or
stop-loss before the minimum position lifetime has elapsed is not an error: it
is a settlement that is not due yet, and it must not revert.

The principal transitions are:

```text
Pending
  ├─ delay, lifetime, or fresh observation missing ────> Pending
  ├─ conditional trigger not crossed ─────────────────> Pending
  ├─ eligible successful execution ───────────────────> Executed
  ├─ eligible market-style expected failure ──────────> Failed
  ├─ valid owner cancellation of limit entry ─────────> Cancelled
  ├─ keeper cleanup at or after entry expiry ─────────> Expired
  └─ referenced position removed by forced action ────> Superseded
```

Attached TP and SL instructions follow the same pending/triggered/executed
concept but are stored inside the position rather than as independent pending
action records.

### 8.2 Market-order lifecycle

Market-style commitments include:

- market-open entries;
- increases;
- partial decreases; and
- voluntary full closes.

They are binding after creation. The owner cannot cancel them merely because
the market moved unfavourably.

A market-open entry remains pending until execution or expiry. Position
mutations have no separately funded expiry cleanup and therefore remain pending
until settlement or forced-position cleanup.

The first eligible ordinary execution attempt that is not displaced by
liquidation and can safely fund its configured reward is terminal:

```text
if not delay_satisfied
   or not lifetime_satisfied
   or not fresh_for_commit:
    remain Pending
    pay no reward

else if expected execution checks fail:
    transition to Failed
    pay the action's keeper reward
    refund action escrow
    charge no opening or closing fee

else:
    transition to Executed
    perform complete settlement
```

`RequiresLiquidation` is the only non-terminal safety exception for position
mutations. Every other eligible attempt terminates; when the position cannot
pay the full failure reward, the reward is reduced rather than the action
preserved (§7.0).

The action cannot survive a failed eligible attempt and retry against a later
price. This applies even when the expected failure is economically harmless,
because selective retries would make the commitment optional after the trader
has seen the outcome.

If a referenced position is already liquidatable, an ordinary position-action
attempt is not treated as a chargeable expected failure. It returns
`RequiresLiquidation`, remains pending, and pays nothing. The liquidation call
then removes the position and supersedes the action.

### 8.3 Limit-order lifecycle

A limit-open entry is a resting conditional commitment:

```text
Pending
  ├─ too early or no post-commit observation ──────────> Pending
  ├─ observed price has not crossed trigger ───────────> Pending
  ├─ crossed and all execution checks pass ────────────> Executed
  ├─ crossed but expected execution check fails ───────> Failed
  ├─ owner cancellation before expiry ─────────────────> Cancelled
  └─ keeper cleanup at or after expiry ────────────────> Expired
```

An untriggered check is not an execution attempt and pays no reward. Once the
trigger is crossed on an eligible observation, the attempt becomes terminal:
slippage, capacity, margin, market-state, or exposure-cap failure cannot leave
the order resting for another observation.

The owner may cancel only while:

```text
action exists and now < expires_at
```

Cancellation refunds all escrow and pays no fee or keeper reward. At
`now >= expires_at`, execution and owner cancellation are closed; the expiry
cleanup path pays the keeper reward and refunds the remainder.

### 8.4 Voluntary position-action lifecycle

An open position can have at most one ordinary pending mutation:

```text
pending_mutation_action_id =
    None
    or one Increase, Decrease, or Close action ID
```

Creation fails if this reference is already occupied. This prevents two
commitments from assuming the same starting size, collateral, and debt
baselines. The reference is cleared only by execution, terminal expected
failure, or forced-position cleanup.

An increase may carry added-collateral escrow. A decrease or close carries no
cash escrow because its payment source is the existing position. All three use
the per-market delay and post-commit observation gate.

At the first eligible attempt:

- a successful action performs its accounting and removes the pending record;
- an expected deterministic failure pays the action-specific reward from the
  position, refunds added-collateral escrow, and removes the record;
- if paying the ordinary failure reward would violate minimum collateral or
  make the unchanged position liquidatable, the reward is capped at what the
  position can pay, possibly zero, and the action still terminates;
- an unexpected failure reverts and preserves the action; and
- liquidation or ADL can supersede the action without paying its ordinary
  action reward.

TP and SL may coexist with the one ordinary mutation because they are attached
conditional exits. If a TP, SL, liquidation, or ADL removes the position first,
the ordinary mutation is superseded and any added-collateral escrow is fully
refunded.

### 8.5 Delay and expiry boundaries

Every trader-requested price-sensitive action freezes its market delay at
creation:

```text
execute_after = created_at + order_execution_delay_seconds_at_creation
```

The configured market value is validated from 1 through 30 seconds and starts
at 5 seconds. Updating the market later does not move an existing action's
boundary.

Delay uses an inclusive boundary:

```text
too_early = now < execute_after
delay_satisfied = now >= execute_after
```

An action that removes exposure has a second, independent timing gate. Unlike
`execute_after`, it is not frozen at creation: it is read from the position at
settlement, so a size increase that lands between creation and settlement moves
it forward.

```text
lifetime_satisfied =
    now >= position.last_size_increase_at + min_position_lifetime
```

The gate is measured from `last_size_increase_at`, not `opened_at`, and it
applies uniformly to decrease, close, take-profit, and stop-loss. This is
deliberate. A size increase therefore re-locks the position for
`min_position_lifetime`, including against its own attached stop-loss, so for
one minute after aggregating exposure the only available exit is liquidation.

The alternative — exempting the risk-reducing stop-loss from the re-lock —
was considered and rejected. Exempting stop-loss alone leaves take-profit as
the obvious churn bypass unless it is exempted too, and exempting both turns
open, take-profit just above entry, exit into a way around the minimum
lifetime entirely. Keeping one rule for all four exits is the simpler
invariant, and the exposure it leaves is bounded by `min_position_lifetime`
and covered by liquidation.

Both gates are inclusive and both are non-terminal. A decrease, close,
take-profit, or stop-loss that is submitted before either boundary returns
`NotReady`; it does not revert, does not consume the action, and pays no
reward.

Market and limit entries additionally freeze `expires_at`, which must be
strictly later than `execute_after`. Expiry uses:

```text
executable = now < expires_at
expired = now >= expires_at
```

At exactly `expires_at`, execution is forbidden and expiry cleanup is allowed.
There is no timestamp at which both can succeed.

The delay is necessary but insufficient. A delayed action without a newer
qualifying observation remains `NotReady` indefinitely, except that an entry
can still reach expiry.

### 8.6 Fresh-price requirement

Creation stores the observation time of the authenticated price available when
the trader commits:

```text
commit_observed_at
```

Settlement obtains a fresh uncached aggregate and requires:

```text
fill_observed_at > commit_observed_at
```

Equality fails. A cache-write timestamp, transaction timestamp, block number,
or elapsed delay cannot replace the observation cursor because none proves
that the price contains information created after commitment.

The observation stamp is the oldest source timestamp contributing to the
accepted aggregate. This conservative choice ensures every source supporting
the fill is newer than the commitment cursor.

If no qualifying observation exists:

- the action remains pending;
- no keeper reward is paid;
- no escrow moves;
- no fee is charged; and
- no exposure or debt baseline changes.

TP and SL instructions record a new commitment cursor when attached or
replaced. Triggers attached during an entry fill use that fill's observation as
their cursor, so they require a later observation before closing the new
position.

### 8.7 Slippage failure

The acceptable-price predicate protects the trader independently of any
trigger:

| Action | Direction | Accepted fill |
|---|---|---|
| Open or increase | Long | `price <= acceptable_price` |
| Open or increase | Short | `price >= acceptable_price` |
| Decrease or close | Long | `price >= acceptable_price` |
| Decrease or close | Short | `price <= acceptable_price` |

An acceptable price of zero disables the bound.

For a market-style pending action, slippage failure on the first eligible
observation is terminal. The keeper receives the action reward, entry or added
collateral is refunded as applicable, and the action cannot retry.

For a limit entry, the trigger and slippage checks are separate. A missing
trigger leaves the order pending. Once triggered, slippage failure is terminal.

TP and SL are standing conditional instructions rather than market-style
one-attempt commitments. If the trigger is crossed but the exit slippage bound
does not pass, the instruction remains attached and no reward is paid. It may
execute on a later qualifying observation.

### 8.8 Capacity failure

Pending opens and increases do not reserve risk capacity, market-side size, or
base exposure. They also do not affect funding skew or open interest before
successful settlement.

Capacity is evaluated from the post-fee, post-reward, post-mutation state:

```text
total_risk_units_after * BPS
    <= cash_lp_equity_after * risk_capacity_limit_bps
```

Market-side size and base caps are checked at the same time.

For a market or triggered limit entry, insufficient capacity is an expected
terminal failure:

- pay the open or limit keeper reward from escrow;
- refund the remainder;
- charge no opening fee;
- create no position; and
- remove the pending order.

For an increase, insufficient capacity is likewise terminal after the action
becomes eligible. The keeper receives `keeper_increase_reward` from the
existing position if that payment is safe, added-collateral escrow is fully
refunded, no opening fee is charged, and existing exposure remains unchanged.

Concurrent pending orders can each appear individually fillable. Atomic
settlement means the first successful transaction consumes capacity and later
transactions reassess against the resulting state.

### 8.9 Expected terminal failure and transaction reversion

An expected terminal failure is a valid business outcome, not a transaction
error. Examples include:

- slippage outside a committed bound;
- unavailable global capacity;
- a market-side exposure cap;
- a market state that blocks new risk;
- insufficient post-charge initial margin;
- insufficient post-action maintenance for a surviving position; and
- another deterministic validation that can legitimately change between
  creation and settlement.

The transaction completes successfully while recording `Failed`, paying the
keeper, refunding escrow, and removing the action. A revert would undo all four
effects and incorrectly leave the trader with a free retry.

These conditions are not terminal attempts:

- execution delay not elapsed;
- minimum position lifetime not elapsed;
- no qualifying post-commit observation;
- an untriggered limit, TP, or SL;
- a TP or SL whose standing exit-price bound does not pass; and
- a voluntary position action displaced by liquidation eligibility.

They leave the action unchanged and pay nothing. An eligible attempt whose
keeper reward the position cannot fully fund is not in this list: it
terminates with a reduced reward (§7.0).

Unexpected conditions revert atomically. These include invalid authorization,
wrong action kind, nonexistent state, arithmetic overflow, negative pending
index deltas, aggregate mismatches, claim underflow, an invalid oracle proof,
or failure of a required token transfer. Reversion preserves the action,
escrow, claims, indices, position, and physical cash exactly as they were before
the call.

### 8.10 Refund behavior

Every escrowed unit has one terminal destination:

| Outcome | Opening fee | Keeper reward | Owner refund | Position collateral |
|---|---:|---:|---:|---:|
| Successful entry | Collected | Open or limit reward | `0` | Remaining escrow |
| Expected entry failure | `0` | Open or limit reward | Remaining escrow | `0` |
| Owner limit cancellation | `0` | `0` | Complete escrow | `0` |
| Entry expiry cleanup | `0` | Expiry reward | Remaining escrow | `0` |
| Successful increase | Collected on added size | Increase reward from position | `0` | Complete added-collateral escrow |
| Expected increase failure | `0` | Increase reward from position | Complete added-collateral escrow | Existing position remains |
| Liquidation or ADL supersedes increase | `0` | Only forced-action reward | Complete added-collateral escrow | Position is removed |

Refunds always go to the owner frozen in the action. The settlement caller
cannot redirect them.

An escrow-bearing record is removed only after its accounting reaches zero.
The individual escrow debit, `action_escrow_total` debit, keeper transfer, and
refund transfer are one atomic transition.

### 8.11 Keeper payment on terminal attempts

Exactly one keeper-reward field is selected per call:

| Outcome | Reward |
|---|---|
| Successful or expected-failed market entry | `keeper_open_reward` |
| Successful or expected-failed triggered limit entry | `keeper_limit_order_reward` |
| Successful or expected-failed increase | `keeper_increase_reward` |
| Successful or expected-failed decrease | `keeper_decrease_reward` |
| Successful or expected-failed voluntary close | `keeper_close_reward` |
| Successful TP | `keeper_tp_reward` |
| Successful SL | `keeper_sl_reward` |
| Entry expiry cleanup | `keeper_expiry_reward` |
| Successful liquidation | `keeper_liquidation_reward` |
| Successful ADL | `keeper_adl_reward` |
| Settled or failed LP request resolution | `keeper_lp_resolve_reward` |

No reward is paid for creation, a not-ready call, an untriggered conditional
order, owner cancellation, TP/SL slippage waiting, forced cleanup of a different
pending action, or a reverted transaction.

Cleanup performed inside liquidation or ADL is not a second keeper action. The
caller receives only the liquidation or ADL reward, not the reward of the
superseded voluntary action or an expiry reward.

Entry and expiry rewards come from action escrow. Increase, decrease, close,
TP, and SL rewards come from position value. ADL uses payable position value or
collateral. Liquidation alone has an LP-backed guarantee for a price-gap
shortfall.

### 8.12 Liquidation and ADL precedence

Forced safety actions do not require a trader commitment, execution delay, or
post-commit observation. They use one authenticated current-price snapshot and
the current accrued indices.

Precedence is:

```text
individual position is liquidatable
    => liquidation before every voluntary mutation, TP, SL, or ADL

position is not liquidatable and its side requires forced deleveraging
    => ADL may supersede voluntary position actions and triggers

otherwise
    => eligible voluntary action may settle normally
```

When liquidation or ADL removes a position:

```text
1. Mark the forced action as the position's terminal cause.
2. Remove complete position exposure and collateral state.
3. Clear TP and SL instructions.
4. Remove the ordinary pending mutation, if present.
5. Refund its complete added-collateral escrow, if any.
6. Clear the reverse pending-action reference.
7. Pay only the forced action's keeper reward.
```

A pending entry has no position to liquidate and is not directly superseded.
It still rechecks market state, margin, and capacity if it later becomes
eligible.

All races are resolved by atomic ordering. If voluntary settlement completes
first, the later forced call finds no eligible position. If liquidation or ADL
completes first, the later voluntary call finds no pending action or position.

### 8.13 Duplicate and replay prevention

The following rules prevent double settlement:

- Position IDs and action IDs are monotonically increasing and never reused.
- One pending action record exists for one action ID.
- One position stores at most one ordinary pending mutation reference.
- A terminal transition removes the pending record and clears its reverse
  reference in the same transaction.
- A second call using a consumed ID fails before any transfer or reward.
- Two keepers racing for one action cannot both be paid; only the first
  committed terminal transaction finds the pending record.
- Limit cancellation, expiry cleanup, and execution all consume the same
  record, so only one can win.
- At exact expiry, only cleanup is valid.
- TP and SL are cleared when either one fully closes the position, so both
  cannot execute.
- A full close removes every remaining size, base-exposure, and risk-unit
  remainder.

Events are evidence of terminal outcomes but are not replay authorization. The
presence of live authoritative state is always required before settlement.

### 8.14 One action per settlement call

Each public settlement call accepts exactly one action or position identifier.
There is no array input, mixed-action dispatcher, per-call batch limit, partial
batch result, duplicate-within-batch rule, or cross-action ordering policy.

Internal cleanup caused by the selected action does not violate this rule. For
example, liquidation may clear the position's pending close and triggers, but
it remains one liquidation action and pays one liquidation reward.

LP request resolution likewise processes only the FIFO head. Keepers optimize
through transaction selection and submission strategy rather than protocol
batch semantics. A general batch interface can be considered separately in the
future; it is not part of this specification.

## 9. Safety and accounting invariants

The rules in this section are the non-negotiable properties of the system.
They apply across every successful state transition, regardless of which
public operation caused it. A transition that cannot preserve them must not
commit.

Some properties are absolute accounting identities. Others are admission or
settlement conditions: they must hold when a new state is accepted, but a
later price movement can make the live position unhealthy or push utilization
above its admission limit. That later economic change is handled by
liquidation, ADL, and withdrawal restrictions; it is not an accounting
failure.

### 9.1 Cash ownership conservation

Physical vault cash is the only authoritative cash balance. Accounting labels
may divide its ownership, but cannot create or destroy it.

In a solvent state:

```text
physical_cash = cash_lp_equity + non_lp_claims

non_lp_claims =
      position_collateral_total
    + pending_receiver_funding_total
    + action_escrow_total
    + protocol_claimable_total
    + referral_claimable_total
```

Every cash transition has two matching sides:

- a transfer into the vault increases physical cash and exactly one ownership
  label, unless it is an unsolicited transfer that belongs to LP residual;
- a transfer out decreases physical cash and the payer's ownership label by
  the same amount; and
- an internal relabelling changes ownership labels without changing physical
  cash.

If `non_lp_claims > physical_cash`, the system reports:

```text
vault_shortfall = non_lp_claims - physical_cash
cash_lp_equity = 0
```

It must not conceal the deficit with a negative unsigned balance, a fabricated
cash counter, or an untracked receivable.

For each aggregate claim:

```text
position_collateral_total = sum(position.stored_collateral)
action_escrow_total       = sum(pending_action.escrowed_collateral)
referral_claimable_total  = sum(referral_balance[referrer])
```

The protocol total and guaranteed receiver-funding total must likewise equal
the obligations created by their respective accounting paths.

### 9.2 Fee distribution conservation

Only a fee actually collected from escrow or position value can be
distributed. For every collected opening or closing fee:

```text
collected_fee = lp_amount + protocol_amount + referral_amount
```

For every collected borrow payment:

```text
collected_borrow = lp_amount + protocol_amount
referral_amount = 0
```

All configured percentage shares use the collected amount as their base.
Floor rounding is applied to the LP and referral shares, and the protocol
receives the exact remainder. Therefore the split cannot over-distribute and
cannot leave unowned fee dust.

LP revenue creates no explicit claim. Removing the collected amount from the
trader-owned label, then adding only the protocol and referral labels, leaves
the LP share in residual equity. Protocol and referral claim totals increase
by exactly their calculated amounts. Keepers receive no percentage of fee
revenue, and no keeper reserve exists.

### 9.3 Funding conservation

Funding is a risk-balancing transfer, not protocol revenue. It never invokes
opening-fee, closing-fee, borrow-fee, referral, or keeper distribution.

Each funding window divides payer flow into two non-overlapping parts:

```text
total payer flow = receiver-backed flow + LP-backed flow
```

Receiver-backed flow is limited by offsetting receiver exposure. The remaining
flow is LP-backed. Separate cumulative indices and separate carried remainders
preserve this ownership split.

The corresponding rules are:

- payer obligations round up;
- receiver credits round down;
- the receiver claim is recognized when receiver-backed flow accrues, not
  when a receiver later touches a position;
- collecting receiver-backed funding from a payer restores the cash that
  economically backs the already-recognized receiver claim;
- collecting LP-backed funding leaves the amount in LP residual equity; and
- no uncollected payer amount is recorded as protocol revenue.

Rounding residue remains assigned to its defined accumulator until a quotient
can be produced or the empty-book reset releases the final unassignable residue
to LPs. No residue can be counted simultaneously as a receiver credit and LP
revenue.

### 9.4 Receiver-funding guarantees

Once receiver-backed funding accrues, it is a senior explicit liability:

```text
pending_receiver_funding_total >= 0
pending_receiver_funding_total = sum(market.pending_receiver_funding)
```

Moving receiver funding into a position must decrease
that position's `market.pending_receiver_funding` and the global total, then
increase position collateral by the same whole-cash amount. Both claim totals
must be sufficient before the move, and neither may underflow.

That sufficiency is a property of the arithmetic, not a check that may fail in
normal operation. It holds because both sides of the split derive from the same
`receiver_backing_scaled` value, because the receiver-side division carries its
remainder under a constant divisor within each window (§4.5.1), and because
each position's credit is floored at its own boundary. Over any sequence of
checkpoints the value distributed through the receiver index equals the accrued
backing minus the retained remainder, and each floored position read is at most
its exact share, so:

```text
sum(receiver credits taken) <= sum(receiver_liability_delta recognized)
```

If an implementation omits the §4.5.1 reset, this bound is violated by up to
`receiver_size / INDEX_PRECISION` whole cash units and the sufficiency check in
`credit_received_funding` reverts, which would prevent a receiver position from
being settled. The reset is what makes the check unreachable rather than merely
defensive; it must not be replaced by a clamp that hides the deficit.

Receiver-backed funding is collected before negative PnL, LP-backed funding,
borrow, keeper rewards, closing fees, and trader payout. If a forced terminal
settlement cannot collect it from the payer, LP residual equity absorbs the
shortfall. The receiver's earned credit is not reduced retroactively.

When one market's complete book is empty and no receiver in that market can
remain entitled to its final residue, empty-book cleanup releases that market's
unassignable amount to LPs and reduces the global total by the same amount. It
may not erase a whole-cash receiver claim belonging to a live or settling
position.

### 9.5 Index monotonicity

Every cumulative borrow and funding index is unsigned and monotonic:

```text
next_index >= previous_index
```

Elapsed time must be non-negative. A position debt baseline must never exceed
the current cumulative entitlement or obligation from which pending value is
derived. A negative raw pending delta indicates corrupted state, incorrect
checkpoint ordering, or a decreasing index and must revert before minimum-fee
logic or another clamp can hide it.

EMA state may move in either signed direction because market skew can change.
That does not permit any cumulative payment index to decrease. Directional
funding uses separate payer-side indices; it does not reverse an existing
index to represent a sign change.

### 9.6 No retroactive rate changes

Time is always accrued under the rate and exposure state that governed the
elapsed interval:

```text
1. accrue global borrow to now using the stored old borrow rate
2. accrue affected market funding to now using the stored old exposure state
3. settle the position's completed index window
4. apply the cash, claim, or exposure mutation
5. refresh utilization-dependent borrow rate from the resulting state
6. establish new position debt baselines and minimum-borrow quote
```

Refreshing a rate before accruing elapsed time would reprice history and is
forbidden. Mutating exposure before checkpointing funding would assign past
funding to exposure that did not yet exist, or remove exposure that did exist,
and is likewise forbidden.

Configuration changes follow the same boundary. The old value prices time up
to the configuration checkpoint; the new value applies only afterward.

### 9.7 Exposure aggregate correctness

Every market and global exposure aggregate equals the sum of its live
positions after each completed mutation. At minimum:

```text
global.total_risk_units = sum(position.risk_units)

market.long_size  = sum(size of live long positions)
market.short_size = sum(size of live short positions)

market.long_base  = sum(base_exposure of live long positions)
market.short_base = sum(base_exposure of live short positions)
```

Opening and increasing add exactly the exposure derived for the added size.
Partial decrease removes proportional base exposure using the specified
rounding rule and re-derives risk units from resulting size. Full close,
liquidation, and ADL remove every
remaining size, base-exposure, and risk-unit unit assigned to the position so
no terminal dust remains in market aggregates.

Pending actions and escrow are not exposure. They affect no open-interest,
base, risk-unit, funding-skew, or capacity aggregate before successful
settlement.

### 9.8 Risk-capacity enforcement

Every successful open or increase must satisfy the post-settlement capacity
condition using the resulting claims, LP equity, fees, keeper reward, and
exposure:

```text
total_risk_units_after * BPS
    <= cash_lp_equity_after * risk_capacity_limit_bps
```

Per-market size and base-exposure caps must pass in the same projected state.
No pending action reserves capacity. Competing actions are checked in atomic
settlement order, so a later action can fail even if it appeared fillable when
created.

LP withdrawal must also preserve its withdrawal utilization limit and all
required risk backing. A price move, newly recognized liability, or cash
change can make live utilization exceed the admission limit after the fact.
That state blocks additional risk and LP withdrawal as defined; it does not
rewrite already-open positions or imply that the original admission check was
skipped.

### 9.9 Position-health consistency

The same effective-collateral formula and the same checkpointed price-and-fee
snapshot must be used for previews, admission checks, liquidation eligibility,
and final settlement decisions:

```text
effective_collateral =
      stored_collateral
    + funding_received
    - receiver_funding_owed
    - lp_funding_owed
    - borrow_due
    + payable_pnl
```

An open or increase must leave the resulting position at or above initial
margin after all entry/increase charges and after the minimum-borrow floor
that its new window will quote. The floor is part of pending borrow from the
first second of the window, so admitting a position without it would accept a
position that is already below initial margin. A partial decrease must pay every
completed senior obligation and leave the survivor with at least minimum
collateral and maintenance margin. Debt baselines cannot reset while any old
window obligation remains unpaid.

A collateral-only addition does not reset funding, borrow, or the quoted
minimum-borrow window. It changes collateral and health only.

If an unchanged position is already liquidatable, or paying an ordinary
failed-action keeper reward would make it unsafe, voluntary settlement cannot
use the LP backstop. Liquidation takes precedence.

### 9.10 Liquidation-reward safety

Liquidation eligibility uses:

```text
maintenance = ceil(size * maintenance_margin_bps / BPS)

liquidation_threshold = max(
    maintenance,
    keeper_liquidation_reward
)

liquidatable = effective_collateral <= liquidation_threshold
```

Every newly accepted surviving position must also satisfy:

```text
stored_collateral >= min_collateral
min_collateral > keeper_liquidation_reward
```

Together, these rules create a liquidation opportunity while the configured
fixed reward should still be present. They do not claim that insolvency is
impossible: a discontinuous price move or delayed settlement can jump through
the threshold. Liquidation therefore pays from remaining position collateral
first and uses LP residual equity only for the reward gap.

The keeper always receives at most the configured fixed liquidation reward,
never a percentage of collateral, size, target health, or trader loss. A
liquidation that reverts pays nothing.

### 9.11 Escrow isolation

Action escrow is an explicit trader-owned claim until its terminal
distribution. While pending it:

- is included in `action_escrow_total`;
- is excluded from cash LP equity and free risk capacity;
- is not position collateral;
- earns no position PnL or funding;
- pays no opening or closing fee; and
- cannot be used by a different action or position.

Every escrow-bearing terminal transition satisfies exactly one of these
identities:

```text
successful entry:
    escrow_before =
      keeper_reward_paid_from_escrow
        + opening_fee_collected
        + initial_position_collateral

expected entry failure or entry expiry:
    escrow_before = keeper_reward_paid_from_escrow + owner_refund

owner limit cancellation:
    escrow_before = owner_refund

successful increase with added collateral:
    escrow_before = amount_converted_to_position_collateral

failed or superseded increase with added collateral:
    escrow_before = owner_refund

action.escrowed_collateral_after = 0
```

For a successful increase, the complete added-collateral escrow joins the
position first. The opening fee and increase keeper reward are then separate
debits from the resulting position value; they are not second distributions of
the same escrow.

The individual escrow and `action_escrow_total` decrease by identical amounts.
The action record cannot be removed while positive escrow remains attached.
Refunds always use the owner stored at commitment; the caller cannot redirect
them.

Entry keeper rewards are read from active configuration rather than frozen in
the action. Their global bounds against `min_collateral`, together with the
minimum escrow checked at entry creation, guarantee that a valid open, limit,
or expiry reward remains payable after a later configuration update.

### 9.12 No opening fee on failed entry

An opening fee is collected only after an entry or increase passes every
execution check and is committed successfully.

For an expected failed market entry or triggered limit entry:

```text
opening_fee_collected = 0
position_created = false
keeper_reward = configured action reward
owner_refund = escrow - keeper_reward
```

For an owner-cancelled limit entry, both the opening fee and keeper reward are
zero and the complete escrow is refunded. For expiry cleanup, the opening fee
is zero; only the expiry reward is deducted.

An expected failed increase likewise charges no opening fee on the proposed
added size, leaves existing exposure unchanged, and refunds all added-
collateral escrow. Its action-specific keeper reward may be taken from the
existing position only when doing so leaves that position safe.

### 9.13 Closing-fee boundaries

Closing fees apply only to voluntary decreases, voluntary closes, TP, and SL.
Liquidation and ADL never charge them.

For each fee-bearing settlement:

```text
if payable_price_pnl <= 0:
    collected_closing_fee = 0
else:
    size_component = ceil(closed_size * close_size_fee_bps / BPS)
    pnl_component  = ceil(payable_price_pnl * close_pnl_fee_bps / BPS)
    nominal_fee    = min(
        max(size_component, pnl_component),
        payable_price_pnl
    )

    profit_after_senior_items = max(
        0,
          payable_price_pnl
        + funding_received
        - receiver_backed_funding_owed
        - lp_backed_funding_owed
        - borrow_fee
        - keeper_reward
    )

    collected_closing_fee = min(nominal_fee, profit_after_senior_items)
```

Consequently:

```text
0 <= collected_closing_fee <= payable_price_pnl
```

The fee can reduce the current settlement's remaining price profit to zero,
but cannot consume original collateral, previously stored profit, added
collateral, or receiver-funding credit. Any nominal amount above the
collectible boundary is waived immediately, creates no claim, and is not bad
debt.

### 9.14 Single settlement and keeper payment

One pending action ID can reach a terminal state only once. The pending record,
its reverse position reference, all escrow, and the selected keeper payment
are consumed or cleared in one atomic transition.

Each successful or expected-terminal settlement selects exactly one keeper
reward. Internal cleanup does not add rewards. In particular:

- liquidation or ADL that supersedes a pending mutation pays only the forced-
  action reward;
- expiry cleanup pays only the expiry reward;
- TP or SL pays only its own reward when it executes successfully; and
- not-ready, untriggered, owner-cancelled, superseded, and reverted calls pay
  no ordinary action reward.

Monotonic non-reused IDs and record removal prevent replay. Two callers racing
for the same action cannot both succeed or both be paid.

### 9.15 Rounding direction

Rounding follows ownership and risk direction consistently:

- trader and funding-payer obligations round up;
- trader and funding-receiver credits round down;
- opening and closing fees round up before their explicit caps;
- long base exposure rounds down;
- short base exposure rounds up;
- long value used for PnL rounds down;
- short buyback value used for PnL rounds up;
- remaining base exposure after a partial reduction rounds down, with the
  removed portion receiving the difference — a conservation rule, not a
  direction rule, because the two portions always sum to the pre-reduction
  base (§2.11);
- remaining risk units are re-derived from resulting size;
- LP and referral percentage shares round down; and
- the protocol receives fee-split remainder;
- LP shares minted for a deposit round down; and
- assets paid for an LP withdrawal round down.

Each repeated time or distribution division carries its own remainder until
its defined reset. A remainder belonging to one index, ownership stream, or
market cannot be reused by another. A terminal exposure removal clears the
position's proportional remainder so aggregate dust cannot survive its owner.

No payer floor, fee cap, or `max` operation may be applied before checking for
an invalid negative raw accrual.

### 9.16 Atomic reversion

An unexpected failure changes nothing. All state writes and token transfers in
one operation are atomic, including:

- index, timestamp, EMA, and remainder updates;
- position collateral and aggregate-collateral updates;
- exposure aggregates and risk state;
- pending-action records and reverse references;
- escrow, protocol, referral, and receiver-funding claims;
- keeper payments, trader refunds, payouts, and LP transfers; and
- events describing the outcome.

If authorization, oracle proof, arithmetic, aggregate consistency, claim
sufficiency, or token transfer fails, the operation reverts to its exact prior
state and pays nobody. A revert is not a terminal business outcome and does not
consume the pending action.

Expected execution failures are deliberately different: they commit the
defined `Failed` result, keeper payment, refund, and record removal. Treating an
expected failure as a revert would preserve a free retry and violate the order
lifecycle.

## 10. Configuration reference

Configuration separates vault-wide policy from market-specific risk and fee
policy. Every amount is stored at `PRICE_PRECISION`, every percentage is in
basis points, and every duration is in whole seconds unless stated otherwise.

Initial values are activation policy, not immutable constants. An authorized
configuration change may alter them only after validation and any required
checkpoint. Changing a parameter never repairs or excuses an already-invalid
accounting state.

### 10.1 Global parameters

Global parameters affect the complete vault.

#### 10.1.1 Position, borrow, and funding policy

| Parameter | Unit | Initial value | Meaning |
|---|---:|---:|---|
| `min_collateral` | Cash | `10,000,000` (`$1.00`) | Minimum stored collateral for every surviving position |
| `min_position_lifetime` | Seconds | `60` | Time after open or the latest size increase before voluntary exposure removal |
| `min_borrow_fee_seconds` | Seconds | `900` | Duration used to quote the monetary minimum for each borrow window |
| `funding_half_life_seconds` | Seconds | `43,200` | Shared EMA half-life used by all funding markets |
| `risk_capacity_limit_bps` | Bps | `8,500` | Maximum admitted risk units relative to cash LP equity |
| `base_borrow_rate_bps_day` | Bps/day on risk units | `25` | Borrow rate at zero utilization |
| `max_variable_borrow_bps_day` | Bps/day on risk units | `250` | Maximum utilization-dependent addition to the borrow rate |
| `borrow_exponent_bps` | Bps exponent | `20,000` | Utilization-curve exponent; `20,000` represents `2.0` |

#### 10.1.2 Revenue policy

| Parameter | Unit | Initial value | Meaning |
|---|---:|---:|---|
| `fee_lp_revenue_share_bps` | Bps | `9,000` | LP share of collected opening and closing fees |
| `borrow_lp_revenue_share_bps` | Bps | `9,000` | LP share of collected borrow |
| `referral_fee_share_bps` | Bps | `250` | Referrer share of collected opening and closing fees when eligible |

The protocol percentage is derived rather than configured:

```text
open_or_close_protocol_share_without_referrer =
    BPS - fee_lp_revenue_share_bps

open_or_close_protocol_share_with_referrer =
    BPS - fee_lp_revenue_share_bps - referral_fee_share_bps

borrow_protocol_share =
    BPS - borrow_lp_revenue_share_bps
```

With the initial values, opening and closing fees pay LPs `90%` and the
protocol `10%` without a referrer. With a referrer they pay LPs `90%`, the
referrer `2.5%`, and the protocol `7.5%`, subject to integer residue going to
the protocol. Borrow pays LPs `90%` and the protocol `10%`.

#### 10.1.3 Keeper rewards

Every action has an independent fixed reward:

| Parameter | Initial value | Ordinary payment source |
|---|---:|---|
| `keeper_open_reward` | `2,500,000` (`$0.25`) | Entry escrow |
| `keeper_limit_order_reward` | `2,500,000` (`$0.25`) | Entry escrow |
| `keeper_increase_reward` | `2,500,000` (`$0.25`) | Position value |
| `keeper_decrease_reward` | `2,500,000` (`$0.25`) | Position value |
| `keeper_close_reward` | `2,500,000` (`$0.25`) | Position value |
| `keeper_tp_reward` | `2,500,000` (`$0.25`) | Position value |
| `keeper_sl_reward` | `2,500,000` (`$0.25`) | Position value |
| `keeper_expiry_reward` | `2,500,000` (`$0.25`) | Entry escrow |
| `keeper_liquidation_reward` | `2,500,000` (`$0.25`) | Position value, with LP backing for a price-gap shortfall |
| `keeper_adl_reward` | `2,500,000` (`$0.25`) | Affected position value |
| `keeper_lp_resolve_reward` | `2,500,000` (`$0.25`) | LP request escrow or released assets |

Equal initial values make each successful action equally attractive to a
keeper. The independent fields allow later tuning without coupling unrelated
actions. No percentage keeper reward, keeper fee share, keeper reserve, or
user-selected execution budget exists.

#### 10.1.4 LP and bounded-operation policy

| Parameter | Unit | Initial value | Meaning |
|---|---:|---:|---|
| `max_active_markets` | Count | `8` | Maximum active markets included in synchronized vault operations |
| `global_hard_cap_factor_limit_bps` | Bps | `10,000` | Maximum sum of configured side hard-cap factors across active markets |
| `max_withdraw_utilization_bps` | Bps | `8,000` | Maximum post-withdrawal utilization |
| `min_deposit_nav_factor_bps` | Bps | `8,000` | Minimum marked-NAV-to-cash-equity factor for an ordinary LP deposit |
| `lp_request_delay_seconds` | Seconds | Profile-specific | Delay assigning an LP request to a synchronized price round |

The initial LP request delays are operational profiles:

| Profile | `lp_request_delay_seconds` |
|---|---:|
| Local development | `60` |
| Public test environment | `3,600` |
| Production environment | `86,400` |

The delay is explicit at activation and may be overridden by policy. It is not
inferred from the network or transaction cadence.

### 10.2 Per-market parameters

Each market independently configures its fees, funding response, margin,
capacity bounds, forced-risk thresholds, and trader-action delay.

#### 10.2.1 Fees, funding, and risk units

| Parameter | Unit | Initial value | Meaning |
|---|---:|---:|---|
| `open_fee_bps` | Bps of size added | `0` | Opening fee on opens and size increases |
| `close_size_fee_bps` | Bps of size removed | `5` | Size component of the closing fee (`0.05%`) |
| `close_pnl_fee_bps` | Bps of positive payable PnL | `1,000` | PnL component of the closing fee (`10%`) |
| `max_funding_rate_bps_day` | Bps/day on position size | `80` | Payer funding rate at full blended skew |
| `instant_weight_bps` | Bps | `3,000` | Weight of live skew versus EMA history |
| `market_risk_factor_bps` | Bps of position size | `1,000` | Portion of notional converted into risk units |

`open_fee_bps = 0` keeps opening free at activation while retaining the full
accounting path. Neither opening nor closing fee uses market skew.

#### 10.2.2 Margin and side-risk thresholds

| Parameter | Unit | Initial value | Meaning |
|---|---:|---:|---|
| `initial_margin_bps` | Bps of position size | `500` | Required effective collateral after opening or adding risk |
| `maintenance_margin_bps` | Bps of position size | `250` | Ordinary health floor and one component of liquidation eligibility |
| `recovery_pnl_factor_bps` | Bps of cash LP equity | `250` | Threshold below which a restricted side can recover |
| `warning_pnl_factor_bps` | Bps of cash LP equity | `400` | Threshold that latches the side into warning |
| `adl_pnl_factor_bps` | Bps of cash LP equity | `500` | Threshold permitting ADL |
| `hard_cap_pnl_factor_bps` | Bps of cash LP equity | `600` | Threshold and payout factor for hard-cap settlement |

The PnL-factor thresholds apply independently to each market side. The global
hard-cap-factor limit bounds their aggregate configuration across the active
market registry.

#### 10.2.3 Exposure limits and execution delay

| Parameter | Unit | Initial value | Meaning |
|---|---:|---:|---|
| `max_long_size_open_interest` | Notional | `1,000,000,000,000,000` | Long-side size ceiling |
| `max_short_size_open_interest` | Notional | `1,000,000,000,000,000` | Short-side size ceiling |
| `max_long_base_exposure` | Base units | `1,000,000,000,000,000,000` | Long-side base ceiling |
| `max_short_base_exposure` | Base units | `1,000,000,000,000,000,000` | Short-side base ceiling |
| `order_execution_delay_seconds` | Seconds | `5` | Delay for trader-requested price-sensitive actions |

At seven-decimal notional precision, the initial size ceiling is
`$100,000,000` per side. Exposure ceilings are admission bounds, not targets.

### 10.3 Parameter validation

Validation occurs both when configuration is activated and when an individual
market is added or updated. Every configured integer must fit the widest
intermediate arithmetic used by its formulas.

#### 10.3.1 Global validation

```text
min_collateral > 0
min_collateral > keeper_liquidation_reward
keeper_open_reward <= min_collateral
keeper_limit_order_reward <= min_collateral
keeper_expiry_reward <= min_collateral

min_position_lifetime >= 0
min_borrow_fee_seconds > 0

60 <= funding_half_life_seconds <= 31,536,000

0 < risk_capacity_limit_bps <= BPS
0 <= max_withdraw_utilization_bps <= BPS
0 <= min_deposit_nav_factor_bps <= BPS

0 <= base_borrow_rate_bps_day <= BPS
0 <= max_variable_borrow_bps_day <= BPS
0 < borrow_exponent_bps <= 100,000

fee_lp_revenue_share_bps + referral_fee_share_bps <= BPS
borrow_lp_revenue_share_bps <= BPS

max_active_markets > 0
0 < global_hard_cap_factor_limit_bps <= BPS
lp_request_delay_seconds > 0
```

All fixed keeper rewards must be non-negative cash amounts. The three entry-
escrow bounds ensure a later valid reward increase cannot exceed the minimum
collateral already required from every pending entry. Entry creation still
adds action-specific checks against the order's actual escrow:

```text
market entry escrow >= keeper_open_reward
limit entry escrow  >= keeper_limit_order_reward
expiring entry escrow >= keeper_expiry_reward
```

The complete entry checks also include opening fee, minimum collateral, and
initial margin.

For every active-market set:

```text
active_market_count <= max_active_markets

sum(hard_cap_pnl_factor_bps for every active market side)
    <= global_hard_cap_factor_limit_bps
```

#### 10.3.2 Per-market validation

```text
0 <= open_fee_bps <= BPS
0 <= close_size_fee_bps <= BPS
0 <= close_pnl_fee_bps <= BPS

0 <= max_funding_rate_bps_day <= BPS
0 <= instant_weight_bps <= BPS
0 < market_risk_factor_bps <= BPS

0 < maintenance_margin_bps <= initial_margin_bps <= BPS

0 <= recovery_pnl_factor_bps
recovery_pnl_factor_bps
    < warning_pnl_factor_bps
    < adl_pnl_factor_bps
    < hard_cap_pnl_factor_bps
    <= BPS

0 < max_long_size_open_interest  <= 10,000,000,000,000,000
0 < max_short_size_open_interest <= 10,000,000,000,000,000
0 < max_long_base_exposure       <= 1,000,000,000,000,000,000
0 < max_short_base_exposure      <= 1,000,000,000,000,000,000

1 <= order_execution_delay_seconds <= 30
```

There is deliberately no ordering relationship between the size-based and
PnL-based closing-fee rates. The charged target is always their maximum and
the profit caps provide the economic bound.

#### 10.3.3 Configuration-update boundaries

A parameter update follows these rules:

1. Authenticate the configuration authority.
2. Validate the complete proposed configuration and every cross-parameter
   relationship.
3. Accrue each affected time-based accumulator to the update timestamp using
   the old parameter.
4. Store the new parameter only after that checkpoint.
5. Refresh any forward-looking derived rate or risk state.
6. Emit the old and new values with the effective timestamp.

Fields affecting borrow accrual include the base rate, variable rate,
exponent, and any value that changes cash LP equity or admitted risk. Funding
half-life, maximum funding rate, and instant weight require the affected
funding window to checkpoint before replacement. Threshold changes recompute
the affected risk state from one authenticated snapshot.

Changing `market_risk_factor_bps` additionally requires both sides of the
market to have zero open interest and zero risk units.

The following values are frozen when an action or borrow window is created:

| Value | Frozen for |
|---|---|
| `execute_after` derived from `order_execution_delay_seconds` | Each pending trader action and attached TP/SL instruction |
| `commit_observed_at` | Each pending trader action and attached TP/SL instruction |
| `expires_at` | Each expiring entry order |
| `stored_minimum_borrow_fee` | Each active borrow window |

A later configuration update does not rewrite these stored values. Other fee,
reward, margin, capacity, and risk parameters are read from active
configuration when settlement occurs. A pending action therefore guarantees
its committed trader inputs and timing cursor, not an immutable future
governance configuration.

Entry orders do not snapshot keeper rewards. They use the active open, limit,
or expiry reward at terminal settlement. The global bounds against
`min_collateral`, combined with the entry's minimum escrow requirement, ensure
that any valid later reward remains payable. If the active opening fee, reward,
margin, or capacity policy leaves insufficient collateral to create a healthy
position, the entry follows the normal terminal expected-failure path: pay the
active action reward, refund the remainder, charge no opening fee, and create
no position.

#### 10.3.4 Required design resolutions

The following policies must be resolved before this specification is treated
as implementation-complete:

1. **Marked-NAV loss recognition.** Capping aggregate side loss by aggregate
   side collateral can count collateral belonging to one position as
   collectible against another position's loss. Choose either conservative NAV
   that ignores unrealized trader losses, per-position collectible-loss
   accounting with bounded aggregates, or another rule that cannot socialize
   position collateral implicitly.
2. **Hard-cap payout allocation.** Recomputing a side payout factor after each
   settlement makes payouts depend on settlement order and does not impose one
   stable aggregate cap. Choose a snapshotted side-wide payout epoch or replace
   the dynamic payout factor with a deterministic position-local bound.
3. **Liquidation-reward guarantee.** The current payment primitive requires
   enough LP cash for a reward gap, while the prose promises the complete fixed
   reward. Choose an explicitly reserved reward, a separately funded backstop,
   or best-effort payment language. A protocol cannot guarantee payment from
   LP equity when LP equity is already zero.
4. **Failed position-action rewards.** *Resolved.* Finality is unconditional
   and only the reward amount is contingent: an eligible attempt on a
   non-liquidatable position always terminates, paying the keeper whatever the
   position can fund without breaching minimum collateral or its liquidation
   threshold, possibly nothing. `RewardUnavailable` no longer exists.
   Position-action creation does not escrow a keeper reward. See §7.0.

Items 1 through 3 remain open. Until they are decided, implementations must
not infer a policy for them from the illustrative pseudocode.

### 10.4 Deployment defaults

The complete initial parameter set is:

```text
GLOBAL
min_collateral                       = 10,000,000       # $1.00
min_position_lifetime                = 60              # 1 minute
min_borrow_fee_seconds               = 900             # 15 minutes
funding_half_life_seconds            = 43,200          # 12 hours
risk_capacity_limit_bps              = 8,500           # 85%
base_borrow_rate_bps_day             = 25
max_variable_borrow_bps_day          = 250
borrow_exponent_bps                  = 20,000          # exponent 2.0
fee_lp_revenue_share_bps             = 9,000           # 90%
borrow_lp_revenue_share_bps          = 9,000           # 90%
referral_fee_share_bps               = 250             # 2.5%
max_active_markets                   = 8
global_hard_cap_factor_limit_bps     = 10,000
max_withdraw_utilization_bps         = 8,000           # 80%
min_deposit_nav_factor_bps           = 8,000           # 80%

KEEPER REWARDS
keeper_open_reward                   = 2,500,000        # $0.25
keeper_limit_order_reward            = 2,500,000        # $0.25
keeper_increase_reward               = 2,500,000        # $0.25
keeper_decrease_reward               = 2,500,000        # $0.25
keeper_close_reward                  = 2,500,000        # $0.25
keeper_tp_reward                     = 2,500,000        # $0.25
keeper_sl_reward                     = 2,500,000        # $0.25
keeper_expiry_reward                 = 2,500,000        # $0.25
keeper_liquidation_reward            = 2,500,000        # $0.25
keeper_adl_reward                    = 2,500,000        # $0.25
keeper_lp_resolve_reward             = 2,500,000        # $0.25

PER MARKET
open_fee_bps                         = 0                # 0% of size added
close_size_fee_bps                   = 5                # 0.05% of size removed
close_pnl_fee_bps                    = 1,000            # 10% of payable PnL
max_funding_rate_bps_day             = 80
instant_weight_bps                   = 3,000            # 30% live skew
market_risk_factor_bps               = 1,000            # 10% of notional
initial_margin_bps                   = 500              # 5%
maintenance_margin_bps               = 250              # 2.5%
recovery_pnl_factor_bps              = 250              # 2.5%
warning_pnl_factor_bps               = 400              # 4%
adl_pnl_factor_bps                   = 500              # 5%
hard_cap_pnl_factor_bps              = 600              # 6%
max_long_size_open_interest          = 1,000,000,000,000,000
max_short_size_open_interest         = 1,000,000,000,000,000
max_long_base_exposure               = 1,000,000,000,000,000,000
max_short_base_exposure              = 1,000,000,000,000,000,000
order_execution_delay_seconds        = 5

LP REQUEST DELAY PROFILE
local development                    = 60               # 1 minute
public test environment              = 3,600            # 1 hour
production environment               = 86,400           # 1 day
```

The removed parameters are not set to zero and retained as dead configuration;
they do not exist. This includes skew-tiered opening or closing fees, a minimum
borrow index delta, a keeper revenue share, a keeper reserve, user execution
budgets, percentage liquidation or ADL rewards, a maximum ADL reward, and an
insolvency-touch reward.

## 11. End-to-end examples

These examples use the initial parameters unless a different rate is stated.
Dollar values are displayed to seven decimal places where rounding matters.
They assume no referral unless one is explicitly included.

Common values are:

```text
PRICE_PRECISION                    = 10,000,000
BPS                                = 10,000
market_risk_factor_bps             = 1,000       # 10% of size
initial_margin_bps                 = 500         # 5% of size
maintenance_margin_bps             = 250         # 2.5% of size
open_fee_bps                       = 0
close_size_fee_bps                 = 5           # 0.05% of size removed
close_pnl_fee_bps                  = 1,000       # 10% of payable PnL
min_borrow_fee_seconds             = 900         # 15 minutes
every keeper reward                = $0.25
```

Each example begins from a valid solvent state. Unmentioned funding is zero,
prices are authenticated, delays and minimum lifetimes have elapsed, and the
relevant action uses a qualifying post-commit price observation.

### 11.1 Successful leveraged market open

A trader commits a `$100,000` BTC long with `$5,100` submitted collateral.
The commitment price is `$49,900`, its acceptable maximum is `$50,100`, and a
keeper later settles it at `$50,000`.

At creation:

```text
submitted collateral transferred to vault = $5,100.00
action_escrow_total increase               = $5,100.00
position exposure created                  = $0
risk capacity reserved                     = $0
```

At settlement:

```text
fill price       = $50,000 <= $50,100 acceptable maximum
base exposure    = floor($100,000 / $50,000) = 2 BTC
risk units       = floor($100,000 * 10%)      = $10,000

opening fee      = ceil($100,000 * 0%) = $0.00
open reward      = $0.25

position collateral = $5,100.00 - $0.00 - $0.25
                    = $5,099.75

initial margin       = ceil($100,000 * 5%)
                    = $5,000.00

projected minimum borrow at 25 bps/day on $10,000 risk units
                     = ceil($10,000 * 25 / 10,000 * 900 / 86,400)
                     = $0.0260417

$5,099.75 >= $5,000.0260417                 => margin passes
post-settlement risk units pass capacity    => capacity passes
```

The action escrow becomes zero, the keeper receives `$0.25`, and `$5,099.75`
is relabelled as position collateral. The position starts its funding and
borrow baselines at the settlement indices. With the opening fee set to zero,
LP, protocol, and referral opening-fee revenue are all zero.

### 11.2 Market open rejected by slippage

The same trader commits the same long and transfers `$5,100`. The acceptable
maximum remains `$50,100`, but the first eligible fill is `$50,200`:

```text
fill price = $50,200 > $50,100 acceptable maximum
result     = terminal expected failure

opening fee       = $0.00
keeper reward     = $0.25
owner refund      = $5,100.00 - $0.25 = $5,099.75
position created  = no
exposure added    = $0
risk units added  = $0
```

The action record and its complete escrow label are removed. The keeper and
refund transfers total the original `$5,100`, so no cash or claim is stranded.
The failed action cannot retry at another price.

### 11.3 Profitable close below the size-fee threshold

A `$100,000` position has `$5,000` stored collateral and closes with `$30` of
positive payable price PnL. It has no funding. The 15-minute minimum borrow
quote was made at `25` bps/day on `$10,000` risk units:

```text
actual short-window borrow < minimum

borrow due = ceil(
    $10,000 * 25 / 10,000 * 900 / 86,400
) = $0.2604167

close reward = $0.25
```

The closing-fee calculation is:

```text
size component = ceil($100,000 * 0.05%) = $50.00
pnl component  = ceil($30 * 10%)         = $3.00
target         = max($50, $3)            = $50.00
nominal fee    = min($30, $50)           = $30.00

profit after senior items =
    $30.00 - $0.2604167 - $0.25
  = $29.4895833

collected closing fee = min($30.00, $29.4895833)
                      = $29.4895833
waived closing fee    = $0.5104167
```

Final settlement is:

```text
starting collateral      $5,000.0000000
+ payable PnL                $30.0000000
- borrow                      $0.2604167
- keeper reward               $0.2500000
- closing fee                $29.4895833
------------------------------------------------
trader payout             $5,000.0000000
```

The trader keeps all original collateral but none of this small price profit.
The closing fee did not consume collateral; senior charges reduced how much of
the nominal closing fee was collectible.

Without a referrer, the collected closing fee splits as:

```text
LP residual       = floor($29.4895833 * 90%) = $26.5406249
protocol claim    =                         =  $2.9489584
```

The borrow splits into `$0.2343750` for LP residual and `$0.0260417` for the
protocol.

### 11.4 Profitable close where the PnL fee dominates

A `$100,000` position with `$5,000` stored collateral closes after one day
with `$2,000` of payable price PnL. The borrow rate was a constant `87.5`
bps/day on its `$10,000` risk units:

```text
borrow due = $10,000 * 0.875% = $87.50
close reward = $0.25

size component = $100,000 * 0.05% = $50.00
pnl component  = $2,000 * 10%      = $200.00
nominal fee    = max($50, $200)     = $200.00

profit after senior items = $2,000 - $87.50 - $0.25
                          = $1,912.25
collected closing fee     = min($200, $1,912.25)
                          = $200.00
```

Final payout:

```text
$5,000 + $2,000 - $87.50 - $0.25 - $200 = $6,712.25
```

Assume this trader has an eligible referrer. The closing fee distributes:

```text
LP residual       = $180.00
referrer claim    =   $5.00
protocol claim    =  $15.00
```

Borrow separately distributes `$78.75` to LP residual and `$8.75` to the
protocol. The referrer and keeper receive no share of borrow.

### 11.5 Losing close

A `$100,000` position has `$5,000` stored collateral, `$1,000` of payable
price loss, `$25` of borrow due, and no funding:

```text
starting collateral      $5,000.00
- payable price loss      $1,000.00
- borrow                     $25.00
- close reward                $0.25
- closing fee                 $0.00
-------------------------------------
trader payout             $3,974.75
```

The closing fee is zero because payable price PnL is negative. The position
still pays borrow and the fixed close reward. Collected borrow distributes
`$22.50` to LP residual and `$2.50` to the protocol.

### 11.6 Increase and borrow-window reset

A position begins with:

```text
size                     = $100,000
risk units               =  $10,000
stored collateral        =  $10,000
rate at old-window quote = 35 bps/day on risk units
rate during elapsed time = 35 bps/day on risk units
old-window age           = 5 minutes
```

The trader commits a `$50,000` size increase with `$3,000` additional
collateral. The added collateral is transferred at creation but remains action
escrow until settlement.

The old window's actual borrow is:

```text
actual borrow = ceil(
    $10,000 * 35 / 10,000 * 300 / 86,400
) = $0.1215278
```

Its stored 15-minute minimum is:

```text
minimum borrow = ceil(
    $10,000 * 35 / 10,000 * 900 / 86,400
) = $0.3645834

old-window borrow due = max($0.1215278, $0.3645834)
                      = $0.3645834
```

Settlement first charges the completed old window against the pre-existing
position collateral, then applies the increase:

```text
old stored collateral                 $10,000.0000000
- old-window borrow                        $0.3645834
+ added collateral                     $3,000.0000000
- opening fee on $50,000                    $0.0000000
- increase keeper reward                   $0.2500000
-----------------------------------------------------
resulting stored collateral            $12,999.3854166

resulting size                         $150,000
resulting risk units                    $15,000
required initial margin                  $7,500
```

The position passes initial margin. Suppose the post-mutation utilization
refresh sets the new borrow rate to `40` bps/day. The old debt and minimum are
discarded after payment, the debt baseline resets at the current index, and
the full resulting exposure receives a new minimum:

```text
new minimum borrow = ceil(
    $15,000 * 40 / 10,000 * 900 / 86,400
) = $0.6250000
```

The new window starts with zero actual index accrual and a stored minimum of
`$0.6250000`. The `$50,000` increase is not tracked as a separate fee tranche.

### 11.7 Partial decrease

A long position entered at `$50,000` with:

```text
size               = $100,000
base exposure      = 2 BTC
risk units         = $10,000
stored collateral  = $10,000
```

The trader decreases `25%`, removing `$25,000` of size at a `$52,000` fill.
The proportional removal is:

```text
base removed       = 0.5 BTC
base remaining     = 1.5 BTC
risk removed       = $2,500
risk remaining     = $7,500
payable PnL removed = 0.5 * ($52,000 - $50,000)
                    = $1,000
```

At this checkpoint the complete old position window contains:

```text
funding received              = $10.00
receiver-backed funding owed  =  $5.00
LP-backed funding owed        =  $2.00
borrow due                    =  $1.00
decrease reward               =  $0.25
```

The closing fee on the removed portion is:

```text
size component = $25,000 * 0.05% = $12.50
pnl component  = $1,000 * 10%     = $100.00
nominal fee    = $100.00

profit after senior items =
    $1,000 + $10 - $5 - $2 - $1 - $0.25
  = $1,001.75

collected closing fee = $100.00
```

Capitalization and mutation leave:

```text
starting stored collateral      $10,000.00
+ funding received                  $10.00
- receiver funding                   $5.00
- LP funding                         $2.00
- borrow                             $1.00
+ realized payable PnL            $1,000.00
- decrease reward                    $0.25
- closing fee                      $100.00
--------------------------------------------
resulting stored collateral      $10,901.75
resulting size                   $75,000.00
```

No cash is paid out during the decrease; residual realized profit remains
position collateral. The old funding and borrow baselines are fully settled.
If the refreshed post-decrease borrow rate is `30` bps/day, the new `$7,500`
risk exposure starts with:

```text
new minimum borrow =
    $7,500 * 0.30% * 15 / 1,440
  = $0.2343750
```

### 11.8 Funding payer and receiver

A settled market has `$400,000` long and `$100,000` short at a common price.
The skew EMA has converged, so blended skew is `0.6` and longs pay. With an
`80` bps/day maximum funding rate:

```text
payer rate = 80 * 0.6^2 = 28.8 bps/day
long payer flow = $400,000 * 0.288% = $1,152.00

receiver fraction = $100,000 / $400,000 = 25%
receiver-backed funding = $1,152 * 25% = $288.00
LP-backed funding       = $1,152 - $288 = $864.00
```

Assume both sides close flat after one day and the borrow rate was `87.5`
bps/day on risk units:

```text
long risk units   = $40,000
long borrow       = $40,000 * 0.875% = $350.00
long close reward = $0.25
long net cost      = $1,152 + $350 + $0.25
                   = $1,502.25

short risk units   = $10,000
short borrow       = $10,000 * 0.875% = $87.50
short funding received                  = $288.00
short close reward                      = $0.25
short net funding benefit = $288 - $87.50 - $0.25
                          = $200.25
```

Both closes have zero price PnL, so both closing fees are zero. Funding sends
`$288` to the receiver and `$864` to LP residual; it sends nothing to the
protocol, referrer, or keeper. The combined `$437.50` borrow payment separately
sends `$393.75` to LP residual and `$43.75` to the protocol.

### 11.9 Liquidation after a violent price movement

A `$10,000` position has `$500` stored collateral. Its maintenance margin and
liquidation threshold before a gap are:

```text
maintenance margin = $10,000 * 2.5% = $250.00
fixed liquidation reward            =   $0.25
liquidation threshold = max($250, $0.25) = $250.00
```

A violent price move produces `$501` of payable loss before a keeper can act.
The position also has `$1` of borrow due and no funding:

```text
effective collateral = $500 - $501 - $1 = -$2.00
-$2.00 <= $250.00                         => liquidatable
```

Forced settlement applies the priority order:

```text
position collateral available                  $500.00
negative PnL requested                          $501.00
negative PnL collected                          $500.00
uncollectible negative PnL reported               $1.00

borrow requested                                  $1.00
borrow collected                                  $0.00
uncollected borrow                                 $1.00  # forgone revenue

liquidation reward                                 $0.25
paid from position                                 $0.00
paid from LP residual                              $0.25

closing fee                                        $0.00
trader payout                                      $0.00
```

The position and all its exposure are removed. The keeper remains willing to
perform the liquidation because the fixed reward is guaranteed even though the
price jumped through the intended collateral buffer. The uncollected borrow
does not become a protocol claim.

### 11.10 Automatic deleveraging

Cash LP equity is `$1,000,000`, and one market side has `$80,000` of aggregate
positive PnL. Its PnL factor is `8%`, above the `6%` hard-cap threshold:

```text
hard-cap value = $1,000,000 * 6% = $60,000
side payout factor = $60,000 / $80,000 = 0.75
```

A position selected for ADL has `$5,000` stored collateral and `$10,000` raw
positive PnL. It owes `$25` of borrow and no funding:

```text
raw position PnL       = $10,000.00
payable position PnL   = $10,000 * 0.75 = $7,500.00
ADL keeper reward      = $0.25
closing fee            = $0.00

trader payout = $5,000 + $7,500 - $25 - $0.25
              = $12,474.75
```

The uniform hard-cap factor applies before the position-specific settlement.
ADL removes the complete position, pays only the fixed ADL reward from its
value, charges no closing fee, and clears its pending mutation and triggers.
The side risk state is recalculated after exposure is removed.

### 11.11 Expired-order cleanup

A trader transfers `$5,100` for an entry order. It remains unexecuted until
its exact expiry boundary. At `now == expires_at`, execution and owner
cancellation are closed, while keeper cleanup is permitted:

```text
entry collateral held as action escrow = $5,100.00
active expiry reward                    =     $0.25

opening fee collected                   =     $0.00
keeper receives                         =     $0.25
owner refund                            = $5,099.75
position created                        = no
```

The keeper transfer and owner refund equal the complete escrow. The action and
escrow claim are removed, no exposure or fee revenue is created, and the order
cannot later execute. A valid later reward increase would use the new active
expiry reward, bounded by `min_collateral`, with the remainder refunded in the
same way.
