# Risk Parameters Reference

Every tunable knob in Stellars Finance lives on-chain and is admin-settable. Defaults are seeded in `shared/src/lib.rs` and applied by `ConfigManager::initialize`. Hard ceilings / floors live alongside the defaults and only move via a contract upgrade (subject to the 24h timelock).

This doc is the single-source reference for **what the parameter means**, **what the current default is**, and **what range you are allowed to set it to**.

## Conventions

- `bps` (basis points): `10_000 bps = 100%`, `100 bps = 1%`, `1 bp = 0.01%`.
- Time values are unix seconds.
- USDC token amounts (collateral, size, fees, escrows) use the underlying token's decimals. At the current deployment USDC has 7 decimals, so `1 USDC = 10_000_000` raw. Constants below are raw; the "(N USDC)" gloss assumes the 7-decimal deployment.
- Oracle prices are scaled by `PRECISION = 10_000_000` (1e7). `PRECISION` is conceptually the price scale, not the token scale — they happen to coincide numerically on this 7-decimal deployment, but the contracts treat them as independent.
- Index accumulators (`acc_borrow_index`, `acc_funding_index`) are stored at `INDEX_PRECISION = 1e14`.

Errors are quoted as `ContractError::Variant = NN` so you can map an on-chain revert directly back to the rule that fired.

---

## 1. FeeSplits — revenue distribution

Set via `ConfigManager.update_fee_splits(caller, FeeSplits { lp_bps, dev_bps, staker_bps })`.

These three bps determine how every **revenue dollar** is partitioned: Open fee, the borrow-fee component, the Funding cut, and any TP/SL execution-fee escrow forfeited on Liquidation. (Execution bounties — Liquidation bounty + TP/SL escrow payout to the Executor — are a separate track; see § 2.)

### `lp_bps`
- Default: `9_000` (90%)
- Range: any `u32`, subject to the sum constraint below.
- Meaning: stays in the Vault's `total_assets` (auto-compounds into `sLP` share price).

### `dev_bps`
- Default: `1_000` (10%)
- Range: any `u32`, subject to the sum constraint below.
- Meaning: accrues to `unclaimed_fees`; admin pulls via `Vault.claim_fees(caller, recipient)`.

### `staker_bps`
- Default: `0` (no stakers onboarded in V1)
- Range: any `u32`, subject to the sum constraint below.
- Meaning: reserved for V2 sticky-LP staking; today accrues to `unclaimed_fees` alongside `dev_bps`.

### Validation
- `lp_bps + dev_bps + staker_bps == 10_000` exactly. (`InvalidFeeSplitSum = 22`).
- Components are NOT individually constrained to be non-zero — a `0` slice is legal.

---

## 2. FeeConfig — execution bounties + open fee

Set via `ConfigManager.set_fee_config(caller, FeeConfig { open_fee_bps, liquidation_bounty_bps, tp_sl_execution_fee })`.

These pay the **Executor** of a permissionless Close path (Liquidation or TP/SL OrderExecution). The Liquidation bounty has strict priority over the Revenue split — it is paid out of absorbed collateral first, and only the remainder is sliced.

### `open_fee_bps`
- Default: `10` (0.1%)
- Validation cap: `MAX_OPEN_FEE_BPS = 100` (1%) (`InvalidOpenFee = 44`).
- Type: `u32`
- Meaning: revenue fee charged on Increase, computed as `size * open_fee_bps / 10_000`. Travels Trader → PM → Vault, then sliced by FeeSplits.

### `liquidation_bounty_bps`
- Default: `100` (1%)
- Validation cap: `MAX_LIQUIDATION_BOUNTY_BPS = 1_000` (10%) (`InvalidLiquidationBounty = 45`).
- Type: `u32`
- Meaning: Executor's cut on Liquidation, computed as `min(collateral * liquidation_bounty_bps / 10_000, absorbed_collateral)`. Funded from the trader's absorbed collateral; never from LP capital.

