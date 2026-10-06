# Audit package

WinTrader perpetual DEX on Stellar Soroban. Liquidity providers deposit USDC
into a vault and act as counterparty to leveraged long and short positions,
priced by a third-party SEP-40 oracle. Trader actions are two-phase: commit,
then a permissionless keeper settles at a fresh price.

**Audit commit:** the tag `audit-2026-10-06` (not a `v*` tag; those publish to npm).

## Scope

In scope, all under `contracts/`:

| Crate | Role |
|---|---|
| `shared` | Types, constants, maths (`math.rs`, `fixed.rs`), per-config validation, shared events, the timelocked-upgrade trait, and the contract interfaces, including the SEP-40 price-feed client |
| `config-manager` | Role registry, two-step admin transfer, upgrade timelock |
| `market-governor` | Configuration proposals, timelocks, conservative fast path; installs into the PositionManager |
| `position-manager` | Positions, pending actions, fees, funding, borrow, risk states, liquidation, ADL, settlement, LP accounting snapshot |
| `vault` | Holds all collateral cash; LP share token (`sLP`); deposit and withdrawal settlement |
| `request-router` | FIFO queue for delayed LP deposits and withdrawals; `skip_head` escape hatch |

That is about 10k lines of Rust, `soroban-sdk 23.5.2`.

Out of scope:

- **The price oracle.** Production uses a third-party SEP-40 provider. The
  protocol's side of that boundary is in scope (`shared/src/price_feed.rs`,
  `position-manager/src/snapshot.rs::read_stamped_price`, the decimals
  rescaling, and how prices are used). The provider's contract and data are
  not. The trust assumption and its configuration requirements are K-1 in
  [`KNOWN_ISSUES.md`](KNOWN_ISSUES.md).
- **`mocks/`** (`mock-oracle`, `mock-token`): test-only. The mock oracle
  accepts prices from anyone and is never deployed outside local or test
  networks.
- **Everything under `packages/`** (TypeScript bindings and helpers),
  **`scripts/`**, and the off-chain keeper, indexer and app (other repos).
  The deploy script's guardrails (mocks local-only, role separation on
  mainnet) are described in the threat model for context.

## Read first

1. [`THREAT_MODEL.md`](../../THREAT_MODEL.md): STRIDE analysis, every finding
   and its status, the remediation log, and measured on-chain costs.
2. [`KNOWN_ISSUES.md`](KNOWN_ISSUES.md): accepted risks and design choices.
3. [`docs/design/trading-fees-and-settlement-specification.md`](../design/trading-fees-and-settlement-specification.md):
   the economic specification. Its status note lists where it predates the
   code; where they differ, the code wins.
4. [`docs/integration/offchain.md`](../integration/offchain.md): keeper entry
   points and every event.

Suggested focus areas are §7 of the threat model.

## Build and verify

The canonical build runs in a pinned Linux image, because rustc orders
functions differently on Apple-silicon and x86-64 Linux hosts. CI reproduces
it byte for byte.

```bash
make repro          # needs Docker; output in target/repro/wasm32v1-none/release/
cargo test --workspace
```

Pins: rustc 1.98.1 (`rust-toolchain.toml`), stellar CLI 27.0.0 (bundles the
WASM optimizer), image `rust:1.98.1@sha256:a8a5f0a1…df0546`. Source paths are
remapped, so the output does not depend on where the repo is checked out.

Canonical SHA-256 of the optimized WASM at the audit commit:

| Contract | Bytes | SHA-256 |
|---|---|---|
| `config_manager` | 32,073 | `1a7e6e6a35ca449be16c923eefab867664c15785859dc6fc2d9eeb40db4e8a7c` |
| `market_governor` | 43,037 | `af0b6d68cf57b2799d965d734316c55aaf4099a008bceb60fe66d34e6b105a1e` |
| `position_manager` | 128,027 | `b4f65c8f052726529cac5862e7a713f1c8421b6e2a36b84e465634f6d92c312c` |
| `vault` | 77,929 | `b7229af393134d0c3deb90a5e029b5b4608e1e3ec0405051bb3eeb99ee7c8a61` |
| `request_router` | 37,384 | `d9ce1a84c05faabdcf970b8d1c13b2b37272279af2bdc54a4e16960b773414dd` |

The network caps contract size at 131,072 bytes; the build fails above it.

## Tests

174 tests (`cargo test --workspace`), including:

- `position-manager/tests/spec_*.rs`: suites derived from the specification;
- `tests/invariants.rs`: a seeded random walk over every protocol path that
  reconciles cash, claims, aggregates and router escrow after every step;
- `tests/threat_model.rs`, `tests/governor.rs`, `tests/review_findings.rs`:
  a regression test for every fixed finding.
