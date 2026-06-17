# Test cases

Reference-style coverage map. For every user-visible flow we list the
automated tests that cover it (so you don't re-test what's already
deterministic) and the manual eyes-on checks worth doing before a release.

The strategy: automation handles the deep cases, humans handle the things
automation can't see — visual regressions, real-chain timing, wallet
extension UX, oracle outages.

**Anchor convention**: `path/to/file.ts > describe > "it"`. Anchors target
the test name rather than a line number so they survive whitespace and
reordering — `grep -n '"<text>"' file.ts` recovers the location. When a
single flow is covered by several `it`s, we list each.

## Wallet — connect / status

### W1. Install Freighter (extension missing)
- **Automated**:
  - `packages/frontend/e2e/specs/home.spec.ts > Home > "offers an Install
    Freighter CTA when the extension isn't detected"` — header CTA visible
    when extension state is "missing".
  - `packages/frontend/src/wallet/freighter.test.ts > getFreighterStatus >
    "returns kind: missing when the extension isn't installed"` — wrapper
    maps `isConnected.isConnected === false` to `{kind: "missing"}`.
- **Manual**:
  - Disable the Freighter extension in Chrome, hard-reload `/` → header
    shows "Install Freighter", clicking opens the extension store.

### W2. Connect (extension installed, not allowed)
- **Automated**:
  - `packages/frontend/e2e/specs/wallet.spec.ts > Wallet connect >
    "clicking Connect Wallet (extension present, not allowed) updates UI
    to connected"` — click flips the in-page mock's `allowed` flag; header
    updates to the short-address chip.
  - `packages/frontend/src/wallet/freighter.test.ts > requestFreighterPermission`
    — both `"resolves silently when the user accepts"` and `"throws when
    the user denies"` are asserted.
- **Manual**:
  - Real Freighter: lock the wallet, reload `/`, click "Connect Wallet" →
    Freighter popup appears → approve → header shows the short address +
    network pill.

### W3. Already connected on first paint
- **Automated**:
  - `packages/frontend/e2e/specs/wallet.spec.ts > Wallet connect > "shows
    connected address + network when the wallet starts connected"` —
    header renders the short address + network when initial state is
    "connected".
- **Manual**:
  - Unlock Freighter, hard-reload → header should not flash the
    "Connect Wallet" CTA before settling.

## Liquidity provision — vault deposit & withdraw

### V1. Vault KPI strip (TVL / free / reserved)
- **Automated**:
  - `packages/frontend/e2e/specs/vault.spec.ts > Vault page > "renders TVL,
    free liquidity, and reserved values from the API"` — three KPI cards
    read `total_assets`, `free_liquidity`, `reserved_usdc` from `/api/vault`.
  - `packages/api/tests/rest.test.ts > GET /vault > "returns vault row
    spliced with last_unpause_time from protocol config"` — backend join
    behavior. Also `"falls back to '0' when no protocol config row exists"`
    and `"returns 404 when the vault row is missing"`.
- **Manual**:
  - Visit `/vault` against a live stack → numbers match `vault.total_assets`
    on-chain (read via `make` or the Stellar explorer).

### V2. Paused state pill
- **Automated**:
  - `packages/frontend/e2e/specs/vault.spec.ts > Vault page > "flags the
    paused state with a Paused pill"` — pause pill renders when
    `vault.is_paused === true`. Companion: `"hides the paused pill when
    the vault is running"`.
- **Manual**:
  - Pause vault via admin → `/vault` shows the Paused pill within ~5s
    (depends on SSE round-trip).

### V3. Deposit USDC → receive shares
- **Automated**:
  - Rust contract tests: `contracts/vault/src/tests/test_deposit.rs`.
  - Live-stack: `packages/simulation/scenarios/deposit-withdraw.ts`.
  - **No browser E2E** — needs real RPC signing; covered by the live tier.