### `tp_sl_execution_fee`
- Default: `5_000_000` raw (0.5 USDC at 7 decimals)
- Validation: `>= 0` AND `<= MAX_TP_SL_EXECUTION_FEE = 100_000_000_000` raw (10_000 USDC at 7 decimals). (`InvalidTpSlExecutionFee = 46`).
- Type: `i128`
- Meaning: flat USDC charged the first time a Position has TP or SL set. Held on the Position's `execution_fee_escrow`. Routed at Close by Close kind:
  - `User` (full close) or `Deleverage` → refunded to trader.
  - `OrderExecution` → paid to Executor.
  - `Liquidation` → forfeited to Vault, sliced by FeeSplits.
  - `User` (partial close) → escrow stays on the surviving Position.
- Charged once per Position. Adding TP after SL (or vice versa) does not double-stack.

---

## 3. ProtocolLimits — global risk & timing

Set via `ConfigManager.update_protocol_limits(caller, ProtocolLimits { ... })`.

### `min_collateral`
- Default: `10_000_000` raw (1 USDC at 7 decimals)
- Validation: `>= 1` (`InvalidMinCollateral = 30`).
- Type: `i128`
- Meaning: floor on Position collateral on every Increase. Blocks dust positions that would be uneconomic to liquidate.

### `max_utilization_ratio`
- Default: `8_500` (85%)
- Validation: `>= 1` AND `<= 10_000` (`InvalidMaxUtilization = 31`).
- Type: `i128` (bps)
- Meaning: caps `reserved_usdc / safe_basis` post-Increase. Lower = more LP buffer, less notional capacity.

### `funding_cut_bps`
- Default: `500` (5%)
- Validation cap: `MAX_FUNDING_CUT_BPS = 3_000` (30%) (`InvalidFundingCut = 32`).
- Type: `u32`
- Meaning: protocol's cut of *positive* funding fees the trader would otherwise receive. Negative funding flows entirely to the receiving side.

### `adl_pnl_bps`
- Default: `9_000` (90%)
- Validation: `>= MIN_ADL_PNL_BPS = 5_000` (50%) AND `<= 10_000` (`InvalidAdlPnl = 33`).
- Type: `u32`
- Meaning: ADL fires when `(realized + total_unrealized) / safe_basis > adl_pnl_bps / 10_000`. The 50% floor prevents an admin from configuring continuous ADL.

### `adl_utilization_bps`
- Default: `9_500` (95%)
- Validation: `>= 1` AND `<= 10_000` (`InvalidAdlUtilization = 34`).
- Type: `u32`
- Meaning: ADL fires when `reserved_usdc / safe_basis > adl_utilization_bps / 10_000`.

### `liquidation_threshold_bps`
- Default: `200` (2%)
- Validation cap: `1_000` (10%, i.e. `BPS / 10`) (`InvalidLiquidationThreshold = 35`).
- Type: `u32`
- Meaning: a Position is liquidatable when `effective_health < collateral * liquidation_threshold_bps / 10_000`. `0` collapses to strict `health < 0`. Higher = liquidate earlier with a thicker safety margin.

### `cooldown_duration`
- Default: `300` (5 minutes)
- Validation cap: `MAX_COOLDOWN_DURATION` = ~46 days (matches the TTL of the storage slot it writes) (`InvalidCooldownDuration = 36`).
- Type: `u64`
- Meaning: LP-share lockup applied on every deposit / mint. Locked in at deposit time — later admin changes neither release nor extend pending locks. Propagates on LP-share transfer: receiver inherits `max(theirs, sender's)`. Zero-asset deposits revert (`ZeroAmount`) to prevent third parties from extending a victim's cooldown.

### `min_position_lifetime`
- Default: `60` (60 seconds)
- Validation cap: `86_400` (24h) (`InvalidMinPositionLifetime = 37`).
- Type: `u64`
- Meaning: minimum time between opening (or last increasing) a Position and being allowed to `decrease_position` / `execute_order`. Anti-front-run lock against oracle-latency round-trips. `liquidate_position` and `deleverage_position` deliberately bypass it.

---

## 4. BorrowRateConfig — borrow & funding curve

Set via `ConfigManager.update_borrow_rate_config(caller, BorrowRateConfig { ... })`. All rate fields are annualized basis points.

### `base_borrow_rate_bps`
- Default: `100` (1% APR)
- Validation: `>= 0` (`InvalidBorrowRateNegative = 40`).
- Type: `i128`
- Meaning: floor APR paid by every open Position regardless of utilization.

