# Fee and Order Mechanics Working Decision Note

Date: 2026-08-30  
Status: Working decisions for review  
Scope: Fee mechanics and order-execution decisions

This note records the decisions made during the implementation-planning
session. It is intentionally separate from the canonical design documents
and implementation backlog. Once the remaining questions are resolved, these
decisions will be worked into those documents before implementation.

No runtime behavior has been changed by this note.

## 1. Design principles

- Charge for risk borne by the vault, not merely for trader activity.
- Keep the fee model simple enough to explain directly from its formulas.
- Opening and closing fees do not depend on market skew.
- Payer obligations round up and trader credits round down.
- Borrow and funding remain separate obligations from the closing fee.

## 2. Funding remains unchanged

Funding continues to price and rebalance directional exposure.

```text
receiver_share = min(1, receiver_base / payer_base)
lp_share       = 1 - receiver_share
```

- The exposure-matched portion goes to traders on the receiving side.
- The unmatched portion goes to LP residual equity.
- No funding is allocated to protocol revenue or the keeper reserve.
- The recipient split is exposure-driven and is not configurable.
- Existing funding-rate parameters remain configurable.

## 3. Opening fee

Add a single configurable opening-fee rate without skew tiers.

```text
open_fee_bps
opening_fee = ceil(added_size * open_fee_bps / BPS)
```

The deployment default is zero:

```text
open_fee_bps = 0
```

Therefore adding the field does not initially change trader cost. The fee is
based on position size: an initial open charges it on the initial size and an
increase charges it only on the added size. An increase is therefore treated
the same as opening that additional exposure.

The opening fee is deducted from the collateral amount submitted by the
trader rather than transferred on top of it:

```text
net_collateral_added = submitted_collateral - opening_fee
```

The position's minimum-collateral and margin checks use the net collateral
after this deduction. The UI must show both the fee and the resulting usable
position collateral.

There is no separate leverage multiplier. For the same collateral, higher
leverage already creates a larger position size and therefore a larger
size-based opening fee and borrow obligation. For the same position size,
leverage does not change the vault's directional exposure. Additional risk
from liquidation timing is handled through margin and liquidation rules
rather than an opening-fee surcharge.

## 4. Fee revenue distribution

Keeper compensation is not a share of fee revenue. The existing 5% keeper
revenue share moves to the protocol.

Opening and closing fees use the same configurable distribution:

```text
LP share        = 90%
protocol share  = 10% without a referrer
referral share  = 2.5% when a referrer exists
protocol share  = 7.5% when a referrer exists
```

The referral share applies to both opening and closing fees. It is carved
entirely from the protocol share, so it does not dilute LP revenue.

Borrow uses a separate configurable distribution because LPs are the risk
counterparty:

```text
LP share        = 90%
protocol share  = 10%
keeper share    = 0%
referral share  = 0%
```

Funding distribution remains separate and unchanged as described in §2.

## 5. Minimum borrow period

Replace the opaque minimum borrow index delta with an explicit configurable
duration:

```text
min_borrow_fee_seconds
```

The currently discussed deployment default is:

```text
min_borrow_fee_seconds = 900  # 15 minutes
```

The minimum is quoted as a monetary value for the position's full exposure
when a new borrow window begins, using the borrow rate at that time:

```text
minimum_borrow_fee = ceil(
    resulting_risk_units
    * borrow_rate_at_reset
    * min_borrow_fee_seconds
    / (BPS * SECONDS_PER_DAY)
)
```

The position stores one monetary minimum and one borrow baseline. It does not
store exposure tranches or proportional accrued-borrow carry.

```text
pending_borrow = max(
    actual_borrow_since_last_reset,
    stored_minimum_borrow_fee
)
```

Every increase or decrease closes the current borrow window for the entire
position:

```text
1. Accrue the current borrow index.
2. Deduct pending_borrow from the position's existing stored collateral.
3. Apply the size increase or decrease.
4. Refresh the utilization-based borrow rate using the resulting book.
5. Reset the position's borrow baseline to the current global index.
6. Quote and store a fresh minimum_borrow_fee for the full resulting exposure
   using the refreshed post-mutation borrow rate.
```

