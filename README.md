# win-trader / contracts

Soroban smart contracts (Rust) for the wintrader perpetual DEX, plus the
generated TypeScript bindings and protocol TS libraries that the rest of the
system consumes from npm.

This is one of four repos:

- **contracts** (this repo) — contracts + publishes `@win-trader/{bindings,config,protocol-math,protocol-clients}`
- **offchain** — indexer · keeper · simulation · publishes `@win-trader/data`
- **oracles** — CEX / third-party price publishers
- **app** — API (Bun + Hono) + frontend

## Layout

- `contracts/` — Rust workspace: `vault`, `position-manager`, `config-manager`, `oracle-router`, `oracle`, `shared`, `interfaces`
- `mocks/` — `mock-token`, `mock-oracle` (test-only contracts)
- `test-suites/` — integration + fuzz tests
- `packages/bindings/` — `@win-trader/bindings`: generated TS clients (one per contract), produced by `make bind`
- `packages/protocol-math/` — `@win-trader/protocol-math`: pure TS mirror of on-chain math (quotes, fees, PnL, liquidation price), no network calls
- `packages/protocol-clients/` — `@win-trader/protocol-clients`: helpers to instantiate a binding against a network + signer
- `packages/config/` — `@win-trader/config`: network/address registry (`addresses.json`) + protocol constants
- `scripts/` — deploy and admin scripts (deploy, upgrade, grant-keepers, add-market, provision-keys)

## Prerequisites

- Rust + `wasm32v1-none` target, and the `stellar` CLI
- Node ≥ 18 and `pnpm`

## Common commands

Contracts (Rust):

- `make build` — compile contracts to WASM
- `make optimize` — optimize the WASM with `stellar contract optimize`
- `make test` — run the Rust test suite
- `make bind` — `optimize` + generate and build the TS bindings into `packages/bindings/`

TS packages:

- `pnpm install`
- `pnpm -r build` — builds `config`, `protocol-math`, `protocol-clients`, and the bindings
- `pnpm -r typecheck`

Local network + deploy:

- `make up` / `make down` / `make reset` — local Stellar network
- `make deploy` / `make deploy-testnet` / `make deploy-mainnet` — deploy and record addresses
- `make cex-oracles-testnet` — deploy and wire Binance/KuCoin oracle contracts on testnet
- `make deploy-testnet-full` — provision testnet keys, deploy core contracts, then deploy/wire CEX oracles
- `make upgrade-local` / `make upgrade-testnet`, `make grant-keepers`, `make add-market`

## Publishing

`@win-trader/bindings`, `@win-trader/protocol-math`, `@win-trader/protocol-clients`,
and `@win-trader/config` publish to **public npm**. Bump the version in the relevant
`package.json`, then push a `v*` tag (or run the **publish** workflow manually).
CI builds and publishes via the `NPM_TOKEN` repo secret. Already-published versions
are skipped, so re-running is safe.

## Addresses

`make deploy*` records contract addresses in two places:

- `deployments/<network>.json` — the **canonical** per-network artifact that
  off-chain services inject at runtime via the `ADDRESSES_JSON` env var (a path
  to the file). A testnet deploy refreshes only `deployments/testnet.json`.
- `packages/config/addresses.json` — the combined all-networks file shipped in
  `@win-trader/config` as the local-dev fallback.

`scripts/split-deployments.sh <network>` regenerates a network's file from
`addresses.json`; the deploy scripts call it automatically. A service sets
`NETWORK` + `ADDRESSES_JSON=…/deployments/<network>.json`; with neither set, the
config loader falls back to the in-package combined file.

## Testnet deployment handoff

Run the on-chain testnet deploy from this repo:

```bash
make deploy-testnet-full
```

That produces:

- `deployments/testnet.json` — copy this to the VPS beside each Docker stack
  as `deployments/testnet.json`.
- `.env.testnet` — contains generated service secrets. Copy only the values the
  service needs into the sibling repo `.env.testnet` files:
  `KEEPER_SECRET` for `offchain`, and `BINANCE_ORACLE_SECRET` /
  `KUCOIN_ORACLE_SECRET` for `oracles`.

Do not commit `.env.testnet`.
