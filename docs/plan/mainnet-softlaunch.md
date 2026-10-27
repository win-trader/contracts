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

Generate every key fresh for mainnet. Testnet keys have sat in Coolify, some
of them as build arguments of old images; none of them may hold a mainnet role.

### Multisig for the cold roles

A role is an ordinary Stellar account, and Soroban's `require_auth` on an
account checks its signers against the account's medium threshold. So the
cold roles (admin, upgrader, unpauser, oracle, protocol) become native
multisig accounts with no contract change. The pauser stays a single key: the
kill switch has to work alone, and a pause cannot take funds. The keeper and
the publishers stay single hot keys.

After the deploy, for each cold role (2-of-3 shown; `ROLE` is the account,
`S1`–`S3` the signers, e.g. two Ledgers and an offline backup):

```bash
# 1. Add the signers. The master key still signs alone here.
for s in $S1 $S2 $S3; do
  stellar tx new set-options --source-account ROLE --signer $s --signer-weight 1 --network mainnet
done
# 2. Require two signatures for everything, and retire the master key in the
#    same transaction: from here only the signers can act.
stellar tx new set-options --source-account ROLE \
  --low-threshold 2 --med-threshold 2 --high-threshold 2 --master-weight 0 --network mainnet
```

Check the account on Horizon (`/accounts/ROLE`: three signers of weight 1,
thresholds 2, master weight 0) before moving on to the next role. A mistake in step 2 locks the account for good, so do it on
testnet first with the same commands.

To act as a multisig role, build the transaction, simulate it (which adds the
Soroban resources and fee), let each signer sign the envelope, then submit:

```bash
stellar contract invoke --source-account ROLE --build-only --network mainnet \
  --id <contract> -- <function> ... \
  | stellar tx simulate --network mainnet > tx.xdr
stellar tx sign --sign-with-ledger --network mainnet < tx.xdr > tx1.xdr   # signer 1
stellar tx sign --sign-with-key s2 --network mainnet < tx1.xdr > tx2.xdr  # signer 2
stellar tx send --network mainnet < tx2.xdr
```

The role is the transaction source, so the envelope signatures authorize the
call. A built transaction expires with its time bounds; collect the
signatures within a few minutes, or rebuild.

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