This settles the previous window completely and starts actual borrow accrual
from zero again. An initial open starts the first window. A full close,
liquidation, or ADL settles the current window but does not create another
one because no position remains.

The minimum remains a floor, not an additional fee: actual accrued borrow
satisfies the minimum rather than being added on top of it. Each increase or
decrease deliberately starts a new minimum-fee window for the complete
remaining position.

Liquidation health must include the same pending obligation:

```text
pending_borrow = max(actual_borrow_since_last_reset, stored_minimum_borrow_fee)

effective_collateral =
    stored_collateral
  + payable_pnl
  + funding_received
  - funding_owed_to_receivers
  - funding_owed_to_lps
  - pending_borrow
```

This makes the position liquidatable before a normal touch discovers that
its existing collateral cannot cover borrow settlement. A voluntary increase
or decrease of a liquidatable position is rejected and a keeper must execute
liquidation. A price gap can still make the position insolvent; the previously
agreed LP shortfall backstop applies.

A collateral-only addition does not settle borrow, reset the borrow baseline,
or quote a new minimum. It only increases stored collateral and then runs the
applicable collateral and health checks. Only a position-size mutation starts
a fresh borrow window.

## 6. Closing fee

The closing fee has two independently configurable components and no skew
tier:

```text
size_fee = ceil(size_removed * close_size_fee_bps / BPS)
pnl_fee  = ceil(payable_price_pnl * close_pnl_fee_bps / BPS)

close_size_fee_bps = 5       # deployment default: 0.05% of size removed
close_pnl_fee_bps  = 1000    # deployment default: 10% of payable realized PnL
```

Only payable positive price PnL participates. `payable_price_pnl` means price
PnL after any hard-cap payout factor and LP-equity clamp. Funding credits and
stored collateral do not make a losing trade eligible for a closing fee.

The target closing fee is the larger component:

```text
target_closing_fee = max(size_fee, pnl_fee)
```

The fee charged to a winning close is capped at its payable price PnL:

```text
if payable_price_pnl <= 0:
    closing_fee = 0
else:
    closing_fee = min(payable_price_pnl, target_closing_fee)
```

Therefore the closing fee alone can reduce price profit to zero but can never
turn positive price PnL into negative price PnL.

The normal closing fee applies to:

- voluntary full closes;
- partial decreases, using the removed size and realized payable price PnL;
- take-profit execution; and
- stop-loss execution.

Liquidation and ADL do not pay a protocol closing fee. Their keeper rewards
and all accrued borrow and funding obligations remain separate settlement
items.

### 6.1 Examples

Assume the size-based fee is `$50` and the PnL-share fee is smaller than `$50`
in the first examples:

| Payable price PnL | Target fee | Closing fee | Price profit after closing fee |
|---:|---:|---:|---:|
| `-$25` | n/a | `$0` | `-$25` |
| `$0` | `$50` | `$0` | `$0` |
| `$30` | `$50` | `$30` | `$0` |
| `$50` | `$50` | `$50` | `$0` |
| `$60` | `$50` | `$50` | `$10` |

For a sufficiently large win, the PnL-share component can exceed the
size-based component. For example, with the 10% PnL rate:

```text
payable_price_pnl = $2,000
size_fee          = $50
pnl_fee           = $200
closing_fee       = max($50, $200) = $200
price profit left = $1,800
```

The trader's price-profit result is continuous and monotonic: earning one
additional unit of payable PnL never reduces the amount left after the
closing fee.

### 6.2 Relationship to borrow and funding

Borrow and funding are independent of price PnL and are still settled on
every applicable close or decrease:

```text
trader_result =
      stored_collateral
    + payable_price_pnl
    + funding_received
    - funding_paid_to_receivers
    - funding_paid_to_lps
    - borrow_fee
    - keeper_reward
    - closing_fee
```

For negative price PnL:

- `closing_fee` is zero;
- the negative PnL is deducted from collateral;
- accrued borrow and net funding obligations are still collected.

For positive price PnL below the target closing fee:

- the closing fee may consume the payable price PnL remaining after senior
  settlement items;
- price profit after the closing fee is zero;
- accrued borrow and net funding obligations are still collected separately.

