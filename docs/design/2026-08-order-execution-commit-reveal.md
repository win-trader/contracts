# Order Execution: Commit and Reveal

## 1. Purpose

This document uses ASD-STE100 Simplified Technical English.

This document specifies how a position action selects its execution price.
It replaces the direct-execution model for trader-initiated actions.
It does not change fee, funding, borrow, or LP share mechanics.

The current model lets a trader select a price that the trader has already
seen. This document removes that ability.

Section references with the form §N.N point to
`2026-07-fee-vault-contract-mechanics-ste100.md`, which stays the canonical
specification for all accounting.

## 2. Technical terms

| Term | Meaning |
|---|---|
| commit | The first phase of an order. The trader records intent and escrows cash. |
| reveal | The second phase of an order. A caller supplies a price the trader could not have seen at commit. |
| observation stamp | The oldest source update time that contributes to a median price. |
| freshness gate | The requirement that the reveal observation stamp is later than the commit observation stamp. |
| fill window | The bounded interval of observation stamps that a reveal can use. |
| resting order | An order that survives a failed fill attempt. |
| terminal order | An order that a failed fill attempt removes. |
| execution reward | The cash a caller receives for a successful reveal. |
| executor | Any account that calls a reveal entry point. |
| latency arbitrage | A trade that profits from the difference between a public market price and a stale protocol price. |

The word `must` identifies a requirement.
The word `can` identifies an ability.

## 3. The problem

### 3.1 The exposure

The protocol prices every position action from the OracleRouter cache.
That cache follows the public market with a delay.
The delay has these components:

| Component | Current value |
|---|---|
| Publisher poll interval | 1 s |
| Publisher confirming samples | 2 samples |
| Minimum interval between pushes | 500 ms |
| Force publish after | 20 s |
| Stellar ledger close | about 5 s |

The delay is about 5 s to 10 s in normal conditions.
The delay is longer when the network is congested.
The delay is longer when the confirming-samples gate holds a genuine move.

A trader who reads a public exchange can see a price move before the
protocol sees it. The trader can open a position at the stale price and
close the position after the protocol adopts the new price. The profit
comes from the vault. The LP pays it.

### 3.2 Why the fee schedule does not stop it

The protocol charges no fee for an open or increase action (§11.1).
The protocol charges a closing fee only out of realized positive price PnL
(§11.1). A close without realized profit pays zero.

Therefore the cost of a latency arbitrage trade is a small share of the
arbitrage profit. The trade keeps the complement share. A failed attempt
costs only borrow and funding for the holding period.

The fee schedule cannot solve this problem.
A fee large enough to deter the arbitrage would tax all honest flow.

### 3.3 Why a single transaction cannot solve it

The chain cannot distinguish a trader who has seen the current price from a
trader who has not. Soroban exposes no transaction submission time. The
earliest observable time is the ledger that executes the transaction, which
is later than the moment the trader decided to send it.

Intra-ledger transaction order is not controllable. A rule that requires a
same-ledger observation does not work, because the trade can execute before
the oracle push in the same ledger and still read the stale value.

Therefore the action must use two phases.

## 4. Principle

A trader must commit before the protocol reveals the price that fills the
commitment.

The design has three requirements. All three are necessary. A design that
satisfies only two is worse than the current model, because it gives the
trader a free option instead of a stale price.

1. **Binding commitment.** The trader must not be able to withdraw the
   commitment after the reveal.
2. **Independent reveal.** The fill price must carry information that did
   not exist at commit.
3. **Terminal failure.** A market order that cannot fill must not rest and
   retry.

Requirement 3 is the least obvious. An order that retries after a failed
fill gives the trader repeated attempts and lets the trader keep only the
favourable outcome. That reproduces the original arbitrage through the
slippage bound.

## 5. The freshness cursor

### 5.1 Stamped price

The OracleRouter must expose the observation stamp with the price.

```text
StampedPrice {
    price:       i128,   // median, scaled by PRICE_PRECISION
    observed_at: u64,    // oldest source update in the median
}
```

Add this entry point to the OracleRouter interface:

```text
get_price_stamped(env, symbol) -> StampedPrice
```

The stored cache entry already holds both fields. `CachedPrice` records
`price`, `fetched_at`, and `oldest_source_update`. The entry point exposes
data that already exists.

### 5.2 The stamp must be the oldest source update

Use `oldest_source_update` as `observed_at`.
Do not use `fetched_at`.

`fetched_at` records when the router aggregated the median. `get_price`
requires no authorization and writes the cache. Therefore an attacker who
commits at time `T` can immediately call `get_price` and write
`fetched_at = T + 1` onto a median built from source data observed at
`T − 8`. A gate on `fetched_at` accepts that price. The price carries only
pre-commit information. The gate fails to protect.

