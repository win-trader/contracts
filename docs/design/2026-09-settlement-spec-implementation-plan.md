# Implementation plan — Trading, Fees, and Settlement Specification

Target: bring `contracts/` to `docs/design/trading-fees-and-settlement-specification.md`.
**The spec is the source of truth. Where the current contract disagrees, the contract changes.**

Status: plan only. No code written yet. Tests are deliberately out of scope for
this pass (see [Deferred](#deferred)).

Section references (`§x.y`) are to the settlement spec unless stated otherwise.

---

## 0. Verdict up front

**This is a rewrite of `position-manager`, not a migration of it.**

Three facts force that reading:

1. **Every trader action changes shape.** The spec's actions are two-phase
   (commit an instruction with a price cursor, settle later against a strictly
   newer observation). Today `open_position`, `increase_position`, and
   `decrease_position` price and execute in one call. There is no incremental
   path from one to the other — the storage model, the failure taxonomy, and
   the keeper economics all hang off the two-phase split.
2. **Every stored record changes layout.** `Position`, `Market`, `MarketSide`,
   `Ledger`, `GlobalConfig`, and `MarketConfig` all gain and lose fields. A
   `state_version` migration that rewrites live positions is more work than
   redeploying, and §5.11 forbids trading through a half-migrated vault anyway.
3. **The economics change.** Opening fees appear, closing fees stop being
   skew-tiered, keeper rewards become eleven fixed cash amounts instead of a
   per-position execution budget plus a percentage reserve, and the hard-cap
   payout factor becomes a stored snapshot instead of a live recomputation.

So: **redeploy, do not migrate.** Round rotation already requires a redeploy
for the current `GlobalConfig` layout change, and the ritual exists
(`docs` → round-rotation runbook). This removes the entire §12.4 migration
work-stream from the critical path — `state_version` still ships as the guard
for *future* upgrades, but no migration routine is written for *this* one.

### Blast radius — it is not only the position manager

Two crates are rewritten, two take real surgery, two are light. Nothing is
untouched.

| Crate | Verdict | What changes |
|---|---|---|
| `position-manager` | **Rewrite** | Two-phase lifecycle, funding segments, fee model, keeper rewards, risk-state semantics, failure taxonomy |
| `shared` | **Rewrite** | `types.rs` is almost entirely re-shaped (§5); `math.rs` gains 256-bit helpers and loses the old transcendentals (§2.1) |
| `oracle-router` | **Surgery** | `read_stamped_price` returning `observed_at`; settlement reads never answered from a retained value; §12.7.3 degraded mode; error renumber. Median/quorum/deviation core survives intact |
| `vault` | **Surgery** | LP gate changes (`Warning` stops blocking; `ADL`/`HardCap` block withdrawal), resolve reward paid from released assets, §12.1 decimals checks, error renumber |
| `request-router` | **Light** | Delete `Expired` and the round-assignment cutoff, pay the resolve reward from deposit escrow, error renumber |
| `config-manager` | **Light** | Split `PAUSER` into pause vs unpause authority (§12.3), drop the keeper allowlist from ADL, error renumber |
| `packages/protocol-math` (TS) | **Rewrite** | Not spec-driven — it mirrors contract behaviour for the app and keeper, and every formula it mirrors changed. See P10-02 |

The two "light" crates are light only because the spec left their hardest
parts in the position manager: the config timelock of §12.3 lives in the PM,
not in `config-manager`, which keeps owning only the *upgrade* timelock.

**What the spec does and does not name.** §12.5 names four contracts —
position manager, vault, oracle router, configuration manager — and §12.7 is
an oracle-router section in all but title. It refers to the request router
only as "the request contract" (§5.9, §7.12) and never mentions `shared` or
any off-chain package, because it specifies the protocol's economics and
operational contract, not this workspace's crate layout. So the rows for
`shared`, `request-router` and `protocol-math` above are this plan mapping the
spec onto the code that exists; the other four are the spec's own division.
Where that mapping exposes something the spec did not decide — the request
router's error range — it is raised as an open decision rather than guessed
at.

### What survives

| Kept | Why |
|---|---|
| `ledger.rs` claim-bucket verbs and the three-leg collateral choke point | Exactly the §6 `add_position_collateral` / `remove_position_collateral` contract, already enforced in one place |
| `borrow::accrue` | §6.1 verbatim, including the carried remainder |
| Vault / PositionManager split, `transfer_claim` vs `transfer_safety_claim` | §9.1 conservation with a safety-path exemption — the shape the spec needs |
| `risk::assess` / `risk::apply` pure-then-effectful split | §6.16 is written the same way |
| OracleRouter median + deviation + quorum aggregation | §12.7 requires exactly this; only the read interface and the caching change |
| RequestRouter FIFO + escrow-outside-the-vault | §5.9 and §7.17 rely on the escrow sitting with the router |
| `settle.rs` phase-pipeline *structure* | The phases are renamed and reordered, but "one context threaded through named phases" is the right shape for §6.10 |
| Upgrade timelock plumbing (`TimelockedUpgradeable`) | Orthogonal; reused for §12.3's config timelock |

### What is deleted outright

Deletion is the cheapest phase and it shrinks everything downstream, so it
goes first.

| Deleted | Replaced by |
|---|---|
| `Position.execution_budget`, `fund_execution_budget`, `withdraw_execution_budget`, `Bucket::ExecutionBudget` | Fixed keeper rewards paid from escrow or position value (§3.5) |
| `Ledger.risk_keeper_reserve_total`, `Bucket::KeeperReserve`, `risk_keeper_revenue_share_bps` | Nothing — §5.1: "no keeper revenue share, no keeper reserve" |
| `max_insolvent_touch_reward` + the insolvency-touch payout in `liquidate.rs` | Nothing — §10.4: "no insolvency-touch reward" |
| `liquidation_reward_bps`, `adl_reward_bps`, `max_adl_reward` | `keeper_liquidation_reward`, `keeper_adl_reward` (fixed cash) |
| `close_fee_low_bps` / `close_fee_high_bps`, `fees::tiered_close_fee_bps`, `math::skew_abs` | `close_size_fee_bps` + `close_pnl_fee_bps`, no skew tier (§3.2) |
| `borrow_exponent_bps`, `math::borrow_rate_exp`, `math::neg_log2` | Fixed square (§3.3.1, §6.14) |
| `min_borrow_index_delta` | `min_borrow_fee_seconds` + per-window `stored_minimum_borrow_fee` (§3.3.2) |
| `math::EXP2_FRAC` (47 rounded entries), `INV_LN2_NUM`/`INV_LN2_DEN` | `HALF_POW` (48 floored entries), `LN2` (§2.1.3) |
| `LpRequestStatus::Expired` and the `round.previous_timestamp` cutoff in `requests.rs` | §7.17: "There is no `Expired` outcome" |
| NAV loss recognition capped at side collateral (`snapshot.rs`) | §2.3: unrealized trader loss is **not recognized at all** |
| `math::smul_div`, `math::remaining` | `mul_div_trunc`, `derive_partial_removal` (§6.13) |

---

## 1. Ground rules for the whole effort

- **One phase per merge, each phase compiles.** Phases are ordered by
  dependency, not by section number.
- **Commit straight to main.** No feature branch, no status files at the
  workspace root — this document is the only plan artefact.
- **`shared/` is the only place a formula lives once.** If the position
  manager and `packages/protocol-math` need the same number, the Rust is
  normative and the TypeScript mirrors it (§4.12 requires the preview and the
  mutating checkpoint to be arithmetically identical).
- **No clamp may hide a negative pending amount** (§2.11, §9.5). The only two
  clamps the spec permits are `quadratic_integral` truncation residue (§6.2.1)
  and the payment-time cash limit inside `apply_payable_pnl` (§6.5).
- Delegate mechanical conversion work (table transcription, field renames
  across call sites, binding regeneration) to cheaper models; keep the
  arithmetic and lifecycle phases here.

---

## Phase 1 — Delete

Nothing below depends on anything above it. Do it all in one pass.

- [ ] **P1-01** Remove `execution_budget` from `shared::Position` and
      `shared::EntryOrder`/`EntryOrderParams`; delete
      `position/fund_execution_budget.rs`, `position/withdraw_execution_budget.rs`,
      their trait methods on `PositionManager`, and their events.
- [ ] **P1-02** Delete `Bucket::ExecutionBudget` and
      `Ledger.execution_budget_total`. (It returns in Phase 3 renamed as
      `action_escrow_total` — deleting first keeps the two meanings from
      blurring.)
- [ ] **P1-03** Delete `Bucket::KeeperReserve`, `Ledger.risk_keeper_reserve_total`,
      `GlobalConfig.risk_keeper_revenue_share_bps`, and the keeper slice in
      `fees::split_revenue`. Drop the `risk_keeper_reserve_total` view method.
- [ ] **P1-04** Delete the insolvency-touch reward path in
      `position/liquidate.rs` and `GlobalConfig.max_insolvent_touch_reward`;
      delete `events/insolvency_reward_paid.rs`.
- [ ] **P1-05** Delete `MarketConfig.liquidation_reward_bps`,
      `MarketConfig.adl_reward_bps`, `GlobalConfig.max_adl_reward`, and their
      use in `settle::finalize_close` / `position/deleverage.rs`. Liquidation
      and ADL temporarily pay nothing; Phase 6 restores fixed rewards.
- [ ] **P1-06** Delete `fees::tiered_close_fee_bps`, `math::skew_abs`, and
      `MarketConfig.close_fee_low_bps` / `close_fee_high_bps`. Closing fee
      temporarily becomes zero; Phase 5 restores it.
- [ ] **P1-07** Delete `math::borrow_rate_exp`, `math::neg_log2`,
      `math::borrow_rate` (the test-only reference), and
      `GlobalConfig.borrow_exponent_bps`. `borrow::refresh_rate` gets the
      fixed square in Phase 4; leave a `todo!()`-free placeholder that returns
      `base * INDEX_PRECISION` so the crate builds.
- [ ] **P1-08** Delete `GlobalConfig.min_borrow_index_delta` and the
      `borrow_floor` branch in `funding::pending_fees`.
- [ ] **P1-09** Delete `math::smul_div` and `math::remaining`.
- [ ] **P1-10** Delete `LpRequestStatus::Expired` and the
      `round.previous_timestamp >= execute_after` branch in
      `request-router/src/requests.rs::resolve_next`.
- [ ] **P1-11** Delete the loss-recognition branch in
      `snapshot::build_snapshot` — recognize `max(raw_side_pnl, 0)` only
      (§2.3). Keep the single-conversion `PRICE_PRECISION` numerator trick.
- [ ] **P1-12** Delete `validation.rs` entries for every removed field. Leave
      the file; it is rewritten wholesale in Phase 3.

---

## Phase 2 — Arithmetic floor

Everything downstream is defined in terms of these five primitives, so they
land before any caller does. `shared/src/math.rs` gains a sibling
`shared/src/fixed.rs` for the transcendentals.

### 2a. 256-bit helpers (§2.1.1)

Use `soroban_sdk::U256` / `I256` rather than hand-rolling limb arithmetic —
they are host types, so the widening is a host call, not a WASM software
multiply. If metering turns out to bite on the hot path (`refresh_borrow_rate`
runs after *every* mutation), the fallback is a two-`u128`-limb implementation
in `shared`, measured before it is written.

- [ ] **P2-01** `mul_div_floor(a, b, d)` — widen both operands to 256-bit, form
      the product, divide, **range-check the narrowing back to 128-bit**, error
      on failure. The narrowing check is what turns a silent wrap into a
      revert; it is not optional.
- [ ] **P2-02** `mul_div_ceil(a, b, d)` — same, `(p + d - 1) / d`. Drop the
      current `checked_add(denominator - 1)` overflow dance; at 256-bit it
      cannot overflow.
- [ ] **P2-03** `mul_div_trunc(a, b, d)` — **signed**, truncating toward zero.
      Replaces `smul_div`. Callers: signed skew (§3.4.1), blend coefficient `B`
      and every decayed value derived from it (§6.2.1).
- [ ] **P2-04** `carried_div(n, d, r)` — **`n` is a 256-bit value, not
      `i128`.** §6.2 passes it a funding weight that reaches ~`1e40`. Typing
      the numerator as `i128` overflows on the first funding checkpoint of a
      busy market. `d`, the quotient, and the remainder are 128-bit and are
      range-checked.
- [ ] **P2-05** Keep `add`/`sub`/`mul` checked at 128-bit; confirm every
      standalone arithmetic site outside the four helpers is checked. No
      saturating or wrapping operation may appear anywhere in the crate
      (§2.1.1: an overflow is an unexpected failure under §8.9 and must
      revert).

### 2b. Transcendentals (§2.1.2, §2.1.3)

- [ ] **P2-06** Transcribe `HALF_POW[1..=48]` from §2.1.3 **exactly**. The
      current `EXP2_FRAC` is 47 entries rounded-to-nearest; the spec's table is
      48 entries floored. **16 of the 47 shared entries differ by one unit**,
      and the current entry 47 is `100_000_000_000_000` where the spec floors
      to `99_999_999_999_999` — a value of exactly `INDEX_PRECISION` is a
      no-op multiply, so that entry currently contributes nothing at all. The
      table is reference data, not a derivation to redo at a different
      precision; every rounding in these primitives must truncate so the error
      is one-directional and a decay factor is never overstated.
- [ ] **P2-07** `LN2 = 69_314_718_055_994` as a stored constant. Replaces the
      `INV_LN2_NUM / INV_LN2_DEN` ratio.
- [ ] **P2-08** Rewrite `exp2_neg(x)` to the §2.1.2 signature: `x` is an
      `INDEX_PRECISION`-scaled value, **not** an `(elapsed, half_life)` pair.
      Split into whole `n` and fractional `f`; `n >= 128 → 0`; 48 iterations;
      **apply the shift last**, after the fractional product. The current
      implementation already shifts last — keep that, change everything else.
- [ ] **P2-09** New `log2(y)` for `y >= INDEX_PRECISION` (§2.1.2). Integer part
      from bit length, 48 fraction iterations by repeated squaring. This is a
      different function from the deleted `neg_log2`, with a different domain
      and a single caller (`t_star` in §6.2.1) — do not try to reuse it for the
      borrow curve, which no longer needs a logarithm at all.
- [ ] **P2-10** Add the §2.1.3 conformance vectors as constants in the crate so
      the QA phase can assert them without re-deriving: the seven `exp2_neg`
      rows, the six `log2` rows, and the six borrow-curve rows. Note in the
      code comment that `log2(3)` is expected to return `158_496_250_072_105`
      against an exact `158_496_250_072_115` — low by `6e-14` relative, inside
      `DECAY_TOLERANCE`, and in the safe direction.
- [ ] **P2-11** Add `DECAY_TOLERANCE = 1e-12 relative` as a documented bound
      (used by QA, not by the contract).

### 2c. Constants

- [ ] **P2-12** `shared/src/constants.rs`: confirm `PRICE_PRECISION = 1e7`,
      `INDEX_PRECISION = 1e14`, `BPS = 1e4`, `SECONDS_PER_DAY = 86_400` — all
      already correct. Add `SHARE_SCALE = 1e6`. Update the stale scale-mapping
      doc comment, which still points at the superseded 2026-07 ste100 doc.

---

## Phase 3 — Configuration and storage shapes

Land the types before the logic that reads them, so every later phase compiles
against the final layout.

### 3a. Global configuration (§5.1, §10.1, §10.3.1, §10.4)

- [ ] **P3-01** Rewrite `shared::GlobalConfig`. Keep: `min_collateral`,
      `min_position_lifetime`, `funding_half_life_seconds`,
      `risk_capacity_limit_bps`, `base_borrow_rate_bps_day`,
      `max_variable_borrow_bps_day`, `referral_fee_share_bps`,
      `max_active_markets`. Rename `lp_revenue_share_bps` →
      `fee_lp_revenue_share_bps`, `hard_cap_factor_limit_bps` →
      `global_hard_cap_factor_limit_bps`. **Add**: `max_order_lifetime_seconds`,
      `max_market_order_lifetime_seconds`, `min_borrow_fee_seconds`,
      `borrow_lp_revenue_share_bps`, `config_timelock_seconds`,
      `hard_cap_relatch_band_bps`.
- [ ] **P3-02** New `shared::KeeperRewards` struct with the eleven independent
      fields of §5.10: `open`, `limit_order`, `increase`, `decrease`, `close`,
      `tp`, `sl`, `expiry`, `liquidation`, `adl`, `lp_resolve`. Independent
      fields even though every initial value is `2_500_000`. Watch the Soroban
      30-character UDT field-name limit.
- [ ] **P3-03** Fold `LpConfig` (`max_withdraw_utilization_bps`,
      `min_deposit_nav_factor_bps`, `lp_request_delay_seconds`) into the global
      config surface per §5.1, or keep it a separate struct and validate it in
      the same pass — decide once, note the decision here, do not split the
      validation.
- [ ] **P3-04** Deployment defaults from §10.4, as named constants in `shared`,
      not scattered literals. Include the three `lp_request_delay_seconds`
      profiles (local `60`, test `3_600`, production `86_400`).

### 3b. Market configuration (§5.3, §10.2, §10.3.2)

- [ ] **P3-05** Rewrite `shared::MarketConfig`. **Add**: `open_fee_bps`,
      `close_size_fee_bps`, `close_pnl_fee_bps`,
      `order_execution_delay_seconds`. Keep the rest. Removed fields are
      already gone from Phase 1.
- [ ] **P3-06** Enforce §5.3's rule that `market_risk_factor_bps` may change
      **only while both sides have zero open interest and zero risk units** —
      otherwise every live position's canonical risk units would diverge from
      the value derived from its size, and there is no bounded way to rewrite
      them.

### 3c. Records

- [ ] **P3-07** `shared::Position` (§5.5): **add** `opened_at`,
      `stored_minimum_borrow_fee`, `pending_mutation_action_id: Option<u64>`.
      **Rename** `last_increased_time` → `last_size_increase_at`. **Change**
      `take_profit` / `stop_loss` from bare `i128` trigger prices to
      `Option<TriggerInstruction>`. Confirm there is still no stored entry
      price, cached PnL, or cached health value.
- [ ] **P3-08** New `shared::TriggerInstruction { trigger_price,
      acceptable_price, committed_at, execute_after, commit_observed_at }`
      (§5.5).
- [ ] **P3-09** `shared::MarketSide` (§5.4): **add** `hard_cap_payout_factor`
      (initialized to `INDEX_PRECISION`) and `hard_cap_reference_pnl`.
- [ ] **P3-10** `shared::Market` (§5.4): replace the four flat remainders with
      **two remainder groups**, `long_payer_remainders` and
      `short_payer_remainders`, each holding `receiver_payer_remainder`,
      `lp_payer_remainder`, `receiver_liability_remainder`,
      `receiver_distribution_remainder`. Long-payer and short-payer carries are
      never reused by one another. **Add** `pending_receiver_funding` (this
      market's share of the global liability total).
- [ ] **P3-11** `Ledger` (§5.2): **rename** `lp_blocked_side_count` →
      `restricted_market_side_count`. **Add** `action_escrow_total`. Confirm
      the five claim totals are exactly §2.5's five:
      `position_collateral_total`, `pending_receiver_funding_total`,
      `action_escrow_total`, `protocol_claimable_total`,
      `referral_claimable_total`.
- [ ] **P3-12** New `PendingAction` record (§5.6): common header
      (`action_id`, `owner`, `market_id`, `kind`, `created_at`,
      `execute_after`, `commit_observed_at`, `escrowed_collateral`) plus a
      `kind`-selected immutable payload — `MarketOpen`, `LimitOpen`,
      `Increase`, `Decrease`, `Close`. Add `StorageKey::PendingAction(u64)` and
      `NextActionId`. The payload freezes every trader-controlled economic
      input; settlement may read current vault state but never replaces a
      committed size, trigger, price bound, direction, or collateral amount
      with new caller input.
- [ ] **P3-13** Add `Ledger.state_version` and the §12.4 guard
      (`require ledger.state_version == STATE_VERSION` on every operation). No
      migration routine is written — see §0.
- [ ] **P3-14** Storage classes per §12.4: instance for ledger/config/
      authorities, persistent for position / pending action / LP request /
      market / referral. Every persistent entry must be extendable
      permissionlessly — `bump_position` exists; add the equivalent for pending
      actions, markets, and referral entries.

### 3d. Validation and governance

- [ ] **P3-15** Rewrite `validation.rs` to §10.3.1 and §10.3.2 in full,
      including the three bounds that exist for **arithmetic** rather than
      economic reasons: `min_borrow_fee_seconds <= 86_400`,
      `min_position_lifetime <= 86_400`, and the §10.2.3 size/base ceilings.
      Add the cross-parameter rules: `min_collateral > keeper_liquidation_reward`,
      **every** keeper reward `<= min_collateral` (§5.10 explains why this is
      load-bearing and not cosmetic), `fee_lp_revenue_share_bps +
      referral_fee_share_bps <= BPS`, the four-way PnL-factor ordering, and
      `60 <= max_market_order_lifetime_seconds <= max_order_lifetime_seconds`.
- [ ] **P3-16** Two-phase config change (§12.3): `propose_configuration` stores
      the validated proposal with `effective_at = now + config_timelock_seconds`;
      `apply_configuration` checkpoints every affected accumulator **under the
      old value**, then stores. Reuse the existing `TimelockedUpgradeable`
      plumbing shape rather than inventing a second timelock.
- [ ] **P3-17** Timelock exemptions — and **only** these two: setting `paused`,
      and any change validated as moving a bound in the more conservative
      direction (lowering an exposure ceiling, lowering
      `risk_capacity_limit_bps`, raising a margin requirement). Exempt changes
      still checkpoint under the old value and still emit.
- [ ] **P3-18** Split the authorities (§12.3): `configuration_authority`,
      `pause_authority` (may set but **not clear** `paused`),
      `unpause_authority`, `oracle_authority`, `protocol_recipient`. Today
      `pause` and `unpause` share `ROLE_PAUSER` — split them, because pausing
      is a fast safety action and unpausing re-admits risk.

---

## Phase 4 — Index and checkpoint system

### 4a. Borrow (§4.2, §4.10, §4.11, §6.1, §6.14)

- [ ] **P4-01** `borrow::accrue` — already §6.1-correct. Re-verify against the
      spec text after the 256-bit helper swap and leave it alone otherwise.
- [ ] **P4-02** `refresh_borrow_rate` (§6.14): fixed square, no exponent.
      `u = mul_div_floor(utilization, INDEX_PRECISION, BPS)` — converted to
      `INDEX_PRECISION` **before** squaring, because `utilization / BPS` as an
      integer division collapses to `0` or `1`. Both addends are formed at
      `INDEX_PRECISION` before they are summed.
- [ ] **P4-03** `initialize_borrow_window` (§4.10, §6.7): set `borrow_debt =
      ceil(risk_units * borrow_index / INDEX_PRECISION)` and
      `stored_minimum_borrow_fee = ceil(risk_units * current_borrow_rate *
      min_borrow_fee_seconds / (INDEX_PRECISION * BPS * SECONDS_PER_DAY))`,
      **after** the exposure mutation and **after** the rate refresh.
- [ ] **P4-04** `settle_borrow_window_for_survivor` (§6.7): the surviving path
      requires full payment from stored collateral and reverts otherwise. The
      terminal path computes the same `pending.due` but may collect less; only
      the collected amount becomes revenue, and no replacement window opens.
- [ ] **P4-05** `calculate_pending_borrow` (§6.3): check `raw_actual >= 0`
      **before** applying the minimum. A minimum may raise a valid obligation;
      it may never conceal a broken baseline.
- [ ] **P4-06** Wire the §3.3.3 reset sequence into every size mutation:
      accrue → compute `max(actual, stored minimum)` → deduct from *existing*
      collateral → mutate size → refresh rate → reset baseline → quote a new
      minimum. No tranche, proportional remainder, or old minimum carries
      forward.

### 4b. Funding (§4.3–4.6, §4.13, §6.2)

This is the single largest correctness delta in the plan.

- [ ] **P4-07** New pure `integrate_funding_window_by_sign(...) ->
      FundingWindow { segments, ema_after, payer_side_at_window_end,
      displayed_rate_at_window_end }` (§6.2.1). Reads no storage, writes none.
- [ ] **P4-08** **Split the window at a sign change.** The current
      `funding_window` returns one weight and picks the payer from
      `sign(∫ I dt)` — the exact thing §3.4.3 forbids, because it assigns
      funding generated on one side of the crossing to the other. Implement the
      §2.1.2 endpoint test: a crossing exists iff `sign(A + B) != sign(A + B*d_end)`.
      Comparing `A + B` against `A` alone tests for a crossing on `[0, ∞)` and
      will split windows at a point outside themselves.
- [ ] **P4-09** `d_star = -A/B`, `t_star = H * log2(IP²/d_star) / IP`, used at
      full precision and **not** rounded to a whole second before integration.
      Keep both guards — `require d_end < d_star < INDEX_PRECISION` and
      `require 0 < t_star < elapsed` — even though the endpoint test should
      make them unreachable. They are cheap, and a wrong sign test otherwise
      integrates a window at ~150× its true length, silently.
- [ ] **P4-10** Per-segment `d1`/`d2` are measured **from the window origin**,
      not from the segment's own start. The second segment starts at `d_star`,
      not at `INDEX_PRECISION`. Restarting the decay per segment roughly
      doubles the funding attributed to the second one.
- [ ] **P4-11** `J1`/`J2` carried at `INDEX_PRECISION`, with the single
      compensating division at the end (§6.2.1). Flooring them to whole seconds
      is a `1e-5` relative error — seven orders of magnitude outside
      `DECAY_TOLERANCE` and the dominant error in the whole calculation.
- [ ] **P4-12** Return segments in **chronological order**. Order does not
      change the arithmetic but does change which carried remainder each
      division sees.
- [ ] **P4-13** Clamp `quadratic_integral` at zero (truncation residue on a
      mathematically non-negative quantity). Document why this is *not* the
      forbidden clamp of §2.11 — nothing here subtracts a stored baseline from
      a monotonic index.
- [ ] **P4-14** Rewrite `funding::accrue` (§6.2) to process each segment:
      select payer/receiver aggregates and the remainder group **keyed by that
      segment's payer direction**; derive `receiver_weight` /`lp_weight` from
      opposing base exposure; advance both payer indices through `carried_div`;
      derive the guaranteed liability and the receiver credit index from the
      same `receiver_backing_scaled` value so neither can exceed the other's
      justification. Advance the EMA, display fields, and timestamp in **every**
      branch, including when no payer side has exposure.
- [ ] **P4-15** `market.pending_receiver_funding` per market, kept equal to its
      share of `ledger.pending_receiver_funding_total` (§5.12). Today only the
      global total exists.
- [ ] **P4-16** **`reset_receiver_distribution_remainder`** (§4.5.1, §6.13) —
      missing entirely today. When a side's `size_open_interest` changes, zero
      the **opposite** payer stream's `receiver_distribution_remainder`. The
      long-payer stream distributes to short receivers, so a change on the
      short side clears the long-payer carry. Without this, §9.4's sufficiency
      check in `credit_received_funding` becomes reachable and a receiver
      position cannot be settled. It must not be replaced by a clamp.
- [ ] **P4-17** Empty-book cold start (§4.13): seed `skew_ema` from the first
      position's **live skew**, not zero. Already present as
      `funding::cold_start` — keep, and re-verify it runs after the exposure is
      added.
- [ ] **P4-18** Empty-**market** reset (§4.13, §6.17): when one market's book
      empties, zero its EMA, display fields, and both remainder groups, and
      release *that market's* `pending_receiver_funding` to LP equity. Today
      `funding::release_residue` fires only when `open_position_count == 0`
      vault-wide — per §4.13 a market does not wait for every other market to
      empty. Keep a `verify_no_final_receiver_residue` assertion for the
      vault-empty case.
- [ ] **P4-19** Cumulative indices and checkpoint timestamps are **never** reset,
      including across an empty period and across market deregistration.

### 4c. Ordering and previews

- [ ] **P4-20** Enforce the §4.9 ten-step mutation order at every state-changing
      entry point, with step 7 (receiver-distribution remainder reset) and step
      9 (borrow-rate refresh) in the right places. Refreshing a rate before
      accruing elapsed time reprices history and is forbidden (§9.6).
- [ ] **P4-21** Read-only preview path (§4.12): virtually advance the global
      index and the market's funding indices on an in-memory copy, derive the
      pending amounts, return them **without saving any state or remainder**.
      The preview and the mutating checkpoint must use identical arithmetic.
- [ ] **P4-22** An action touching one market checkpoints only that market; an
      LP action that marks NAV checkpoints the global index and **every** active
      market from one synchronized snapshot. `max_active_markets` is what keeps
      that bounded.

---

## Phase 5 — Core accounting algorithms (§6)

- [ ] **P5-01** `calculate_pending_funding` (§6.4) — the three-way index read
      with the non-negativity checks. Close to today's `funding::pending_fees`;
      drop the borrow floor (now per-window) and keep the rounding directions
      (payer obligations ceil, receiver credits floor).
- [ ] **P5-02** `credit_received_funding` (§6.4) — an ownership relabel that
      decrements **both** `market.pending_receiver_funding` and the global
      total before adding to position collateral, with sufficiency required on
      both. Today only the global total moves.
- [ ] **P5-03** `calculate_raw_pnl` (§6.5) — unchanged from `math::pnl`.
- [ ] **P5-04** `calculate_payable_pnl` (§6.5): **read** the side's stored
      `hard_cap_payout_factor`; never recompute it. The current
      `risk::payable_pnl` derives the factor live from equity on every
      settlement, which makes payouts order-dependent — each settlement moves
      both `cash_lp_equity` and the side's aggregate PnL, so the next position
      is measured against a shrunken denominator.
- [ ] **P5-05** `snapshot_hard_cap_factor` (§6.5): runs on the transition
      **into** `HardCap`, records `hard_cap_reference_pnl` as the denominator
      it used, and caps the factor at `INDEX_PRECISION`.
- [ ] **P5-06** Re-latch band (§6.16): while a side remains in `HardCap`, take
      a fresh snapshot once `side_positive_pnl >= reference * (BPS +
      hard_cap_relatch_band_bps) / BPS`. **One-directional** — it fires only on
      growth. A side whose profit falls keeps its factor; lowering the
      denominator would raise the factor and pay later exits more than earlier
      ones. Leaving `HardCap` clears the factor to `INDEX_PRECISION` and the
      reference to zero.
- [ ] **P5-07** **Refresh the side risk state from the action's own price
      snapshot *before* calculating payable PnL, never after** (§6.5). This is
      a rule, not a property of any one operation: §7.2 and §7.9–§7.12 refresh
      immediately after their checkpoints, §7.13 from its liquidation snapshot,
      §7.14 by applying the transition before valuing the position. Refreshing
      afterwards lets the first position out of a newly-crossed side settle
      unscaled and latch the side on its way out — a first-mover advantage on
      exactly the run the state exists to stop.
- [ ] **P5-08** `apply_payable_pnl` (§6.5): the payment-time cash clamp
      (`min(payable_pnl, cash_lp_equity)`) lives **here and only here**, and
      returns `unpaid_profit` so the caller can report it. Move it out of
      `Settlement::begin`, where it currently contaminates the closing-fee base
      and the reported payable figure. It must not reach effective collateral
      or any health check — a position's health is a property of the position,
      not of the vault's cash balance at that instant.
- [ ] **P5-09** A surviving path must assert `uncollectible_loss == 0` and
      revert otherwise; only terminal settlement may consume a nonzero
      remainder and report it as bad debt.
- [ ] **P5-10** `calculate_effective_collateral` (§6.6, §9.9) — one formula,
      used identically by previews, admission checks, liquidation eligibility,
      and final settlement. Pending borrow includes the active minimum-borrow
      floor.
- [ ] **P5-11** `calculate_opening_fee` (§6.8) — new. `ceil(added_size *
      open_fee_bps / BPS)`, applied to an initial open's full size and to an
      increase's added size only. A collateral-only addition never calls it.
- [ ] **P5-12** `calculate_closing_fee` (§6.9) — new shape.
      `max(size_component, pnl_component)`, then `nominal = min(payable_pnl,
      target)`, then `collectible = min(nominal, profit_after_senior_items)`.
      The senior items are funding received (as a credit), receiver-backed and
      LP-backed funding owed, borrow due, and the keeper reward. Only
      `collectible` is debited; `nominal - collectible` is waived immediately,
      is never stored, and is not bad debt.
- [ ] **P5-13** `distribute_open_close_revenue` (§6.11): LP floor, referral
      floor, protocol takes the **exact remainder**. LP revenue gets no stored
      credit — leaving it in the residual is what credits LPs.
- [ ] **P5-14** `distribute_borrow_revenue` (§6.11): separate split using
      `borrow_lp_revenue_share_bps`, no referral share. Funding never calls
      either function.
- [ ] **P5-15** Referral on **opening** fees as well as closing (§3.6). Today
      `referral::accrue_from_close` is the only path.
- [ ] **P5-16** `pay_keeper_from_escrow`, `pay_keeper_from_position`,
      `pay_liquidation_keeper` (§6.12). Only three payments are capped at what
      their source holds: the failure reward on a position action (§7.0),
      `keeper_lp_resolve_reward`, and `keeper_liquidation_reward`. Every other
      payment requires its full amount and reverts — a voluntary settlement
      that cannot pay for itself should not complete.
- [ ] **P5-17** `keeper_reward_for(action_kind)` — the one-to-one map of §6.12.
      Exactly one reward per settlement call; internal cleanup adds none.
- [ ] **P5-18** `derive_added_exposure` (§6.13): re-derive `risk_after` from the
      **complete resulting size**, not by accumulating independently rounded
      tranches. Require positive added base and positive added risk.
- [ ] **P5-19** `derive_partial_removal` (§6.13): remaining base floors and the
      removed portion takes the difference, so the two always sum to the
      pre-reduction base. Remaining risk units are re-derived from resulting
      size; removed risk is the difference. A full close removes the exact
      remainder with no proportional rounding.
- [ ] **P5-20** `capitalize_for_surviving_mutation` (§6.10): credit received
      funding, then require collateral covers receiver funding + LP funding +
      borrow **in full**, then collect each. The capped
      `collect_up_to_position_value` helper is for terminal settlement only.
- [ ] **P5-21** `settle_terminal_position` (§6.10): the full §3.8 waterfall —
      credit funding received, credit positive payable PnL, collect
      receiver-backed funding, apply negative PnL, collect LP-backed funding,
      collect borrow, pay the keeper reward, collect the closing fee from
      remaining current-settlement profit only, pay out the residual.
- [ ] **P5-22** `evaluate_liquidation` (§6.15): `threshold = max(maintenance,
      keeper_liquidation_reward)` and `liquidatable = effective <= threshold`.
      Today the check is `effective >= maintenance → healthy` — a strict
      inequality with no reward floor, so a position sitting exactly at
      maintenance is not liquidatable and the reward is not reserved at all.
      Return one assessment object reused by settlement so eligibility and
      payment cannot use different prices or fee snapshots.
- [ ] **P5-23** `evaluate_side_risk_state` (§6.16) — close to today's
      `risk::risk_state_for`; keep. Add the `restricted_market_side_count`
      maintenance and the two payout-factor transitions.
- [ ] **P5-24** `side_accepts_new_exposure(side)` (§6.16.1) = `not paused and
      risk_state in {Normal, Warning}`. Two behaviour changes: **`Warning` no
      longer blocks** (it is a latch that makes recovery sticky, not a stop),
      and **pause folds into this one predicate** rather than being checked at
      each call site. Today `open.rs` and `increase.rs` block on
      `!= RiskState::Normal` and call `require_not_paused` separately.
- [ ] **P5-25** The opposite side is never restricted by this side's state —
      opening against a restricted side reduces its net aggregate PnL, which is
      the trade that resolves the condition.
- [ ] **P5-26** `release_residual_accounting_dust` (§6.17) and the fee-split
      residue rule (protocol takes the remainder, by construction of §6.11).

---

## Phase 6 — Two-phase action lifecycle (§7, §8)

The core rewrite. Everything here is new surface; almost nothing is edited in
place.

### 6a. Common machinery (§7.0, §8.1)

- [ ] **P6-01** Predicates: `fresh_for_commit = fill_observed_at >
      commit_observed_at` (**equality fails**), `delay_satisfied = now >=
      execute_after`, `entry_price_allowed`, `exit_price_allowed`. An
      `acceptable_price` of zero disables the bound.
- [ ] **P6-02** Outcome enum: `Executed`, `Failed(reason)`, `Cancelled`,
      `Expired`, `Superseded` as terminal results; `NotReady`, `Pending`,
      `RequiresLiquidation` as non-terminal returns that change no state and
      pay nothing. **These return, they do not panic.** Today slippage and
      capacity failures panic — which reverts, hands the trader a free retry,
      and pays no keeper.
- [ ] **P6-03** `fail_entry_action` (§7.0): pay the action reward from escrow,
      refund the remainder to the **owner frozen in the action**, remove the
      record, charge no opening fee, emit the terminal failure.
- [ ] **P6-04** `fail_position_action` (§7.0): `payable_reward = min(reward,
      max(0, stored_collateral - min_collateral), max(0, effective_collateral -
      liquidation_threshold - 1))`. **The reward is variable; the finality is
      not.** An eligible attempt on a non-liquidatable position always
      terminates, even at a zero reward — refusing to terminate would leave
      `pending_mutation_action_id` occupied forever (position mutations have
      neither cancel nor expiry) and would turn a survived attempt into a free
      retry.
- [ ] **P6-05** A liquidatable position returns `RequiresLiquidation` without
      consuming the action or paying a reward. That is the only non-terminal
      safety exception for position mutations.
- [ ] **P6-06** Action IDs are monotonic, never reused, and consumed
      permanently on removal (§8.13). A second call with a consumed ID fails
      before any transfer or reward.
- [ ] **P6-07** One action per settlement call (§8.14). No array input, no
      batch dispatcher.

### 6b. Entries (§7.1–7.6)

- [ ] **P6-08** `create_market_open_order` (§7.1): validate, check
      `side_accepts_new_exposure`, bound `expires_at` by
      `max_market_order_lifetime_seconds`, require escrow covers opening fee +
      keeper reward + `min_collateral`, require escrow covers
      `keeper_expiry_reward`, require post-charge collateral clears initial
      margin, read the commit price, transfer collateral into escrow, store the
      pending action. **No capacity is reserved, no price is chosen, no fee is
      collected, no position is created.** Market orders are binding — no
      cancel.
- [ ] **P6-09** `settle_market_open` (§7.2): expiry check → delay check →
      fresh-observation check → checkpoints → **refresh risk state** →
      preflight against the hypothetical post-settlement state → succeed or
      `fail_entry_action`. The first eligible attempt is terminal.
- [ ] **P6-10** `projected_minimum_borrow` in the preflight (§7.2). Without it
      a position can be admitted exactly at initial margin and be below it the
      moment its borrow window is quoted, because the minimum-borrow floor is
      part of pending borrow from the window's first second. Every input is
      known at preflight. The same term belongs in the increase preflight,
      evaluated against the **full resulting** risk units.
- [ ] **P6-11** `create_limit_open_order` (§7.3): as market-open, but bounded by
      `max_order_lifetime_seconds`, requiring a positive trigger, and freezing
      `trigger_above` against the authenticated commit price. Cancellable by
      the owner.
- [ ] **P6-12** `settle_limit_open` (§7.4): an untriggered observation returns
      `Pending` and does **not** consume the order. Once triggered, the attempt
      is terminal and settles exactly like a market open with
      `keeper_limit_order_reward`.
- [ ] **P6-13** `cancel_limit_open` (§7.5): owner only, before expiry, full
      refund, no fee, no keeper reward.
- [ ] **P6-14** `clean_expired_entry` (§7.6): permissionless, pays
      `keeper_expiry_reward` from escrow, refunds the remainder. At exactly
      `expires_at` execution is forbidden and cleanup is allowed — there is no
      timestamp at which both succeed.
- [ ] **P6-15** Attached TP/SL from an entry fill take **that fill's
      observation** as their commitment cursor, so they cannot close the
      position on the same observation that opened it.

### 6c. Position mutations (§7.7–7.10)

- [ ] **P6-16** `add_collateral` (§7.7) stays **immediate** — it adds no price
      exposure. It charges no fee or reward, settles nothing, resets no
      baseline, does not change `stored_minimum_borrow_fee`, and does **not**
      restart the minimum-lifetime clock. A liquidatable owner may use it to
      rescue the position.
- [ ] **P6-17** `create_increase` (§7.8) with the two dust checks against the
      **commitment** price: added base `> 0` and resulting risk units strictly
      greater than current. These are `require`s at settlement (§6.13), so
      failing them would revert and leave the action occupying
      `pending_mutation_action_id` permanently — a size add of one unit on a
      market priced in the tens of thousands is enough to brick the position.
      Rejecting at creation is what makes the settlement-time requires
      unreachable.
- [ ] **P6-18** `settle_increase` (§7.8): capitalize the old window first, then
      move escrowed collateral in, then the opening fee, then the keeper
      reward, then add exposure, then the health/capacity/cap checks, then
      reset baselines and open the new borrow window. The completed old window
      is paid from **pre-existing** collateral, before added collateral joins.
- [ ] **P6-19** `create_decrease` / `settle_decrease` (§7.9). The three guards
      that keep this path and the terminal path from diverging:
      `uncollectible_loss == 0`, `unpaid_profit == 0`, and the preflight check
      that cash LP equity covers the payable profit to be credited.
- [ ] **P6-20** `create_close` / `settle_close` (§7.10). Always targets the
      complete remaining exposure at settlement; no size is stored.
- [ ] **P6-21** `pending_mutation_action_id` — at most one ordinary pending
      mutation per position (§8.4). Creation fails if occupied. Cleared only by
      execution, terminal failure, or forced-position cleanup.

### 6d. Triggers and forced actions (§7.11–7.14)

- [ ] **P6-22** `set_take_profit` / `clear_take_profit` and the stop-loss
      mirror (§7.11, §7.12), each recording a fresh `commit_observed_at` and
      `execute_after` when attached or replaced.
- [ ] **P6-23** `execute_take_profit` / `execute_stop_loss`: both close the
      position **in full** — no partial take-profit, neither takes a size. Both
      pay only their own reward (`keeper_tp_reward` / `keeper_sl_reward`), not
      a close or decrease reward in addition. Delete the current
      `execute_order.rs` requirement that `execution_budget > 0`.
- [ ] **P6-24** TP/SL slippage behaves differently from a market-style action:
      a crossed trigger whose exit bound fails leaves the instruction
      **attached** and pays nothing (§8.7). It may execute on a later
      qualifying observation.
- [ ] **P6-25** **`min_position_lifetime` gates all four exits** — decrease,
      close, take-profit, and stop-loss — measured from `last_size_increase_at`
      and read at settlement, not frozen at creation (§8.5). Today it is
      enforced only on `decrease`, and it panics with `TooEarly`; it must
      return `NotReady`. A size increase re-locks the position against its own
      stop-loss; that exposure is bounded by `min_position_lifetime` and
      covered by liquidation, and exempting stop-loss alone makes take-profit
      the obvious churn bypass.
- [ ] **P6-26** `liquidate` (§7.13): one snapshot for eligibility **and**
      settlement — the function cannot reassess with a later price midway
      through. Pays `keeper_liquidation_reward` from position value, then LP
      residual for the gap, capped at what exists; the liquidation completes
      either way. No closing fee.
- [ ] **P6-27** `execute_adl` (§7.14): gate on `next_state is ADL or HardCap`
      re-evaluated from the current book on every call, plus `position_raw_pnl >
      0`. Candidate selection stays **unranked** — the state gate is what bounds
      the mechanism, not the selection order. Fixed `keeper_adl_reward`, no
      closing fee.
- [ ] **P6-28** Forced-action cleanup (§8.12): remove exposure, clear both
      triggers, remove the ordinary pending mutation, refund its complete
      added-collateral escrow, clear the reverse reference, pay **only** the
      forced action's reward.
- [ ] **P6-29** Precedence (§8.12): liquidatable → liquidation before every
      voluntary mutation, TP, SL, or ADL; not liquidatable and side needs
      deleveraging → ADL may supersede; otherwise the voluntary action settles.
- [ ] **P6-31** **Drop the keeper allowlist from ADL.** §7.0: "`require keeper
      authorization` means the caller authenticates the address that will
      receive the reward. It does not require membership in a privileged keeper
      allowlist; execution remains permissionless." Today
      `deleverage_position` requires `ROLE_KEEPER`, and so does
      `update_indices`. Both become permissionless — ADL is bounded by the
      state gate (P6-27), not by who calls it, and a checkpoint pays no reward.
      **`publish_round` stays permissioned**: §12.7.2 keeps round publication
      as the one permissioned operation and names the liveness dependency that
      follows.

### 6e. Referrals (§7.15, §7.16)

- [ ] **P6-30** Mostly present and correct. Verify: code owner immutable,
      self-referral rejected, trader may re-point, changing the mapping affects
      only fees collected afterward, and the claim debits before the transfer.

---

## Phase 7 — Oracle interface

- [ ] **P7-01** `read_stamped_price(symbol) -> StampedPrice { price,
      observed_at }` (§12.7.1). `observed_at` is the **oldest** source
      timestamp contributing to the accepted aggregate — the router already
      computes this as `oldest_source_update`; it just is not returned.
- [ ] **P7-02** **No call may answer from a retained value.** The current
      `get_price` serves a cache. A retained stamp is backdated by however long
      it was retained, so `commit_observed_at` becomes older than the moment
      the trader committed, and a fill then satisfies `fill_observed_at >
      commit_observed_at` against an observation that predates the commitment —
      the test passing with no new information having arrived. On the forced
      paths, a retained price is a price the caller chose: a keeper who can pin
      a momentary adverse print can liquidate a currently-healthy position, or
      latch a side into `HardCap`. Route every settlement read through the
      existing uncached `fetch_fresh_price` path and return the stamp with it.
- [ ] **P7-03** Keep synchronized rounds as they are (§12.7.2) — separate
      mechanism, separate purpose, already uncached. Note the liveness
      dependency: if rounds stop, LP deposits and withdrawals stop while
      positions keep trading and liquidating on per-symbol prices.
- [ ] **P7-04** §12.7.3: **a risk-reducing operation must not be blocked by a
      guard whose purpose is to protect risk-adding operations.** The deviation
      guard is currently absorbing — while sources disagree, every open *and*
      every close reverts together and the protocol cannot trade its way out.
      This matches the known oracle-publisher wedge. The spec states the
      requirement and leaves the mechanism to the oracle side: a wider bound for
      liquidation, a documented fallback aggregate, or an explicit degraded
      mode. **One guard governing both directions is not acceptable.** Pick one
      — this is the one open decision that blocks Phase 7.
- [ ] **P7-05** §12.7.4 router-side bounds: source decimals exactly `7`,
      `min_required_sources >= 2`, `max_deviation_bps <= 10_000`, source count
      `<= 16`. All already enforced — verify and document that
      `staleness_threshold` is the deliberate choice, because it silently
      widens the window a trader commits against without any §10 parameter
      changing.

---

## Phase 8 — LP path (§7.17)

- [ ] **P8-01** `keeper_lp_resolve_reward` paid on **every** terminal outcome
      except a failed withdrawal. A deposit pays it from asset escrow **before
      conversion**, so no share is minted against value paid to the executor. A
      successful withdrawal pays it from the assets it releases, after every
      capacity and health check has passed on the full amount.
- [ ] **P8-02** A **failed withdrawal pays no reward** — its escrow is shares,
      not cash, and it releases no assets; taking the reward in shares would
      confiscate part of an LP's stake for an outcome they did not cause.
- [ ] **P8-03** `fail_lp_request` fails rather than reverts (§7.17). Only the
      FIFO head is resolvable, so a `require` would let one unsatisfiable
      request block every LP behind it for as long as the condition held.
- [ ] **P8-04** Deposit gate: `min_deposit_nav_factor_bps` is a guard on the
      **conversion arithmetic**, not a market judgement. First deposit into an
      empty vault is exempt; a vault with shares outstanding and zero cash
      equity accepts no deposit. A deposit is **not** gated on side risk state
      at all — it adds LP equity and lowers every side's factor.
- [ ] **P8-05** Withdrawal gates: `assets_to_pay <= free_lp_capital`,
      post-withdraw utilization `<= max_withdraw_utilization_bps`,
      `vault_shortfall == 0`, and **no active market side in `ADL` or
      `HardCap`**. `Warning` does not block, for the same reason it does not
      block new exposure. Today `can_create_lp_request` blocks on
      `lp_blocked_side_count == 0`, which blocks on `Warning` too.
- [ ] **P8-06** Conversion offsets: `marked_vault_nav + 1` and `share_supply +
      SHARE_SCALE`. Today's `VIRTUAL_ASSETS = 1` / `VIRTUAL_SHARES = 1_000_000`
      already match — rename to the spec's names and leave the arithmetic
      alone.
- [ ] **P8-07** Marked NAV per §2.3: `max(cash_lp_equity - Σ max(raw_side_pnl,
      0), 0)`. The loss-recognition branch was deleted in P1-11; confirm the
      consequence is documented — LP share price understates while traders are
      collectively losing and steps up as losses are realized, and that
      asymmetry is the point.
- [ ] **P8-08** §12.1 token requirements: `require decimals(vault_asset) == 7`
      and `decimals(share_token) == 13` at initialization. The current
      `set_decimals_offset(6)` produces 13 for a 7-decimal asset, but the check
      is not written down.
- [ ] **P8-09** No partial fills, no persistent pending-withdrawal cash claim.
      In a clean terminal vault the final LP may withdraw all residual cash LP
      equity so conversion rounding cannot strand ownerless assets. (Today's
      `shares == supply` special case is close — re-verify against §7.17.)

---

## Phase 9 — Errors, events, pause, lifetime

### 9a. Error taxonomy (§12.5)

- [ ] **P9-01** Renumber to disjoint ranges: position manager `1–99`, vault
      `100–199`, oracle router `200–299`, config manager `300–399`. Today every
      one of the four numbers from `1`, and **code `9` is currently owned by
      four different contracts at once** — `SlippageExceeded` (PM),
      `ArithmeticError` (vault), `InsufficientSources` (router), and
      `UpgradeTimelockNotElapsed` (request router). The first two of those have
      opposite remedies, which is why the app currently blames slippage for
      oracle outages.
- [ ] **P9-01b** **Spec gap: §12.5 assigns no range to `request-router`.** It
      lists four owners and the protocol has five contracts. Decide and record
      it: either fold the request router into the vault's `100–199` (they are
      two halves of one LP path and never both in a caller's stack) or open
      `400–499`. Do not leave it numbering from `1`.
- [ ] **P9-02** **Wrap cross-contract errors, never pass them through.** When
      the position manager's call into the router fails, it returns its own
      error carrying the underlying one.
- [ ] **P9-03** Group codes by cause: Authorization, Not found, State,
      Validation, Oracle, Accounting, Arithmetic. `Accounting` and `Arithmetic`
      must be unreachable through ordinary use; if a well-formed call can
      trigger either, that is a defect, not a user error.
- [ ] **P9-04** Expected terminal failures are **not errors** — slippage,
      capacity, exposure cap, and a blocked side all complete successfully and
      record `Failed` with a reason. They appear in results, not in error codes.

### 9b. Events (§12.6)

- [ ] **P9-05** Common `EventHeader { event_version, ledger_timestamp,
      market_id, actor }` on every event.
- [ ] **P9-06** The fifteen required events of §12.6 with their required
      fields. Notable additions: `ActionCommitted`, `ActionFailed`,
      `ActionCancelled`/`ActionExpired`, `RiskStateChanged` with the PnL
      factor, `FundingCheckpoint` **per segment**, `RevenueDistributed` with
      all three shares, `ConfigurationProposed`/`ConfigurationApplied`.
- [ ] **P9-07** Amounts are emitted as **collected**, never nominal. A waived
      closing fee and an uncollected borrow are reported as zero collected,
      with the waived amount separate. An indexer that sums nominal fees will
      not reconcile against the ledger.
- [ ] **P9-08** `PositionClosed` must carry **profit the vault could not pay**
      (§6.5). A trader receiving less than their recognized profit is the
      single outcome most likely to be mistaken for an accounting error, and
      events are the only durable record — §5.6 removes the pending record and
      §5.14 removes the position.
- [ ] **P9-09** Every event that changes cash ownership carries enough to
      reproduce the change, so §9.1 is checkable from the event stream alone.

### 9c. Pause (§12.2)

- [ ] **P9-10** **A pause stops the vault taking on risk. It never stops anyone
      shedding it.** Implemented through `side_accepts_new_exposure` (P5-24),
      not scattered `require_not_paused` calls.
- [ ] **P9-11** A pending entry or increase that becomes eligible during a
      pause takes the ordinary expected-failure route: it terminates, pays its
      reward, refunds escrow, charges no opening fee. A pause **drains** the
      risk-adding queue rather than freezing it, so no order waits for an
      unpause that may never come.
- [ ] **P9-12** Allowed while paused: cancel, expiry cleanup, add collateral,
      decrease, close, TP, SL, liquidation, ADL, referral claims, and all
      checkpoints. Blocked: creating or settling anything that adds exposure,
      creating or resolving LP requests, and **claiming protocol revenue** (the
      same authority can generally pause; leaving both open creates a
      pause-and-drain path that costs nothing to close). Referral balances are
      ordinary user funds and are not withheld.
- [ ] **P9-13** Accrual never pauses. Both checkpoint clocks advance across a
      pause exactly as they would otherwise — which is only fair because exits
      stay open.

### 9d. Storage lifetime and market lifecycle

- [ ] **P9-14** `register_market` / `deregister_market` (§7.18). Deregistration
      requires zero open interest, zero base exposure, zero
      `pending_receiver_funding`, both sides `Normal`, and no pending action
      referencing the market. Indices and checkpoint timestamps are **retained,
      not reset**, so a later re-registration cannot rewind an index a
      historical position was priced against.
- [ ] **P9-15** `initialize_vault` (§7.18): the borrow clock starts at
      initialization and the rate starts at the base rate, so the first position
      does not inherit index growth from an epoch that had no positions.
- [ ] **P9-16** §5.14 cleanup on every terminal transition, and §12.4's rule
      that nothing economic is stored as temporary and every persistent entry is
      permissionlessly extendable.

---

## Phase 10 — Downstream (out of contract scope, blocks deploy)

These are not part of the contract work but they gate the redeploy. Track them
here so they are not discovered at rotation time.

- [ ] **P10-01** Regenerate `packages/bindings/*` for every changed contract.
- [ ] **P10-02** **`packages/protocol-math` must mirror the new arithmetic
      exactly.** No spec rule demands this — §4.12's identical-arithmetic rule
      governs the *on-chain* preview against the *on-chain* checkpoint (P4-21),
      not an off-chain mirror. It is required because the app and keeper quote
      from this package, and every formula it mirrors changed: 256-bit
      intermediates (`BigInt` is native there), the floored `HALF_POW` table,
      `log2`, the segment-split funding window, per-window minimum borrow, and
      the new fee formulas. A quote that disagrees with settlement is a
      user-visible defect even though no invariant catches it.
- [ ] **P10-03** `offchain` keeper: the Decision Kernel now decides *which
      pending action to settle* as well as which position to liquidate, and
      must respect `execute_after`, the fresh-observation cursor, and the
      terminal-first-attempt semantics. Fixed rewards replace budget-funded
      execution.
- [ ] **P10-04** `offchain` indexer: new event set, `event_version`, new
      terminal action states. Historical terminal states exist only as events.
- [ ] **P10-05** `app`: two-phase order UX (commit → pending → settled/failed),
      `NotReady` vs `Failed` surfacing, opening-fee display, per-window minimum
      borrow, and the new error ranges (which finally lets the UI stop blaming
      slippage for oracle outages).
- [ ] **P10-06** Round-rotation redeploy: this is a **redeploy**, not an
      upgrade. Follow the existing four-repo ritual; `GlobalConfig` and
      `MarketConfig` layouts both change, as does every persistent record.
- [ ] **P10-07** Reconcile the unpushed `@win-trader/data` publisher checkout
      **before** this deploy — the offchain schema divergence is a known
      outstanding item and this rotation will touch it.

---

## Deferred

- **Tests.** Explicitly out of scope for this pass, by request. The spec
  supplies its own test corpus for when they land: §2.1.3 conformance vectors
  (assertable exactly), §11's twelve worked end-to-end examples, and §9's
  sixteen invariants. §2.1.2 additionally names three properties an
  implementation "must test directly": `exp2_neg` monotonicity, checkpoint-split
  equivalence within `checkpoint_count * DECAY_TOLERANCE`, and bit-identical
  determinism across nodes.
- **§12.4 migration routine.** Not written — this is a redeploy (§0). The
  `state_version` guard still ships, for future upgrades.
- **Batch settlement interface.** §8.14 explicitly defers it.
- **Pull-model payouts.** §12.1 notes the alternative to direct transfers (a
  sixth claim entry) and states that this specification pays directly and
  accepts the frozen-account liveness dependency. Do not build the pull model.

---

## Open decisions

These block the phases named; everything else is determined by the spec.

1. **P7-04 — how the oracle stops the deviation guard from blocking exits.**
   Wider bound for forced paths, documented fallback aggregate, or explicit
   degraded mode. Oracle-side decision; the spec deliberately does not
   prescribe. *Blocks Phase 7.*
2. **P2-01 — `soroban_sdk::U256` host types vs a two-limb software
   implementation.** Plan assumes the host types. `refresh_borrow_rate` runs
   after every mutation, so measure before committing. *Blocks nothing; revisit
   if metering bites.*
3. **P3-03 — whether `LpConfig` stays a separate struct.** Cosmetic; pick one
   and validate in a single pass either way.
4. **P9-01b — which error range the request router gets.** §12.5 assigns four
   ranges to five contracts. Fold into the vault's `100–199`, or open
   `400–499`. *Blocks Phase 9a only.*

---

## Phase dependency order

```
P1 delete
  └─ P2 arithmetic ──┬─ P4 indices ─┬─ P5 algorithms ─┬─ P6 lifecycle ─┬─ P9 errors/events/pause
     P3 types ───────┘              │                 │                │
                                    └─ P7 oracle ─────┘                │
                                                      P8 LP path ──────┘
                                                                        └─ P10 downstream
```

P3 (types) and P2 (arithmetic) are independent of each other and can land in
either order. P7 (oracle) is independent of P4/P5 but must precede P6, which
needs the stamped read. P8 (LP) needs P5's NAV and risk-state rules but not
P6's lifecycle.