### `slope1_bps`
- Default: `500` (5%)
- Validation: `>= 0` AND `<= slope2_bps` (`InvalidBorrowRateNegative = 40`, `InvalidSlopeOrdering = 42`).
- Type: `i128`
- Meaning: slope of the borrow curve *below* `optimal_utilization`. APR at utilization `u <= optimal` is `base + slope1 * u / 10_000` (where `u` is in bps). At the kink (`u = optimal`), the rate is `base + slope1 * optimal / 10_000` — for defaults that's `100 + 500 * 8000 / 10000 = 500 bps = 5% APR`.

### `slope2_bps`
- Default: `5_000` (50%)
- Validation: `>= slope1_bps` AND `<= MAX_SLOPE2_BPS = 20_000` (200%) (`InvalidSlopeOrdering = 42`, `InvalidSlopeTooSteep = 43`).
- Type: `i128`
- Meaning: marginal slope *above* `optimal_utilization`. APR at `u > optimal` is `base + slope1 * optimal / 10_000 + slope2 * (u - optimal) / 10_000`. Steep on purpose — ramps fees on the marginal Position to push utilization back down.

### `optimal_utilization_bps`
- Default: `8_000` (80%)
- Validation: `>= 1` AND `<= 10_000` (`InvalidOptimalUtilization = 41`).
- Type: `i128`
- Meaning: the kink point of the borrow curve.

### `base_funding_rate_bps`
- Default: `100` (1% APR)
- Validation: `>= 0` (`InvalidBorrowRateNegative = 40`).
- Type: `i128`
- Meaning: funding rate scalar applied to OI imbalance — `rate = base_funding_rate * (long_oi - short_oi) / (long_oi + short_oi)`. Positive ⇒ longs pay shorts. Negative funding flows entirely to the receiving side.

---

## 5. Per-market max leverage

Set via `PositionManager.set_max_leverage(caller, symbol, max_leverage)`. Per symbol — each market can run its own ceiling.

- Default: `0` until set per market. Unset markets cannot be opened (`MarketNotConfigured`).
- Validation: `>= MIN_LEVERAGE = 2` (`LeverageBelowFloor`) AND `<= MAX_LEVERAGE_CAP = 200` (`LeverageCapExceeded`).
- Type: `i128`
- Meaning: caps `size <= collateral * max_leverage`.
- Note: setting `1` is rejected to keep "I want to disable this market" routed through `disable_market` (which emits a distinct event). Disable / enable are pauser-only.

---

## 6. Market disable

Set via `PositionManager.disable_market(caller, symbol)` / `enable_market(caller, symbol)`. PAUSER role.

A disabled market rejects `increase_position` calls (`MarketDisabled`) but still permits all close paths and keeper operations. Use this when a market has bad oracle health or needs to be wound down — distinct from full PM `pause`.

---

## 7. Oracle safety (OracleConfig)

Set via `OracleRouter.set_oracle_config(caller, OracleConfig { ... })`. All fields rejected as one `InvalidConfig = 8` on validation failure.

**Unlike the ConfigManager structs below, `OracleRouter::initialize` does NOT seed any OracleConfig.** Admin must call `set_oracle_config(...)` immediately after deploy, otherwise the first `get_price` call panics with `NotInitialized`. `shared::constants::DEFAULT_MIN_REQUIRED_SOURCES = 2` exists but is not currently applied by any initializer; it's a suggested deploy-script default.

### `max_deviation_bps`
- Default: deployment-time choice (no constant).
- Validation: `> 0` AND `<= MAX_DEVIATION_BPS_CEILING = 10_000` (100%).
- Type: `i128` (bps)
- Meaning: maximum spread across the source pool, computed as `max(max - median, median - min) * 10_000 / median` on the sorted valid prices. Above this triggers `PriceDeviationTooHigh = 5`.

### `staleness_threshold`
- Default: deployment-time choice.
- Validation: `> 0`.
- Type: `u64` (seconds)
- Meaning: any SEP-40 source older than `now - staleness_threshold` is filtered.