- **Manual** (live stack):
  1. Connect Freighter, mint USDC via `/faucet`.
  2. `/vault` → enter amount → Deposit.
  3. Confirm in Freighter → toast pending → success.
  4. Wallet balance decreased, vault `total_assets` increased by the same
     amount within ~10s, share count rendered in the deposit card.

### V4. Withdraw shares → receive USDC (with cooldown)
- **Automated**:
  - Rust contract tests, cooldown enforcement:
    `contracts/vault/src/tests/test_lockup_expires_at.rs` — covers
    `lockup_expires_at = now + cooldown_duration`, the
    `test_withdraw_reverts_before_lockup_expiry` / `..._succeeds_at_lockup_expiry`
    / `..._succeeds_after_lockup_expiry` triad, and the frozen-at-deposit
    rule via `test_admin_shortening_cooldown_does_not_release_existing_lockup`.
  - Rust contract tests, insufficient-liquidity revert:
    `contracts/vault/src/tests/test_withdraw.rs` covers the
    `InsufficientFreeLiquidity` (#4) panic. This file uses
    `cooldown_duration: 0` in its fixture, so it does **not** assert the
    cooldown path — that's covered by `test_lockup_expires_at.rs` above.
  - Live-stack: `packages/simulation/scenarios/deposit-withdraw.ts` is a
    happy-path lifecycle (fund LP → deposit → assert shares/totals/
    free-liquidity > 0 → redeem → assert balance restored ±1%). It does
    **not** advance time, set a non-zero cooldown, or assert any revert
    path. Pre-cooldown / post-cooldown coverage at the live tier is a gap
    (tracked in `test-plan.md`).
  - Error mapping table: `packages/frontend/src/lib/contract-errors.ts`
    maps Vault discriminant #8 → `CooldownNotElapsed` with the friendly
    "LP withdrawal cooldown hasn't elapsed yet." message. **Not yet
    asserted in a unit test** — the registry isn't iterated.
- **Manual** (live stack):
  1. Try to withdraw before cooldown expires → toast "LP withdrawal
     cooldown hasn't elapsed yet."
  2. Wait the cooldown → withdraw succeeds, USDC arrives, share count
     reduces.

## Trading — open, close, manage

### T1. Markets list
- **Automated**:
  - `packages/frontend/e2e/specs/markets.spec.ts > Markets list` — four
    `it`s cover the full surface:
    - `"lists every market the API returns"`
    - `"shows the live-count pill matching the markets length"`
    - `"renders the error block when the markets endpoint fails"`
    - `"market cards link through to /trade/:symbol"`
- **Manual**: visit `/markets` against live data → symbols match
  `addresses.json` tickers; price ticks update on SSE.

### T2. Trade view — header / mark / bias
- **Automated**:
  - `packages/frontend/e2e/specs/trade.spec.ts > Trade page > "renders the
    symbol header and mark price"` — symbol text + NumberFlow value via
    `expectNumberFlowValue`.
  - `packages/frontend/e2e/specs/trade.spec.ts > Trade page > "renders bias
    gauge from the OI split"` — 60% long → Bullish.
  - `packages/frontend/e2e/specs/trade.spec.ts > Trade page > "doesn't
    render the bias gauge as 'Bullish' for a bear-heavy market"` — 20%
    long → Bearish.
  - `packages/frontend/src/components/ui/bias-gauge.test.tsx > <BiasGauge />`
    — every branch: empty placeholder, bullish, bearish, neutral, scaled
    string OI, size prop, huge-bigint safety.
- **Manual**: `/trade/BTCUSD` → chart loads candles; ticker updates;
  bias gauge needle position matches the OI split visually.

### T3. Order form — Long / Short toggle, leverage slider, validation
- **Automated**:
  - `packages/frontend/e2e/specs/trade.spec.ts > Trade page > "shows the
    order form with Long / Short controls"` — toggle buttons render.
  - Form validation logic in `OrderForm.tsx` (leverage clamp, TP/SL
    direction checks) is **not yet covered by vitest** — see the
    test-plan gap list.
- **Manual**:
  - Toggle Long → Short → confirm liquidation-line preview flips above/
    below mark.
  - Slide leverage past market max → "Exceeds max leverage" warning,
    button disabled.
  - Set TP below mark on a long → "Invalid TP" inline hint.

### T4. Open position (long or short)
- **Automated**:
  - Rust: `contracts/position-manager/src/tests/test_increase_position.rs`.
  - Live-stack: `packages/simulation/scenarios/normal-usage.ts`.
  - Contract-error mapping smoke test:
    `packages/frontend/src/lib/contract-errors.test.ts > parseContractError`
    — asserts the parser's discriminant-routing path against representative
    PositionManager (#6) and Vault (#4) codes. The full discriminant →
    message table lives in `contract-errors.ts` and isn't iterated by the
    unit test.
  - **No browser E2E** — needs real RPC.
- **Manual** (live stack):
  1. `/trade/BTCUSD` → collateral 100 USDC, 5× long → Open.
  2. Confirm in Freighter → toast pending → success.
  3. Position row appears on `/portfolio` with size 500, entry near mark
     price.

### T5. Close position
- **Automated**:
  - Rust: `contracts/position-manager/src/tests/test_decrease_position.rs`
    and `test_liquidate.rs` cover the four close kinds.
  - Live-stack: `packages/simulation/scenarios/normal-usage.ts` covers
    user-close happy path; PnL settles to the trader.
- **Manual** (live stack):
  1. From `/portfolio`, click "Close" on a position.
  2. Toast → success.
  3. Position removed from list; USDC balance changed by PnL ± fees.

### T6. TP / SL trigger
- **Automated**:
  - Rust: `contracts/position-manager/src/tests/test_tp_sl.rs` covers both
    directions.
  - Live-stack: `packages/simulation/scenarios/extreme-volatility.ts`
    drives prices past staged TP/SL.
- **Manual**: set TP just above mark → push oracle past TP → keeper
  closes within a few seconds → toast on the trader's UI; PnL credited.

### T7. Liquidation
- **Automated**:
  - Rust: `contracts/position-manager/src/tests/test_liquidate.rs` (health
    check + fee cap) and `test_liquidation_threshold.rs`.
  - Live-stack: `packages/simulation/scenarios/mass-liquidation.ts`.
- **Manual** (live stack):
  1. Open a position near max leverage.
  2. Move oracle adversely until health < 1.
  3. Keeper closes within a few seconds; `/portfolio` shows position
     removed and PnL row in `/trades` with `event_type = liquidation`.

### T8. ADL (auto-deleveraging)
- **Automated**:
  - Rust: `contracts/position-manager/src/tests/test_adl.rs` covers ADL
    eligibility + target selection.
  - Live-stack: `packages/simulation/scenarios/imbalanced-oi.ts` drives
    funding/PnL to ADL conditions.
- **Manual**: rare; relies on extreme conditions. If conditions are met,
  expect a trade event with `event_type = adl` for one of the most-
  profitable counter-side traders.

## Faucet (testnet only)

### F1. Mint mock USDC
- **Automated**:
  - `packages/frontend/e2e/specs/faucet.spec.ts > Faucet > "renders the
    mint form and preset amount buttons"` — page renders + three preset
    buttons exist.
  - `packages/frontend/e2e/specs/faucet.spec.ts > Faucet > "amount input
    updates when a preset is clicked"` — clicking a preset chip echoes its
    value into the amount input.
- **Manual** (live stack): connect wallet, click 10000 preset, Mint →
  toast success → wallet balance increases by 10,000.

## Read-only views

### R1. Home / landing
- **Automated**:
  - `packages/frontend/e2e/specs/home.spec.ts > Home > "renders the hero,
    stats strip, and featured markets"` — featured-markets render from
    mocked API responses.
  - `packages/frontend/e2e/specs/home.spec.ts > Home > "shows TVL pulled
    from the vault endpoint"` — TVL pulled from `/api/vault` via
    `expectNumberFlowValue`.
- **Manual**: visit `/` against live data → featured-markets card prices
  tick on SSE; "Total value locked" reads the same as on `/vault`.

### R2. Portfolio (connected)
- **Automated**:
  - `packages/frontend/e2e/specs/portfolio.spec.ts > Portfolio page` —
    three `it`s cover the full surface:
    - `"prompts to connect when no wallet is attached"`
    - `"shows 'No open positions' when the trader has none"`
    - `"renders a position row when the trader has one"`
- **Manual**: with a position open, `/portfolio` shows the row with PnL,
  borrow fee, funding fee, and health all moving forward over time.

### R3. Leaderboard
- **Automated**:
  - `packages/frontend/e2e/specs/leaderboard.spec.ts > Leaderboard` —
    three `it`s:
    - `"renders one row per trader returned by /leaderboard"`
    - `"shows the empty state when there are no traders"`
    - `"renders the error block when leaderboard fetch fails"`
  - `packages/api/tests/rest.test.ts > GET /leaderboard > "normalizes
    counts to numbers and nullable last_trade_at"` — server-side
    aggregation normalises null counts. Companion: `"exercises the
    close/wins/losses fallback for null counts"`.
- **Manual**: after a sim run, `/leaderboard` should rank traders by
  realized PnL descending; current user's row is highlighted.

### R4. Insights (internal)
- **Automated**:
  - `packages/frontend/e2e/specs/insights.spec.ts > Insights (hidden
    internal dashboard)` — both `it`s:
    - `"renders the protocol overview section"`
    - `"renders without crashing when no markets are returned"`
- **Manual**: numbers match what `/vault` + `/markets` + `/prices` show
  on the same page.

## SSE real-time

### S1. Live price updates
- **Automated**:
  - `packages/frontend/src/api/sse.test.tsx > useStreamPrices` — four
    `it`s: `"inserts a new price into an empty cache"`, `"replaces an
    existing symbol in place"`, `"appends a brand-new symbol when the
    cache has other rows"`, `"ignores malformed JSON and logs a parse
    error"`. Plus `"closes the EventSource on unmount"`.
  - `packages/api/tests/sse.test.ts > GET /prices (SSE) > "queries
    oracle_prices by id and writes a 'price' event"` — backend
    `/stream/prices` queries the row by id and emits a `price` event.
    Companion edge cases: payload-without-id, deleted-row-on-fetch.
- **Manual**: open `/markets` → watch a symbol's price for ~10s →
  it must tick without a manual refresh.

### S2. Reconnect after a backend bounce
- **Automated**:
  - `packages/frontend/src/api/sse.test.tsx > streamEvents underlying
    transport > "logs (but doesn't throw) when the EventSource fires an
    error event"` — asserts the warn log on `EventSource.onerror`; **does
    not assert** actual reconnection (browser-native, not under our
    control). The manual check below is the real signal.
- **Manual**: `make backend-down && make backend-up` → frontend
  reconnects within ~10s; price ticks resume.

## What to do when a flow isn't here

This file is the index, not the spec. If you're adding a flow that
isn't listed:

1. Cover it at the cheapest tier you can — pure logic → vitest, hook /
   component → vitest, route render → Playwright, real-chain → sim.
2. Add a numbered subsection here pointing at the new test refs as
   `file > describe > "it"` anchors — no line numbers.
3. If the flow needs a manual step, add it under "Manual" — bullets,
   not numbered steps unless the order matters.
4. Be precise about what each ref actually asserts. If a test only
   spot-checks two of N cases, say so. If something has no unit test,
   write "Not yet asserted in a unit test." rather than glossing over.
5. Don't let a filename stand in for its contents — `deposit-withdraw.ts`
   sounds like it covers withdrawal edge cases, but reads as a happy
   path. Open the file before citing it.
