#!/usr/bin/env bash
# upgrade.sh — Timelocked WASM upgrade of deployed protocol contracts.
#
# Upgrades are two transactions separated by the contract's timelock
# (ConfigManager: its own upgrade timelock; the others:
# max(that, PositionManager.config_timelock_seconds)):
#
#   PHASE=propose  upload the freshly built WASM and call
#                  `propose_upgrade(caller, wasm_hash)` on each target
#   PHASE=execute  after the timelock, call `upgrade(new_wasm_hash, operator)`
#                  with the hash of the same local build
#
# A PAUSER holder can `cancel_upgrade` in between. Both phases need the UPGRADER
# role; set UPGRADE_SOURCE to that identity (default: admin, local/testnet only).
#
# Usage:
#   PHASE=propose NETWORK_KEY=testnet bash scripts/upgrade.sh
#   PHASE=execute NETWORK_KEY=testnet bash scripts/upgrade.sh
#   CONTRACTS="vault:vault" PHASE=propose bash scripts/upgrade.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=lib/protocol.sh
source "$ROOT/scripts/lib/protocol.sh"
UPGRADE_SOURCE="${UPGRADE_SOURCE:-admin}"
PHASE="${PHASE:-}"

if [[ "$PHASE" != "propose" && "$PHASE" != "execute" ]]; then
  echo "❌ Set PHASE=propose or PHASE=execute"
  exit 1
fi
if ! stellar keys address "$UPGRADE_SOURCE" >/dev/null 2>&1; then
  echo "❌ Source identity '$UPGRADE_SOURCE' not found — run scripts/provision-keys.sh"
  exit 1
fi
UPGRADER=$(stellar keys address "$UPGRADE_SOURCE")

if [[ -z "$(contract_addr requestRouter)" ]]; then
  echo "❌ '$NETWORK_KEY' is a pre-rewrite deployment. Run a fresh deploy; do not upgrade it in place."
  exit 1
fi

# wasm name : addresses.json key
DEFAULT_CONTRACTS=(
  "config-manager:configManager"
  "market-governor:governor"
  "vault:vault"
  "request-router:requestRouter"
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

echo "=== Building + optimizing WASMs ==="
(cd "$ROOT" && make "$BUILD_TARGET")

confirm_mainnet "upgrade $PHASE"

echo ""
echo "=== $PHASE upgrades on '$NETWORK_KEY' (upgrader $UPGRADER) ==="
for entry in "${TARGETS[@]}"; do
  wasm_name="${entry%%:*}"
  addr_key="${entry##*:}"
  wasm_path="$WASM_DIR/$(echo "$wasm_name" | tr '-' '_').optimized.wasm"
  if [[ ! -f "$wasm_path" ]]; then
    echo "❌ Optimized WASM not found: $wasm_path"
    exit 1
  fi
  contract_id=$(contract_addr "$addr_key")
  if [[ -z "$contract_id" ]]; then
    echo "  ⚠ $wasm_name ($addr_key): no address recorded for $NETWORK_KEY — skipping"
    continue
  fi
  wasm_hash=$(stellar contract info hash --wasm "$wasm_path")

  echo ""
  echo "→ $wasm_name ($contract_id)"
  echo "  hash $wasm_hash"
  if [[ "$PHASE" == "propose" ]]; then
    stellar contract upload \
      --wasm "$wasm_path" \
      --source "$UPGRADE_SOURCE" \
      --rpc-url "$RPC_URL" \
      --network-passphrase "$NETWORK_PASSPHRASE" >/dev/null
    invoke --id "$contract_id" -- propose_upgrade \
      --caller "$UPGRADER" --wasm_hash "$wasm_hash"
  else
    invoke --id "$contract_id" -- upgrade \
      --new_wasm_hash "$wasm_hash" --operator "$UPGRADER"
  fi
done

echo ""
if [[ "$PHASE" == "propose" ]]; then
  echo "=== Proposed. Re-run with PHASE=execute after the timelock, from the same build. ==="
else
  echo "=== Upgraded. Run each contract's \`migrate\` if the release needs it. ==="
fi
