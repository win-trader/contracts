# Stellars Finance for Dummies

A beginner-friendly walkthrough of what the protocol *does*, who the players are, and where the money comes from. Reads top-to-bottom — each section assumes the previous one. If you've never touched a perpetual DEX before, this is the one to read.

For the technical contract walkthrough, see `Readme.md`. For the exact knobs and their valid ranges, see `risk-parameters.md`.

---

## 1. The 60-second pitch

Stellars Finance is a **perpetual exchange** on Stellar. Two groups of users meet at a single pot of money called the **Vault**:

- **LPs (liquidity providers)** deposit USDC into the Vault. They earn yield from trading activity. They are *the house*.
- **Traders** post USDC as collateral and bet on the price of an asset (BTC, ETH, …) going up or down — with leverage, e.g. 10× their collateral. They are *the players*.

There is no order book. The protocol uses an **oracle** (median of CEX prices) for entries and exits. Traders' wins come out of the Vault; traders' losses go into the Vault. LPs are the counterparty to every open position, and the Vault auto-compounds their share price (`sLP`) as the protocol takes fees and absorbs trader PnL.

That's the whole game. Everything below is the wiring.

---

## 2. The five characters

Every action in the protocol is taken by one of these. Knowing who calls what makes the rest much easier to read.

### LP (liquidity provider)
- Deposits USDC into the **Vault**, receives `sLP` shares.
- Wants `sLP` price to go up. It does when (a) the protocol takes fees, and (b) traders, in aggregate, lose. It goes down when traders win, in aggregate.
- Can withdraw any time **after** the cooldown (5 min default) has elapsed AND only as much as the Vault has "free liquidity" for.

### Trader
- Approves the **PositionManager** to spend USDC.
- Calls `increase_position(symbol, size, collateral, is_long, ...)` to open or grow a Position.
- Calls `decrease_position(symbol, size_delta, ...)` to close (fully or partially), once the position is older than `min_position_lifetime` (60s default).
- Optionally calls `set_tp_sl(...)` to attach take-profit / stop-loss prices that anyone can execute on their behalf.

### Executor *(permissionless)*
- Anyone with a wallet and a watcher script. **Not a privileged role.**
- Calls `liquidate_position(trader, symbol)` when a Position's health drops below the threshold. Earns the **liquidation bounty** out of the trader's absorbed collateral.
- Calls `execute_order(trader, symbol)` when a TP or SL price triggers on the oracle. Earns the **TP/SL execution-fee escrow** the trader pre-paid.
- This is a market — many executors compete to be first.

### Keeper *(KEEPER role, granted by Admin)*
- A whitelisted bot network. Does the work that *must* keep happening but doesn't pay an on-trade bounty:
  - `update_indices(symbol)` — advances borrow and dominant-side skew accumulators.
  - `deleverage_position(trader, symbol)` — ADL, the emergency forced-close when the Vault would otherwise become insolvent. Keeper-gated because picking the target requires off-chain reasoning across all open positions.
- Distinct from oracle publishers (`ORACLE` role), so the price-publishing surface can be rotated independently.

### Admin / Pauser / Upgrader *(ConfigManager roles)*
- **Admin**: tunes risk parameters (FeeSplits, ProtocolLimits, CarryingFeeConfig, FeeConfig), grants other roles, claims accrued protocol fees.
- **Pauser**: hits the emergency stop on Vault or PositionManager. Can also veto a pending upgrade.
- **Upgrader**: proposes and executes WASM upgrades. Subject to a 24h timelock.

---

## 3. Where the money lives

There is one Vault. It holds USDC. Every other balance is an accounting line *inside* that Vault, not a separate pot.

Inside the Vault:

- `total_assets` — the actual USDC balance. Everything else is a slice of this.
- `reserved_usdc` — earmarked to back open Position size. Cannot be withdrawn by LPs.
- `unclaimed_fees` — protocol revenue (dev + staker slice) awaiting admin claim. Cannot be withdrawn by LPs.
- `net_global_trader_pnl` — the running mark-to-market of all open trader positions. Positive ⇒ traders are winning and the Vault expects to pay them out. PositionManager pushes amount updates on position / index changes; LP exits require a recent full-book sync across every open market.

