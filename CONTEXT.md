# win-trader

Domain language for the wintrader perpetual DEX, spanning four repos
(`contracts`, `offchain`, `oracles`, `app`). This file names the **deepened
modules** that architecture reviews refer to, so the language stays stable
across repos. General programming patterns (loops, ports, retries) are
deliberately excluded — only concepts specific to this system live here.

## Language

### Realtime delivery (`app`, `offchain`)

**Notification Channel**:
A Postgres `LISTEN/NOTIFY` channel emitted by a DB trigger on a row change; the
registry of channel names and payload shapes lives in `@win-trader/data`.
_Avoid_: topic, event bus.

**Channel Projection**:
The module that maps a `(Notification Channel, payload)` to the SSE event a
client receives — owning the re-fetch-vs-passthrough policy, event name, event
id, and the fanout scope. Lives in `app/packages/api`.
_Avoid_: handler, transformer, serializer.

### Indexing (`offchain/packages/indexer`)

**Page**:
The batch of chain events returned by one RPC cursor step; the unit that commits
atomically together with the cursor advance.
_Avoid_: batch, block, chunk.

**Page Processor**:
The module that applies one **Page** inside a single transaction — dispatching
each event to its handler with per-event error isolation, then writing the
cursor — and reports which events were dropped.
_Avoid_: ingester, consumer.

### Vault view (`app/packages/frontend`)

**LP-fair basis**:
The share-pricing the LP view uses — `free_liquidity + reserved_usdc` — chosen
*instead of* the contract's `total_assets` so that every LP's stake sums to the
claimable pool; deliberately diverges from on-chain `convert_to_assets`, which
carries non-LP claims (unclaimed dev/staker fees + open-trader-PnL liability).
_Avoid_: NAV (unqualified), total_assets basis.

### Keeping (`offchain/packages/keeper`)

**KeeperWorld**:
The immutable snapshot of protocol state (positions, markets, prices, vault,
config, indexer lag) loaded once per tick, over which all keeper decisions are
made without further I/O.
_Avoid_: state, context, snapshot (unqualified).

**Decision Kernel**:
The set of pure functions that, given a **KeeperWorld**, return the work to do
(liquidation candidates, stale-index markets, triggered orders, ADL action) as
plain data — no signing, no chain, no DB.
_Avoid_: strategy, policy engine.

## Relationships

- A row change fires one **Notification Channel**, which the **Channel
  Projection** turns into at most one SSE event.
- The **Page Processor** advances the indexer cursor exactly once per **Page**,
  in the same transaction as the handler writes.
- The **Decision Kernel** reads a **KeeperWorld** and never mutates it; the
  surrounding loop turns the kernel's plain-data output into submitted
  transactions.

## Example dialogue

> **Dev:** "When a `trades_changed` **Notification Channel** fires, does the
> **Channel Projection** trust the payload?"
> **Maintainer:** "No — for trades it treats the payload as a key and re-fetches
> the canonical row from the store. Only `positions` passes the payload through."

> **Dev:** "Can a TP/SL trigger be tested without a chain?"
> **Maintainer:** "Yes — it's part of the **Decision Kernel**, so you hand it a
> **KeeperWorld** snapshot and assert the triggered orders it returns."

## Flagged ambiguities

- "snapshot" was used for both a **KeeperWorld** and a generic DB read —
  reserve **KeeperWorld** for the keeper's per-tick state.