### `cache_duration`
- Default: deployment-time choice.
- Validation: `> 0` AND `<= staleness_threshold`.
- Type: `u64` (seconds)
- Meaning: TTL on the router's median cache. A cache hit is valid only while both `now <= fetched_at + cache_duration` and `now <= oldest_source_update + staleness_threshold`; source freshness can expire the cache before `cache_duration`.

### `min_required_sources`
- Default: none (deploy-time choice). Suggested constant `DEFAULT_MIN_REQUIRED_SOURCES = 2` — not applied automatically.
- Validation: `>= MIN_REQUIRED_SOURCES_FLOOR = 2` AND `<= MAX_ORACLE_SOURCES = 16`.
- Type: `u32`
- Meaning: minimum number of source responses that must clear all filters (positive, fresh, not future-dated) before a median is published. Falling below triggers `InsufficientSources`.

---

## 8. Per-symbol oracle sources

Set via `OracleRouter.set_oracle_sources(caller, symbol, sources: Vec<Address>)`.

- One flat, equally-weighted source pool per symbol. **No primary/secondary tiering** — every source contributes to the same median.
- Deduped by the contract on write (first occurrence wins).
- Length capped at `MAX_ORACLE_SOURCES = 16`. Over-cap reverts with `TooManySources`.
- Per call, the router queries every source, filters out non-positive / future-dated / stale prices (via `staleness_threshold`), and then requires:
  - `valid_count >= 1` (else `StalePrice`).
  - `valid_count >= min_required_sources` (else `InsufficientSources`).
  - one-sided deviation `<= max_deviation_bps` (else `PriceDeviationTooHigh`).
- The median of valid prices is cached and returned. Odd source counts use the middle sorted value; even source counts use the average of the two middle values.
- `set_oracle_sources` is callable by ADMIN (via the ConfigManager cross-call).

---

## 9. Upgrade timelock

Set via `ConfigManager.set_upgrade_timelock(caller, seconds)`. Applies to all four contracts (`Vault`, `PositionManager`, `OracleRouter`, `ConfigManager`).

- Default: `DEFAULT_UPGRADE_TIMELOCK = 86_400` (24h).
- Validation: `>= MIN_UPGRADE_TIMELOCK = 86_400` (24h) (`UpgradeTimelockTooShort = 6`).
- Type: `u64`
- Meaning: minimum seconds between `propose_upgrade(wasm_hash)` and `upgrade(wasm_hash)`. The hash is committed at propose time — `upgrade` refuses to install a different hash. PAUSER can veto via `cancel_upgrade`.

---

## 10. Operational kill-switches

Roles are granted by `ConfigManager.grant_role(caller, role, account)` (ADMIN-only).

- `Vault.pause(caller)` (PAUSER) — blocks `deposit`, `mint`, `withdraw`, `redeem`. `pay_profit`, `reserve_liquidity`, `release_liquidity`, `accrue_fees`, `claim_fees`, `record_absorbed_collateral`, `update_net_pnl`, and `update_net_pnl_full_sync` are NOT pause-gated — closes and PnL syncs must keep working even when LP deposits/withdraws are halted.
- `Vault.unpause(caller)` (PAUSER) — resumes vault.
- `PositionManager.pause(caller)` (PAUSER) — blocks `increase_position`, `update_indices`, `set_tp_sl`. Idempotent; preserves the original `last_pause_time` so re-pause cannot widen the fee-accrual gap.
- `PositionManager.unpause(caller)` (PAUSER) — resumes. The next index update clamps `effective_start = max(last_index_update, last_unpause_time)` so borrow/funding fees do not retroactively accrue across the pause.
- `decrease_position` / `liquidate_position` / `deleverage_position` / `execute_order` deliberately bypass the pause — traders must always be able to reduce risk and bad debt must always be addressable.
- `ConfigManager.propose_admin` / `accept_admin` — two-step admin transfer (current proposes, new accepts) preventing irrecoverable bricking from a typo.
- Per-contract `_migrate` (UPGRADER) — replace WASM and run the migration hook after the timelock elapses.

---

## 11. Quick recipes

### Reduce bad-debt risk
- Raise `liquidation_threshold_bps` (e.g. 200 → 500).
- Lower per-market `max_leverage`.
- Lower `max_utilization_ratio` (e.g. 8500 → 7500).
- Raise `min_position_lifetime` slightly.

