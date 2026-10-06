# Known issues and accepted risks

These are deliberate design choices or residual risks the team has accepted.
They are listed so audit time goes to what is unknown. Each refers to the
finding in [`THREAT_MODEL.md`](../../THREAT_MODEL.md), which has the full
analysis. A finding that shows one of these is *worse* than described here is
in scope and welcome.

## Trust assumptions

### K-1. The price feed is fully trusted (T-01)

Prices come from a third-party SEP-40 provider. The PositionManager checks
only that a price is positive, not stamped in the future, and no older than
`max_price_age_seconds`. It has no deviation bound, confidence interval, or
second source, and the provider stamps its own timestamps. A wrong price
settles orders, liquidates positions, and moves value between traders and
LPs. The feed address can only change through the governor's timelock.

Integration requirements this places on the provider and the configuration:

- The feed must implement SEP-40 `lastprice(Asset)` and `decimals()`. Markets
  are read as `Asset::Other(<market symbol>)`, so registered market symbols
  must match the provider's tickers exactly.
- `decimals()` must be 18 or fewer. Prices are rescaled to 7 decimals,
  flooring when digits are dropped.
- `max_price_age_seconds` must exceed the provider's resolution plus its
  publishing lag. SEP-40 feeds floor timestamps to their resolution
  (Reflector's default is 5 minutes), so the 60-second default would make
  most reads stale.
- A trader action fills only on an observation stamped *after* it was
  committed (§8.6), so fills wait up to one feed resolution.
  `max_market_order_lifetime` must exceed that, or market orders expire
  unfilled.

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