`oldest_source_update` comes from each source contract's own `last_update`.
The router rejects future-dated source timestamps. Therefore an attacker
cannot advance this value.

The stamp is conservative by construction. The median can be dominated by
the oldest contributing source, so the oldest source update is the correct
lower bound on the information age of the median.

### 5.3 The reveal must bypass the cache

`fetch_and_validate_price` can return a cache hit whose `observed_at` is
older than the commit stamp. That would fail the freshness gate although
fresh source data exists.

The reveal path must use the uncached aggregation path.
`fetch_fresh_price` already provides this behaviour for canonical rounds
(§13.1) and must be reused.

The reveal therefore costs one cross-contract call for each configured
source. This is the same cost the round publication path already pays.

### 5.4 Publisher cadence

The gate requires every accepted source to publish after the commit.

`ORACLE_FORCE_PUBLISH_AFTER_MS` must decrease from 20 s to 5 s. Otherwise a
quiet market delays a fill by up to 20 s.

This change stays inside the existing publisher limits.
`ORACLE_MAX_PUSHES_PER_MINUTE` is 60.
`ORACLE_MIN_INTERVAL_BETWEEN_PUSHES_MS` is 500.

The resulting latency profile is correct. A volatile market publishes on
deviation and fills in about 1 s. A quiet market fills in up to 5 s, and a
quiet market carries little arbitrage value.

### 5.5 Rejected alternative: a fixed time delay

Do not gate on elapsed wall-clock time.

A wall-clock delay does not prove that a new observation exists. If the
publisher stops, the delay expires and the reveal uses the same stale price
the trader saw. The delay is satisfied and the protection is absent. The
publisher has stopped in production before, through the outlier-guard
absorbing state.

A delay calibrated to average oracle latency also protects the wrong case.
Arbitrage value is highest when oracle latency is highest. An
average-calibrated delay is shortest relative to the exposure exactly when
the exposure is largest.

The observation stamp is the correct primitive. Time appears only as expiry.

### 5.6 Rejected alternative: round identifiers

`latest_round_id` is monotonic and `publish_round` uses `fetch_fresh_price`.
A gate on `round_id > order.commit_round` is correct and simpler to reason
about.

It is rejected on cost. `ROUND_PUBLISH_INTERVAL_SEC` is 30. A usable fill
latency requires about 5 s, which is about 17,000 additional multi-market
round transactions each day. The stamped-price gate requires no additional
keeper transactions.

## 6. Unified order model

### 6.1 One order, three kinds

All trader-initiated position actions must use one order record and one
reveal entry point.

`place_entry_order` already rejects `trigger_price <= 0`. Therefore zero is
available as the sentinel for "no trigger predicate". This matches the
existing convention where `acceptable_price == 0` means "no bound".

| Kind | `trigger_price` | Predicate | Failure |
|---|---|---|---|
| Market | 0 | none | terminal |
| Limit | > 0, on the far side | `price` crosses down to trigger | resting |
| Stop | > 0, on the near side | `price` crosses up to trigger | resting |

`trigger_above` already distinguishes limit from stop. No new field is
needed for the kind.

### 6.2 The trigger predicate becomes conditional

```text
if order.trigger_price > 0:
    if order.trigger_above:
        triggered = price >= order.trigger_price
    else:
        triggered = price <= order.trigger_price
    if not triggered:
        revert; the order stays pending
```

### 6.3 One gate covers both kinds

The freshness gate applies to every order kind.

For a genuinely resting order the gate is a no-op, because
`order.observed_at` is minutes or hours old and any current observation is
later. The gate binds only on an order committed moments earlier.

Therefore a limit order placed at the current price — a market order in
disguise — is caught by the same gate that catches an explicit market
order. No special case is required.

### 6.4 Terminal and resting failure

```text
terminal_on_miss(order) = (order.trigger_price == 0)
```

A resting order must survive a failed fill attempt.
A terminal order must not.

For a terminal order, every exit path except "the freshness gate is not yet
satisfied" must remove the order, return the collateral, and pay the
execution reward to the executor. This includes the slippage breach, the
risk-capacity breach, the market-side limit breach, the margin failure, and
the market-disabled condition.

The current `execute_entry_order` reverts and leaves the order pending on
all of these. That behaviour is correct for a resting order and unsafe for
a terminal order.

## 7. Order lifecycle

### 7.1 Commit

```text
place_order(env, owner, market, params) -> order_id
```

Use this procedure:

1. Authenticate the owner.
2. Check that the market is active.
3. Validate size, collateral, and execution budget.
4. Validate the trigger price for a resting kind.
5. Validate the attached take-profit and stop-loss.
6. Check the initial collateral requirement against notional.
7. **Escrow collateral and execution budget into the vault.**
8. Read `get_price_stamped` and record `observed_at` and the ledger sequence.
9. Record `expires_at`.
10. Save the order and emit the placed event.

The escrowed collateral is not stored position collateral. It is a new
non-LP claim with its own aggregate, and it must not enter LP cash equity.
It converts to stored position collateral only at fill.

### 7.2 Reveal

```text
execute_order(env, caller, order_id)
```

Use this procedure:

1. Load the order.
2. If the order has expired, remove it and settle as in §7.3. Commit.
3. Check that the market is active. A disabled market is terminal for a
   market order and transient for a resting order.
4. Checkpoint global state and the market.
5. Read `get_price_stamped` on the uncached path.
6. **Apply the freshness gate.** If it fails, revert. The order stays
   pending for every kind.
7. **Apply the fill window.** If the stamp is later than the window,
   revert. The order stays pending.
8. Apply the trigger predicate for a resting kind.
9. Apply the slippage bound.
10. Convert escrow to stored collateral and open through the existing core.
11. Pay the execution reward to the caller from the execution budget.
12. Remove the order and emit the filled event.

Step 6 is the mechanism. Steps 9 through 12 are unchanged from the current
entry-order path except for the escrow conversion.

The freshness gate is:

```text
observed_at > order.observed_at
    and current_ledger_sequence > order.commit_ledger
```

The ledger comparison is secondary protection. A source push can land in the
same ledger as the commit and advance the stamp within one ledger. The
comparison costs one integer test.

### 7.3 Expiry

An expired order must be removable by any caller.

On expiry:

- Return the escrowed collateral to the owner.
- Pay the execution reward to the caller from the execution budget.
- Return the remaining execution budget to the owner.
- Emit the cancelled event with the expired reason.

The reward on expiry prevents abandoned escrow. Without it, no account has
an incentive to clear an order that cannot fill.

### 7.4 Owner cancellation

The owner must not cancel an order while the order is fillable.

An owner who can cancel after seeing the reveal price holds a free option.
That option is worth more than the stale-price arbitrage it replaces. This
is the single most important rule in this document.

Therefore:

- A market order must not accept owner cancellation before `expires_at`.
- A resting order can accept owner cancellation at any time, because a
  resting order is already an option the protocol has chosen to write.
- After `expires_at`, any account can sweep any order under §7.3.

### 7.5 Failure matrix

| Condition | Market order | Resting order |
|---|---|---|
| Freshness gate not satisfied | revert, stays pending | revert, stays pending |
| Fill stamp later than fill window | revert, stays pending | revert, stays pending |
| Trigger not crossed | not applicable | revert, stays pending |
| Slippage bound breached | terminal, refund | revert, stays pending |
| Risk capacity or side limit breached | terminal, refund | revert, stays pending |
| Margin requirement not satisfied | terminal, refund | revert, stays pending |
| Market disabled | terminal, refund | revert, stays pending |
| Expired | terminal, refund | terminal, refund |
| Owner cancels | rejected before expiry | permitted |

Every terminal row pays the execution reward to the caller and returns the
collateral to the owner.

## 8. Escrow

### 8.1 Allowance pull is not a commitment

`execute_entry_order` currently pulls collateral at fill through
`ledger::receive_via_allowance`. A failed pull removes the order with the
`PullFailed` reason and no penalty.

Therefore the owner can revoke the token allowance, or move the balance,
after seeing the reveal price. The fill fails and the owner pays nothing.
This is cancellation under a different name, at zero cost.

The allowance model is safe for a resting order only because a resting order
is already an option. It is not safe for a market order.

### 8.2 Requirement

Collateral and execution budget must move into the vault at commit.

This is a change to the shipped entry-order path and must be applied
regardless of whether the rest of this document is adopted, because the
existing entry order already carries the free-cancellation property through
allowance revocation.

### 8.3 Accounting

The escrow aggregate must satisfy the cash-transition rules (§6).

```text
order_escrow_total     // new global aggregate, a non-LP claim
```

Add it to `non_lp_claims`. Marked NAV nets it out. It must never appear in
LP cash equity while an order is pending.

## 9. Execution and keepers

### 9.1 Execution is permissionless

`execute_order` must accept any authenticated caller. This is already true
of `execute_entry_order`.

Permissionless execution is what prevents the two-phase model from making
the keeper a liveness dependency for basic trading. A trader who cannot wait
for a third-party keeper can submit the reveal transaction for the trader's
own order one ledger later.

