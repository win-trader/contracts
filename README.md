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
- `make upgrade-local` / `make upgrade-testnet`, `make grant-keepers`, `make add-market`

> The `Makefile` still contains service targets from before the split
> (`indexer`, `keeper`, `api`, `frontend`, `server`, `db-*`, `backend-*`, `sim`)
> that reference packages now living in the sibling repos. Those are stale and
> pending removal — use the contract, bindings, and deploy targets above.

## Publishing

`@win-trader/bindings`, `@win-trader/protocol-math`, `@win-trader/protocol-clients`,
and `@win-trader/config` publish to **public npm**. Bump the version in the relevant
`package.json`, then push a `v*` tag (or run the **publish** workflow manually).
CI builds and publishes via the `NPM_TOKEN` repo secret. Already-published versions
are skipped, so re-running is safe.

## Addresses

Deployed contract addresses live in `packages/config/addresses.json` per network.
At runtime, consumers can override the source with the `ADDRESSES_JSON` env var
(path to a JSON file); the in-package file is the local-dev fallback.