What LPs can actually withdraw at any moment — "**free liquidity**" — is:

```
free_liquidity = max(0,
    total_assets
    - reserved_usdc
    - unclaimed_fees
    - max(0, net_global_trader_pnl)
)
```

In plain English: take the wallet balance, subtract what's backing trades, subtract pending fees, subtract what traders are currently owed if they all closed at once, and never go negative. That's what's available.

The `sLP` token (ERC-4626) is a share of `total_assets` — gross, *including* the currently-earmarked `unclaimed_fees`. Until the admin claims, the unclaimed pool sits inside `total_assets`, and sLP price reflects it. When the admin calls `claim_fees`, both `total_assets` and `unclaimed_fees` drop by the same amount — sLP price drops by exactly the claimed portion. What an LP can actually *withdraw* at any moment is gated by `free_liquidity` above, which nets unclaimed_fees out, so LPs never withdraw money that's been earmarked for the protocol revenue pool.

---

## 4. Where the yield comes from

LPs earn yield from **five distinct streams**. Knowing them by name will save you a lot of confusion when reading numbers.

### Stream 1 — Open fee
Every time a trader opens or increases, they pay `size * open_fee_bps / 10_000` USDC up front (0.1% by default). 90% stays in the Vault as LP yield; 10% accrues to protocol revenue. Charged on **notional**, not on collateral — a 50× leveraged Position pays 50× the fee of an unleveraged one of the same collateral.

### Stream 2 — Borrow fee
A position open over time pays a continuous borrow rate that depends on Vault utilization:

- Below 80% utilization, the rate is gentle. At the 80% kink it's 5% APR by default (`base 1% + slope1 5% × 80% = 1% + 4% = 5%`; the formula is `base + slope1 * u / 10_000`).
- Above 80%, the marginal rate jumps to slope2 (default 50%, capped at 200% by upgrade), so each extra percent of utilization adds another 0.50% APR rather than 0.05%.

This compensates LPs for the *opportunity cost* of capital that's been earmarked but not deployed. It also makes utilization self-correcting: too many positions ⇒ borrow rate rockets ⇒ marginal traders close ⇒ utilization comes back down.

Sliced 90/10 LP/protocol at Close time.

### Stream 3 — Skew carrying fee
When one direction dominates, positions on that side pay LPs an additional fee for as long as the imbalance persists. It is not charged at open. The annualized rate grows quadratically with OI concentration and linearly with Vault utilization, up to 50% APR by default. Balanced markets pay zero. The minority side receives no credit, so the model cannot create an unbacked trader receivable. Collected skew fees stay entirely with LPs.

### Stream 4 — Net trader PnL absorbed
The big one. When a position closes underwater (health < 0 or below threshold on liquidation), the trader's collateral is absorbed into the Vault. When a position closes profitable, the Vault pays them. **In aggregate, over time, the LPs are the house.** Traders' losses *are* LP yield. Traders' wins reduce it.

This is also why the protocol enforces a `max_utilization_ratio` (85% default) and ADL — so a winning streak by traders cannot strand the LPs.

### Stream 5 — Forfeited TP/SL escrows
When a trader sets TP or SL, they pay a flat **execution-fee escrow** (default 0.5 USDC at 7 decimals; admin-tunable up to 10_000 USDC) up front, held on the Position itself. Where it goes at Close depends on the Close kind:

- User closes fully, or ADL fires → refunded to trader.
- TP or SL triggers → paid to the Executor.
- Position is liquidated → forfeited to the Vault, sliced 90/10 like other revenue.

So in the liquidation case, both the open-fee revenue and the leftover TP/SL escrow flow to LPs (after the bounty is paid).

---

## 5. Where the *trader* yield comes from (and what it costs)

Traders make money when:
- The mark price moves in their favour enough to exceed open and accumulated carrying fees.

