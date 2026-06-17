#!/usr/bin/env bash
# upgrade.sh — Push new WASM bytecode to already-deployed contracts.
#
# For each upgradeable contract (vault, position-manager, oracle, oracle-router,
# config-manager) we:
#   1. install the freshly-built WASM and capture its hash
#   2. invoke `upgrade(operator, new_wasm_hash)` using `admin` as operator
#
# Auth: admin must hold the UPGRADER role on the ConfigManager — deploy.sh
# grants it on first deploy. If an upgrade is gated behind a separate
# operator key, override with $UPGRADE_SOURCE.
#
# Idempotent in practice: pushing the same WASM is a no-op upgrade
# (Stellar happily accepts the call but nothing changes). The script
# doesn't try to detect that — it's faster to just submit than to fetch
# the on-chain hash and compare.
#
# Usage:
#   NETWORK_KEY=local   bash scripts/upgrade.sh
#   NETWORK_KEY=testnet bash scripts/upgrade.sh
#   CONTRACTS="vault position-manager" NETWORK_KEY=testnet bash scripts/upgrade.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WASM_DIR="$ROOT/target/wasm32v1-none/release"
ADDRESSES_FILE="$ROOT/packages/config/addresses.json"
NETWORK_KEY="${NETWORK_KEY:-local}"
UPGRADE_SOURCE="${UPGRADE_SOURCE:-admin}"

case "$NETWORK_KEY" in
  local)
    RPC_URL="${RPC_URL:-http://localhost:8000/soroban/rpc}"
    NETWORK_PASSPHRASE="${NETWORK_PASSPHRASE:-Standalone Network ; February 2017}"
    ;;
  testnet)
    RPC_URL="${RPC_URL:-https://soroban-testnet.stellar.org}"
    NETWORK_PASSPHRASE="${NETWORK_PASSPHRASE:-Test SDF Network ; September 2015}"
    ;;
  mainnet)
    RPC_URL="${RPC_URL:-https://soroban.stellar.org}"
    NETWORK_PASSPHRASE="${NETWORK_PASSPHRASE:-Public Global Stellar Network ; September 2015}"
    ;;
  *)
    : "${RPC_URL:?required}" "${NETWORK_PASSPHRASE:?required}"
    ;;
esac

if ! command -v jq >/dev/null 2>&1; then
  echo "❌ jq is required — install via 'brew install jq'"
  exit 1
fi
if ! stellar keys address "$UPGRADE_SOURCE" >/dev/null 2>&1; then
  echo "❌ Source identity '$UPGRADE_SOURCE' not found — run scripts/provision-keys.sh"
  exit 1
fi
ADMIN_ADDR=$(stellar keys address "$UPGRADE_SOURCE")

# Map of upgradeable contract → (wasm filename, addresses.json key).
# These contracts derive UpgradeableMigratable, which generates the
# `upgrade(operator, new_wasm_hash)` entrypoint.
#
# `mock-token` is intentionally skipped — it ships without the upgrade
# macro and would fail with "method not found". `oracle` IS upgradeable
# and shared by mock + Binance + KuCoin instances; we only push the new
# WASM to the canonical mock instance here. Use the indexed instances
# (binanceOracle / kucoinOracle) explicitly via $CONTRACTS if you need
# to upgrade those, e.g. `CONTRACTS="binance-oracle:binanceOracle ..."`.
DEFAULT_CONTRACTS=(
  "config-manager:configManager"
  "oracle-router:oracleRouter"
  "oracle:oracle"
  "vault:vault"
  "position-manager:positionManager"
)
if [[ -n "${CONTRACTS:-}" ]]; then
  IFS=' ' read -r -a TARGETS <<< "$CONTRACTS"
else
  TARGETS=("${DEFAULT_CONTRACTS[@]}")
fi

invoke() {
  stellar contract invoke \
    --source "$UPGRADE_SOURCE" \
    --rpc-url "$RPC_URL" \
    --network-passphrase "$NETWORK_PASSPHRASE" \
    "$@"
}

# ---------- Build ----------
# Upgrade ships the optimized WASM, not the bloated raw output.
echo "=== Building + optimizing WASMs ==="
(cd "$ROOT" && make optimize)

# Mainnet confirmation prompt. A single typo in NETWORK_KEY would otherwise
# push fresh bytecode to mainnet without any operator awareness.
if [[ "$NETWORK_KEY" == "mainnet" ]]; then
  read -r -p "type MAINNET to confirm upgrade on mainnet: " _confirm
  if [[ "$_confirm" != "MAINNET" ]]; then
    echo "❌ Aborted — confirmation string did not match."
    exit 1
  fi
fi

# ---------- Upgrade each ----------
echo ""
echo "=== Upgrading contracts on '$NETWORK_KEY' (operator $ADMIN_ADDR) ==="
for entry in "${TARGETS[@]}"; do
  wasm_name="${entry%%:*}"
  addr_key="${entry##*:}"
  wasm_path="$WASM_DIR/$(echo "$wasm_name" | tr '-' '_').optimized.wasm"
  if [[ ! -f "$wasm_path" ]]; then
    echo "❌ Optimized WASM not found: $wasm_path — run 'make optimize' first"
    exit 1
  fi
  contract_id=$(jq -r --arg n "$NETWORK_KEY" --arg k "$addr_key" '.[$n].contracts[$k].address' "$ADDRESSES_FILE")
  if [[ -z "$contract_id" || "$contract_id" == "null" ]]; then
    echo "  ⚠ $wasm_name ($addr_key): no address recorded for $NETWORK_KEY — skipping"
    continue
  fi

  echo ""
  echo "→ $wasm_name ($contract_id)"

  # `stellar contract install` uploads the WASM and prints the hash on stdout
  # (the only thing we keep — `2>/dev/null` swallows the "Uploading…" log).
  echo "  installing WASM…"
  WASM_HASH=$(stellar contract install \
    --wasm "$wasm_path" \
    --source "$UPGRADE_SOURCE" \
    --rpc-url "$RPC_URL" \
    --network-passphrase "$NETWORK_PASSPHRASE" 2>/dev/null)
  echo "  hash $WASM_HASH"

  echo "  invoking upgrade()"
  invoke --id "$contract_id" -- upgrade \
    --operator "$ADMIN_ADDR" \
    --new_wasm_hash "$WASM_HASH"
done

echo ""
echo "=== Done ==="
echo "Upgraded ${#TARGETS[@]} contract(s) on $NETWORK_KEY."
