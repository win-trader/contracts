# Known issues and accepted risks

These are deliberate design choices or residual risks the team has accepted.
They are listed so audit time goes to what is unknown. Each refers to the
finding in [`THREAT_MODEL.md`](../../THREAT_MODEL.md), which has the full
analysis. A finding that shows one of these is *worse* than described here is
in scope and welcome.

## Trust assumptions

### K-1. The price feed is fully trusted (T-01)

Prices come from the in-house `cex-feed` contract: the median of the
Binance and KuCoin publishers that published within `max_source_age_seconds`,
or no price below `min_sources` or past `max_deviation_bps` (T-27). Each
source is stamped `min(observed_at, ledger time)`: the publisher states when
it read the exchange, and the stamp is never later than the ledger, so a
price that lands late cannot pass as observed after a commit it predates. A
publisher can make its own prices look older, never newer. The PositionManager
checks only that a price is positive, not stamped in the future, and no older
than `max_price_age_seconds`. So a quorum of publisher keys is trusted: if
`min_sources` of them lie together, a wrong price settles orders, liquidates
positions, and moves value between traders and LPs. One lying publisher
against an honest one can move the two-source median by up to
`max_deviation_bps` (50 bps by default), because the median of two is their
average. The feed address can only change through the governor's timelock;
the feed's publisher set only through its own.

ORACLE and ADMIN can each remove a publisher at once. That never adds trust,
but it is not free: below `min_sources` it stops pricing until a new set
clears the feed's timelock (at least a day), and with three or more
publishers the choice of which one to drop moves the median. Both roles are
trusted (K-2, E-4).

Operating requirements:

- Keep `min_sources` at 2. At 1, either publisher key alone sets the price.
  `deploy.sh` refuses less on mainnet, and refuses a publisher that is the
  admin, the keeper or a role holder.
- Run the publisher hosts on NTP. A clock behind the ledger makes prices
  look older than they are (fills wait longer); one ahead is clamped.
- A fill needs an observation stamped *after* the commit (§8.6); the feed
  stamps the oldest counted source, so every counted publisher must publish
  after the commit. The publishers force a publish every 20 seconds, so
  `max_market_order_lifetime` (300 s) and `max_price_age_seconds` (60 s)
  leave room; `max_source_age_seconds` (30 s) must stay below the latter.
- With `min_sources = 2`, either exchange or publisher going down stops
  settlement, liquidation and LP resolution until it returns (D-2). That is
  the price of not trusting one key. Alert on publisher lag.
- Registered market symbols must match the tickers the publishers push
  (`BTCUSD`, `ETHUSD`, `XLMUSD`).

### K-2. Upgrade authority can replace any contract's code (T-03)

The UPGRADER role proposes and executes WASM upgrades for every contract
after a timelock: at least one day for the ConfigManager, otherwise
`max(upgrade timelock, config_timelock_seconds)`, two days by default.
PAUSER can cancel a pending upgrade. An upgrade can do anything, including
moving vault cash. Safety rests on custody of the UPGRADER key (intended as
a multisig) and on someone watching `upgprp` events during the window.

### K-3. A compromised MarketGovernor can install any valid config at once (T-24)

The PositionManager accepts configuration only from its governor, but
installs whatever the governor sends that passes the PM's own checks:
per-config validity, the market-count cap, and the hard-cap sum. Within those
bounds, fees, margins and caps can move instantly. The governor's address
is fixed at construction, and its upgrade delay is never shorter than the
config timelock.

### K-4. Some admin settings take effect instantly (T-04)

These are role grants (ADMIN cannot grant ADMIN), the LP deposit NAV gate,
the LP request delay (capped), `set_upgrade_timelock` (floored at one day),
and pause and unpause. `max_withdraw_utilization_bps` is floored at 50%, so
withdrawals cannot be frozen by config. Conservative risk changes (lower
capacity, tighter caps, higher initial margin) and brand-new markets also
install immediately, by design.

## Mechanism design

### K-5. Keepers choose when to settle and whom to deleverage (T-09)

Settlement is permissionless. Whoever settles picks the moment, and so which
fresh observation fills the order, within its execution window. Decreases
and closes have no expiry. ADL may close any profitable position on a side
in ADL or HardCap; no ordering is enforced. The protocol relies on competing
keepers settling promptly.

### K-6. A pending decrease or close blocks further mutations until settled (T-10)

A position holds one pending decrease or close at a time, with no cancel and
no expiry. If settlement returns a non-terminal outcome (`RequiresLiquidation`,
`NotReady`), the action stays pending until a keeper retries it. Until then
the owner cannot submit another decrease or close; TP/SL, liquidation and
ADL still work. The invariant walk reproduces this. Keepers must retry.

### K-7. LP share pricing ignores unrealized trader losses (T-12)

`vault_nav = cash LP equity − Σ positive side PnL`. Sides with net losses,
and fees accrued but not yet collected, are not counted. That is conservative
for withdrawals, but prices deposits below fair value when traders are net
losing, transferring value from existing LPs to depositors who time their
requests. The 24h request delay and the deposit NAV gate limit it.

### K-8. Funding can be pushed by a short-lived large position (T-f)

`instant_weight_bps` (default 30%) mixes live skew into the funding integral
alongside the 12h EMA. A large one-sided position held briefly moves the
rate for that whole side, limited by fees, `min_position_lifetime` (60s),
and open-interest caps.

## Liveness

### K-9. A dead feed for a live market stops exits, liquidations and LP flows (D-2, T-02 residual)

Every settlement, liquidation and LP resolution needs a fresh price for the
markets it touches; LP resolution needs every market with open exposure.
Empty markets are skipped. While a live market's feed is stale, bad debt can
accrue and LP requests wait. PAUSER can `skip_head` a request that has been
resolvable for a day, which refunds it.

### K-10. Keepers must submit with an instruction leeway

`resolve_next` has passed simulation and then failed at submission with
`ResourceLimitExceeded`, because accrual covers more elapsed time when the
transaction lands than when it was simulated. A 2M instruction leeway was
enough on testnet. Measured costs at eight markets: settle 23–31M,
liquidation 25M, LP resolution 100M, against a 400M budget.

## Accounting conventions

### K-11. Rounding always favours the protocol

Rounding is floor or ceil, chosen against the trader and LP. Fee splits floor
the LP share and give the residue to the protocol. Rescaled feed prices floor.
Dust is intentional and covered by the conservation tests.

### K-12. The collateral token's issuer can freeze the vault (T-16)

The vault holds USDC through its Stellar Asset Contract and checks only
`decimals() == 7`. The issuer can freeze or claw back balances; that is
outside the protocol's control.

## Engineering constraints

### K-13. PositionManager size headroom is about 3 KB

The optimized PositionManager is 128,027 bytes against the network's
131,072-byte cap (the build fails above it). A fix that adds substantial code
may need to be offset elsewhere. The fallback is the standalone binaryen
optimizer, which saved 5.5 KB in testing.

### K-14. Some views exist only in test builds

`pending_fees`, `pending_receiver_funding_total`, `protocol_claimable_total`,
and `unclaimed_payout_total` compile only with the `testutils` feature. The
same totals are visible through `accounting_snapshot` and the event stream.