The closing fee is subordinate to funding, borrow, and the applicable fixed
keeper reward. It may only consume net profit remaining from the current
settlement and must never consume the trader's original collateral:

```text
nominal_closing_fee = min(
    payable_price_pnl,
    max(size_fee, pnl_fee)
)

profit_after_senior_items = max(
    0,
      payable_price_pnl
    + funding_received
    - funding_paid_to_receivers
    - funding_paid_to_lps
    - borrow_fee
    - keeper_reward
)

closing_fee = min(
    nominal_closing_fee,
    profit_after_senior_items
)
```

For example, if the nominal closing fee is `$50` but only `$20` of settlement
profit remains after funding, borrow, and the keeper reward, the protocol
collects `$20`. If no settlement profit remains, the closing fee is zero even
when the position still contains original collateral. An uncollected nominal
closing fee is waived and never recorded as bad debt. Fee revenue and referral
shares are calculated from the amount actually collected.

## 7. Keeper execution rewards

Keeper compensation is a direct execution cost charged to the position, not
protocol revenue and not a percentage of protocol fees. Each keeper action
has an independently configurable fixed reward so governance can tune them
separately:

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
```

All of these initially use the same deployment default:

```text
2_500_000  # $0.25 at 1e7 precision
```

An execution selects the reward for that action; rewards are not stacked just
because an action was initiated by an order. Equal defaults make every action
equally attractive to a keeper while preserving the ability to tune them
individually later.

Payment source:

- An executed open or entry order deducts its reward from submitted escrowed
  collateral before creating the position.
- An executed increase, decrease, voluntary close, TP, or SL deducts its
  reward directly from stored position collateral.
- ADL deducts its fixed reward from the trader's payable profit or stored
  collateral.
- Liquidation deducts its fixed reward from remaining position collateral.
  LP equity guarantees any shortfall caused by a price gap.

Minimum-collateral and post-action health checks use collateral after the
applicable keeper reward has been deducted.

### 7.1 Liquidation threshold

The existing global `min_collateral` remains configurable and must be greater
than the configured liquidation keeper reward. Its deployment default must
therefore be reviewed alongside the new reward; the current `$1` minimum is
greater than the initial `$0.25` reward.

Liquidation eligibility preserves enough collateral for the fixed reward in
ordinary market movement:

```text
effective_collateral =
      stored_collateral
    + payable_pnl
    + funding_received
    - funding_paid_to_receivers
    - funding_paid_to_lps
    - borrow_fee

liquidation_threshold = max(
    maintenance_margin(position_size),
    keeper_liquidation_reward
)