The application must expose this path. When the keeper lags, the trader
must be able to fill the trader's own order from the interface.

### 9.2 The fill window

An executor chooses when to submit the reveal. Within a wide interval that
choice is a price selection, which is a residual extractable value surface.
A hostile executor can wait for a stamp that breaches the trader's slippage
bound and thereby force a terminal cancellation.

Bracket the reveal:

```text
order.observed_at < observed_at <= order.observed_at + max_fill_lag
```

A reveal outside the bracket reverts and does not consume the order.

The bracket cannot prove that the executor used the first available
observation. It bounds how far the executor can search. `max_fill_lag`
should be a small multiple of the publisher cadence.

A bracket that is too tight makes orders expire when no executor is
available. That failure is safe, because expiry refunds, and the trader can
always self-execute inside the bracket.

### 9.3 Execution reward

The reward must be a configured fixed amount, not a share of notional.

```text
execution_reward_amount    // GlobalConfig, in vault token units
```

The reward must exceed the Stellar transaction fee and the operating cost
of a scanner. If it does not, no independent operator runs one and the
protocol keeper becomes the only executor in practice.

The reward is paid from the order's execution budget on every terminal path
and on a successful fill. Remaining budget rolls into the opened position's
`execution_budget` for later triggered-order and close execution (§12.4).

### 9.4 Independent operators

An independent operator requires:

- A funded Stellar account.
- A Soroban RPC endpoint.
- The scanner that lists pending orders and applies the freshness gate.

The decision logic is already pure and reusable. `KeeperWorld` and the
Decision Kernel in `offchain/packages/keeper` take a state snapshot and
return work as plain data, with no signing and no chain access. Publishing
the kernel lets an operator run a scanner without reimplementing protocol
rules.

Independent operators must be documented as unprivileged. An operator holds
no role in the ConfigManager and can perform no action that a trader could
not perform.

### 9.5 Accepted surfaces

These properties are accepted, not solved:

- **Wasted work.** Several executors can submit a reveal for one order. One
  succeeds and the others revert and pay the transaction fee. No on-chain
  coordination is proposed.
- **Reward racing.** The reward is fixed, so an executor with lower latency
  wins more often. This is the intended incentive.
- **Bracket selection.** An executor selects a stamp inside the bracket.
  §9.2 bounds the value of that selection but does not remove it.

## 10. Funding the execution reward

The protocol charges no opening fee and must not introduce one. Therefore
the reward cannot be funded from an open action.

### 10.1 Selected model: trader-posted refundable budget

The execution budget (§12.4) is already the correct primitive. It is a
deposit, not a fee. It is supplied separately from collateral, it is
recorded per position, it is tracked by `execution_budget_total`, and
`withdraw_execution_budget` already returns the unused remainder.

Extend it to cover the commit phase:

- The trader posts the budget at commit, together with collateral.
- One `execution_reward_amount` leaves the budget on the terminal or filled
  path.
- The remainder becomes the opened position's execution budget.
- On expiry or cancellation, the remainder returns to the owner.

A trader who never uses a keeper and self-executes recovers the whole
budget minus the reward the trader paid to itself. The net cost of the
model to an honest self-executing trader is the Stellar transaction fee.

This preserves the property that a losing trade pays no protocol fee.

### 10.2 Rejected: fund from the closing fee

The closing fee is charged only on realized positive price PnL (§11.1). A
losing trade pays nothing. Therefore the protocol would pay for the
execution of trades that generate no revenue, and the cost would be
socialized across winning traders. The cost is also unbounded in the number
of orders placed, which creates a spam surface with no deposit to consume.

### 10.3 Rejected: fund from LP cash

The LP is the party this document protects. Charging the LP for the
protection inverts the incentive and adds variance to LP yield that is
unrelated to trading performance.

### 10.4 Open question

The reward on the expiry path (§7.3) is paid to whoever sweeps the order.
Whether a market order that expires unfilled should also forfeit part of the
budget as a spam deterrent is not decided. The argument for forfeiture is
that a free-to-place, free-to-expire order is a cheap way to occupy state.
The argument against is that expiry is usually the protocol's fault, not the
trader's, because it means no fresh observation arrived inside the fill
window.

Resolve this before implementation.

## 11. Interaction with existing mechanisms

### 11.1 Liquidation and ADL

Liquidation and ADL are protocol-initiated and must stay direct. They must
not use the two-phase model. The trader has no price selection in either
path, so no arbitrage exists to remove, and a delay would create bad debt.