### Protect LP solvency under trader winning streaks
- Lower `adl_pnl_bps` so ADL fires sooner (cannot go below 50%).
- Lower `max_utilization_ratio`.
- Raise `cooldown_duration` to discourage flash-LP arbitrage on volatile markets.

### Make Executor work more profitable
- Raise `liquidation_bounty_bps` (cap 10%).
- Raise `tp_sl_execution_fee` (cap 10_000 USDC at 7 decimals) — but note this is a flat fee paid up front by the trader.

### Tighten the oracle layer
- Lower `max_deviation_bps`.
- Lower `staleness_threshold`.
- Lower `cache_duration`.
- Raise `min_required_sources`.
- Add more `sources` to widen the consensus surface (cap 16 per symbol).

### Wind down a specific market
- `PositionManager.disable_market(caller, symbol)` — blocks Increase, leaves Close paths and liquidations open.

### Emergency stop
- PAUSER calls `Vault.pause` and `PositionManager.pause` independently. Existing positions can still be closed; bad debt can still be liquidated.

---

## 12. Where defaults & ceilings live

All constants in `shared/src/lib.rs` (path: `contracts/shared/src/constants.rs`):

**Defaults (seeded by `ConfigManager::initialize`)**
- `DEFAULT_LP_BPS = 9_000`, `DEFAULT_DEV_BPS = 1_000`, `DEFAULT_STAKER_BPS = 0`
- `DEFAULT_OPEN_FEE_BPS = 10` (0.1%)
- `DEFAULT_LIQUIDATION_BOUNTY_BPS = 100` (1%)
- `DEFAULT_TP_SL_EXECUTION_FEE = 5_000_000` (0.5 USDC at 7 decimals)
- `DEFAULT_MIN_COLLATERAL = 10_000_000` (1 USDC at 7 decimals)
- `DEFAULT_COOLDOWN_DURATION = 300` (5 min)
- `DEFAULT_MIN_POSITION_LIFETIME = 60` (60 s)
- `DEFAULT_MAX_UTILIZATION_RATIO = 8_500` (85%)
- `DEFAULT_FUNDING_CUT_BPS = 500` (5%)
- `DEFAULT_ADL_PNL_BPS = 9_000` (90%)
- `DEFAULT_ADL_UTILIZATION_BPS = 9_500` (95%)
- `DEFAULT_LIQUIDATION_THRESHOLD_BPS = 200` (2%)
- `DEFAULT_BASE_BORROW_RATE_BPS = 100`, `DEFAULT_SLOPE1_BPS = 500`, `DEFAULT_SLOPE2_BPS = 5_000`, `DEFAULT_OPTIMAL_UTILIZATION_BPS = 8_000`, `DEFAULT_BASE_FUNDING_RATE_BPS = 100`
- `DEFAULT_UPGRADE_TIMELOCK = 86_400` (24h)
- `DEFAULT_MIN_REQUIRED_SOURCES = 2` — defined but currently unused; OracleConfig is not seeded by `OracleRouter::initialize`

**Hard ceilings / floors (change only via upgrade)**
- `MIN_UPGRADE_TIMELOCK = 86_400` (24h floor)
- `MAX_LEVERAGE_CAP = 200`
- `MIN_LEVERAGE = 2`
- `MAX_DEVIATION_BPS_CEILING = 10_000` (100%)
- `MAX_ORACLE_SOURCES = 16`
- `MIN_REQUIRED_SOURCES_FLOOR = 2`
- `MAX_FUNDING_CUT_BPS = 3_000` (30%)
- `MIN_ADL_PNL_BPS = 5_000` (50%)
- `MAX_SLOPE2_BPS = 20_000` (200%)
- `MAX_OPEN_FEE_BPS = 100` (1%)
- `MAX_LIQUIDATION_BOUNTY_BPS = 1_000` (10%)
- `MAX_TP_SL_EXECUTION_FEE = 100_000_000_000` (10_000 USDC at 7 decimals)
- `MAX_COOLDOWN_DURATION = SHARED_BUMP_SECONDS` (~46 days, matches the storage slot's TTL)
