# Release sign-off

Per-release record. Copy the template below into a dated entry every time
you cut a release (or a meaningful testnet redeploy), fill in the results,
and commit. The git history is the audit trail.

The goal isn't bureaucracy — it's that "we tested before shipping" is a
claim someone can verify a year from now, including the things that were
known-broken but waived at the time.

## Template

Copy this whole block, paste it under the most recent entry below, and
fill in the placeholders.

```
## YYYY-MM-DD · <network> · <commit short SHA>

**Release scope** — what changed since the previous entry, in one or two
lines. Link any related PRs / ADRs.

### Automated suites

- **Rust** (`pnpm test:rust`): X passed / Y failed / Z ignored
- **Backend API** (`pnpm --filter @stellars/api test:coverage`):
  - Cases: N passed
  - Coverage: lines L%, statements S%, branches B%, functions F%
- **Frontend unit** (`pnpm --filter @stellars/frontend test`): N passed
- **Frontend E2E** (`pnpm test:e2e`): N passed
- **Protocol-math** (`pnpm --filter @stellars/protocol-math test`): N passed

If anything failed: attach the failing test names + a one-line reason
(was it a flake? a real regression? hidden by an upstream change?).

### Sim scenarios (live stack)

Run order: `make up && make deploy && make sim`. Paste the assertion
summary from each scenario.

- **normal-usage**: pass/fail + key assertions
- **imbalanced-oi**: pass/fail + key assertions
- **extreme-volatility**: pass/fail + key assertions
- **mass-liquidation**: pass/fail + key assertions
- **deposit-withdraw**: pass/fail + key assertions

### Manual QA checklist

Tick each box after exercising the flow on the live stack. See
[test-cases.md](./test-cases.md) for the underlying scripts.

- [ ] **W1**: Install Freighter CTA when extension is missing
- [ ] **W2**: Connect Wallet flow (Freighter popup → connected chip)
- [ ] **W3**: Already-connected paint (no CTA flash)
- [ ] **F1**: Faucet mints mock USDC; wallet balance updates
- [ ] **V1**: Vault KPI strip matches on-chain totals
- [ ] **V2**: Paused pill renders within ~5s of an admin pause
- [ ] **V3**: Deposit USDC → vault shares minted
- [ ] **V4**: Withdraw → cooldown enforced + post-cooldown success
- [ ] **T1**: Markets list — symbols and live ticks
- [ ] **T2**: Trade view — mark price + bias gauge accurate
- [ ] **T3**: Order form — Long/Short toggle, leverage clamps, TP/SL
       validation
- [ ] **T4**: Open long (and again open short)
- [ ] **T5**: Close position — PnL credited
- [ ] **T6**: TP / SL trigger via keeper
- [ ] **T7**: Liquidation via keeper
- [ ] **T8**: ADL — only verify if conditions arose during the run
- [ ] **R1**: Home page TVL matches `/vault`
- [ ] **R2**: Portfolio shows open positions with live evaluation
- [ ] **R3**: Leaderboard ranks by realized PnL descending
- [ ] **R4**: Insights aggregates match raw `/markets` + `/vault`
- [ ] **S1**: SSE price updates without manual refresh
- [ ] **S2**: Reconnect after `make backend-down && make backend-up`

### Open issues / waivers

Anything broken-but-not-blocking. Each item should have:
- A one-line description of the problem.
- The reason it's waived for this release (severity, workaround, tracked
  issue link).
- Who decided to waive it.

Example:
- `Insights · per-trader OI breakdown is wrong when a trader holds both
  sides on the same market.` — Severity low (display-only), no impact on
  realized PnL. Tracked in #142. Waived by quinn.

### Sign-off

- Reviewed by: <name>
- Date: YYYY-MM-DD
- Verdict: ship / hold / partial (testnet-only / mainnet-only)

---
```

## Past releases

(Newest first. Add new entries above this line.)

<!-- example placeholder — remove when the first real entry lands -->

## 2026-MM-DD · local · <sha>

**Release scope** — Deliverable 6 baseline: backend + frontend test suites
and Playwright E2E in place; this is the first record using this template.

### Automated suites

- **Rust**: pending — run `pnpm test:rust` and paste totals.
- **Backend API**: 59 passed; coverage 100% lines / 99.09% statements /
  97.95% branches / 100% functions (vs. the 95/90 thresholds).
- **Frontend unit**: 119 passed.
- **Frontend E2E**: 28 passed.
- **Protocol-math**: 10 passed.

### Sim scenarios

- pending — run `make sim` and paste assertion summaries here.

### Manual QA checklist

- pending — not yet exercised against a live stack for this entry.

### Open issues / waivers

- Indexer, keeper, and oracle publishers have no automated tests today
  (tracked in `test-plan.md` Known Gaps).
- Browser-driven sign+submit is not E2E-tested against a real RPC —
  Playwright stops at the mock's signedTxLog; live behavior is covered
  by the sim tier and by manual QA on this checklist.

### Sign-off

- Reviewed by: pending
- Date: pending
- Verdict: baseline — not a release record, just the template seed.
