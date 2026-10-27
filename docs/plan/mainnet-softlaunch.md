# Mainnet soft launch

A first mainnet deployment with real USDC at toy size: $1–$3 positions, 3x
leverage, $25 of open interest per side, the operator as the only LP. Its
purpose is to run every path (commit, settle, TP/SL, liquidation, LP queue,
feed outage) against mainnet fees and latency before the limits go up.

Parameters: [`deployments/mainnet-softlaunch.env`](../../deployments/mainnet-softlaunch.env).

## Before

1. The reviewed commit is tagged, and `make repro` hashes are recorded in
   `docs/audit/README.md`.
2. Testnet ran the same commit end to end: the cex-feed with both publishers,
   the ported indexer and keeper, and the app. Every keeper path executed at
   least once (market and limit open, decrease, close, TP, SL, liquidation,
   expiry, LP deposit and withdrawal).
3. Measured on testnet: XLM fee per publisher round and per keeper call. Set
   `KEEPER_REWARD` above the keeper's fee and `ORACLE_PUSH_ON_STALE_SEC` for an
   affordable publisher budget (two publishers × 86,400 / heartbeat
   transactions a day).

## Accounts

Ten accounts, each a separate key, all funded with XLM:

| Account | Holds | Hot? |
|---|---|---|
| admin | ADMIN; deploys | Hardware/multisig after deploy |
| upgrader | UPGRADER | Cold |
| pauser | PAUSER (also cancels upgrades) | Warm: the kill switch |
| unpauser | UNPAUSER | Cold |
| oracle | ORACLE (feed proposals) | Cold |
| protocol | PROTOCOL | Cold |
| binance-oracle, kucoin-oracle | cex-feed publishers | Hot, on the publisher host |
| keeper | settles; earns rewards | Hot, on the backend host |
| LP | the operator's USDC | Wallet |

`deploy.sh` refuses a mainnet run if any role equals admin or UPGRADER equals
PAUSER, and refuses a publisher equal to admin.

## Deploy

```bash
cd contracts
set -a; source deployments/mainnet-softlaunch.env; set +a
NETWORK_KEY=mainnet bash scripts/deploy.sh   # builds with `make repro` (Docker); asks you to type MAINNET
```

Run `scripts/deploy.sh` directly: `make deploy-mainnet` first runs the host
`make optimize`, which requires stellar CLI 27.0.0. The script verifies the
WASM hashes, wiring, roles, and a price per market, then writes
`packages/config/addresses.json` and `deployments/mainnet.json`.

Then, in order:

1. Start the publishers (`oracles`, `NETWORK=mainnet`) and wait until
   `cex-feed.lastprice` returns a price for every market.
2. Publish `@win-trader/config` with the mainnet addresses; roll it into
   `offchain`, `oracles`, and `app`.
3. Start the indexer and keeper (`offchain`, `NETWORK=mainnet`).
4. Deposit the LP seed (suggested $100–$200) through the request router and
   let the keeper resolve it.
5. Deploy the app with LP deposits hidden. Open, close, and liquidate a $1
   position yourself before telling anyone.

## Watch

- Publisher lag: either publisher silent for 30 s stops all settlement
  (K-1, D-2). Alert on it.
- Keeper: failed submissions, `ResourceLimitExceeded`, XLM balance.
- Indexer lag vs. the RPC's latest ledger.
- `BadDebt`, `PayoutDeferred`, and `RiskStateChanged` events.
- Vault cash vs. claims (`accounting_snapshot`).

## Kill switch

PAUSER calls `pause` on the PositionManager (and the vault for LP flows). A
pause stops the protocol taking on risk and never stops anyone shedding it
(spec §12.2): pending opens fail with a refund, while closes, decreases, TP/SL
and liquidations keep working. A compromised publisher: `cex-feed.remove_publisher` (ORACLE or
ADMIN) stops pricing immediately; replace it through `propose_sources`
(one-day timelock).

## Raising limits

`governor.propose_market_config` with higher `max_*_size_open_interest` or
lower `initial_margin_bps` waits `config_timelock_seconds` (one day here), then
anyone applies it. Lowering caps or raising margin applies immediately.
