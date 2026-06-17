# Test plan

How quality is asserted across the Stellars Finance stack — what each test
tier catches, where it runs, and where the gaps still live. Update this file
when a layer changes (new tier, gap closed, threshold moved); use
[test-cases.md](./test-cases.md) for the per-flow runbook and
[sign-off.md](./sign-off.md) for per-release results.

## Test pyramid

Six tiers, ordered cheapest → most realistic. Earlier tiers are deterministic
and fast; later tiers exercise more of the system but pay for it in setup
cost and flakiness budget.

- **Rust unit** (`cargo test`)
  - Per-contract `src/tests/` modules grouped by feature
    (`test_increase_position.rs`, `test_math.rs` — borrow-fee math lives
    here, `test_oracle_sources.rs`, …).
  - Catches: contract math, storage helpers, access control, error variants.
  - Runs in: dev (`make test`), CI.

- **Rust integration** (`test-suites/`)
  - Spins up the full contract graph through `Fixture::deploy()`; exercises
    cross-contract invariants (PositionManager ↔ Vault ↔ OracleRouter ↔
    ConfigManager) with mocked tokens / oracles.
  - Catches: nested-auth flows, role checks across contracts, full
    open/close/liquidation lifecycles.
  - Runs in: dev (`cargo test --workspace`), CI.

- **Backend TS — unit & integration** (`packages/api`, vitest)
  - 59 cases against `buildRestRoutes(db)` and `buildSseRoutes(db, br)`
    using a chainable `FakeDb` (the second adapter at the `QueryRunner`
    seam) and `FakeBroadcaster` (`Subscribable`).
  - Coverage gate: ≥ 95% lines / 95% statements / 95% functions / 90%
    branches on `packages/api/src/*.ts` (current: 100% / 99% / 100% / 98%).
  - Catches: routing, query construction, error/404 paths, SSE projector
    correctness, abort cleanup, NOTIFY-channel parsing.
  - Runs in: `pnpm --filter @stellars/api test` (or `pnpm test` from root).

- **Frontend TS — unit & component** (`packages/frontend`, vitest + RTL)
  - 119 cases across `lib/` (formatters, contract-error parsing, toast),
    `api/` (client, marketTick projection, SSE hook cache patching),
    `wallet/` (Freighter status mapping), and `components/ui/`
    (button, bias bar, bias gauge).
  - Catches: pure-logic regressions, hook → react-query cache patching,
    wallet error/status branches, component classification thresholds.
  - Runs in: `pnpm --filter @stellars/frontend test` (or root `pnpm test`).

- **Frontend E2E** (`packages/frontend/e2e/`, Playwright)
  - 28 specs across every route (home, markets, vault, leaderboard,
    portfolio, faucet, trade, insights, wallet-connect).
  - Wallet is stubbed via Vite alias swap (`VITE_E2E=1` →
    `e2e/fixtures/freighter-mock.ts`); production code unchanged.
  - API is stubbed with Playwright `page.route` via `installApiMocks`
    helpers (`DEFAULT_MARKETS`, `DEFAULT_VAULT`, `DEFAULT_PRICES`, …).
  - Catches: route rendering against full app state, wallet-state UI
    branches, SSE consumer / EventSource wiring, integration of every
    React-Query hook with its consumers.
  - Does **not** exercise real Soroban RPC; signed-tx flows assert on the
    Freighter mock's signedTxLog rather than on-chain inclusion. Real-
    chain confirmation is delegated to the live-stack tier.
  - Runs in: `pnpm --filter @stellars/frontend test:e2e` (or root
    `pnpm test:e2e`).

