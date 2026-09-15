# WinTrader Trading, Fees, and Settlement Specification

## Contents

- [1. The system in plain language](#1-the-system-in-plain-language)
  - [1.1 The vault and liquidity providers](#11-the-vault-and-liquidity-providers)
  - [1.2 Positions, collateral, size, leverage, and PnL](#12-positions-collateral-size-leverage-and-pnl)
  - [1.3 Opening a position](#13-opening-a-position)
  - [1.4 Increasing a position](#14-increasing-a-position)
  - [1.5 Decreasing a position](#15-decreasing-a-position)
  - [1.6 Closing a position](#16-closing-a-position)
  - [1.7 Market and limit orders](#17-market-and-limit-orders)
  - [1.8 Take-profit and stop-loss](#18-take-profit-and-stop-loss)
  - [1.9 Liquidation](#19-liquidation)
  - [1.10 Automatic deleveraging](#110-automatic-deleveraging)
  - [1.11 Keepers](#111-keepers)
  - [1.12 Order creation and settlement](#112-order-creation-and-settlement)
  - [1.13 Fresh-price execution](#113-fresh-price-execution)
  - [1.14 Failed and expired orders](#114-failed-and-expired-orders)
- [2. Accounting model and numerical conventions](#2-accounting-model-and-numerical-conventions)
  - [2.1 Numerical precision and units](#21-numerical-precision-and-units)
  - [2.2 Physical vault cash](#22-physical-vault-cash)
  - [2.3 LP equity](#23-lp-equity)
  - [2.4 Position collateral](#24-position-collateral)
  - [2.5 Explicit claims and liabilities](#25-explicit-claims-and-liabilities)
  - [2.6 Position size and base exposure](#26-position-size-and-base-exposure)
  - [2.7 Risk units](#27-risk-units)
  - [2.8 Raw and payable PnL](#28-raw-and-payable-pnl)
  - [2.9 Effective collateral](#29-effective-collateral)
  - [2.10 Utilization](#210-utilization)
  - [2.11 Rounding rules](#211-rounding-rules)
  - [2.12 Stored and derived values](#212-stored-and-derived-values)
- [3. Fees and rewards](#3-fees-and-rewards)
  - [3.1 Opening fee](#31-opening-fee)
  - [3.2 Closing fee](#32-closing-fee)
  - [3.3 Borrow fee](#33-borrow-fee)
  - [3.4 Funding](#34-funding)
  - [3.5 Fixed keeper rewards](#35-fixed-keeper-rewards)
  - [3.6 Referral rewards](#36-referral-rewards)
  - [3.7 Fee revenue distribution](#37-fee-revenue-distribution)
  - [3.8 Settlement waterfall](#38-settlement-waterfall)
  - [3.9 Bad debt and the LP backstop](#39-bad-debt-and-the-lp-backstop)
  - [3.10 Fee examples](#310-fee-examples)
- [4. Index and checkpoint system](#4-index-and-checkpoint-system)
  - [4.1 Why indices are necessary](#41-why-indices-are-necessary)
  - [4.2 Global borrow index](#42-global-borrow-index)
  - [4.3 Per-market funding indices](#43-per-market-funding-indices)
  - [4.4 Position debt baselines](#44-position-debt-baselines)
  - [4.5 Accrual remainders](#45-accrual-remainders)
  - [4.6 Funding EMA and exact window integration](#46-funding-ema-and-exact-window-integration)
  - [4.7 Global checkpoint](#47-global-checkpoint)
  - [4.8 Market checkpoint](#48-market-checkpoint)
  - [4.9 Checkpoint and mutation order](#49-checkpoint-and-mutation-order)
  - [4.10 Opening a borrow window](#410-opening-a-borrow-window)
  - [4.11 Settling and resetting a borrow window](#411-settling-and-resetting-a-borrow-window)
  - [4.12 Reading pending fees without mutation](#412-reading-pending-fees-without-mutation)
  - [4.13 Empty-book initialization and reset](#413-empty-book-initialization-and-reset)
  - [4.14 Time and rounding invariants](#414-time-and-rounding-invariants)
- [5. Storage model](#5-storage-model)
  - [5.1 Global configuration](#51-global-configuration)
  - [5.2 Global accounting state](#52-global-accounting-state)
  - [5.3 Market configuration](#53-market-configuration)
  - [5.4 Market accounting state](#54-market-accounting-state)
  - [5.5 Position state](#55-position-state)
  - [5.6 Pending-action state](#56-pending-action-state)
  - [5.7 Entry escrow](#57-entry-escrow)
  - [5.8 Referral state](#58-referral-state)
  - [5.9 LP claim state](#59-lp-claim-state)
  - [5.10 Keeper reward configuration](#510-keeper-reward-configuration)
  - [5.11 Aggregate exposure and risk state](#511-aggregate-exposure-and-risk-state)
  - [5.12 Liability totals](#512-liability-totals)
  - [5.13 Derived values that are not stored](#513-derived-values-that-are-not-stored)
  - [5.14 State cleanup](#514-state-cleanup)
- [6. Core accounting algorithms](#6-core-accounting-algorithms)
  - [6.1 Accrue global borrow](#61-accrue-global-borrow)
  - [6.2 Accrue market funding](#62-accrue-market-funding)
  - [6.3 Calculate pending borrow](#63-calculate-pending-borrow)
  - [6.4 Calculate pending funding](#64-calculate-pending-funding)
  - [6.5 Calculate raw and payable PnL](#65-calculate-raw-and-payable-pnl)
  - [6.6 Calculate effective collateral](#66-calculate-effective-collateral)
  - [6.7 Settle a borrow window](#67-settle-a-borrow-window)
  - [6.8 Calculate the opening fee](#68-calculate-the-opening-fee)
  - [6.9 Calculate the closing fee](#69-calculate-the-closing-fee)
  - [6.10 Apply the settlement waterfall](#610-apply-the-settlement-waterfall)
  - [6.11 Distribute collected revenue](#611-distribute-collected-revenue)
  - [6.12 Pay a keeper reward](#612-pay-a-keeper-reward)
  - [6.13 Update exposure aggregates](#613-update-exposure-aggregates)
  - [6.14 Refresh utilization and the borrow rate](#614-refresh-utilization-and-the-borrow-rate)
  - [6.15 Evaluate liquidation eligibility](#615-evaluate-liquidation-eligibility)
  - [6.16 Evaluate ADL state](#616-evaluate-adl-state)
  - [6.17 Release residual accounting dust](#617-release-residual-accounting-dust)
- [7. User-facing operations](#7-user-facing-operations)
  - [7.0 Common predicates and terminal helpers](#70-common-predicates-and-terminal-helpers)
  - [7.1 Create a market-open order](#71-create-a-market-open-order)
  - [7.2 Settle a market-open order](#72-settle-a-market-open-order)
  - [7.3 Create a limit-open order](#73-create-a-limit-open-order)
  - [7.4 Settle a limit-open order](#74-settle-a-limit-open-order)
  - [7.5 Cancel a limit order](#75-cancel-a-limit-order)
  - [7.6 Clean up an expired order](#76-clean-up-an-expired-order)
  - [7.7 Add collateral](#77-add-collateral)
  - [7.8 Create and settle an increase](#78-create-and-settle-an-increase)
  - [7.9 Create and settle a decrease](#79-create-and-settle-a-decrease)
  - [7.10 Create and settle a voluntary close](#710-create-and-settle-a-voluntary-close)
  - [7.11 Execute take-profit](#711-execute-take-profit)
  - [7.12 Execute stop-loss](#712-execute-stop-loss)
  - [7.13 Liquidate a position](#713-liquidate-a-position)
  - [7.14 Execute automatic deleveraging](#714-execute-automatic-deleveraging)
  - [7.15 Register or change a referrer](#715-register-or-change-a-referrer)
  - [7.16 Claim referral revenue](#716-claim-referral-revenue)
  - [7.17 Deposit and withdraw LP liquidity](#717-deposit-and-withdraw-lp-liquidity)
  - [7.18 Initialize the vault and register a market](#718-initialize-the-vault-and-register-a-market)
- [8. Order lifecycle and failure behavior](#8-order-lifecycle-and-failure-behavior)
  - [8.1 Order states](#81-order-states)
  - [8.2 Market-order lifecycle](#82-market-order-lifecycle)
  - [8.3 Limit-order lifecycle](#83-limit-order-lifecycle)
  - [8.4 Voluntary position-action lifecycle](#84-voluntary-position-action-lifecycle)
  - [8.5 Delay and expiry boundaries](#85-delay-and-expiry-boundaries)
  - [8.6 Fresh-price requirement](#86-fresh-price-requirement)
  - [8.7 Slippage failure](#87-slippage-failure)
  - [8.8 Capacity failure](#88-capacity-failure)
  - [8.9 Expected terminal failure and transaction reversion](#89-expected-terminal-failure-and-transaction-reversion)
  - [8.10 Refund behavior](#810-refund-behavior)
  - [8.11 Keeper payment on terminal attempts](#811-keeper-payment-on-terminal-attempts)
  - [8.12 Liquidation and ADL precedence](#812-liquidation-and-adl-precedence)
  - [8.13 Duplicate and replay prevention](#813-duplicate-and-replay-prevention)
  - [8.14 One action per settlement call](#814-one-action-per-settlement-call)
- [9. Safety and accounting invariants](#9-safety-and-accounting-invariants)
  - [9.1 Cash ownership conservation](#91-cash-ownership-conservation)
  - [9.2 Fee distribution conservation](#92-fee-distribution-conservation)
  - [9.3 Funding conservation](#93-funding-conservation)
  - [9.4 Receiver-funding guarantees](#94-receiver-funding-guarantees)
  - [9.5 Index monotonicity](#95-index-monotonicity)
  - [9.6 No retroactive rate changes](#96-no-retroactive-rate-changes)
  - [9.7 Exposure aggregate correctness](#97-exposure-aggregate-correctness)
  - [9.8 Risk-capacity enforcement](#98-risk-capacity-enforcement)
  - [9.9 Position-health consistency](#99-position-health-consistency)
  - [9.10 Liquidation-reward safety](#910-liquidation-reward-safety)
  - [9.11 Escrow isolation](#911-escrow-isolation)
  - [9.12 No opening fee on failed entry](#912-no-opening-fee-on-failed-entry)
  - [9.13 Closing-fee boundaries](#913-closing-fee-boundaries)
  - [9.14 Single settlement and keeper payment](#914-single-settlement-and-keeper-payment)
  - [9.15 Rounding direction](#915-rounding-direction)
  - [9.16 Atomic reversion](#916-atomic-reversion)
- [10. Configuration reference](#10-configuration-reference)
  - [10.1 Global parameters](#101-global-parameters)
  - [10.2 Per-market parameters](#102-per-market-parameters)
  - [10.3 Parameter validation](#103-parameter-validation)
  - [10.4 Deployment defaults](#104-deployment-defaults)
- [11. End-to-end examples](#11-end-to-end-examples)
  - [11.1 Successful leveraged market open](#111-successful-leveraged-market-open)
  - [11.2 Market open rejected by slippage](#112-market-open-rejected-by-slippage)
  - [11.3 Profitable close below the size-fee threshold](#113-profitable-close-below-the-size-fee-threshold)
  - [11.4 Profitable close where the PnL fee dominates](#114-profitable-close-where-the-pnl-fee-dominates)
  - [11.5 Losing close](#115-losing-close)
  - [11.6 Increase and borrow-window reset](#116-increase-and-borrow-window-reset)
  - [11.7 Partial decrease](#117-partial-decrease)
  - [11.8 Funding payer and receiver](#118-funding-payer-and-receiver)
  - [11.9 Liquidation after a violent price movement](#119-liquidation-after-a-violent-price-movement)
  - [11.10 Automatic deleveraging](#1110-automatic-deleveraging)
  - [11.11 Expired-order cleanup](#1111-expired-order-cleanup)
  - [11.12 Funding window split at a sign change](#1112-funding-window-split-at-a-sign-change)
- [12. Operational contract](#12-operational-contract)
  - [12.1 Collateral and share token requirements](#121-collateral-and-share-token-requirements)
  - [12.2 Pause semantics](#122-pause-semantics)
  - [12.3 Authorization and governance](#123-authorization-and-governance)
  - [12.4 Storage lifetime, upgrade, and migration](#124-storage-lifetime-upgrade-and-migration)
  - [12.5 Error taxonomy](#125-error-taxonomy)
  - [12.6 Emitted results](#126-emitted-results)
  - [12.7 Oracle interface](#127-oracle-interface)
  - [12.8 Stated assumptions and residual risks](#128-stated-assumptions-and-residual-risks)

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
would require a separate round-assignment mechanism. What makes the first
eligible observation the one that settles in practice is keeper competition,
which is an assumption about the world rather than a rule — §12.8.1 states it
and what bounds it when it fails.

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
guarantee of settlement at the trigger price. Both instructions close the
position in full; neither takes a size, and there is no partial take-profit.
Both settle the normal borrow and funding obligations, and either can pay a
closing fee when the close realizes eligible profit.

A trader who wants a partial exit at a price uses a decrease, which is a
committed action rather than a standing instruction.

### 1.9 Liquidation

Liquidation forcibly closes a position whose effective collateral has fallen
to or below its required safety threshold. The threshold is deliberately above
zero so a keeper has time and economic reason to remove the risk before all
position value disappears.

Liquidation is a safety action, not a voluntary trade. It pays a fixed keeper
reward but no closing fee. The position's remaining collateral pays its losses,
accrued obligations, and keeper reward as far as possible. If a violent price
movement jumps past the liquidation buffer, LP equity covers the reward
shortfall as far as that equity reaches; the liquidation completes either
way.

### 1.10 Automatic deleveraging

Automatic deleveraging, or ADL, forcibly closes a profitable position when
positions on one side of a market create too much liability for the vault. It
exists to restore solvency and reduce directional risk when ordinary
liquidation is not the relevant remedy.

ADL closes the selected position in full. It does not reduce a position
partially, so each execution removes one whole position's contribution to the
side's liability, and the side's state is re-evaluated afterwards to decide
whether another is permitted.

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

Each rule is stated once, in section 2 through 8 or section 10. Section 9
states invariants over those rules rather than restating them, and section 11
illustrates them with worked figures. Where an example and a rule disagree,
the rule governs.

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

#### 2.1.1 Integer widths and checked arithmetic

"Widest safe intermediate" is a requirement, not a hint, so the widths are
declared here rather than left to the implementation.

| Kind of value | Stored as |
|---|---|
| Cash, size, base exposure, risk units | `i128` |
| Cumulative indices, rates, decay factors, remainders | `u128` |
| Signed skew and its EMA | `i128` |
| Timestamps, durations, counts, identifiers | `u64` |
| Basis-point configuration | `u32` |

Every product formed on the way to a division is computed in **256-bit**,
regardless of how small its operands look. This is the contract for the four
helpers from §6:

```text
mul_div_floor(a, b, d):
    require d > 0
    p = widen_256(a) * widen_256(b)
    q = p / d                      # truncating, exact in 256-bit
    require q fits the declared result width
    return narrow_128(q)

mul_div_ceil(a, b, d):
    as above with q = (p + d - 1) / d

mul_div_trunc(a, b, d):           # signed operands, truncates toward zero
    require d > 0
    p = widen_256(a) * widen_256(b)
    q = trunc_toward_zero(p / d)
    require q fits the declared result width
    return narrow_128(q)

carried_div(n, d, r):
    require d > 0
    t = widen_256(n) + widen_256(r)
    require t / d and t % d both fit their declared widths
    return (narrow_128(t / d), narrow_128(t % d))
```

`mul_div_trunc` is the helper for signed quantities: the signed skew of §3.4.1,
the blend coefficient `B` of §4.6, and the decayed values derived from them.
Truncation toward zero means the magnitude of a signed skew is never
overstated, in either direction, which floor division would not give for a
negative value.

`carried_div` is the one helper whose numerator is **not** bounded by 128 bits.
§6.2 passes it a funding weight that this section itself puts near `1e40`, so
`n` is a 256-bit value that is already the output of a 256-bit product, and
`widen_256(n)` is a no-op on it rather than a widening. Only `d`, the quotient,
and the remainder are declared at 128 bits. An implementation that types the
numerator as `u128` to match the other helpers overflows on the first funding
checkpoint of a busy market.

Every addition, subtraction, and standalone multiplication outside these
helpers is checked and errors on overflow. An overflow is an unexpected
failure under §8.9: it reverts, it is never a terminal business outcome, and
no clamp or saturating operation may absorb it.

A uniform 256-bit product is not paranoia. Three products in this
specification exceed `u128` at configured parameter limits:

```text
funding weight    max_funding_rate_bps_day * A^2 * elapsed
                  1e4 * (1e14)^2 * 3.2e7  ~= 3.2e39   > 3.4e38
                  formed from an INDEX_PRECISION-carried intermediate
                  (§6.2.1) that reaches ~1.7e45 before its final division

LP share minting  deposit_assets * (share_supply + SHARE_SCALE)
                  1e16 * 1e22             = 1e38      ~ at the limit

minimum borrow    risk_units * current_borrow_rate * min_borrow_fee_seconds
                  1e16 * 2e18 * 900       = 1.8e37    within 20x of the limit
```

The first overflows outright at the validated maximum funding rate. The other
two survive today's defaults only by a margin small enough that a single
parameter change could remove it. Computing every product in 256-bit and
checking the narrowed quotient removes the whole class of question, and the
narrowing check is what turns a would-be silent wrap into a revert.

Two validation bounds exist to keep these quantities bounded at all, and are
part of the arithmetic contract rather than economic policy:

```text
min_borrow_fee_seconds <= 86,400
min_position_lifetime  <= 86,400
```

#### 2.1.2 Fixed-point transcendental primitives

Funding integration needs `2^-x` and locating a funding sign change needs
`log2`. Both are evaluated at `INDEX_PRECISION` and both are fully
deterministic. Nothing else in this specification needs a transcendental: the
borrow curve is a plain square (§6.14).

`exp2_neg(x)` returns `2^-x` for `x >= 0`, with `x` and the result scaled by
`INDEX_PRECISION`. Split `x` into its whole and fractional parts, `x = n + f`:

```text
2^-x = 2^-n * 2^-f
```

`f` is an `INDEX_PRECISION`-scaled integer representing the fraction `f / 1e14`
in `[0, 1)`, and `b_i` is the `i`-th binary digit of that fraction, not a bit
of the integer `f`. The digits come out by repeated doubling:

```text
function exp2_neg(x):
    require x >= 0
    n = x / INDEX_PRECISION
    f = x % INDEX_PRECISION
    if n >= 128:
        return 0

    result = INDEX_PRECISION
    rem    = f
    for i in 1 ..= 48:
        rem = rem * 2
        if rem >= INDEX_PRECISION:
            rem = rem - INDEX_PRECISION
            result = mul_div_floor(result, HALF_POW[i], INDEX_PRECISION)

    return result >> n
```

The shift is applied **last**, after the fractional product, so the
multiplications keep full precision and only one truncation is taken at the
end. Shifting first would discard `n` bits of the mantissa before any of them
were used.

`HALF_POW[i]` is the `INDEX_PRECISION`-scaled constant `2^(-2^-i)`, truncated,
a fixed table of 48 entries stored in the contract. The loop is bounded,
branch-free in cost, and uses no division other than the helper's. The table is
the one in §2.1.3.

`log2(y)` for `y >= INDEX_PRECISION` returns its base-2 logarithm at
`INDEX_PRECISION` by the mirror construction — integer part from the bit
length, then one fraction bit at a time by repeated squaring of the normalized
mantissa, again for 48 iterations:

```text
function log2(y):
    require y >= INDEX_PRECISION

    n = bit_length(y / INDEX_PRECISION) - 1
    m = y >> n                       # normalized to [1, 2) at INDEX_PRECISION
    result = n * INDEX_PRECISION

    for i in 1 ..= 48:
        m = mul_div_floor(m, m, INDEX_PRECISION)
        if m >= 2 * INDEX_PRECISION:
            m = m / 2
            result = result + (INDEX_PRECISION >> i)

    return result
```

`log2` exists here for one caller: locating the funding sign change `t_star`
in §6.2.1. An earlier revision also raised utilization to a configurable power
for the borrow curve, which needed a general `pow` built from both primitives.
That exponent is now fixed at two and the curve is a single multiplication
(§6.14), so `pow` is gone and `log2` is called once per funding window rather
than on every action that moves the borrow rate.

One further constant is required, by the window integral in §4.6:

```text
LN2 = 69,314,718,055,994          # ln(2) * INDEX_PRECISION, truncated
```

The declared tolerance, which §4.6 refers to, is:

```text
DECAY_TOLERANCE = 1e-12 relative
```

Each of the at most 48 multiplications truncates by less than one unit in
`1e14`, so the accumulated relative error is below `48 / 1e14`, comfortably
inside the declared bound. Every rounding in these primitives truncates, so
the error is one-directional and a decay factor is never overstated.

Three properties follow, and an implementation must test them directly:

1. `exp2_neg` is monotonically non-increasing in `x`, and `exp2_neg(0)`
   is exactly `INDEX_PRECISION`.
2. Splitting an otherwise unchanged funding window into any number of
   checkpoints reproduces the single-window result within
   `checkpoint_count * DECAY_TOLERANCE` relative error. This is the concrete
   form of invariant 10 in §4.14.
3. The same inputs produce bit-identical outputs on every node. No step
   consults a platform float, a transcendental library, or a wall-clock value.

The funding sign change in §4.6 is located analytically rather than by search.
`I(t) = A + B * d(t)` is monotonic across the window, so a crossing exists
exactly when the two endpoints of that window disagree in sign:

```text
d_end    = exp2_neg(mul_div_floor(elapsed, INDEX_PRECISION, H))
crossing = sign(A + B) != sign(A + mul_div_trunc(B, d_end, INDEX_PRECISION))
```

The endpoint at `elapsed` is what makes this test correct. Comparing `A + B`
against `A` alone tests for a crossing on `[0, infinity)`, because `A` is the
limit of `I(t)` rather than its value at the end of the window. A window whose
EMA is still far from live skew satisfies that weaker test while its crossing
lies hours beyond `elapsed`, and the window would then be split at a point
outside itself.

When the endpoint test passes:

```text
d_star = -A / B                       in (d_end, 1)
t_star = mul_div_floor(H, log2(mul_div_floor(INDEX_PRECISION,
                                             INDEX_PRECISION, d_star)),
                       INDEX_PRECISION)
```

`d_star > d_end` places `t_star` strictly inside `(0, elapsed)`.

`t_star` is used at this precision and is not rounded to a whole second before
the two subintervals are integrated. Its residual error is second order: the
integrand `I(t)^2` vanishes at the crossing, so a small error in `t_star`
perturbs each subinterval's contribution by an amount quadratic in that error.

#### 2.1.3 Constants and conformance vectors

`HALF_POW` is reference data, not a derivation an implementation may redo at a
different precision. Every entry is `floor(INDEX_PRECISION * 2^(-2^-i))`:

```text
HALF_POW[ 1] = 70710678118654
HALF_POW[ 2] = 84089641525371
HALF_POW[ 3] = 91700404320467
HALF_POW[ 4] = 95760328069857
HALF_POW[ 5] = 97857206208770
HALF_POW[ 6] = 98922801319397
HALF_POW[ 7] = 99459942348363
HALF_POW[ 8] = 99729605608547
HALF_POW[ 9] = 99864711289097
HALF_POW[10] = 99932332750265
HALF_POW[11] = 99966160649624
HALF_POW[12] = 99983078893192
HALF_POW[13] = 99991539088661
HALF_POW[14] = 99995769454843
HALF_POW[15] = 99997884705049
HALF_POW[16] = 99998942346931
HALF_POW[17] = 99999471172067
HALF_POW[18] = 99999735585684
HALF_POW[19] = 99999867792754
HALF_POW[20] = 99999933896355
HALF_POW[21] = 99999966948172
HALF_POW[22] = 99999983474084
HALF_POW[23] = 99999991737042
HALF_POW[24] = 99999995868520
HALF_POW[25] = 99999997934260
HALF_POW[26] = 99999998967130
HALF_POW[27] = 99999999483565
HALF_POW[28] = 99999999741782
HALF_POW[29] = 99999999870891
HALF_POW[30] = 99999999935445
HALF_POW[31] = 99999999967722
HALF_POW[32] = 99999999983861
HALF_POW[33] = 99999999991930
HALF_POW[34] = 99999999995965
HALF_POW[35] = 99999999997982
HALF_POW[36] = 99999999998991
HALF_POW[37] = 99999999999495
HALF_POW[38] = 99999999999747
HALF_POW[39] = 99999999999873
HALF_POW[40] = 99999999999936
HALF_POW[41] = 99999999999968
HALF_POW[42] = 99999999999984
HALF_POW[43] = 99999999999992
HALF_POW[44] = 99999999999996
HALF_POW[45] = 99999999999998
HALF_POW[46] = 99999999999999
HALF_POW[47] = 99999999999999
HALF_POW[48] = 99999999999999
```

Entries 46 through 48 are identical because `ln(2) / 2^i` drops below one unit
in `1e14` at `i = 47`. They are kept so the loop bound matches the 46.5 bits of
fraction that `INDEX_PRECISION` can represent, and so the table is indexable
without a special case.

The vectors below are the output of the algorithms exactly as specified in
§2.1.2, not the mathematically exact values. Where they differ, the truncating
algorithm is normative and an implementation must reproduce these integers
bit for bit.

`exp2_neg(x)`:

| `x` as a decimal | `x` | `exp2_neg(x)` |
|---|---|---|
| `0` | `0` | `100000000000000` |
| `0.5` | `50000000000000` | `70710678118654` |
| `1` | `100000000000000` | `50000000000000` |
| `2` | `200000000000000` | `25000000000000` |
| `3.25` | `325000000000000` | `10511205190671` |
| `10` | `1000000000000000` | `97656250000` |
| `127` | `12700000000000000` | `0` |

`exp2_neg(0.5 * IP)` and `exp2_neg(3.25 * IP)` happen to agree with the exact
values to all 14 places. `log2` does not, and that is expected:

`log2(y)`:

| `y` as a decimal | `y` | `log2(y)` |
|---|---|---|
| `1` | `100000000000000` | `0` |
| `2` | `200000000000000` | `100000000000000` |
| `3` | `300000000000000` | `158496250072105` |
| `4` | `400000000000000` | `200000000000000` |
| `4.2` | `420000000000000` | `207038932789131` |
| `10` | `1000000000000000` | `332192809488729` |

`log2(3)` returns `158496250072105` against an exact `158496250072115`, low by
ten units in `1.6e14`, or `6e-14` relative — inside `DECAY_TOLERANCE` and in
the safe direction.

The borrow curve of §6.14, at the initial rate parameters:

| Utilization (bps) | `u` | `u^2` | `current_borrow_rate` |
|---|---|---|---|
| `0` | `0` | `0` | `2500000000000000` |
| `2500` | `25000000000000` | `6250000000000` | `4062500000000000` |
| `5000` | `50000000000000` | `25000000000000` | `8750000000000000` |
| `7500` | `75000000000000` | `56250000000000` | `16562500000000000` |
| `8500` | `85000000000000` | `72250000000000` | `20562500000000000` |
| `10000` | `100000000000000` | `100000000000000` | `27500000000000000` |

The rate column is `base_borrow_rate_bps_day * INDEX_PRECISION +
max_variable_borrow_bps_day * mul_div_floor(u, u, INDEX_PRECISION)`, so it
reads as bps per day at `INDEX_PRECISION`: `25` at zero utilization, `87.5` at
half, `205.625` at the `8,500` bps capacity limit, and `275` at full. The
`87.5` figure is the one §3.10 Example F, §11.4, and §11.8 use, and this table
is where it comes from.

Every entry is exact. `u` is a utilization in basis points scaled to
`INDEX_PRECISION`, so it is a multiple of `1e10` and its square is a multiple
of `1e6` — the division by `INDEX_PRECISION` has no remainder at any reachable
utilization. A test may assert these values exactly.

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
be recognized when valuing LP shares, and it is recognized in one direction
only:

```text
recognized_side_pnl = max(raw_side_pnl, 0)

marked_vault_nav = max(
    cash_lp_equity - sum(recognized_side_pnl),
    0
)
```

Positive trader PnL reduces LP value because it is a liability the vault will
have to pay. Unrealized trader *loss* is not recognized at all. It raises LP
value only when it is actually collected, at which point it is already in
`cash_lp_equity` and needs no separate recognition. Uncollected future borrow
and funding are excluded for the same reason.

This is deliberately conservative, and the alternative was unsound. Capping an
aggregate side loss by that side's aggregate stored collateral — recognizing
`-min(abs(raw_side_pnl), side_stored_collateral)` — treats every position's
collateral as available to cover every other position's loss. A side holding
one healthy position with `$100` of collateral and one failing position with
`$10` of collateral against a `$500` loss would recognize `$110` of collectible
loss when only `$10` can ever be collected, because the healthy trader's
collateral is theirs and will be returned to them.

Fixing that per position is not possible within the cost model of §4.1: the
collectible amount is `min(loss_i, collateral_i)` per position, which moves
continuously with price and cannot be a stored aggregate. Recognizing nothing
is the only bounded rule that cannot overstate.

Two consequences follow. LP share price understates while traders are
collectively losing, and steps up as those losses are realized rather than
accruing smoothly, so a depositor during a trader drawdown gets slightly more
shares than a mark-to-market valuation would give and a withdrawer slightly
less. That asymmetry is the point: an LP cannot withdraw against unrealized
trader losses that may never be collected, so there is no advantage in leaving
while traders are underwater.

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
the position. For a positive raw PnL, the market side's stored payout factor
is applied:

```text
if raw_pnl <= 0:
    payable_pnl = raw_pnl
else:
    payable_pnl = floor(
        raw_pnl * side_hard_cap_payout_factor / INDEX_PRECISION
    )
```

The stored factor is one whole unless the side is latched in `HardCap`, in
which case it is the value snapshotted when the side entered that state
(§6.5). It is uniform across every profitable position on the side, so no
position's recognized profit depends on when it settles relative to another's.

Recognition is separate from payment. What the vault can actually pay is
additionally limited by cash on hand at the moment of payment:

```text
paid_pnl = min(payable_pnl, cash_lp_equity)
```

That limit belongs to the payment step alone. It is not part of payable PnL,
and therefore not part of effective collateral, margin, or liquidation
eligibility — a position's health does not depend on how much cash the vault
happens to hold.

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

`payable_pnl` here is the recognized amount of §2.8 — raw PnL after the side's
stored hard-cap factor — and never the payment-time cash limit. Health is a
property of the position and its market, not of the vault's cash balance at
this instant.

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

This is a conservation rule, not a direction rule, and the table above does
not apply to it. Removed and remaining base always sum to exactly the
pre-reduction base, so the sub-unit is moved between the portion realized now
and the portion still open, never created. Repeated decreases cannot
accumulate an advantage, because the total is conserved at every step and a
final close removes the remainder exactly.

What protects the vault is the split itself: since `floor(x) + floor(y) <=
floor(x + y)`, valuing two portions separately can only produce less trader
value than valuing the whole position once.

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
entry_deductions = opening_fee + applicable entry keeper reward

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

The base is price PnL after any hard-cap payout factor. It is not reduced
again by the payment-time cash limit, which applies to the transfer rather
than to the amount recognized.

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

borrow_rate_bps_day =
      base_borrow_rate_bps_day
    + max_variable_borrow_bps_day * u^2
```

Initial global defaults are:

```text
base_borrow_rate_bps_day    = 25
max_variable_borrow_bps_day = 250
```

The square is fixed, not configured. A configurable exponent would be a
governance dial over the curve's shape, and its cost is paid on every single
action: the rate is refreshed after every mutation that moves risk units or LP
equity (§6.14), and a general power needs roughly a hundred rounds of
fixed-point iteration where a square needs one multiplication. The shape a
square gives — cheap while utilization is low, steep as it approaches the
capacity limit — is the shape this curve is for, and the two rate parameters
remain configurable for its height.

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

Three payments are capped at what their source holds and complete anyway: the
failure reward on a position action (§7.0), `keeper_lp_resolve_reward` on a
request worth less than the reward, and `keeper_liquidation_reward` (§9.10).
Each pays for an action that must not be blocked by its own cost.

Every other reward is required in full, and its action reverts if the position
cannot fund it: a voluntary settlement that cannot pay for itself should not
complete. Of the three capped payments, only the liquidation reward may draw
on LP equity, and only as far as that equity reaches.

#### 3.5.1 Open execution

A successful market-open execution pays `keeper_open_reward` from entry-order
escrow. The opening fee and reward are both removed before the remainder
becomes position collateral.

An eligible market-open attempt that terminates because of an expected failure
still pays this reward from escrow, refunds the remainder, and charges no
opening fee. An ineligible call or reverted transaction pays nothing.

#### 3.5.2 Increase execution

A successful size increase pays `keeper_increase_reward` from stored position
collateral. The reward is included in the post-action health check and is
separate from the opening fee on added size.

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

A price gap can jump past the reserved value. The position's remaining
collateral pays what it can and LP residual equity covers the rest, so in
every ordinary case the keeper receives the full fixed reward. When LP equity
is itself exhausted the payment is capped at what exists and the liquidation
still completes; removing the risk always outranks paying for the removal.
Liquidation pays no closing fee.

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
- a liquidation reward after a price gap, to the extent LP equity exists; and
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
it under a different divisor changes the value it represents.

Three of the four funding remainders divide by a constant — both payer-index
divisions by `INDEX_PRECISION * BPS * SECONDS_PER_DAY`, the
guaranteed-liability division by `INDEX_PRECISION` — so their carries may
persist for the lifetime of the payer stream.

The receiver-distribution division is the exception: it divides by
`receiver.size_open_interest`, which changes whenever a position on the
receiving side is opened, increased, decreased, or removed. A remainder
produced modulo a large receiver size is a small fraction of a large base;
carried into a division by a smaller receiver size it becomes a much larger
fraction of a smaller base, and receivers can be credited more than the
receiver-backed accrual that justifies it.

The rule is therefore:

```text
when a market side's size_open_interest changes, reset the
receiver_distribution_remainder of the opposite side's payer stream to zero
```

The reset runs after the checkpoint and before the exposure mutation, so every
carry lives inside a window of constant receiver size and no accrual is lost
by the reset itself (§4.9).

The discarded fraction is less than one whole cash unit of receiver credit and
is never re-credited, so the reset can only under-distribute. Its
guaranteed-liability counterpart stays in `market.pending_receiver_funding`
and is released to LPs by the empty-book rule in §4.13. With a constant
divisor the cumulative identity is exact, which is what §9.4's guarantee
rests on.

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

A window is integrated one **segment** at a time. An unsplit window is a single
segment spanning the whole of it; a window containing a sign change is two. A
segment runs from `t1` to `t2` with decay factors `d1 = d(t1)` and `d2 = d(t2)`,
and the general forms are:

```text
J1 = H / ln(2)       * (d1 - d2)
J2 = H / (2 * ln(2)) * (d1^2 - d2^2)

quadratic_integral = A^2 * (t2 - t1) + 2*A*B*J1 + B^2*J2

funding_weight = max_funding_rate_bps_day * quadratic_integral

linear_integral = A * (t2 - t1) + B * J1
```

For an unsplit window, `t1 = 0`, `d1 = 1`, and these reduce to the familiar
`(1 - d)` and `(1 - d^2)`. Note that `d1` and `d2` are always measured from the
**window** origin, never from the segment's own start, so the second segment of
a split window carries the decay already accumulated before its crossing.

`J1` and `J2` are durations, but they are carried at `INDEX_PRECISION` rather
than as whole seconds, and §6.2.1 performs the single compensating division at
the end. Flooring them to whole seconds instead would be the dominant error in
the whole calculation: a typical `J1` is on the order of `1e4` seconds, so one
discarded second is `1e-5` relative — seven orders of magnitude outside
`DECAY_TOLERANCE`, and far larger than anything the 48-iteration primitives
contribute.

Two properties of these forms are worth stating because they are directly
testable. The quadratic integral is an integral of a square and is therefore
never negative, so a negative result can only be truncation residue and is
clamped at zero rather than reverting — this is not the forbidden kind of
clamp in §2.11, which concerns a pending amount derived from a monotonic
index. And segment integrals are additive: splitting a window at any interior
point and summing the two quadratic integrals reproduces the one-segment result
for the same window. §11.12 works an example where the two agree exactly.

The scale of each quantity here is load-bearing and is fixed as follows.
`A` and `B` are skew fractions at `INDEX_PRECISION`. `d`, being a decay factor
in `[0, 1]`, is also at `INDEX_PRECISION`. `J1` and `J2` are in seconds. The
products `A^2`, `A*B` and `B^2` therefore carry `INDEX_PRECISION^2`, and so:

```text
quadratic_integral   is INDEX_PRECISION^2 * seconds
funding_weight       is INDEX_PRECISION^2 * bps * seconds
```

`funding_weight` is **not** `INDEX_PRECISION`-scaled. Converting it to an
index delta — cumulative fee per unit of size at `INDEX_PRECISION` — divides
by `INDEX_PRECISION * BPS * SECONDS_PER_DAY`: one `INDEX_PRECISION` to undo
the squared skew, `BPS` to turn basis points into a fraction, and
`SECONDS_PER_DAY` to turn a daily rate into a per-second one. §6.2 performs
exactly that division.

`LN2` is the `INDEX_PRECISION`-scaled natural logarithm of two, a stored
constant:

```text
LN2 = 69,314,718,055,994          # ln(2) * INDEX_PRECISION, truncated
```

`linear_integral` is useful for locating and validating a zero crossing, but
its sign does not select one payer for an interval containing both signs.

For an interval on which `I(t)` keeps one sign, that sign selects the payer and
the non-negative quadratic integral determines the amount. Because `I(t)` is
monotonic while live skew is constant, it has at most one zero crossing in a
checkpoint interval. A crossing exists exactly when `I` disagrees in sign at
the two ends of that interval, `A + B` and `A + B * d`; §2.1.2 states the test
and gives the closed form for `t_star`, which the test places strictly inside
the window. Integrate the two subintervals independently and carry each result
to the corresponding payer stream. The crossing uses the same declared decay quantization as the EMA
calculation; it is not rounded to an arbitrary whole-second boundary before
integration.

The decay function is evaluated deterministically in fixed point by
`exp2_neg`, and the crossing is located analytically by `log2`; both
primitives, their bounded iteration counts, and `DECAY_TOLERANCE` are
specified in §2.1.2. Splitting an unchanged interval into multiple checkpoints
must reproduce the one-window result within that tolerance, multiplied by the
number of checkpoints. When the instant weight is 100%, the EMA contributes
nothing and the result is exactly checkpoint-frequency independent apart from
carried integer division.

### 4.7 Global checkpoint

A global checkpoint advances the borrow index from `last_global_checkpoint` to
a given timestamp, using `current_borrow_rate` for the whole elapsed interval
and carrying the division remainder. `accrue_global_borrow` in §6.1 is the
operation.

It never derives a new rate. Past time is accounted for with the rate already
stored for that interval; refreshing the rate is a separate final step after
every mutation that can change utilization (§6.14). Reversing that order would
reprice history.

Calling it twice at the same timestamp does nothing the second time, and the
clock does not stop for a pause: the next checkpoint includes all elapsed
wall-clock time while positions remained open.

### 4.8 Market checkpoint

A market checkpoint advances one market's funding state to a given timestamp.
`accrue_market_funding` in §6.2 is the operation, and it does four things in
order.

First it integrates the window. The blended skew `I(t)` moves continuously as
the EMA decays, so the interval is integrated in closed form rather than
sampled (§4.6), and it is split at a sign change so each subinterval accrues
to the side that was actually paying during it. This yields one or two
segments, each with a payer side and a non-negative funding weight.

Second it divides each segment's weight between receivers and LPs, in
proportion to how much opposing base exposure exists. A side with no
offsetting exposure sends its whole flow to LPs; a side fully offset sends all
of it to receivers.

Third it advances the indices. The two payer-side indices take their weights
through carried divisions, and both the guaranteed receiver liability and the
receiver credit index derive from the same scaled backing so neither can
exceed the other's justification. Recognizing that liability changes non-LP
claims and therefore cash LP equity, which is why the global borrow rate is
refreshed after the enclosing action rather than inside the checkpoint.

Fourth it advances the EMA, the display fields, and the timestamp. These
advance even when no funding accrued — when no payer side has exposure — so
historical skew keeps decaying correctly.

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
| `max_order_lifetime_seconds` | Seconds | Upper bound on a limit entry's `expires_at`; initial value `604,800` |
| `max_market_order_lifetime_seconds` | Seconds | Upper bound on a market entry's `expires_at`; initial value `300` |
| `min_borrow_fee_seconds` | Seconds | Duration used to quote each monetary minimum-borrow obligation; initial value `900` |
| `funding_half_life_seconds` | Seconds | Shared half-life of every market's funding EMA; initial value `43,200` and never zero |
| `risk_capacity_limit_bps` | Bps | Maximum share of cash LP equity assignable to risk units; initial value `8,500` |
| `base_borrow_rate_bps_day` | Bps/day | Borrow rate when utilization is zero; initial value `25` |
| `max_variable_borrow_bps_day` | Bps/day | Maximum utilization-dependent addition; initial value `250` |
| `fee_lp_revenue_share_bps` | Bps | LP share of collected opening and closing fees; initial value `9,000` |
| `borrow_lp_revenue_share_bps` | Bps | LP share of collected borrow; initial value `9,000` |
| `referral_fee_share_bps` | Bps | Referral share of collected opening and closing fees; initial value `250` and carved from protocol revenue |
| `config_timelock_seconds` | Seconds | Delay between proposing and applying a parameter change; initial value `172,800` |
| `max_active_markets` | Count | Hard bound on the active-market registry and any synchronized LP-accounting loop |
| `global_hard_cap_factor_limit_bps` | Bps | Bound on aggregate configured hard-cap exposure across market sides |
| `hard_cap_relatch_band_bps` | Bps | Growth in a latched side's positive PnL that triggers a fresh hard-cap snapshot; initial value `2,500` |

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

What global configuration deliberately does not contain is equally important.
There is no minimum borrow index delta, user-selected execution budget, keeper
revenue share, keeper reserve, percentage liquidation reward, percentage ADL
reward, maximum ADL reward, and no insolvency-touch reward.

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
pause_authority
unpause_authority
oracle_authority
protocol_recipient
vault_asset
state_version
```

The authorities are distinct roles with distinct powers (§12.3), `vault_asset`
must satisfy the token requirements in §12.1, and `state_version` is the
migration guard defined in §12.4.

The monotonically increasing ID counters prevent identifier reuse. The active
market registry contains each market with live accounting state exactly once
and never exceeds `max_active_markets`. Pausing blocks every path that adds
exposure and none that removes it, and does not alter either accrual
timestamp; §12.2 gives the complete rule.

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
| `hard_cap_payout_factor` | Index | `INDEX_PRECISION` unless the side is latched in `HardCap`; snapshotted on entry and on each band re-latch (§6.5, §6.16) |
| `hard_cap_reference_pnl` | Cash | Side positive PnL the current payout factor was measured against; `0` unless latched in `HardCap` |

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
total share supply until they are burned on success. A failed deposit returns its
collateral escrow less the resolve reward; a failed withdrawal returns its
escrowed shares in full and pays no reward (§7.17).

There is no persistent pending-withdrawal cash claim and no partial LP fill. A
request settles fully or refunds fully, so LP request escrow is not included in
the vault's non-LP cash claims.

That exclusion holds only because the escrow sits outside the vault's token
balance, with the request contract, so it never appears in `physical_cash` and
needs no claim label to offset it. Held inside the vault it would be
indistinguishable from LP residual equity, and a pending deposit would raise
cash LP equity, lower utilization, and enlarge risk capacity before its owner
had bought a single share. Withdrawal escrow is the mirror case: shares, not
cash, remaining in total supply until burned on success.

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
| `keeper_liquidation_reward` | Position value, then LP residual, capped at what exists |
| `keeper_adl_reward` | Position payable profit or collateral |
| `keeper_lp_resolve_reward` | LP request escrow or released assets |

Each field is a fixed cash amount with an initial value of `2,500,000`, or
`$0.25`. The fields are independent even though their initial values match.

The configuration stores no generic execution reward and no user override.
Every settlement kind maps to exactly one field. Validation requires:

```text
every keeper reward <= min_collateral
min_collateral > keeper_liquidation_reward
```

Entry-order creation must also guarantee that its actual escrow can pay the
applicable open, limit, or expiry reward.

Only three payments are capped at what their source holds: the failure reward
on a position action (§7.0), `keeper_lp_resolve_reward`, and
`keeper_liquidation_reward`. Every other payment through
`pay_keeper_from_position` requires its full amount and reverts if the
position cannot fund it — a voluntary settlement that cannot pay for itself
should not complete.

That makes the blanket bound load-bearing rather than cosmetic. Without it,
raising `keeper_close_reward` above `keeper_liquidation_reward` would create
positions that are neither liquidatable, because their effective collateral is
above the liquidation threshold, nor closeable, because
`pay_keeper_from_position` reverts. Bounding every reward by `min_collateral`,
which is itself above `keeper_liquidation_reward`, keeps that gap empty.

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
| Raw and payable PnL | Position exposure, price, and the side's stored payout factor |
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
the state must remain restorable for as long as the economic obligation
exists. §12.4 assigns each of these a storage class and requires every
persistent entry to be extendable permissionlessly.

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

    window = integrate_funding_window_by_sign(
        market.long.base_exposure,
        market.short.base_exposure,
        market.skew_ema,
        market.config.instant_weight_bps,
        global_config.funding_half_life_seconds,
        market.config.max_funding_rate_bps_day,
        elapsed
    )
```

Process each segment of `window.segments` in the order returned, which is
chronological (§6.2.1). Select its payer and
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
    INDEX_PRECISION * BPS * SECONDS_PER_DAY,
    payer_stream.receiver_payer_remainder
)

(lp_payer_delta, payer_stream.lp_payer_remainder) = carried_div(
    lp_weight,
    INDEX_PRECISION * BPS * SECONDS_PER_DAY,
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
market.skew_ema = window.ema_after
market.current_payer_side = window.payer_side_at_window_end
market.current_payer_rate = window.displayed_rate_at_window_end
market.last_funding_checkpoint = now
```

The liability credit changes cash LP equity but not physical cash. The enclosing
action refreshes global borrow only after all such claim changes are complete.

#### 6.2.1 Integrate a funding window by sign

`accrue_market_funding` delegates the whole of the window mathematics to this
function. It is pure: it reads no storage, writes none, and its result is a
function of its arguments alone.

```text
function integrate_funding_window_by_sign(
    long_base,
    short_base,
    skew_ema,
    instant_weight_bps,
    half_life_seconds,
    max_funding_rate_bps_day,
    elapsed
):
    require elapsed > 0
    require half_life_seconds > 0

    # --- live skew, constant for the whole window (§3.4.1) ---
    total_base = long_base + short_base
    if total_base == 0:
        S = 0
    else:
        S = mul_div_trunc(long_base - short_base, INDEX_PRECISION, total_base)

    # --- blend coefficients (§4.6) ---
    w = mul_div_floor(instant_weight_bps, INDEX_PRECISION, BPS)
    A = S
    B = mul_div_trunc(INDEX_PRECISION - w, skew_ema - S, INDEX_PRECISION)

    # --- decay at the window end ---
    d_end = exp2_neg(
        mul_div_floor(elapsed, INDEX_PRECISION, half_life_seconds)
    )

    ema_after = S + mul_div_trunc(skew_ema - S, d_end, INDEX_PRECISION)

    # --- endpoint values of the blended skew ---
    I_start = A + B
    I_end   = A + mul_div_trunc(B, d_end, INDEX_PRECISION)

    # --- split the window at a sign change, if there is one (§2.1.2) ---
    if I_start != 0 and I_end != 0 and sign(I_start) != sign(I_end):
        d_star = mul_div_trunc(-A, INDEX_PRECISION, B)
        require d_end < d_star < INDEX_PRECISION

        t_star = mul_div_floor(
            half_life_seconds,
            log2(mul_div_floor(INDEX_PRECISION, INDEX_PRECISION, d_star)),
            INDEX_PRECISION
        )
        require 0 < t_star < elapsed

        spans = [
            (0,      INDEX_PRECISION, t_star,  d_star, I_start),
            (t_star, d_star,          elapsed, d_end,  I_end)
        ]
    else:
        spans = [
            (0, INDEX_PRECISION, elapsed, d_end, I_start if I_start != 0
                                                 else I_end)
        ]

    # --- integrate each span (§4.6) ---
    segments = []
    for (t1, d1, t2, d2, I_sign_source) in spans:
        if t2 <= t1:
            continue

        j1 = mul_div_floor(
            half_life_seconds * (d1 - d2), INDEX_PRECISION, LN2
        )
        j2 = mul_div_floor(
            half_life_seconds * (
                  mul_div_floor(d1, d1, INDEX_PRECISION)
                - mul_div_floor(d2, d2, INDEX_PRECISION)
            ),
            INDEX_PRECISION,
            2 * LN2
        )

        # INDEX_PRECISION^3 * seconds; every product formed in 256-bit
        quadratic_scaled =
              A * A * (t2 - t1) * INDEX_PRECISION
            + 2 * A * B * j1
            + B * B * j2

        # one compensating division returns the declared
        # INDEX_PRECISION^2 * seconds of §4.6
        quadratic_integral = max(0, quadratic_scaled / INDEX_PRECISION)

        if I_sign_source > 0:
            payer = Long
        else if I_sign_source < 0:
            payer = Short
        else:
            continue                  # no funding accrues on a zero segment

        segments.append(Segment {
            payer_side:     payer,
            funding_weight: max_funding_rate_bps_day * quadratic_integral
        })

    return FundingWindow {
        segments,
        ema_after,
        payer_side_at_window_end:
            Long if I_end > 0 else Short if I_end < 0 else None,
        displayed_rate_at_window_end:
            mul_div_floor(
                max_funding_rate_bps_day * abs(I_end),
                abs(I_end),
                INDEX_PRECISION
            )
    }
```

Four things about this function are load-bearing.

The segments come back in **chronological order**, which is what §6.2 means by
processing them in that order. Order does not change the arithmetic, but it
does change which carried remainder each division sees, and a window whose
crossing is replayed out of order will not reproduce the same indices.

`d1` and `d2` are decay factors measured from the window origin, so the second
segment starts at `d_star` rather than at `INDEX_PRECISION`. Restarting the
decay at each segment would integrate the second one as though the EMA were
fresh and would roughly double the funding attributed to it.

`require d_end < d_star < INDEX_PRECISION` and `require 0 < t_star < elapsed`
are the guards that the endpoint test of §2.1.2 is supposed to make
unreachable. They are cheap, and an implementation that gets the sign test
wrong fails loudly at the checkpoint rather than silently integrating a window
at 150 times its true length.

The clamp on `quadratic_integral` is bounded truncation residue on a quantity
that is mathematically non-negative, which is why it is a clamp and not a
revert. It is not the negative-pending-amount case that §2.11 and §9.5 forbid
hiding: nothing here is derived by subtracting a stored baseline from a
monotonic index.

`displayed_rate_at_window_end` is a display field only (§5.4). It is the
quadratic rate at the window's final blended skew, and no obligation is ever
read from it.

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

Payable PnL applies the side's stored payout factor to positive raw PnL, and
nothing else. The payment-time cash limit is applied later, by the code that
moves the money:

```text
function calculate_payable_pnl(position_raw_pnl, market_side):
    if position_raw_pnl <= 0:
        return position_raw_pnl

    if market_side.risk_state != HardCap:
        return position_raw_pnl

    return mul_div_floor(
        position_raw_pnl,
        market_side.hard_cap_payout_factor,
        INDEX_PRECISION
    )
```

The factor is read, not recomputed. It is snapshotted once when the side
enters `HardCap` and stays fixed for as long as the side remains there:

```text
function snapshot_hard_cap_factor(side, market_config, price, cash_lp_equity):
    side_positive_pnl = max(
        calculate_raw_pnl(
            side.direction,
            side.size_open_interest,
            side.base_exposure,
            price
        ),
        0
    )

    hard_cap_value = mul_div_floor(
        cash_lp_equity,
        market_config.hard_cap_pnl_factor_bps,
        BPS
    )

    side.hard_cap_reference_pnl = side_positive_pnl

    if side_positive_pnl <= hard_cap_value:
        side.hard_cap_payout_factor = INDEX_PRECISION
    else:
        side.hard_cap_payout_factor = mul_div_floor(
            hard_cap_value,
            INDEX_PRECISION,
            side_positive_pnl
        )
```

The same function serves both the initial latch and every later re-latch. It
reads the book and LP equity as they are at the moment it runs and records the
denominator it used, which is what the band in §6.16 is measured against.

`snapshot_hard_cap_factor` runs exactly on the transition into `HardCap`, as
part of applying the risk-state change in §6.16. Leaving `HardCap` clears the
factor back to `INDEX_PRECISION`. A side that re-enters later takes a fresh
snapshot from the book at that moment.

Nothing evaluates a side's risk state on a price move alone, so the stored
state is only as current as the last action that touched the market. Every
operation that reads a payout factor must therefore refresh that state from its
own price snapshot **before** it calculates payable PnL, never after. This is a
rule, not a property of any one operation: §7.2 and §7.9 through §7.12 refresh
it immediately after their checkpoints, §7.13 from its own liquidation
snapshot, and §7.14 by applying the side transition before it values the
position. Refreshing afterwards would let the first position
to exit a side that has crossed the threshold settle at a factor of one whole
and latch the side only on its way out, leaving every position behind it scaled
— a first-mover advantage on exactly the run the state exists to stop. The
mirror case is a side that has economically recovered but is still stored as
`HardCap`, whose first exit is scaled when it should not be.

The factor must not be recomputed at settlement. Each settlement pays out and
removes exposure, so both `cash_lp_equity` and the side's aggregate positive
PnL move; a factor derived again afterwards measures the next position against
a shrunken denominator. Payouts would become order-dependent, and their sum
would bear no relation to `hard_cap_value`. With one snapshot, every position
on the side is scaled by the same number and settlement order cannot change any
individual outcome.

A snapshot held forever would not bound the total paid. It bounds it at the
moment of latching, and if the side stays latched while its raw aggregate PnL
keeps growing, payable PnL grows with it at the frozen ratio — a side latching
at `$80,000` of profit against a `$60,000` cap carries `0.75`, and pays
`$150,000` if its profit runs to `$200,000`, never re-snapshotting because its
PnL factor never falls back below the threshold.

The band in §6.16 is what closes that. The side records the denominator it was
last measured against, and once current positive PnL exceeds it by
`hard_cap_relatch_band_bps` the side takes a fresh snapshot from the book and
LP equity as they then stand. Between re-latches nothing changes: every
position on the side is scaled by one number and settlement order cannot affect
any individual outcome. The running exposure becomes bounded at
`(BPS + hard_cap_relatch_band_bps) / BPS` of the cap value measured at the most
recent latch, which at the initial `2,500` is `1.25x`.

The band is self-correcting in the direction that matters. Each re-latch reads
the current `cash_lp_equity`, which by then is lower for every payout already
made, so the new cap value is smaller and the new factor is lower than the one
it replaces. A side whose profit keeps running is scaled harder each time it
crosses a band boundary, rather than being scaled once and then paying an
ever-larger multiple of a cap nobody re-measured.

`2,500` is chosen so the band does not churn. A 25% move in a side's aggregate
profit while it is already in `HardCap` is a substantial event, not tick noise,
so ordinary price movement does not re-latch and the order-independence
property holds across the settlements that actually occur. Narrowing the band
tightens the bound and costs fairness between traders separated by a re-latch;
widening it does the reverse. The payment-time clamp below remains the final
backstop underneath either.

Two properties of the snapshot are deliberate. It cannot exceed
`INDEX_PRECISION`, so a side entering `HardCap` while its aggregate profit is
still below the cap simply pays in full until that changes. And it is not
revised downward if LP equity falls further while the side stays latched: the
per-position clamp below is what keeps a payout inside the cash that actually
exists, and re-snapshotting on every price move would reintroduce the
order-dependence the snapshot exists to remove.

Payable PnL is what settlement may recognize. What it may actually *pay* is
additionally limited by cash on hand, and that limit belongs to the payment,
not to the valuation:

```text
paid_pnl = min(payable_pnl, cash_lp_equity_at_payment)
```

This clamp is applied by `apply_payable_pnl` and by terminal settlement. It is
deliberately **not** part of `calculate_payable_pnl`, and therefore not part
of effective collateral or any health check. A position's health is a property
of that position; it must not change because the vault is temporarily short of
cash. Were the clamp part of the valuation, a position with large unrealized
profit and thin stored collateral would become liquidatable as LP equity fell
— liquidated while in profit, for a payout the same clamp had already reduced
to nothing. Vault cash shortage is answered by ADL and by the hard-cap factor,
not by liquidating winners.

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
        credited = min(payable_pnl, available_cash_lp_equity)
        add_position_collateral(position, side, ledger, credited)
        return PnlResult {
            credited,
            unpaid_profit: payable_pnl - credited,
            uncollectible_loss: 0
        }

    loss = abs(payable_pnl)
    collected_loss = min(loss, position.stored_collateral)
    remove_position_collateral(position, side, ledger, collected_loss)

    return PnlResult {
        credited: 0,
        uncollectible_loss: loss - collected_loss
    }
```

This is where the payment-time cash limit of §2.8 is applied, and the only
place it is applied. Crediting is capped by the equity that exists at the
moment of the credit, and any shortfall is returned as `unpaid_profit` so the
caller can report it. Profit the vault could not pay is neither a claim nor a
receivable — the vault has no cash to owe it from — but it is never silently
dropped either: every terminal settlement emits it (§12.6), because a trader
receiving less than their recognized profit is the single outcome most likely
to be mistaken for an accounting error.

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
        pnl_result = apply_payable_pnl(
            position,
            side,
            ledger,
            inputs.payable_pnl,
            inputs.available_cash_lp_equity
        )
        # pnl_result.unpaid_profit is reported, never carried

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

    pay the reward selected by
        keeper_reward_for(inputs.action_kind, inputs.global_config),
        from the source §5.10 assigns it

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

    lp_backstop = min(
        reward - from_position,
        derive_cash_lp_equity(ledger, physical_cash)
    )

    paid = from_position + lp_backstop
    transfer_cash(keeper, paid)

    return KeeperPayment {
        from_position,
        from_lp_backstop: lp_backstop,
        unpaid: reward - paid
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

    u = mul_div_floor(utilization, INDEX_PRECISION, BPS)
    variable_factor = mul_div_floor(u, u, INDEX_PRECISION)

    ledger.current_borrow_rate =
          global_config.base_borrow_rate_bps_day * INDEX_PRECISION
        + global_config.max_variable_borrow_bps_day * variable_factor
```

`current_borrow_rate` is bps per day at `INDEX_PRECISION` (§4.2), and both
addends are formed at that scale before they are summed. The base rate is a
plain bps number and is scaled explicitly; the variable term is a bps number
multiplied by an already-scaled factor, so it arrives at the same scale
without a second multiplication. At full utilization with the initial
parameters the result is `275 * INDEX_PRECISION`.

`u` is converted to `INDEX_PRECISION` before squaring rather than written as
`utilization / BPS`, which as an integer division would collapse to `0` or `1`.
The square is exact at every utilization the curve can reach, so this function
introduces no approximation at all. Refresh runs after every completed mutation
that changes total risk units, physical cash, or any non-LP claim affecting
cash LP equity, which is why its cost matters.

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
        side_for(position)
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
restricted state. Two transitions additionally move the payout factor:

```text
entering HardCap from any other state:
    snapshot_hard_cap_factor(side, market_config, price, cash_lp_equity)

remaining in HardCap, when the side has grown past its band:
    if side_positive_pnl >= mul_div_floor(
           side.hard_cap_reference_pnl,
           BPS + global_config.hard_cap_relatch_band_bps,
           BPS
       ):
        snapshot_hard_cap_factor(side, market_config, price, cash_lp_equity)

leaving HardCap for any other state:
    side.hard_cap_payout_factor = INDEX_PRECISION
    side.hard_cap_reference_pnl = 0
```

The re-latch condition is one-directional: it fires only when the side's
positive PnL has *grown* past the band. A side whose profit falls while it
stays latched keeps its factor, because lowering the denominator would raise
the factor and pay later exits more than earlier ones for no reason the vault
benefits from. Recovery is what the state machine already handles, through
`recovery_pnl_factor_bps` and the transition out of `HardCap`.

No other transition touches it, and no settlement recomputes it. A side that
stays latched in `HardCap` across many settlements keeps the factor it was
given on entry (§6.5).

ADL execution uses one assessment snapshot for candidate eligibility, payable
PnL, keeper reward, and exposure removal.

#### 6.16.1 What a risk state restricts

A side's risk state answers one question: may this side take on more exposure?

```text
function side_accepts_new_exposure(side):
    return not ledger.paused
       and side.risk_state in { Normal, Warning }
```

A pause is folded into this one predicate rather than checked separately at
every call site, so pausing behaves as a vault-wide restricted state; §12.2
explains what follows from that.

This is the predicate §7.2 and §7.8 call "market still accepts new exposure",
and it is evaluated on the side the action would add to, after the risk state
has been refreshed from the action's own price snapshot.

The states mean:

| State | New exposure on this side | Meaning |
|---|---|---|
| `Normal` | Allowed | Aggregate profit on this side is a small fraction of LP equity |
| `Warning` | Allowed | Elevated, and the side cannot return to `Normal` until it falls below the recovery threshold |
| `ADL` | Blocked | Forced deleveraging of this side is permitted |
| `HardCap` | Blocked | Payouts on this side are additionally scaled by the side factor |

Two consequences are deliberate. `Warning` restricts nothing by itself: it
makes recovery sticky, keeping a side latched until its factor falls below
`recovery_pnl_factor_bps` so it cannot oscillate in and out of restriction on
small price moves. And the opposite side is never restricted by this side's
state, because opening against a restricted side reduces skew and reduces that
side's net aggregate PnL — the trade that resolves the condition. Blocking
both sides would leave closure as the only path back to `Normal`.

`restricted_market_side_count` counts every side that is not `Normal`, so a
nonzero count means at least one side is latched, not that one is blocking. It
is the cheap global reader that avoids scanning the market registry; the
blocking question is answered per side by `side_accepts_new_exposure`.

Candidate selection for ADL is specified with the user-facing operation in
§7.14 rather than hidden inside this pure assessment.

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

### 7.0 Common predicates and terminal helpers

Every operation executes atomically and processes one action. A settlement
caller cannot submit an array or combine unrelated opens, closes,
liquidations, or ADL actions in one call.

The two operations that must run before any of the rest — initializing the
vault and registering a market — are specified last, in §7.18, because every
value they establish is defined by the sections in between.

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
terminal failure result. The explicit `RequiresLiquidation` safety outcome
remains non-terminal. An unexpected invariant failure reverts and leaves the
action unchanged.

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

Refusing to terminate when the full reward is unpayable would leave an
eligible action pending indefinitely while `pending_mutation_action_id` blocks
every further increase, decrease, or close on that position — and position
mutations have neither a cancel operation nor an expiry, so the only exits
would be `add_collateral` or liquidation. It would also break first-attempt
finality: an action that survives an eligible attempt is a free retry.

The variable cap is not exploitable. Suppressing the reward requires holding a
position at minimum collateral or at its liquidation threshold, one tick from
liquidation, which cannot be maintained as a strategy; the saving is at most
one fixed reward per action. Keepers may skip an action whose payable reward
is too small, and the owner can always settle it themselves to clear the slot.
No keeper reward is escrowed at position-action creation.

### 7.1 Create a market-open order

```text
function create_market_open_order(owner, request):
    require owner authorization
    require not paused
    require side_accepts_new_exposure(target side)   # §6.16.1
    require request.size > 0
    require request.submitted_collateral > 0
    validate direction, acceptable price, and optional TP/SL prices

    execute_after =
        now + market.config.order_execution_delay_seconds
    require request.expires_at > execute_after
    require request.expires_at <= now + max_market_order_lifetime_seconds

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

    commit_price = read_stamped_price(request.market_id)

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
    if now >= action.expires_at:
        return Expired without state change or reward
    if now < action.execute_after:
        return NotReady without state change or reward

    fill = read_stamped_price(action.market_id)

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
    entry_price_allowed(action.direction, fill.price,
                        action.acceptable_price)
    side_accepts_new_exposure(target side)          # §6.16.1
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
cover `keeper_expiry_reward` as well. It bounds `expires_at` by
`max_order_lifetime_seconds` rather than by the market-entry cap, because a
resting order is meant to rest (§8.5). It additionally requires a positive
trigger price and freezes the trigger direction relative to the authenticated
commit price:

```text
function create_limit_open_order(owner, request):
    validate and escrow exactly as market-open creation, except
    require request.expires_at <= now + max_order_lifetime_seconds
    require request.trigger_price > 0

    commit_price = read_stamped_price(request.market_id)

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
    if now >= action.expires_at:
        return Expired without state change or reward
    if now < action.execute_after:
        return NotReady without state change or reward

    fill = read_stamped_price(action.market_id)

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

    commit_price = read_stamped_price(position.market_id)

    require mul_div_floor(
        request.size_added, PRICE_PRECISION, commit_price.price
    ) > 0
    require mul_div_floor(
        position.size + request.size_added,
        market.config.market_risk_factor_bps,
        BPS
    ) > position.risk_units

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

The two dust checks are what keep an increase settleable. `derive_added_exposure`
requires positive added base and positive added risk (§6.13), and those are
`require`s, so failing them is an unexpected failure under §8.9: the call
reverts and the action survives. A position mutation has no cancel operation
and no expiry, so an increase small enough to round either quantity to zero
would occupy `pending_mutation_action_id` permanently and leave the position
with no increase, decrease, or close available for the rest of its life — only
an attached trigger or liquidation could still exit it. A size add of one unit
on a market priced in the tens of thousands is enough to do it. Rejecting at
creation, against the commitment price, is what makes the settlement-time
requires unreachable rather than merely defensive.

The commitment price is the right reference even though settlement prices the
action at the fill. Base rounds to zero only for a size add near the price
itself, and no fill inside a market's ordinary movement turns a size that
cleared this check into one that cannot. The risk-unit check does not depend on
price at all.

Successful settlement:

```text
function settle_increase(action_id, keeper):
    require keeper authorization
    load matching pending action and position
    require position.pending_mutation_action_id == action_id
    if now < action.execute_after:
        return NotReady without state change or reward

    fill = read_stamped_price(position.market_id)
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

    preflight side_accepts_new_exposure(position side), the complete
        resulting position, capacity, and market caps

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
senior = capitalize_for_surviving_mutation(
    position,
    side,
    ledger,
    market,
    pending_funding,
    pending_borrow,
    global_config
)

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
add_exposure(
    position,
    side,
    ledger,
    exposure.size_added,
    exposure.base_added,
    exposure.risk_added
)

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

    commit = read_stamped_price(position.market_id)
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
    if now < action.execute_after:
        return NotReady without state change or reward
    if now < position.last_size_increase_at + min_position_lifetime:
        return NotReady without state change or reward

    fill = read_stamped_price(position.market_id)
    if fill.observed_at <= action.commit_observed_at:
        return NotReady without state change or reward

    accrue global borrow and market funding to now
    evaluate and apply current market risk state
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
        remaining health, minimum collateral, and that cash LP equity
        covers the payable profit to be credited

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
senior = capitalize_for_surviving_mutation(
    position,
    side,
    ledger,
    market,
    pending_funding,
    pending_borrow,
    global_config
)

pnl_result = apply_payable_pnl(
    position,
    side,
    ledger,
    payable_pnl,
    cash_lp_equity
)
require pnl_result.uncollectible_loss == 0
require pnl_result.unpaid_profit == 0

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
require surviving effective collateral > liquidation_threshold

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

Three guards keep this path and the terminal path from diverging.
`uncollectible_loss == 0` enforces §6.5: a surviving position may never carry
a loss its collateral could not absorb, so the action reverts rather than
leaving the deficit attached. `unpaid_profit == 0` is its mirror: a surviving
position may not realize profit the vault could not pay, because unlike a
terminal settlement it has no result in which to report the shortfall and its
closing fee would be computed from profit that was never credited. The
decrease preflight therefore checks that cash LP equity covers the payable
profit, and the action takes the ordinary expected-failure path if it does
not. The `min` against stored collateral mirrors §6.10 so both paths debit the
same way; preflight already guarantees the fee is payable, so it is defence in
depth, not a licence to collect a partial fee where a full one was due.

### 7.10 Create and settle a voluntary close

Creation is identical to decrease creation except the action kind is `Close`
and no size is stored; it always targets the position's complete remaining
exposure at settlement.

```text
function create_close(position_id, owner, acceptable_price):
    require owner authorization and position ownership
    require position.pending_mutation_action_id is None

    commit = read_stamped_price(position.market_id)
    create Close action with commit cursor and execute-after timestamp
    set position.pending_mutation_action_id
```

Settlement:

```text
function settle_close(action_id, keeper):
    require keeper authorization
    load matching action and position
    if now < action.execute_after:
        return NotReady without state change or reward
    if now < position.last_size_increase_at + min_position_lifetime:
        return NotReady without state change or reward

    fill = read_stamped_price(position.market_id)
    if fill.observed_at <= action.commit_observed_at:
        return NotReady without state change or reward

    accrue global borrow and market funding to now
    evaluate and apply current market risk state
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

    commit = read_stamped_price(position.market_id)
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
    if now < instruction.execute_after:
        return NotReady without state change or reward
    if now < position.last_size_increase_at + min_position_lifetime:
        return NotReady without state change or reward

    fill = read_stamped_price(position.market_id)
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

    checkpoint, evaluate and apply current market risk state, then calculate
        complete-position funding, borrow, and PnL
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
    checkpoint, evaluate and apply current market risk state, then calculate
        complete-position funding, borrow, and PnL
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

    price = read_stamped_price(position.market_id)
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
        using LP residual cash for any gap, capped at what exists
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
        unpaid keeper reward, if the two sources could not cover it
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
    price = read_stamped_price(position.market_id)

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

Candidate selection is deliberately unranked. Any position on the restricted
side with positive raw PnL is eligible, and the protocol does not require the
keeper to pick the largest winner, the most leveraged one, or any particular
order. Ranking would mean either sorting positions on chain, whose cost grows
with the number of traders and which §4.1 rules out for exactly that reason,
or an eligibility rule elaborate enough to need its own accounting. Neither is
worth it here.

What bounds the mechanism is the state gate, not the selection. `require
side_assessment.next_state is ADL or HardCap` is re-evaluated from the current
book on every call, so each execution that removes profitable exposure lowers
the side's PnL factor, and once it falls below `adl_pnl_factor_bps` no further
ADL is permitted on that side. A keeper cannot keep taking positions after the
condition has cleared, and cannot use ADL against a side that was never
restricted.

The accepted cost is fairness between winners: a keeper may take a small
profitable position while a larger one remains open, so being deleveraged is
not proportional to how much of the liability a trader represents. The
protection a trader has is the state gate and the fixed, capped reward, not a
queue position.

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

    transfer collateral from owner into LP request escrow
        (held by the request contract, not the vault)

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
somebody's job.

No LP depends on a third party to get their request resolved. `executor` is
any authenticated account, including the request's own owner. Because only the
FIFO head is resolvable, an owner queued behind others clears the queue by
calling the operation once per request ahead of theirs, collecting each
reward on the way, so the manual path funds itself. The keeper reward exists
to make that path unnecessary, not to make it exclusive.

No round-assignment rule pins a request to one specific price round. The
property such a rule would protect is real — whoever picks the settlement
moment picks the NAV, and a withdrawing LP who can choose the moment exits at
an inflated share price and leaves the mark-to-market loss with the LPs who
stay — but it is the same problem as a trader choosing the observation that
fills a market order, and it has the same answer everywhere in this
specification: a mandatory delay fixes the earliest possible moment, and a
fixed reward makes a competing party settle at the first opportunity, so the
party with an interest in waiting does not control the timing.

The residual exposure is the one stated in §1.7 and §12.8.1: the guarantee
rests on a competing executor existing, not on a rule naming one round. With a
delay measured in hours or a day, the window in which to compete is wide.

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

if not deposit_eligible or shares_to_mint == 0:
    fail_lp_request(request, executor, reason)

transfer resolve_reward from escrow to the executor
transfer remaining asset escrow into the vault
mint shares_to_mint to owner
mark request Settled and advance FIFO pointer
refresh global borrow rate
```

The reward is deducted before conversion, so the depositor mints shares for
the assets that actually reach the vault and no share is minted against value
paid to the executor.

`deposit_eligible` is the rule `min_deposit_nav_factor_bps` exists for, and it
is a guard on the conversion arithmetic rather than a judgement about market
conditions:

```text
if share_supply == 0:
    deposit_eligible = true
else if cash_lp_equity == 0:
    deposit_eligible = false
else:
    deposit_eligible =
        marked_vault_nav * BPS
            >= cash_lp_equity * min_deposit_nav_factor_bps
```

Minting divides by `marked_vault_nav + 1`. As NAV falls toward zero with
shares still outstanding, that denominator collapses and a deposit of any size
mints an unbounded number of shares, diluting every existing holder to
nothing. The virtual offsets `+1` and `+ SHARE_SCALE` keep the arithmetic
defined, not fair. At the initial `1,000`, deposits stop once recognized trader
profit reaches nine tenths of cash LP equity, which is the regime where the
conversion actually degenerates.

The gate is set close to that regime on purpose, because every basis point of
extra conservatism here is paid for in the worst possible state. A deposit adds
LP equity and therefore lowers every side's PnL factor — the direction the
vault wants in exactly the conditions that make this gate bind. A value like
`8,000` would stop deposits once recognized profit reached a fifth of cash
equity, which four sides sitting at `4.9%` reach while every one of them is
merely in `Warning` and nothing is restricted: the vault would refuse rescue
capital in a state it is not even treating as an emergency, for an arithmetic
margin it does not need. The depositor is already protected without the gate,
because marked NAV deducts recognized trader profit before conversion, so
depositing into a vault under stress is priced rather than subsidized. The gate
exists to keep the denominator away from zero, and nothing more.

The first deposit into an empty vault is exempt because there are no holders
to dilute. A vault with shares outstanding and no cash equity accepts no
deposit at all; it is recapitalized by governance or not at all, and this
operation is not the path for it.

Successful withdrawal settlement uses the pre-withdrawal state:

```text
assets_to_pay = mul_div_floor(
    withdrawal_shares,
    marked_vault_nav + 1,
    share_supply + SHARE_SCALE
)

if assets_to_pay > free_lp_capital
   or post-withdraw utilization > max_withdraw_utilization_bps
   or vault_shortfall > 0
   or any active market side is in ADL or HardCap:
    fail_lp_request(request, executor, reason)

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

A withdrawal removes LP equity, and LP equity is the denominator of every
side's PnL factor, so paying one out pushes every side closer to restriction.
It is refused while the vault is short of its claims, and while any active side
is in `ADL` or `HardCap`. `Warning` does not block it, for the same reason it
does not block new exposure (§6.16.1): it is a latch that makes recovery
sticky, not a stop.

A deposit is not gated on side risk state at all. It adds LP equity and
therefore lowers every side's factor, which is the direction the vault wants.

A failed resolution is an expected terminal outcome, not a revert:

```text
function fail_lp_request(request, executor, reason):
    if request.kind == Deposit:
        reward = min(keeper_lp_resolve_reward, request.escrowed_amount)
        transfer reward from escrow to the executor
        refund the remaining collateral escrow to the owner
    else:
        reward = 0
        return the complete escrowed shares to the owner

    mark request Failed
    advance the FIFO pointer
    emit the terminal result(reason, reward)
```

A failed *withdrawal* pays no reward. Its escrow is shares, not cash, and a
failed withdrawal releases no assets, so there is nothing to pay from; taking
the reward in shares would confiscate part of an LP's stake for an outcome
they did not cause. The executor is compensated by the deposits and successful
withdrawals in the same queue, and a queue standing on a failing withdrawal is
cleared by the owner themselves at the cost of gas alone (§7.17).

Failing rather than reverting is what keeps the queue moving. Only the FIFO
head is resolvable, so a `require` here would let one unsatisfiable request —
a withdrawal larger than free capital, or any request while a side is latched
— block every LP behind it for as long as that condition held.

There is no `Expired` outcome: a request that is not yet resolvable stays
`Pending` and is retried, and a request that becomes resolvable either settles
or fails. There are no partial fills and no persistent withdrawal cash
claims. In a clean terminal vault, the final LP
may withdraw all residual cash LP equity so conversion rounding cannot strand
ownerless assets.

### 7.18 Initialize the vault and register a market

These are the two operations that must exist before any other can be called,
and they are the only ones the configuration authority performs on the
economic state.

```text
function initialize_vault(authority, config, vault_asset, share_token,
                          oracle, authorities):
    require not ledger.initialized

    validate the complete global configuration under §10.3.1
    require decimals(vault_asset) == 7          # §12.1
    require decimals(share_token) == 13

    store the configuration and the four authorities
    ledger.borrow_index            = 0
    ledger.borrow_index_remainder  = 0
    ledger.current_borrow_rate     = base_borrow_rate_bps_day
                                     * INDEX_PRECISION
    ledger.last_global_checkpoint  = now
    every claim total                = 0
    ledger.total_risk_units          = 0
    ledger.open_position_count       = 0
    ledger.restricted_market_side_count = 0
    next_position_id = next_action_id = next_lp_request_id = 1
    next_lp_request_to_resolve       = 1
    active_market_ids                = empty
    ledger.paused                    = false
    ledger.state_version             = STATE_VERSION
    ledger.initialized               = true
```

The global borrow clock starts at the moment of initialization, so the first
position opened does not inherit index growth from an epoch that had no
positions in it. The rate starts at the base rate because utilization is zero.

```text
function register_market(authority, market_id, market_config):
    require configuration authority
    require ledger.initialized
    require market_id is not already registered
    require len(active_market_ids) < max_active_markets

    validate market_config under §10.3.2
    require the resulting sum of hard-cap factors over every active
        market side stays within global_hard_cap_factor_limit_bps

    for each side in { Long, Short }:
        size_open_interest      = 0
        base_exposure           = 0
        stored_collateral_total = 0
        risk_units              = 0
        risk_state              = Normal
        hard_cap_payout_factor  = INDEX_PRECISION
        hard_cap_reference_pnl  = 0

    all six funding indices  = 0
    skew_ema                 = 0
    both remainder groups    = zeroed
    pending_receiver_funding = 0
    current_payer_side       = None
    current_payer_rate       = 0
    last_funding_checkpoint  = now

    append market_id to active_market_ids
    emit market-registered result
```

A market's funding indices start at zero and its checkpoint starts at the
moment of registration, which is why §4.13's empty-book rule seeds `skew_ema`
from the first position's live skew rather than leaving it at zero: an empty
book has no skew, and starting a one-sided market from a balanced history
would hand it a temporary funding discount.

Removing a market is the mirror and is permitted only from a fully quiet
state:

```text
function deregister_market(authority, market_id):
    require configuration authority
    require both sides have zero size_open_interest and zero base_exposure
    require market.pending_receiver_funding == 0
    require both sides are Normal
    require no pending action references this market

    remove market_id from active_market_ids
```

Its cumulative indices and checkpoint timestamp are retained, not reset, so a
later re-registration cannot rewind an index that a historical position was
priced against.

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

The first, second and fourth clauses are timing gates that a later call can
satisfy, so failing one yields `NotReady`. None of them is an execution
attempt, none consumes the action, and none pays a reward — calling a
decrease, close, take-profit, or stop-loss before the minimum position
lifetime has elapsed is a settlement that is not due yet, and it must not
revert.

The expiry clause is different in direction. An expired entry never becomes
ready, so failing it yields `Expired` rather than `NotReady`, and the only
operation it admits is the cleanup path of §7.6.

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
strictly later than `execute_after`. Its upper bound differs by kind:

```text
market entry: expires_at <= now + max_market_order_lifetime_seconds
limit entry:  expires_at <= now + max_order_lifetime_seconds
```

A resting limit order is supposed to wait, so a week is the point of it. A
market order is not: its purpose is to execute at the next qualifying
observation, and an entry that is still live hours later is a limit order
without a trigger — which this protocol already offers, with a trigger. Sharing
one bound let a trader take the limit order's lifetime for a commitment that
was never meant to rest, and §12.8.1's keeper assumption was carrying the
difference. `300` seconds is sixty times the default execution delay, wide
enough for keeper latency and for a slow source to publish (§12.7.4), and short
enough that the option §8.5 describes below has no material value.

Expiry uses:

```text
executable = now < expires_at
expired = now >= expires_at
```

At exactly `expires_at`, execution is forbidden and expiry cleanup is allowed.
There is no timestamp at which both can succeed.

The delay is necessary but insufficient. A delayed action without a newer
qualifying observation remains `NotReady` indefinitely, except that an entry
can still reach expiry.

The upper bound on `expires_at` keeps expiry reachable and bounds an option
the trader would otherwise hold. Between `execute_after` and `expires_at` an
entry order is a standing right to be filled at a price inside its bound, and
nothing obliges anyone to settle it — the trader may act as their own keeper
and wait. Keeper competition normally closes that window (§1.7), but that is
an assumption about the world rather than a rule;
`max_market_order_lifetime_seconds` is the rule for a market entry, and
`max_order_lifetime_seconds` for a limit entry, capping how long the option
runs if no keeper appears.

A position mutation needs no equivalent. Its holder already owns the position,
so waiting to close is not a right the commitment granted them.

### 8.6 Fresh-price requirement

Creation stores the observation time of the authenticated price available when
the trader commits:

```text
commit_observed_at
```

Settlement obtains a fresh aggregate and requires:

```text
fill_observed_at > commit_observed_at
```

Equality fails. A transaction timestamp, block number, or elapsed delay cannot
replace the observation cursor because none proves that the price contains
information created after commitment.

The observation stamp is the oldest source timestamp contributing to the
accepted aggregate. This conservative choice ensures every source supporting
the fill is newer than the commitment cursor.

Both reads must be freshly aggregated. A stamp retained from an earlier read is
backdated by however long it was retained, which satisfies the comparison
without any new observation having arrived; §12.7.1 states this as a
requirement on the oracle.

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
- a market side in `ADL` or `HardCap`, which blocks new risk on that side
  (§6.16.1);
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
collateral. Liquidation alone may draw on LP residual equity for a price-gap
shortfall, and only as far as that equity reaches.

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
liquidation, and ADL remove every remaining unit of size, base exposure, and
risk assigned to the position, so no terminal dust remains in market
aggregates.

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

The payment is best-effort at that second step, and deliberately so. The
reward is capped at position collateral plus whatever cash LP equity exists,
so a keeper can receive less than the configured amount, or nothing, when both
sources are empty. The liquidation completes regardless.

Promising the full reward unconditionally was not implementable and was worse
than useless. A protocol cannot pay out of LP equity that is zero, and the
earlier formulation turned that impossibility into a revert — which would have
blocked liquidation precisely in the state where an unliquidated position is
most dangerous, and left the position accruing borrow against a vault that had
already run out of cash. Capping the payment keeps the risk-removal path open
at all times; the incentive degrades before the mechanism does.

The bound in the other direction is unchanged and absolute. The keeper
receives at most the configured fixed liquidation reward, never a percentage
of collateral, size, target health, or trader loss. Any unpaid remainder is
reported in the liquidation result as forgone keeper revenue; it creates no
claim, no receivable, and no bad debt. A liquidation that reverts pays
nothing.

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


The fee is calculated by `calculate_closing_fee` in §6.9, which is the single
normative statement of the formula; §3.2 explains the two components and their
caps. This section states only what must be true of the result, whatever
arithmetic produced it:

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

The per-quantity rounding directions are tabulated once, in §2.11. That table
is normative; this section states the properties the table exists to produce,
so a change to one direction can be checked against them.

Rounding never favours the trader over the vault, with the single documented
exception of a partial reduction's base split, which is a conservation rule
rather than a direction rule because the two portions always sum to the
pre-reduction base (§2.11).

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
| `max_order_lifetime_seconds` | Seconds | `604,800` | Longest permitted limit-entry lifetime, one week (§8.5) |
| `max_market_order_lifetime_seconds` | Seconds | `300` | Longest permitted market-entry lifetime, five minutes (§8.5) |
| `min_borrow_fee_seconds` | Seconds | `900` | Duration used to quote the monetary minimum for each borrow window |
| `funding_half_life_seconds` | Seconds | `43,200` | Shared EMA half-life used by all funding markets |
| `risk_capacity_limit_bps` | Bps | `8,500` | Maximum admitted risk units relative to cash LP equity |
| `base_borrow_rate_bps_day` | Bps/day on risk units | `25` | Borrow rate at zero utilization |
| `max_variable_borrow_bps_day` | Bps/day on risk units | `250` | Maximum utilization-dependent addition to the borrow rate |

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
| `keeper_liquidation_reward` | `2,500,000` (`$0.25`) | Position value, then LP residual as far as it reaches |
| `keeper_adl_reward` | `2,500,000` (`$0.25`) | Affected position value |
| `keeper_lp_resolve_reward` | `2,500,000` (`$0.25`) | LP request escrow or released assets |

Equal initial values make each successful action equally attractive to a
keeper. The independent fields allow later tuning without coupling unrelated
actions. No percentage keeper reward, keeper fee share, keeper reserve, or
user-selected execution budget exists.

#### 10.1.4 LP and bounded-operation policy

| Parameter | Unit | Initial value | Meaning |
|---|---:|---:|---|
| `config_timelock_seconds` | Seconds | `172,800` | Delay between proposing and applying a parameter change (§12.3) |
| `max_active_markets` | Count | `8` | Maximum active markets included in synchronized vault operations |
| `global_hard_cap_factor_limit_bps` | Bps | `10,000` | Maximum sum of configured side hard-cap factors across active markets |
| `hard_cap_relatch_band_bps` | Bps | `2,500` | Growth in a latched side's positive PnL that triggers a fresh hard-cap snapshot (§6.5) |
| `max_withdraw_utilization_bps` | Bps | `8,000` | Maximum post-withdrawal utilization |
| `min_deposit_nav_factor_bps` | Bps | `1,000` | Minimum marked-NAV-to-cash-equity factor for an ordinary LP deposit |
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
market is added or updated. The declared widths in §2.1.1 make "fits its
formulas" checkable rather than aspirational: every product on the way to a
division is formed in 256-bit and its quotient is range-checked on the way
back to 128 bits, so validation's job is to bound the inputs that feed those
products, not to re-derive their headroom.

The bounds that exist for arithmetic rather than economic reasons are the two
duration caps in §2.1.1 and the ceilings on size and base exposure in §10.2.3.
A configuration that satisfies every rule below cannot overflow any formula in
this specification.

#### 10.3.1 Global validation

```text
min_collateral > 0
min_collateral > keeper_liquidation_reward
every keeper reward <= min_collateral

0 <= min_position_lifetime  <= 86,400
0 <  min_borrow_fee_seconds <= 86,400
0 <  max_order_lifetime_seconds <= 2,592,000
60 <= max_market_order_lifetime_seconds <= max_order_lifetime_seconds

60 <= funding_half_life_seconds <= 31,536,000

0 < risk_capacity_limit_bps <= BPS
0 <= max_withdraw_utilization_bps <= BPS
0 <= min_deposit_nav_factor_bps <= BPS

0 <= base_borrow_rate_bps_day <= BPS
0 <= max_variable_borrow_bps_day <= BPS

fee_lp_revenue_share_bps + referral_fee_share_bps <= BPS
borrow_lp_revenue_share_bps <= BPS

max_active_markets > 0
0 < global_hard_cap_factor_limit_bps <= BPS
0 < hard_cap_relatch_band_bps <= BPS
lp_request_delay_seconds > 0
config_timelock_seconds > 0
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

A parameter update is proposed and applied in two phases separated by
`config_timelock_seconds`, with the exemptions listed in §12.3. Each phase
follows these rules:

1. Authenticate the configuration authority.
2. Validate the complete proposed configuration and every cross-parameter
   relationship.
3. Accrue each affected time-based accumulator to the update timestamp using
   the old parameter.
4. Store the new parameter only after that checkpoint.
5. Refresh any forward-looking derived rate or risk state.
6. Emit the old and new values with the effective timestamp.

Fields affecting borrow accrual include the base rate, variable rate, and any
value that changes cash LP equity or admitted risk. Funding
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
| `expires_at`, bounded at creation by the lifetime cap for its kind | Each expiring entry order |
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

### 10.4 Deployment defaults

The complete initial parameter set is:

```text
GLOBAL
min_collateral                       = 10,000,000       # $1.00
min_position_lifetime                = 60              # 1 minute
max_order_lifetime_seconds           = 604,800         # 7 days, limit entries
max_market_order_lifetime_seconds    = 300             # 5 minutes, market entries
min_borrow_fee_seconds               = 900             # 15 minutes
funding_half_life_seconds            = 43,200          # 12 hours
risk_capacity_limit_bps              = 8,500           # 85%
base_borrow_rate_bps_day             = 25
max_variable_borrow_bps_day          = 250
fee_lp_revenue_share_bps             = 9,000           # 90%
borrow_lp_revenue_share_bps          = 9,000           # 90%
referral_fee_share_bps               = 250             # 2.5%
config_timelock_seconds              = 172,800         # 48 hours
max_active_markets                   = 8
global_hard_cap_factor_limit_bps     = 10,000
hard_cap_relatch_band_bps            = 2,500           # 25%
max_withdraw_utilization_bps         = 8,000           # 80%
min_deposit_nav_factor_bps           = 1,000           # 10%

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

lp_request_delay_seconds  BY DEPLOYMENT PROFILE
local development                    = 60               # 1 minute
public test environment              = 3,600            # 1 hour
production environment               = 86,400           # 1 day
```

Parameters this protocol does not have are absent rather than present and set
to zero: there are no skew-tiered opening or closing fees, no minimum borrow
index delta, no keeper revenue share, no keeper reserve, no user execution
budget, no percentage liquidation or ADL reward, no maximum ADL reward, and no
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
                     = $0.2604167

$5,099.75 >= $5,000.2604167                 => margin passes
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
hard-cap value     = $1,000,000 * 6%      = $60,000
side payout factor = $60,000 / $80,000    = 0.75
```

That factor is stored on the side at the moment it latches into `HardCap`,
along with the `$80,000` denominator it was measured against. Neither moves
again until the side's positive PnL grows past `$100,000` — the `2,500` bps
band of §6.5 — or the side leaves `HardCap`.

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

Now suppose a second position on the same side, identical in every respect,
is deleveraged immediately afterwards. The first settlement moved cash LP
equity twice: down `$7,500` for the profit credited to the position label, and
up `$22.50` for the borrow collected, of which `$2.50` became a protocol
claim. The side's aggregate positive PnL fell by the `$10,000` of raw profit
that left with the position:

```text
cash LP equity   = $1,000,000 - $7,500 + $22.50 = $992,522.50
hard-cap value   = $992,522.50 * 6%             =    $59,551.35
side positive    = $80,000 - $10,000            =    $70,000.00
recomputed factor = $59,551.35 / $70,000        =     0.8507336
```

A factor recomputed at this point would pay the second trader
`$8,507.3357142`, against the `$7,500` the first received for an identical
position — a difference of over `$1,000` decided by nothing but which one a
keeper reached first. The stored snapshot is what makes both receive `$7,500`.

It is the band, not the snapshot, that keeps the side's total payout near the
`$60,000` measured when it latched. A frozen factor bounds the side only at the
instant of latching: if the price keeps running, raw aggregate profit grows and
every position still pays `0.75` of a larger number, so `$200,000` of side
profit would pay out `$150,000` against a `$60,000` cap.

With the band, the side re-latches the moment its positive PnL reaches
`$100,000`. Suppose it does so with cash LP equity down to `$950,000` after the
payouts already made:

```text
hard-cap value     = $950,000 * 6%        = $57,000
side payout factor = $57,000 / $100,000   = 0.57
reference PnL      = $100,000
```

Every position on the side now scales at `0.57` instead of `0.75`, and the next
re-latch waits for `$125,000`. The running exposure is held to `1.25x` the cap
value measured at the most recent latch, and each crossing measures against an
LP equity that the previous band's payouts have already reduced.

ADL removes the complete position, pays only the fixed ADL reward from its
value, charges no closing fee, and clears its pending mutation and triggers.
The side risk state is recalculated after exposure is removed, and if that
recalculation takes the side out of `HardCap` the stored factor returns to one
whole.

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

### 11.12 Funding window split at a sign change

This is the conformance case for §6.2.1. Every integer below is the output of
the specified algorithm and an implementation must reproduce all of them.

A market has `110` base units long and `90` short, so live skew is exactly
`+0.1`. Its EMA still carries a strongly short-dominated history at `-0.5`. Two
days elapse with the book unchanged:

```text
long_base                 = 1,100,000,000
short_base                =   900,000,000
skew_ema (E0)             = -50,000,000,000,000      # -0.5
instant_weight_bps        = 3,000                    # w = 0.3
funding_half_life_seconds = 43,200                   # H = 12 hours
max_funding_rate_bps_day  = 80
elapsed                   = 172,800                  # 2 days = 4 half-lives
```

The coefficients and endpoints:

```text
S       =  10,000,000,000,000        # +0.1
A       =  10,000,000,000,000
B       = -42,000,000,000,000        # 0.7 * (E0 - S)
d_end   =   6,250,000,000,000        # 2^-4, exactly

I_start = A + B          = -32,000,000,000,000      # shorts pay
I_end   = A + B * d_end  =  +7,375,000,000,000      # longs pay
```

The endpoints disagree in sign, so the window splits. Had the test compared
`A + B` against `A` instead, it would also have said "split" — but so would it
for a ten-minute window of this same book, where `I` never changes sign at all
and `t_star` lands a day beyond the window's end. That is the case §2.1.2
exists to exclude:

```text
d_star = 23,809,523,809,524          # -A/B, and d_end < d_star < 1
t_star = 89,440                      # seconds, inside (0, 172,800)
```

Segment one, `t = 0` to `89,440`, shorts paying, `d` running from `1` to
`d_star`:

```text
j1                 = 4,748,527,677,440,269,774
j2                 = 2,939,564,752,701,152,030
quadratic_integral = 20,910,289,747,150,055,707,600,000,000,000
```

Segment two, `t = 89,440` to `172,800`, longs paying, `d` running from `d_star`
to `d_end`:

```text
j1                 = 1,094,387,238,160,076,781
j2                 =   164,483,796,211,532,077
quadratic_integral =  2,044,641,364,626,780,877,880,000,000,000
```

The additivity property of §4.6 holds exactly here. Integrating the window as
one segment gives `22,954,931,111,776,836,585,480,000,000,000`, and the two
segments sum to precisely that — a difference of zero, not merely one inside
`DECAY_TOLERANCE`. Against a high-precision evaluation the three integrals are
off by `2.8e-14`, `-2.7e-13`, and `1.9e-15` relative. Note that the second is
*negative*: the composite integral is not one-directional even though each
primitive inside it is, because `B` is negative and the `2*A*B*j1` term
inverts the direction of its truncation. A test asserting one-sided error on
the integral is testing something the specification does not claim.

Carrying segment one into §6.2 with `$1,000,000` of short size paying and
`$1,100,000` of long size receiving — longs have more base than shorts, so the
receiver fraction is one whole and nothing goes to LPs:

```text
receiver_backed_payer_index_short += 19,361,379,395
    carried remainder                44,004,456,608,000,000,000,000

receiver_backing_scaled = 1,000,000 * 10^7 * 19,361,379,395

liability_delta         = 1,936,137,939          # $193.6137939
    carried remainder      50,000,000,000,000

receiver_index_long    += 17,601,253,995
    carried remainder      5,000,000,000,000
```

And the EMA advances for the whole window regardless of the split:

```text
ema_after = 7,375,000,000,000            # +0.07375
```

Two days of a book that is only mildly long-skewed have moved the EMA from
`-0.5` to `+0.07375`, and along the way the short side paid for the first
twenty-five hours on the strength of its own history before the long side took
over. That is the mechanism of §3.4.2 doing exactly what it is for.

## 12. Operational contract

Sections 2 through 11 define the economics. This section defines what the
surrounding system must provide for those economics to hold: the token the
vault holds, what pausing means, who may change what, how state survives an
upgrade, how failures are reported, what the protocol emits, what it requires
of the oracle, and what it assumes rather than guarantees.

Reentrancy is not treated here. The host rejects an attempt to re-enter a
contract already on the call stack, so the ordering rules in §4.9 and §6.10 —
accrue before mutate, credit before collect — exist for accounting
correctness, not as a reentrancy defence, and must not be weakened on the
grounds that reentrancy is impossible.

### 12.1 Collateral and share token requirements

The vault holds exactly one collateral token, `vault_asset`, and its balance is
the sole authority for physical cash (§2.2). That makes the token part of the
trust boundary, so its properties are requirements rather than assumptions.

```text
decimals(vault_asset) == 7
```

Every cash amount in this specification is stated at `PRICE_PRECISION = 10^7`
and compared directly against the token balance. A token with different
decimals would make every claim wrong by a power of ten, and no conversion
factor is permitted, because §2.2 forbids maintaining a second authoritative
cash counter that a conversion would amount to. Seven decimals is also the
Stellar convention, so this is a check rather than a constraint.

The token must further satisfy:

- **No transfer fee.** §9.1 requires a transfer into the vault to increase
  physical cash and exactly one ownership label by the same amount. A token
  that delivers less than it debits breaks that identity on the first deposit.
- **No rebasing and no balance change outside transfers.** An unsolicited
  increase is tolerated and belongs to LPs (§2.2), but a decrease the vault did
  not authorize creates a silent shortfall against claims it has already
  recognized.
- **No supply or precision change after activation.**

Two properties are trusted rather than required, and the consequences are
stated here rather than discovered later. The issuer can freeze or blacklist an
account, and a transfer can fail for reasons the vault cannot inspect. A failed
transfer reverts the whole operation (§8.9), which is correct for accounting
and bad for liveness: a blacklisted trader's liquidation reverts, and the
position keeps accruing borrow while nobody can remove it.

An implementation that cannot accept that dependency converts terminal payouts
to a pull model — credit an owed balance, let the owner withdraw it separately
— at the cost of a sixth entry in the claim equation of §2.5. This
specification pays directly and accepts the dependency.

The LP share token is a separate token controlled by the vault:

```text
decimals(share_token) == 13        # 7 collateral decimals + SHARE_SCALE
mint and burn authority == vault only
```

Shares are ordinary transferable tokens. A holder who acquires shares outside
the request queue still redeems them through it, so the FIFO delay, the
free-capital bound, and the withdrawal gates of §7.17 apply to every holder
identically. A secondary market may therefore price shares below their marked
value whenever the queue is long or withdrawals are gated; that discount is
information about the queue, not a claim against the vault.

### 12.2 Pause semantics

The rule is:

**A pause stops the vault taking on risk. It never stops anyone shedding it.**

That single principle decides every operation, and it is implemented through
one existing predicate rather than a check scattered across the call sites:

```text
function side_accepts_new_exposure(side):
    return not ledger.paused
       and side.risk_state in { Normal, Warning }
```

A pause is therefore a vault-wide restricted state, and every path that adds
exposure already knows what to do. A pending market open, limit open, or
increase that becomes eligible during a pause takes the ordinary
expected-failure route of §8.9: it terminates, pays its action reward, refunds
its escrow, charges no opening fee, and creates no position. A pause drains
the pending risk-adding queue rather than freezing it, so no order waits for
an unpause that may never come.

| Operation | While paused |
|---|---|
| Create any entry order or increase | Rejected at creation |
| Settle a pending entry or increase | Terminates as an expected failure |
| Cancel a limit order, clean up an expired order | Allowed |
| Add collateral | Allowed |
| Decrease, close, take-profit, stop-loss | Allowed |
| Liquidation, ADL | Allowed |
| Create or resolve an LP request | Rejected; pending requests wait |
| Claim referral revenue | Allowed |
| Claim protocol revenue | Rejected |
| Global and market checkpoints | Always run |

`require LP requests are currently allowed` in §7.17 means `not paused`.
Pending LP requests are safe to leave waiting precisely because the `Expired`
outcome no longer exists: the queue resumes where it stopped, with every
request's escrow intact.

Two entries in that table need their reasons stated. Protocol revenue claims
are blocked because the same authority can generally pause; leaving both
available at once creates a pause-and-drain path that costs nothing to close.
Referral balances are ordinary user funds and are not withheld.

Accrual never pauses. The global borrow index, every market's funding index,
and both checkpoint clocks advance across a pause exactly as they would
otherwise, and §4.7 already states this for borrow. That is only fair because
exits stay open: a trader who does not want to keep paying borrow through an
incident can close, and a position that becomes unhealthy can still be
liquidated. A pause that blocked exits while continuing to charge for time
would be charging for a service it had withdrawn.

### 12.3 Authorization and governance

Four authorities appear in the storage model. They are distinct roles and
should be distinct keys.

| Authority | May |
|---|---|
| `configuration_authority` | Propose and apply parameter changes, register markets |
| `pause_authority` | Set `paused`; may not clear it |
| `unpause_authority` | Clear `paused` |
| `oracle_authority` | Name the contract that supplies authenticated prices |
| `protocol_recipient` | Claim accumulated protocol revenue |

Pausing and unpausing are split on purpose. Pausing is a safety action whose
worst case is lost volume, so it should sit behind a fast key that can act
without ceremony. Unpausing re-admits risk, so it belongs with the slower
authority alongside configuration.

Parameter changes are two-phase and delayed:

```text
propose_configuration(authority, proposal):
    validate the complete proposal under §10.3
    store it with effective_at = now + config_timelock_seconds

apply_configuration(proposal_id):
    require now >= proposal.effective_at
    checkpoint every accumulator the change affects, under the old value
    store the new values
    emit old and new values with the effective timestamp
```

The delay exists because almost every parameter here can move value between
parties who cannot react instantly. Raising `close_pnl_fee_bps` taxes open
positions at settlement; lowering `hard_cap_pnl_factor_bps` reduces payouts on
a side that is already restricted; changing `maintenance_margin_bps` makes
positions liquidatable that were not. A timelock does not prevent any of that,
but it makes it observable in advance, which is the difference between a
governance action and a surprise.

```text
config_timelock_seconds initial value: 172,800   # 48 hours
```

Two categories are exempt, and only these two: setting `paused`, and any
change that is validated to move a bound in the more conservative direction —
lowering an exposure ceiling, lowering `risk_capacity_limit_bps`, raising a
margin requirement. A protocol that must wait 48 hours to become safer has the
timelock pointed the wrong way. Every exempt change still checkpoints under
the old value first, and still emits.

Nothing in this specification grants an authority the ability to move position
collateral, escrow, referral balances, or LP equity directly. There is no
administrative transfer, no forced position closure outside liquidation and
ADL, and no path from any authority to a trader's funds other than the
economics defined in sections 3 and 6.

### 12.4 Storage lifetime, upgrade, and migration

§5.14 requires that no live position, pending action, escrow, referral
balance, or protocol claim disappears through storage expiry. On Soroban that
is a statement about storage type and TTL, so it is made concrete here.

| State | Storage | TTL |
|---|---|---|
| Global ledger, configuration, authorities | Instance | Extended on every operation |
| Position, pending action, LP request | Persistent | Extended on every touch |
| Market configuration and accounting | Persistent | Extended on every touch |
| Referral code owner, referrer map, balance | Persistent | Extended on every touch |

Nothing economic is stored as temporary. Every persistent entry must be
extendable permissionlessly, because a position whose owner has gone quiet
must still be liquidatable, and a referral balance must survive its owner's
inactivity. An implementation that lets an entry expire has destroyed a claim,
which no rule in §9 permits.

`state_version` is the migration guard:

```text
STATE_VERSION = the version this build understands

every operation:
    require ledger.state_version == STATE_VERSION
```

An upgrade that changes any stored layout ships with a migration that is the
only operation permitted to run against the previous version, and that
advances `state_version` when it completes. Until it has run, every other
entry point rejects. This is deliberately blunt: a half-migrated vault whose
aggregates no longer equal the sum of their records violates §5.11, and there
is no safe way to keep trading through that.

A migration that cannot complete in one transaction must be resumable and must
leave the vault rejecting operations until it finishes. It may not interleave
with settlement.

### 12.5 Error taxonomy

Errors are part of the interface. A caller that cannot distinguish "your price
bound was missed" from "the oracle has too few sources" cannot report anything
useful, and a front end that guesses will guess wrong.

Each contract owns a disjoint numeric range, assigned once and never reused:

| Range | Owner |
|---|---|
| `1–99` | Position manager |
| `100–199` | Vault |
| `200–299` | Oracle router |
| `300–399` | Configuration manager |

Two rules follow:

1. **A code is never reused across contracts.** Disjoint ranges make a raw
   code globally unambiguous without needing to know which contract produced
   it. Two contracts numbering from `1` would each own a code `9`, and a
   caller receiving it could not tell a rejected price bound from a loss of
   oracle quorum — two conditions with opposite remedies.
2. **A cross-contract error is wrapped, never passed through.** When the
   position manager calls the oracle router and the call fails, it returns its
   own error carrying the underlying one. Propagating the inner code unchanged
   is what makes a foreign code look native.

Codes are grouped by cause so a caller can react to a class without
enumerating every member:

| Class | Meaning for the caller |
|---|---|
| Authorization | The caller is not who this operation requires |
| Not found | The identifier does not exist or was already consumed |
| State | The vault or market is in a state that forbids this operation, including paused |
| Validation | The arguments are structurally invalid |
| Oracle | No qualifying price: stale, unavailable, or insufficiently corroborated |
| Accounting | An invariant from §9 would be violated; always a bug or corruption |
| Arithmetic | Overflow or a failed narrowing check from §2.1.1 |

The `Accounting` and `Arithmetic` classes must never be reachable through
ordinary use. If either can be triggered by a well-formed call, that is a
defect in this specification or its implementation, not a user error.

Expected terminal failures are not errors. Slippage, insufficient capacity, an
exposure cap, and a blocked market side all complete successfully and record
`Failed` with a reason (§8.9). They appear in results, not in error codes.

### 12.6 Emitted results

Every terminal outcome emits exactly one structured event. Events are the only
durable record of a terminal state — §5.6 removes the pending record and §5.14
removes the position — so an off-chain consumer that misses an event cannot
reconstruct it from state.

Every event carries a common envelope:

```text
EventHeader {
    event_version
    ledger_timestamp
    market_id            # absent for vault-wide events
    actor                # the caller credited with the action
}
```

The required events and the fields a consumer cannot do without:

| Event | Required fields |
|---|---|
| `ActionCommitted` | action id, kind, owner, payload, `execute_after`, `commit_observed_at`, escrow |
| `ActionSettled` | action id, kind, resulting position id, fill price, `fill_observed_at`, keeper reward |
| `ActionFailed` | action id, kind, reason, keeper reward paid, owner refund |
| `ActionCancelled` / `ActionExpired` | action id, refund, keeper reward |
| `PositionOpened` | position id, owner, direction, size, base exposure, collateral, entry price |
| `PositionChanged` | position id, size and base delta, realized payable PnL, every senior item collected, closing fee, resulting state |
| `PositionClosed` | position id, reason, payable PnL, profit the vault could not pay, senior items, closing fee, payout, unpaid amounts |
| `Liquidated` | position id, effective collateral, threshold, reward from position, reward from LP, unpaid reward, bad debt |
| `Deleveraged` | position id, side factor before and after, payout factor applied, payable PnL, reward |
| `FundingCheckpoint` | market, payer side per segment, index deltas, liability delta, EMA after |
| `BorrowCheckpoint` | index delta, rate applied, rate after |
| `RiskStateChanged` | market, side, previous state, next state, PnL factor |
| `RevenueDistributed` | source, collected amount, LP, protocol, and referral shares |
| `LpRequestCreated` / `LpRequestResolved` | request id, owner, kind, escrow, shares, assets, NAV used, reward |
| `ConfigurationProposed` / `ConfigurationApplied` | field, old value, new value, effective timestamp |

Three requirements make these usable rather than decorative:

- Amounts are emitted as **collected**, never as nominal. A waived closing fee
  and an uncollected borrow are reported as what they were, zero collected,
  with the waived amount separate. This mirrors §3.7: shares are calculated
  from what was collected, and an indexer that sums nominal fees will not
  reconcile against the ledger.
- Every event that changes cash ownership carries enough to reproduce the
  change. The sum of an event's parts equals the amount it moved, so §9.1 is
  checkable from the event stream alone.
- `event_version` is bumped whenever a field's meaning changes, never silently
  reused. Consumers pin the version they understand.

### 12.7 Oracle interface

Every price-sensitive rule in this specification rests on two things the
oracle must supply: an authenticated price, and an **observation stamp** that
says when the data behind that price was produced. §8.6 defines the stamp as
the oldest source timestamp contributing to the accepted aggregate, and the
entire fresh-price guarantee of §1.13 is a comparison between two such stamps.

The oracle aggregates a median across sources, rejecting any source that is
stale beyond `staleness_threshold`, future-dated, non-positive, or scaled at
other than seven decimals, then rejecting the whole aggregate if fewer than
`min_required_sources` survived or if the spread exceeds `max_deviation_bps`.
The stamp accompanies that median.

#### 12.7.1 Required read interface

```text
StampedPrice {
    price          # i128 at PRICE_PRECISION
    observed_at    # oldest contributing source timestamp
}

read_stamped_price(symbol)  -> StampedPrice
latest_round_id()           -> u64
get_round(round_id)         -> OracleRound
```

`read_stamped_price` is the single price primitive named throughout sections 7
and 8 — at commitment, at settlement, and on the forced paths alike. This
specification requires one property of it:

> Every call returns an aggregate computed from source data read during that
> call, together with a stamp naming the oldest source behind it. No call
> answers from a value retained by an earlier call.

How the oracle is built, and whether it caches internally for its other
consumers, is its own concern and not described here. What it may not do is
serve one of these reads from a retained value. Two rules in this
specification depend on that, and on nothing else about the oracle.

At commitment, a retained stamp is backdated by however long it was retained,
so `commit_observed_at` is older than the moment the trader committed. A fill
can then satisfy `fill_observed_at > commit_observed_at` against an observation
that predates the commitment — the test passing with no new information having
arrived, which is exactly what §1.13 exists to prevent. If anyone can cause a
value to be retained, whoever picks that moment picks which observation every
later commitment is measured against.

On the forced paths, §7.13 and §7.14 use one snapshot for both eligibility and
settlement, so a retained price is a price the caller chose. A keeper able to
pin a momentary adverse print can liquidate against it after the market has
recovered, closing a position that is currently healthy at a price that no
longer exists, and can latch a side into `HardCap` or admit an ADL the same
way. That these paths compare a price against a threshold rather than against
an earlier observation does not make a stale price safe; the caller's ability
to choose it is what does the damage.

`observed_at` is required on every read. Without it `commit_observed_at` and
`fill_observed_at` cannot be populated, and every rule built on them — the
fresh-price requirement, the `NotReady` outcome, the terminal first-attempt
semantics — has nothing to compare.

#### 12.7.2 Rounds

Synchronized rounds are a separate mechanism with a separate purpose: they
stamp every active market at one timestamp so LP accounting can mark the whole
vault consistently (§4.9). A round aggregates each active market from source data read at
publication time and records `id`, `timestamp`, `previous_id`,
`previous_timestamp`, and one price per symbol.

Round publication is permissioned where everything else in this protocol is
permissionless, which makes it a liveness dependency: if rounds stop, LP
deposits and withdrawals stop with them, while positions continue to trade and
liquidate normally on per-symbol prices.

Rounds are also why `max_active_markets` is bounded. A round iterates every
active market and aggregates each from scratch, so the registry bound of §5.1
is what keeps that operation inside a transaction budget.

#### 12.7.3 Failure behaviour, and what it costs

Every rejection is a panic, so the calling operation reverts:

| Condition | Meaning |
|---|---|
| No configured sources | The symbol was never set up |
| Every source stale, future-dated, or non-positive | Total feed outage |
| Fewer than `min_required_sources` valid | Quorum lost |
| Spread exceeds `max_deviation_bps` | Sources disagree |
| Median or deviation arithmetic overflows | A source returned an absurd value |

For trader actions a revert is the correct outcome and costs little: the
action stays pending, nothing is charged, and it settles when prices return.
The only difference from a `NotReady` return is that a revert emits nothing,
so an off-chain consumer sees an unexplained failed transaction rather than a
reason.

For forced actions the cost is real. Liquidation and ADL both need a price,
and both revert without one, so **during an oracle outage the protocol cannot
reduce risk while borrow and funding keep accruing** (§4.7). Positions that
should have been liquidated are liquidated later, at whatever price returns,
with the interim accrual still owed and LP equity absorbing whatever the
collateral no longer covers.

The deviation guard deserves specific attention. Sources disagreeing past
`max_deviation_bps` rejects the aggregate, and while the disagreement persists
the state is absorbing: every open and every close reverts together, and the
protocol cannot trade its way out. A guard that exists to stop the vault
pricing *new* risk badly must not also block the operations that *remove*
risk. This specification states the requirement and does not prescribe the
mechanism:

> A risk-reducing operation must not be blocked by a guard whose purpose is to
> protect risk-adding operations.

Whether that is met by a wider bound for liquidation, a documented fallback
aggregate, or an explicit degraded mode is an oracle-side decision. What is
not acceptable is one guard governing both directions.

#### 12.7.4 Configuration owned elsewhere

These values are enforced by the router, not by this protocol, and every
number in sections 2 and 10 assumes them:

| Value | Requirement |
|---|---|
| Source price decimals | Exactly `7`, matching `PRICE_PRECISION`; validated when sources are set |
| `min_required_sources` | At least `2` |
| `max_deviation_bps` | At most `10,000` |
| Source count | At most `16` |

`staleness_threshold` is the one to choose deliberately. It bounds how old the
data behind an accepted fill can be, so it is the real width of the window a
trader is committing against — the execution delay of §8.5 measures the wait,
and this measures the freshness. Setting it far above the oracle's publication
cadence widens that window silently, without any parameter in §10 changing.

### 12.8 Stated assumptions and residual risks

Some properties of this design are not guaranteed by any rule in it. They hold
because of how the surrounding world behaves, or they do not hold at all and
the exposure is accepted. Each entry names what is assumed, what happens if
the assumption fails, and what bounds the damage.

#### 12.8.1 There is a competing keeper

Almost every settlement in this protocol is performed by whoever cares to
perform it, paid a fixed reward. The design leans on that in one place where a
rule would otherwise be needed: whoever chooses *when* to settle also chooses
*which price* settles, and the protocol never names a mandatory observation.

The answer everywhere is the same. A keeper earns the same reward for
settling immediately and for settling late, and any keeper can take the
reward, so the first observation after `execute_after` is the one that pays.
The party with an interest in waiting does not control the timing, because
waiting hands the reward to someone else.

If no competing keeper exists, that reasoning fails and every trader-committed
action becomes a free option running until its deadline. What bounds it:
the entry lifetime caps of §8.5 — five minutes for a market entry, a week for
a limit one — and for position mutations the fact
that the holder already owns the position and gains no right by committing
(§8.5). What does not bound it is anything else, and a deployment with a
single keeper — or one operated only by the protocol itself — should treat
this as the assumption most worth monitoring.

The fixed `$0.25` reward is the whole incentive. It is not indexed to position
size, gas price, or urgency, so it is a bet that settling remains profitable
at the smallest position the vault admits. Liquidating a maximum-leverage
position near its threshold pays the same as settling a resting limit order.
If that stops being true, the failure is silent: keepers simply stop, and the
first visible symptom is unliquidated positions rather than an error.

#### 12.8.2 Fills happen at the oracle price, with no spread

A trader opens and closes at the aggregated oracle mid. There is no spread, no
price impact, and no size-dependent execution penalty, so a large position
costs the same per unit as a small one.

This is a deliberate simplification and it has a cost: the vault is the
counterparty to every trade at a price it did not quote. Adverse selection —
traders systematically taking the side that is about to be right — is not
priced at execution at all. It is priced afterwards, and only in aggregate, by
funding on directional imbalance (§3.4) and borrow on consumed capacity
(§3.3).

The execution delay and fresh-observation rule (§1.13) are what keep this from
being exploitable at the tick level, by ensuring a trader cannot commit after
seeing a move and settle against the price they already saw. They do not
address a trader who is simply better informed over minutes or hours. That
exposure sits with LPs and is compensated by fee and borrow revenue, not
eliminated.

#### 12.8.3 Funding is measured on base and charged on size

Skew, and therefore who pays, is derived from base exposure (§3.4.1). The
payer's obligation and the receiver's credit are both per unit of position
size (§3.4.4). The two bases differ whenever the sides entered at different
average prices.

The aggregate is unaffected: the receiver-backed portion can never exceed the
payer flow, and the split between receiver-backed and LP-backed is exact. What
shifts is distribution — the credit per unit of receiver size does not exactly
track the offsetting exposure that unit provides.

Using base for both would make the fee base move with entry price, so a
position's funding obligation would depend on where it opened rather than on
what it is worth. Using size for both would make skew insensitive to the
actual directional exposure the market carries. Each quantity is used where it
answers the right question, and the residual is a distribution effect between
receivers rather than a leak.

#### 12.8.4 The oracle answers, and the token behaves

Liquidation and ADL both require a price and both revert without one, while
borrow and funding keep accruing (§12.7.3). Nothing here bounds the length of
an outage or the loss it can produce; the mitigations are operational.

The collateral token is trusted not to freeze a participant (§12.1). A frozen
account cannot receive a payout, the transfer reverts, and the liquidation
reverts with it.

#### 12.8.5 What is genuinely guaranteed

For contrast, these hold regardless of keeper behaviour, oracle availability,
or market conditions, because they are properties of the arithmetic:

- cash ownership conservation (§9.1) — labels divide physical cash, never
  create it;
- fee distribution conservation (§9.2) — only collected amounts are
  distributed, and the split sums exactly;
- index monotonicity (§9.5) — no cumulative index decreases, and a negative
  pending amount reverts rather than being clamped;
- no retroactive repricing (§9.6) — elapsed time is always charged at the rate
  that was in force;
- exposure aggregate correctness (§9.7) — every aggregate equals the sum of
  its records after each transition;
- single settlement (§9.14) — one action reaches a terminal state once and
  pays one reward; and
- atomic reversion (§9.16) — an unexpected failure changes nothing.

A reader deciding how much to rely on a given behaviour should check which of
these two lists it belongs to.