A liquidation can remove a position that has a pending decrease order. The
orphaned order must terminate cleanly, return the execution budget, and must
not panic on a missing position.

### 11.2 Triggered orders

Take-profit and stop-loss orders already execute against the current price
and already carry the freshness properties of a resting order. Setting a
trigger moves no cash, so `set_tp_sl` stays direct.

### 11.3 Closes and decreases

The two-phase model must cover the close and decrease paths. The reverse
attack is symmetric: a trader can exit a losing position at a stale
favourable price.

Both GMX v2 and Synthetix delay closes for this reason.

A close order escrows no collateral. It escrows only the execution budget,
which the position already holds.

### 11.4 LP settlement rounds

No change. LP settlement uses canonical rounds (§13.1) and the delayed
request cutoff (§13.2). Those paths already require a fresh uncached price.

### 11.5 Competition round deadline

A pending order at the round deadline must expire and refund. The winner
measure is realized plus unrealized value at the deadline, and an unfilled
order changes neither. Filling an order after the deadline would let a
trader select a post-deadline price for a pre-deadline commitment.

## 12. Configuration

Add to `GlobalConfig`:

| Parameter | Meaning | Suggested |
|---|---|---|
| `execution_reward_amount` | Fixed reward for a reveal or sweep | above the Stellar fee |
| `max_fill_lag` | Width of the fill window, seconds | small multiple of publisher cadence |
| `order_expiry_seconds` | Commit lifetime | about 15 s, three ledgers |

Change in the oracle publisher:

| Constant | From | To |
|---|---|---|
| `ORACLE_FORCE_PUBLISH_AFTER_MS` | 20,000 | 5,000 |

All three `GlobalConfig` additions change the stored layout. A redeploy is
required. The next round rotation already requires a redeploy for the
minimum-borrow-fee pack and the referral buckets, so this change lands in
the same window.

## 13. Required invariants

**13.1 No pre-commit information.** For every filled order, the observation
stamp of the fill price must be strictly later than the observation stamp
recorded at commit.

**13.2 No free option.** For every order, exactly one of these must hold at
every moment: the order is fillable and the owner cannot cancel it; or the
order has expired and any account can sweep it.

**13.3 No free retry.** An order with `trigger_price == 0` must be removed
by the first reveal attempt that satisfies the freshness gate, whatever the
outcome of that attempt.

**13.4 Escrow conservation.** `order_escrow_total` must equal the sum of
escrowed collateral over all pending orders, and must be included in
`non_lp_claims`.

**13.5 No trapped cash.** For every order, some account must be able to
reach a state with no order, no escrow, and no execution budget, without
requiring the cooperation of any other account.

## 14. What this does not solve

**Resting orders remain written options against the vault.** An entry order
and a triggered order fill at the oracle price when the trigger crosses, and
the LP is always the passive side. The LP did not choose to make a market
and receives no spread for doing so. The freshness gate cannot change this,
because a resting order is an option by construction.

The remedy is a skew-adjusted fill price, so that a resting fill does not
occur at the median. That is a separate change and should follow this one.

**Executor stamp selection inside the fill window** is bounded by §9.2 and
not removed.

**Oracle failure** halts trading rather than mispricing it. This is the
intended failure mode and an improvement on the current behaviour, where an
outage surfaces as a reverted open with an error code the interface reports
as a slippage breach.

## 15. Implementation sequence

1. `StampedPrice` in `shared/src/types.rs`; `get_price_stamped` on the
   OracleRouter trait and contract, on the uncached path.
2. Escrow aggregate and cash transitions in the PositionManager ledger.
   Include it in `non_lp_claims`.
3. Convert the shipped entry-order path from allowance pull to escrow. This
   step stands alone and closes §8.1.
4. Extend the order record with `observed_at`, `commit_ledger`, and
   `expires_at`; make the trigger predicate conditional.
5. Add the freshness gate and the fill window to the reveal path.
6. Add `terminal_on_miss` and apply it to every exit path.
7. Restrict owner cancellation for a market order; add the sweep reward.
8. Extend to the close and decrease paths.
9. `execution_reward_amount`, `max_fill_lag`, `order_expiry_seconds` in
   `GlobalConfig` with bounds validation.
10. Lower `ORACLE_FORCE_PUBLISH_AFTER_MS` to 5,000.
11. Regenerate the TypeScript bindings.
12. Keeper scanner for pending orders; publish the Decision Kernel for
    independent operators.
13. Two-phase open and close in the application, with a pending state and a
    self-execute path.
14. Document the order model as §12.5 of the canonical specification. The
    entry-order documentation at §12.4 is already outstanding and must be
    written in the same pass.