Traders pay:
- **Open fee** — every Increase. 0.1% of notional.
- **Borrow fee** — continuous, scales with utilization. The fee builds quietly on the `acc_borrow_index` and is settled at Close.
- **Skew carrying fee** — continuously paid only while the position is on the dominant side; settled at Close and routed to LPs.
- **TP/SL execution-fee escrow** — flat 0.5 USDC by default, charged the first time TP or SL is set. Refunded on full user close, paid to Executor on TP/SL trigger, forfeited on liquidation.
- **Liquidation bounty** (only if liquidated) — 1% of collateral by default, paid out of the trader's seized collateral before LP/protocol see anything.

There are no taker / maker fees because there is no order book.

---

## 6. The five user stories

### 6.1 LP: deposit, hold, withdraw

```
1. Alice approves Vault to spend 1_000 USDC.
2. Alice calls Vault.deposit(1_000 USDC, Alice, Alice, Alice).
   → Vault transfers USDC in, mints sLP at the current share price.
   → Vault records Alice's LockupExpiresAt = now + 5 min.
3. Time passes. The Vault's total_assets grows from trading fees and shrinks
   when traders win. sLP price tracks it.
4. After the cooldown elapses, Alice calls Vault.withdraw(500 USDC, Alice, Alice, Alice)
   if and only if free_liquidity >= 500 USDC AND her sLP balance covers the redeem.
```

Two things to know:
- **The cooldown is frozen at deposit time.** If admin changes `cooldown_duration` afterwards, Alice's existing lock is unaffected. Re-depositing extends the lock to `now + cooldown_duration`.
- **The cooldown propagates on `sLP` transfer.** If Alice sends `sLP` to Bob, Bob's expiry becomes `max(Bob's expiry, Alice's expiry)`. This kills the "send-to-fresh-address" cooldown bypass.

### 6.2 Trader: open, hold, close

```
1. Bob approves PositionManager to spend his USDC.
2. Bob calls PositionManager.increase_position(
       trader=Bob, symbol="BTC", size=50_000 USDC, collateral=5_000 USDC,
       is_long=true, take_profit=0, stop_loss=0, acceptable_price=...
   )
   → PM refreshes the BTC market's borrow + side-specific skew indices.
   → PM checks leverage (10x here, OK if max_leverage >= 10).
   → PM checks utilization cap (85% default).
   → PM pulls 5_000 USDC collateral + 50 USDC open fee from Bob.
   → PM forwards 50 USDC to the Vault (45 USDC to LP slice, 5 USDC to unclaimed_fees).
   → Vault reserves 50_000 USDC.
   → Position stores additive base exposure plus borrow/skew fee-debt baselines.
3. Time passes. Indices grow → Bob's borrow fee accrues silently.
4. Bob calls PositionManager.decrease_position(Bob, "BTC", 50_000, acceptable_price=...)
   → Must be >= 60s after the last increase.
   → PM evaluates PnL from additive base exposure and subtracts borrow + skew fees:
     effective_health = collateral + pnl - borrow_fee - skew_fee.
   → If effective_health > collateral, Vault pays the excess to Bob. Otherwise PM
     returns Bob's share and absorbs the rest into the Vault.
   → Borrow fees are sliced 90/10 LP/protocol; collected skew fees stay 100% with LPs.
   → Vault releases the 50_000 USDC reservation.
```

Fee accrual never iterates over all positions. Global indices integrate rates over time, while each position stores fee debt. Increasing a position adds debt at the current index, preserving fees already accrued on the older slice.

### 6.3 Trader: TP/SL with permissionless execution

```
1. Bob is long BTC at entry 60_000. He calls
   PositionManager.set_tp_sl(Bob, "BTC", take_profit=70_000, stop_loss=55_000).
   → PM pulls 0.5 USDC escrow from Bob (default `tp_sl_execution_fee`), stored on the Position.
2. Bob walks away. Carol's bot watches the BTC oracle.
3. BTC pumps to 70_000. The next oracle update has mark_price >= 70_000.
4. Carol calls PositionManager.execute_order(Carol, Bob, "BTC").
   → PM evaluates Bob's Position at the current mark.
   → Bob is paid out — he keeps his gains.
   → Carol is paid the 0.5 USDC escrow.
```

If the same scenario instead happens with stop_loss triggering at 55_000, Bob still gets paid out his (reduced) collateral, and Carol still gets the escrow. The path is symmetric.

### 6.4 Executor: liquidation

