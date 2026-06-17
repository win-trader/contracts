#!/usr/bin/env bash
# grant-keepers.sh — Grant the KEEPER role in ConfigManager to one or more
# Stellar accounts. Accepts space-separated public keys via $KEEPERS, or a
# newline-separated file via $KEEPERS_FILE. Reads the ConfigManager address
# from packages/config/addresses.json for $NETWORK_KEY (default: local).
#
# Usage:
#   KEEPERS="GAAA... GBBB..." make grant-keepers
#   KEEPERS_FILE=keepers.txt make grant-keepers
#   NETWORK_KEY=testnet RPC_URL=https://soroban-testnet.stellar.org KEEPERS="..." make grant-keepers
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ADDRESSES_FILE="$ROOT/packages/config/addresses.json"
NETWORK_KEY="${NETWORK_KEY:-local}"
RPC_URL="${RPC_URL:-http://localhost:8000/soroban/rpc}"
NETWORK_PASSPHRASE="${NETWORK_PASSPHRASE:-Standalone Network ; February 2017}"
SOURCE="${SOURCE:-admin}"

if ! command -v jq >/dev/null 2>&1; then
  echo "❌ jq is required — install via 'brew install jq'"
  exit 1
fi

if [[ ! -f "$ADDRESSES_FILE" ]]; then
  echo "❌ $ADDRESSES_FILE not found — run 'make deploy' first"
  exit 1
fi

CM_ID=$(jq -r ".networks.${NETWORK_KEY}.contracts.configManager.address" "$ADDRESSES_FILE")
if [[ -z "$CM_ID" || "$CM_ID" == "null" ]]; then
  echo "❌ ConfigManager address not found for network '$NETWORK_KEY' in $ADDRESSES_FILE"
  exit 1
fi

# Collect keeper pubkeys from $KEEPERS (space-separated) and/or $KEEPERS_FILE.
KEYS=()
if [[ -n "${KEEPERS:-}" ]]; then
  read -r -a EXTRA <<< "$KEEPERS"
  KEYS+=("${EXTRA[@]}")
fi
if [[ -n "${KEEPERS_FILE:-}" ]]; then
  if [[ ! -f "$KEEPERS_FILE" ]]; then
    echo "❌ KEEPERS_FILE='$KEEPERS_FILE' does not exist"
    exit 1
  fi
  while IFS= read -r line; do
    line="${line%%#*}"            # strip comments
    line="${line//[[:space:]]/}"  # strip whitespace
    [[ -z "$line" ]] && continue
    KEYS+=("$line")
  done < "$KEEPERS_FILE"
fi

if [[ ${#KEYS[@]} -eq 0 ]]; then
  echo "❌ No keeper addresses provided. Set KEEPERS=\"GAAA... GBBB...\" or KEEPERS_FILE=keepers.txt"
  exit 1
fi

ADMIN_ADDR=$(stellar keys address "$SOURCE")
echo "ConfigManager: $CM_ID"
echo "Admin:         $ADMIN_ADDR"
echo "Granting KEEPER role to ${#KEYS[@]} address(es) on '$NETWORK_KEY':"

invoke() {
  stellar contract invoke \
    --source "$SOURCE" \
    --rpc-url "$RPC_URL" \
    --network-passphrase "$NETWORK_PASSPHRASE" \
    "$@"
}

for key in "${KEYS[@]}"; do
  echo "  → $key"
  invoke --id "$CM_ID" -- grant_role \
    --caller "$ADMIN_ADDR" \
    --role KEEPER \
    --account "$key"
done

echo ""
echo "✓ Granted KEEPER role to ${#KEYS[@]} address(es)."