- **Live-stack scenarios** (`packages/simulation/scenarios`)
  - Five narrative scenarios run against an actual deployed local stack
    (postgres + indexer + api + on-chain contracts):
    - `normal-usage` — happy-path open/close/withdraw across users
    - `imbalanced-oi` — heavy one-sided OI to drive funding rates
    - `extreme-volatility` — large oracle moves, TP/SL triggers
    - `mass-liquidation` — many positions liquidated in one ledger
    - `deposit-withdraw` — happy-path LP lifecycle (deposit → asserts
      shares + totals + `free_liquidity > 0` → redeem → balance restored
      ±1%). Does **not** assert cooldown or revert paths; those live in
      the Rust unit tier (see V4 in `test-cases.md`).
  - Catches: RPC-level timing, indexer event handling, oracle staleness,
    keeper liveness, real-chain rounding.
  - Runs in: `make sim` (sequential) or `make sim-one SCENARIO=foo` (one).
  - Not in CI by default — requires a running `make up` stack and admin keys.

## Coverage by surface

By surface area rather than tier — useful when adding a feature, to ask
"which test layers should I extend?"

- **Contracts** (vault, position-manager, config-manager, oracle-router)
  - Rust unit (math, storage, access)
  - Rust integration (cross-contract flows)
  - Live-stack sim (real-chain rounding + timing)

- **Indexer** (`packages/indexer`)
  - **No automated tests** today. Behavior is implicitly tested by the
    live-stack sim (DB rows materialize when scenarios run) and by the
    backend tests reading those tables. See `docs/adr/0002-indexer-event-types-per-handler-casts.md`
    for why we deferred typed events / handler-level tests.
  - Gap to revisit when a second event consumer ships.

- **Keeper** (`packages/keeper`)
  - **No automated tests** today. Validated end-to-end via
    `mass-liquidation` scenario.
  - Gap: liquidation-decision pure logic could be unit-tested.

- **Oracle publishers** (`packages/oracle-{base,binance,kucoin}`)
  - **No automated tests** today. CEX integrations are validated by
    operator dashboards.
  - Gap: deviation / staleness handling could be unit-tested.

- **API endpoints** — backend vitest tier
- **SSE streaming** — backend vitest tier (with cleanup-path coverage
  through Playwright fixture-driven cancellation)
- **Frontend components / hooks** — frontend vitest tier
- **User flows** — frontend Playwright tier
- **End-to-end happy paths** — live-stack sim tier

## Current results

Last run: see [sign-off.md](./sign-off.md) for the most recent release record.

Rough order-of-magnitude (post-Deliverable-6 baseline):

- Rust: ~600+ cases across the four production contracts.
- Backend API: 59 cases, 100% line coverage.
- Frontend unit: 119 cases.
- Frontend E2E: 28 cases.
- Sim scenarios: 5 scenarios.

## Running everything

- `pnpm test` — every TS package's `test` script (vitest + bun test).
- `pnpm test:e2e` — Playwright suite (auto-starts dev server on port 5174
  with `VITE_E2E=1`).
- `pnpm test:coverage` — backend coverage report (terminal + HTML in
  `packages/api/coverage/`).
- `pnpm test:rust` — `cargo test --workspace`.
- `pnpm test:all` — TS → E2E → Rust, in sequence. Use before a release.
- `make sim` — live-stack scenarios (requires `make up` and a deploy).

## CI vs local

- **CI**: `pnpm test`, `pnpm test:e2e`, `pnpm test:rust`. Coverage threshold
  enforced on `packages/api`.
- **Local before release**: above + `make sim`.
- **Manual gates**: see [test-cases.md](./test-cases.md) and
  [sign-off.md](./sign-off.md).

## Known gaps (tracked, not blockers)

- Indexer event handlers have no unit/integration tests — see ADR-0002.
- Keeper has no automated tests — would benefit from pure-logic unit
  tests on the scan-and-trigger decision.
- Oracle publishers have no automated tests — CEX adapters validated via
  live operator dashboards.
- The Freighter-driven sign-and-submit path is not E2E-tested against a
  real chain — the Playwright tier stops at the mock's signedTxLog; the
  live-stack sim tier hits real RPC but doesn't drive the browser.
- LP cooldown is covered at the Rust tier (`test_lockup_expires_at.rs`)
  but not at the live-stack tier — `deposit-withdraw.ts` uses
  `cooldown_duration: 0` and doesn't advance time. A `cooldown` scenario
  that deposits, attempts withdrawal pre-expiry (expects revert), advances
  time, then withdraws successfully would close this.