```
1. Bob is long BTC at 60_000 with 5_000 collateral, 50_000 size (10×). BTC dumps to 54_000.
2. Bob's PnL = 50_000 * (54_000 - 60_000) / 60_000 = -5_000 USDC.
3. Plus accumulated borrow and skew carrying fees. effective_health goes negative.
4. Once effective_health < collateral * liquidation_threshold_bps / 10_000 (default 2%, i.e. < 100 USDC),
   Carol's bot is allowed to call:
   PositionManager.liquidate_position(Carol, Bob, "BTC").
5. PM seizes Bob's 5_000 USDC collateral.
   → Carol gets liquidation_bounty = min(5_000 * 100 / 10_000, absorbed) = 50 USDC.
   → The remaining absorbed_collateral goes into the Vault.
   → The bounty has *priority* over the revenue split — LPs only see what remains after Carol.
   → The Position is deleted, its reservation released.
```

Note: there is no whitelist. Carol is whoever wins the race. The bounty is the auction prize.

### 6.5 Keeper: ADL (emergency forced profit-take)

ADL exists for one scenario: traders are winning so much that the Vault would soon be unable to pay everyone out. The protocol forcibly closes the *most profitable, highest-leverage* winning position at the current oracle price, paying the trader in full, and freeing up reservation + reducing the Vault's liability.

ADL is the *only* close path keepers control directly (the trigger condition requires off-chain ranking across positions).

```
1. Keeper bot computes the off-chain ranking and picks Bob's profitable BTC long.
2. Keeper calls PositionManager.deleverage_position(keeper, Bob, "BTC").
3. PM checks: combined_pnl / safe_basis > adl_pnl_bps (90%) OR
              reserved / safe_basis > adl_utilization_bps (95%).
   If neither is breached, the call reverts.
4. PM checks: Bob's PnL > 0. (You can only ADL a winner.)
5. PM pays Bob out fully (collateral + PnL - fees), releases his reservation,
   refunds his TP/SL escrow if any. No bounty — keepers do this for the
   protocol's health, not for a per-call payout.
```

The trader is not penalized — they get what they would have gotten from a full close at the current mark — but they don't get to choose the timing.

---

## 7. The risk safety belt — how the protocol stays solvent

In order of "fires first":

1. **min_collateral** floors how small a Position can be. Stops dust.
2. **max_leverage (per market)** caps `size <= collateral * max_leverage`. Stops a single Position from being so leveraged that liquidation can't recover anything.
3. **max_utilization_ratio** stops new positions when `reserved / safe_basis` would exceed 85%. The Vault always keeps a buffer.
4. **liquidation_threshold_bps** triggers liquidation when health drops below 2% of collateral — early enough to leave room for the bounty.
5. **Borrow rate kink** ramps fees steeply above 80% utilization, *paying* traders to close before things get tight.
6. **ADL triggers** (adl_pnl_bps = 90%, adl_utilization_bps = 95%) — last line of defence. Keepers force-close profitable positions to prevent insolvency. Never used in normal operation.
7. **LP cooldown_duration** (5 min default) — LPs can't flash-withdraw right before a known oracle update.
8. **min_position_lifetime** (60s default) — traders can't open-and-close in the same oracle window.
9. **Oracle deviation gate** — `max_deviation_bps` limits how far registered sources can drift from the median when the market is configured with multiple oracle sources.
10. **Upgrade timelock** (24h, immutable floor) — there is no way for the admin to push a hostile WASM and have it take effect before users can withdraw.
11. **Pause** — emergency stop. Even paused, traders can close (`decrease_position`) and bad debt can still be liquidated.

You can think of it as concentric circles. The trader's bad day costs them their collateral first; the bounty pays the executor next; the LPs absorb anything left; and ADL is the trap door beneath all of it for when the math says "this isn't sustainable."

---

## 8. Worked example — full LP+Trader+Executor lifecycle

Assume defaults throughout. Vault starts with 1_000_000 USDC, sLP price = 1.0.

### t=0: Alice deposits

Alice puts in 100_000 USDC. She gets ~100_000 sLP. `total_assets = 1_100_000`. Her lockup expires at `t + 5 min`.