liquidatable = effective_collateral <= liquidation_threshold
```

A violent price gap can jump through this threshold. In that case the keeper
still receives the full fixed reward and LP equity covers the shortfall. This
is an accepted vault risk cost of guaranteeing liquidation execution.

### 7.2 Remove user-selected execution budgets

The user-selected execution budget and its separate claim bucket are removed.
Positions and orders no longer store a trader-chosen budget, and there are no
budget funding, withdrawal, spending, or refund operations. Keeper rewards
are automatic configured amounts deducted from position collateral as above.

## 8. Delayed market and limit entry

Every market and limit entry uses a commit-then-settle lifecycle. Immediate
same-call position creation is not allowed.

Each market has its own configurable execution delay:

```text
order_execution_delay_seconds
execute_after = created_at + order_execution_delay_seconds
```

Settlement requires both conditions:

```text
now >= execute_after
fill_observed_at > commit_observed_at
```

The fill price must therefore come from an oracle observation created strictly
after the order commitment. Elapsed time alone is not sufficient: if the
oracle has not produced a newer qualifying observation, the order cannot
settle even after its configured delay has elapsed.

This rule applies equally to market and limit entries. The per-market delay
uses these validation bounds and deployment default:

```text
minimum = 1 second
default = 5 seconds
maximum = 30 seconds
```

### 8.1 Market-order terminal behavior

A committed market order is binding until its expiry. The owner cannot cancel
it while it is pending.

Its first eligible execution attempt is terminal. Once the configured delay
has elapsed and a qualifying post-commit oracle observation exists, that
attempt either fills the order successfully or ends it as failed. A failed
attempt must not leave the market order pending for a later price or oracle
observation.

The exact escrow cleanup and refund behavior for each terminal outcome remains
to be specified separately.

### 8.2 Limit-order pending and cancellation behavior

A limit order remains pending while its trigger price has not been crossed.
An unsuccessful price check does not consume or terminate it.

The owner may cancel a pending limit order at any time before it executes.
The exact cancellation cleanup and refund behavior remains to be specified
with the other terminal order outcomes.

### 8.3 Expiry boundary

Market and limit orders use the same exact expiry boundary:

```text
executable = now < expires_at
expired    = now >= expires_at
```

An order cannot execute at its expiry timestamp.

### 8.4 Expired-order cleanup

Expired market and limit orders may be cleaned up permissionlessly by a
keeper. A successful expiry cleanup pays the configured fixed
`keeper_expiry_reward`. This reward is independently configurable and starts
with the same `$0.25` deployment default as the other keeper actions.

The reward is deducted from collateral already escrowed with the order. It
must not require a new transfer from an absent owner. The remaining escrow is
refunded according to the terminal cleanup rules.

### 8.5 Entry-order collateral escrow

Creating a market or limit entry order transfers the trader's full submitted
collateral into order escrow immediately. The escrow guarantees that the funds
needed for later execution and keeper compensation are already committed.

Escrowed collateral is not position collateral and does not enter the active
position book before successful execution. A terminal order outcome must
either convert the applicable escrow into position collateral and charges or
distribute/refund it according to that outcome's cleanup rules.

### 8.6 Failed execution caused by slippage

Because the trader transfers the full submitted collateral at commitment,
execution cannot fail because the trader is unavailable or cannot fund the
order. Order creation must reject collateral that is already insufficient for
the committed order under the creation-time checks.

If the first eligible execution attempt fails its slippage constraint, the
attempt is terminal. The keeper receives the configured reward for the
attempted action from order escrow, and all remaining escrow is refunded to
the trader. No position is created and no opening fee is charged.

### 8.7 Vault capacity at execution

Entry orders do not reserve vault risk capacity at commitment. Capacity is
rechecked during the eligible execution attempt using the current vault
state. Pending orders therefore cannot lock capacity.

If current capacity is insufficient, the attempt is terminal. The keeper
receives the configured reward for the attempted action from order escrow,
all remaining escrow is refunded to the trader, no position is created, and
no opening fee is charged.

### 8.8 Owner cancellation distribution

When the owner cancels a pending limit order, the entire escrow is refunded
to the owner. No opening fee or keeper reward is charged. Cancellation creates
no position and terminates the order.

### 8.9 Successful entry distribution

A successful market or limit entry execution distributes order escrow in this
order:

```text
initial_position_collateral =
    escrowed_collateral
    - opening_fee
    - keeper_action_reward
```

The applicable action reward is selected without stacking rewards. The
remainder becomes the new position's stored collateral. Minimum-collateral,
initial-margin, and health validation use this post-fee, post-reward position
collateral.

### 8.10 Expected failures versus reverts

An eligible attempt that fails an expected execution condition, including
slippage, unavailable vault capacity, or another deterministic order
validation, is handled as a successful terminal settlement transaction:

```text
mark order failed
pay keeper action reward from escrow
refund remaining escrow
charge no opening fee
create no position
return success
```

This path must not revert, because a revert would also undo the terminal order
state, keeper payment, and refund. A missing qualifying post-commit oracle
observation means the order is not yet eligible; it remains pending and no
reward is paid. Unexpected invariant violations or program errors revert
atomically, leave the order unchanged, and pay nothing.

## 9. No batch settlement in the initial implementation

The initial implementation settles exactly one action per contract call. It
does not accept an array of order IDs or mix opens, closes, increases,
decreases, liquidations, or ADL actions in one settlement call.

This removes batch-size configuration, duplicate-input behavior, cross-action
ordering rules, mixed-result handling, and batch resource accounting from the
current scope. Keepers submit separate transactions for separate actions.

A generic mixed-action batch function may be added later as an optimization.
It is not part of the current implementation plan or interface.
