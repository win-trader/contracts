# Monorepo → polyrepo split (win-trader)

Canonical reference for splitting this monorepo into four repos under the GitHub org **`win-trader`** (https://github.com/win-trader), with a full `@stellars/*` → `@win-trader/*` rebrand.

**Governing rule:** decouple *in place* inside this monorepo first (Phase 1), then move folders into fresh repos (Phase 2). Never both at once. The new repos live under `~/win-trader/`.

---

## Target architecture

Four repos, split on the two boundaries that matter — visibility (public vs private) and deploy unit:

- **`win-trader/contracts`** — *public*. Rust contract workspace + `mocks/` + `test-suites/`. Publishes the public TS packages. Owns the canonical address registry (`deployments/<network>.json`) and the deploy/admin scripts.
- **`win-trader/offchain`** — *private*. `indexer`, `keeper`, `simulation`, `migrations`, postgres sidecar. Publishes the private `@win-trader/data`. Owns the one-shot `migrate`.
- **`win-trader/oracles`** — *private*. `oracle-base`/`oracle-binance`/`oracle-kucoin` and future third-party adapters (e.g. DIA). Consumes only public packages.
- **`win-trader/app`** — *private*. `api` (Bun+Hono container) + `frontend` (ui).

Note on DIA / third-party oracles: if a source exposes its *own* on-chain oracle contract, integrating it is a **contracts-repo** change (register its address as an `OracleRouter` source + a `deployments` entry) with no publisher service. The `oracles` repo is only for sources where we must relay an off-chain feed on-chain (CEX polling).

### Published packages — 5 total, GitHub Packages, `@win-trader` scope

- Public (from `contracts`): `@win-trader/bindings` (generated), `@win-trader/protocol-math`, `@win-trader/protocol-clients`, `@win-trader/config`.
- Private (from `offchain`): `@win-trader/data` (drizzle schema + pool factory + LISTEN/NOTIFY channels; renamed from `db`).

The three protocol packages are kept separate (not merged into one `protocol`) — re-scope + re-home only, to minimise migration churn. Merging is a trivial later follow-up if ever wanted.

`bindings` vs `protocol`: `bindings` is the generated, mechanical ABI client (`stellar contract bindings typescript`), regenerated on every WASM/ABI change. `protocol-clients`/`protocol-math` are hand-written — the glue that instantiates a binding against a network/signer, plus pure TS math that re-derives on-chain computations (quotes/fees/PnL/liq price) with no network call.

### Dependency direction (acyclic)

- `frontend` → protocol-math/clients/config + bindings (public only)
- `oracles` → config/clients + bindings (public only)
- `api` → config + data
- `indexer`/`keeper` → config/clients + bindings + data

Verified at planning time: the runtime services already import **none** of each other, so no untangling is required — only re-homing.

---

## Key decisions and rationale

- **App stays a containerized Bun+Hono service** (Docker/Coolify), not Cloudflare Workers. The Workers/Hyperdrive/`./node`-split premise from earlier drafts was aspirational. A Workers rewrite, if ever, happens later *inside* the app package — the split does not account for it.
- **`addresses.json` is runtime-injected**, not versioned in any package. It's a deploy artifact that rotates per deploy. Canonical copy = committed `deployments/<network>.json` in `contracts` (the repo that performs the deploy is the only one that knows new addresses). Consumers read it via an env-pointed path (`ADDRESSES_JSON`).
- **Registry = GitHub Packages**, single `@win-trader` scope (scope must equal the org for GitHub Packages). Free. Every consuming repo carries a `read:packages` token (CI: `GITHUB_TOKEN`; local: PAT) — required even for public packages. No paid npm. Promote bindings/protocol to public npm only if an external integrator needs tokenless installs (non-breaking add).
- **Fresh snapshots** for all four repos; archive this monorepo private. History is verified secret-clean (only `.env.example` ever committed) but not carried forward; blame continuity lives in the archive.
- **pnpm** for workspaces/scripts everywhere; bun only as the TS runtime where it earns it. Each repo is internally a pnpm workspace. Cross-repo dev = a local pnpm override when a change spans repos — no meta-repo.
- **No image registry by default** — Coolify builds each repo's image from source on deploy. Pre-built images only if a real need appears, and then private GHCR only.
- **Publish on git tag** (`vX.Y.Z`), manual semver, no Changesets. Cross-repo bumps manual for now.
- **Secrets** (keeper/oracle signer seeds, `DATABASE_URL`, RPC creds) = per-repo Coolify env, never committed.

---

## Execution

### Phase 1 — decouple + rebrand in place (still one monorepo)

1. **Scope rename** `@stellars/*` → `@win-trader/*` across every `package.json` and import; regenerate lockfile; build green.
2. **`config`: externalize `addresses.json`.** Replace the in-package `readFileSync` default with a loader reading an env-pointed path (`ADDRESSES_JSON`); keep types + `constants.ts`. Repoint consumers (frontend, indexer, oracle-base, simulation). Reshape data toward `deployments/<network>.json`.
3. **`db` → `@win-trader/data`.** Rename; confirm export surface (schema + `getDb`/`getPool` + `CHANNELS`); no `.`/`node` split (containerized). Repoint api, indexer, keeper, simulation.
4. **Acceptance gate:** full workspace builds + typechecks + all tests green; graph is exactly the target and acyclic. Move nothing until this passes.

### Phase 2 — create repos + copy (fresh snapshots)

5. Create the 4 repos under `win-trader`; scaffold each: pnpm-workspace, root `package.json`, `.npmrc` (`@win-trader` → GitHub Packages + token), tsconfig base.
6. Copy each repo's folders per the homes above.
7. Swap cross-repo `workspace:*` → published version ranges; within-repo deps stay `workspace:*`.
8. Initial commit each. Archive this monorepo private.

### Phase 3 — bootstrap publish + CI + deploy, in dependency order

9. `contracts` first: CI builds WASM → gen bindings → publishes all 4 public packages on tag. Seed the registry with a manual publish so downstream can resolve.
10. `offchain`: consume published contracts packages; publish `data` on tag; Coolify builds indexer/keeper/migrate from source; wire env.
11. `oracles` + `app` (parallel): consume published packages; Coolify builds from source; wire env + signer keys.
12. End-to-end smoke: deploy local network, run `simulation`, confirm data flows indexer → DB → api → frontend.