### t=10s: Bob opens

Bob longs BTC at 60_000. Size = 50_000 USDC, collateral = 5_000 USDC (10×). TP at 66_000.

- Open fee = `50_000 × 10 / 10_000 = 50 USDC`.
- TP/SL escrow = 0.5 USDC (default `tp_sl_execution_fee`).
- Bob's wallet pays 5_000 (collateral) + 50 (open fee) + 0.5 (escrow) = **5_050.5 USDC** to PositionManager.
- PM forwards the 50 USDC open fee to the Vault. The 5_000 collateral and 0.5 USDC escrow stay on PM.
- Vault now has `total_assets = 1_100_050` USDC, of which `reserved = 50_000` and `unclaimed_fees = 5` (the 10% dev slice of the 50 USDC open fee). The LP slice (45 USDC) is the implicit remainder of `total_assets` and bumps sLP price.

### t=10 min: index update

Keeper has been calling `update_indices` every minute. Utilization is 50_000/1_100_050 ≈ 4.5% — well below the 80% kink — so the borrow rate is `base + slope1 * u / 10_000 = 100 + 500 * 455 / 10000 ≈ 123 bps ≈ 1.23% APR`. Over 10 minutes Bob has accrued roughly `50_000 × 0.0123 × 600 / 31_536_000 ≈ 0.012 USDC` of borrow fee. Negligible.

### t=20 min: BTC pumps to 66_000

Now the next oracle tick sees `mark >= take_profit`. Carol's bot fires `execute_order`:

- Bob's `pnl = 50_000 × (66_000 − 60_000) / 60_000 = 5_000 USDC` of profit.
- Borrow fee ≈ 0.025 USDC; skew fee is zero if OI is balanced.
- `effective_health = collateral + pnl − borrow_fee − skew_fee = 5_000 + 5_000 − 0.025 − 0 = 9_999.975 USDC`.
- Settlement: PM returns Bob's 5_000 collateral; Vault tops up the additional 4_999.975 USDC profit. Bob's net = `+4_999.975` on a 5_000 stake. Good day.
- Carol gets the 0.5 USDC escrow (Bob's prepaid TP/SL fee).
- The 0.025 USDC borrow fee is `reslice_revenue`'d 90/10 → 0.0025 USDC to `unclaimed_fees`, the rest stays in `total_assets` as LP yield.
- Vault `total_assets` dropped by ~5_000 USDC (Bob's profit payout) minus the few-cents fee revenue.

**Net for Alice's LP position**: she owns roughly `100_000 / 1_100_000 ≈ 9.09%` of `sLP`. The Vault paid Bob ~5_000 USDC of profit, so Alice's NAV drops by `5_000 × 9.09% ≈ 454 USDC`. She gained roughly `0.022 USDC × 9.09% ≈ 0.002 USDC` from her LP slice of the borrow fee. Net: she's down ~454 USDC. The protocol counts on many traders averaging out over time.

### Alternative: BTC dumps to 54_000 instead

At t=20 min BTC is at 54_000:

- Bob's pnl = `50_000 × (54_000 − 60_000) / 60_000 = −5_000 USDC`. Collateral was 5_000.
- `effective_health = 5_000 + (−5_000) − 0.025 + 0 ≈ −0.025 USDC`.
- Below the liquidation threshold (`5_000 × 200 / 10_000 = 100 USDC`). Carol's bot calls `liquidate_position`.
- `trader_payout = max(0, −0.025) = 0`, so `pm_to_vault = collateral_delta − 0 = 5_000 USDC`.
- Carol's bounty = `min(collateral × 100 / 10_000, pm_to_vault) = min(50, 5_000) = 50 USDC`. Capped at the raw bounty, not by `pm_to_vault` here.
- `vault_absorbed = pm_to_vault − bounty = 5_000 − 50 = 4_950 USDC`. Vault gains 4_950 from PM.
- TP/SL escrow (0.5 USDC) is forfeited to the Vault as revenue and sliced 90/10 (0.05 USDC to unclaimed_fees, 0.45 USDC to LP).
- Borrow fee distributable = `min(0.025, 4_950) = 0.025` USDC, sliced 90/10.
- Vault `total_assets` ends at roughly `1_100_050 + 4_950 + 0.5 = 1_105_000.5` USDC.

**Net for Alice**: her 9.09% share of the Vault grows by ~`(4_950 + 0.5) × 9.09% ≈ 450 USDC`. Good day for LPs.

---

## 9. Common confusions, cleared up

- **"Are LPs paying keepers?"** Almost never. The liquidation bounty comes out of the trader's seized collateral, *not* LP capital. The TP/SL escrow is pre-paid by the trader. Keepers running ADL don't get paid per-call — they run as a service for protocol health. The "5% to keepers" line from older docs no longer reflects the current design; `keeper_bps` no longer exists.

- **"What's the difference between Keeper and Executor?"** Keeper is a *role* — a whitelist run by Admin. They handle `update_indices` and `deleverage_position`. Executor is *anyone* — they handle `liquidate_position` and `execute_order` and get paid per call. Different incentive models, different permission models.

- **"Why are there different fee routes?"** Open and borrow fees use FeeSplits. Skew fees stay with LPs because they compensate directional risk. Execution bounties pay the person submitting a permissionless close.

- **"What is `safe_basis`?"** A denominator used for utilization and ADL ratios: `total_assets - unclaimed_fees`. Excludes pending fees (which would distort the cap) and excludes net PnL (which would make oracle wicks bias the cap). Always ≤ total_assets.

- **"Can the admin steal LP money?"** The admin can claim `unclaimed_fees` — but those are dev+staker revenue, not LP capital, and that pot is bounded by `total_assets - reserved` so it cannot dip into LP funds. The admin can change risk parameters, but they cannot change LP balances directly. Upgrades take 24h, so an LP can withdraw before a hostile change activates.

- **"Why does the cooldown propagate on transfer?"** Without it, Alice could deposit, wait 1 minute, send her `sLP` to a fresh address, and have that address withdraw immediately. The propagation closes that loop.

- **"Why is there a 60s position lifetime?"** Pure oracle anti-front-run. Without it, you could open a Position the millisecond after a known CEX move and close it on the next on-chain oracle tick for free PnL. 60s of real market risk kills that arbitrage.

---

## 10. Glossary (in one place)

- **Vault** — the single USDC pool that backs everything. Issues `sLP` shares.
- **PositionManager** — the trading engine. Tracks every Position, every Market, every index.
- **OracleRouter** — median of multiple SEP-40 sources, with a deviation gate and a cache.
- **ConfigManager** — the governance / role store; the source of truth for FeeSplits, ProtocolLimits, CarryingFeeConfig, FeeConfig.
- **sLP** — the ERC-4626 share token. Auto-compounds.
- **Position** — one trader's exposure on one Market: size, collateral, additive base exposure, fee debts, direction.
- **Market** — one symbol's long/short OI, additive exposures, and carrying-fee accumulators.
- **MarketTick** — a frozen snapshot of a Market at a moment in time, used to evaluate Positions consistently.
- **Reservation** — USDC earmarked in the Vault behind open size. Not withdrawable by LPs.
- **Open fee** — fee on Increase. Revenue.
- **Borrow fee** — continuous fee on open size, utilization-driven. Revenue.
- **Skew carrying fee** — time-based dominant-side surcharge paid to LPs; no minority-side credit.
- **Liquidation bounty** — payout to the Executor on Liquidation. Funded from absorbed collateral. Has priority over the revenue split.
- **TP/SL execution-fee escrow** — flat USDC the trader pre-pays when setting TP or SL. Goes to refund / Executor / Vault depending on Close kind.
- **Revenue split** — `FeeSplits { lp_bps, dev_bps, staker_bps }`. Applies to every revenue dollar. Default 90/10/0.
- **Execution bounties** — the per-call payments to whoever calls a permissionless close. `FeeConfig { open_fee_bps, liquidation_bounty_bps, tp_sl_execution_fee }`.
- **ADL** — Auto-Deleverage. Keeper-only emergency forced profit-take.
- **Cooldown** — LP-side lockup from deposit to first withdraw.
- **min_position_lifetime** — anti-front-run lock from open to close.

That's the protocol. Everything else is implementation detail.
