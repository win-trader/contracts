#!/usr/bin/env bash
# add-market.sh — Incrementally wire a new ticker into an existing
# deployment. Reuses the deployed OracleRouter + PositionManager:
#   1. registers the ticker on OracleRouter with primaries chosen from
#      whichever of {mock-oracle, binance, kucoin} are present in
#      addresses.json (so this works whether or not CEX oracles are up)
#   2. sets max leverage on PositionManager
#
# Use this instead of re-running deploy.sh when the rest of the system
# is healthy and you only need to add a market. deploy.sh would wipe
# state by re-deploying every contract.
#
# Usage:
#   bash scripts/add-market.sh XLMUSD              # default 50× max leverage
#   MAX_LEVERAGE=20 bash scripts/add-market.sh XLMUSD
#   NETWORK_KEY=testnet bash scripts/add-market.sh XLMUSD
#
# Pre-req: the symbol's mapping must already exist in the binance/kucoin
# source maps (packages/oracle-{binance,kucoin}/src/source.ts) so live
# publishers can resolve the CEX symbol once the router routes to them.
set -euo pipefail

SYMBOL="${1:-}"
if [[ -z "$SYMBOL" ]]; then
  echo "❌ usage: $0 <SYMBOL>  (e.g. XLMUSD)"
  exit 1
fi

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ADDRESSES_FILE="$ROOT/packages/config/addresses.json"
NETWORK_KEY="${NETWORK_KEY:-local}"
MAX_LEVERAGE="${MAX_LEVERAGE:-50}"
SOURCE="${SOURCE:-admin}"

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
if ! stellar keys address "$SOURCE" >/dev/null 2>&1; then
  echo "❌ Source identity '$SOURCE' not found — run scripts/provision-keys.sh"
  exit 1
fi
ADMIN_ADDR=$(stellar keys address "$SOURCE")

# Pull deployed contract IDs.
contract_addr() {
  jq -r --arg net "$NETWORK_KEY" --arg k "$1" '.[$net].contracts[$k].address // ""' "$ADDRESSES_FILE"
}
OR_ID=$(contract_addr oracleRouter)
PM_ID=$(contract_addr positionManager)
MOCK_ORACLE_ID=$(contract_addr oracle)
BINANCE_ID=$(contract_addr binanceOracle)
KUCOIN_ID=$(contract_addr kucoinOracle)

if [[ -z "$OR_ID" || -z "$PM_ID" ]]; then
  echo "❌ OracleRouter / PositionManager addresses missing for '$NETWORK_KEY' in $ADDRESSES_FILE"
  echo "   Run 'NETWORK_KEY=$NETWORK_KEY make deploy' first."
  exit 1
fi

# Build the source list from whichever addresses are populated.
# Empty strings (not-yet-deployed CEX oracles) drop out — the router only
# learns about sources we actually have. OracleRouter uses a flat source
# list with a `min_required_sources` quorum (no primary/secondary tiering).
SOURCES=()
[[ -n "$MOCK_ORACLE_ID" ]] && SOURCES+=("$MOCK_ORACLE_ID")
[[ -n "$BINANCE_ID"     ]] && SOURCES+=("$BINANCE_ID")
[[ -n "$KUCOIN_ID"      ]] && SOURCES+=("$KUCOIN_ID")
if [[ ${#SOURCES[@]} -eq 0 ]]; then
  echo "❌ No oracle sources available for $SYMBOL — deploy at least the mock oracle"
  exit 1
fi
SOURCES_JSON=$(printf '%s\n' "${SOURCES[@]}" | jq -R . | jq -sc .)

invoke() {
  stellar contract invoke \
    --source "$SOURCE" \
    --rpc-url "$RPC_URL" \
    --network-passphrase "$NETWORK_PASSPHRASE" \
    "$@"
}

echo "=== Adding market '$SYMBOL' on '$NETWORK_KEY' (max leverage ${MAX_LEVERAGE}×) ==="
echo "  sources: $SOURCES_JSON"

echo "  oracle_router.set_oracle_sources($SYMBOL, …)"
invoke --id "$OR_ID" -- set_oracle_sources \
  --caller "$ADMIN_ADDR" \
  --symbol "$SYMBOL" \
  --sources "$SOURCES_JSON"

echo "  position_manager.set_max_leverage($SYMBOL, $MAX_LEVERAGE)"
invoke --id "$PM_ID" -- set_max_leverage \
  --caller "$ADMIN_ADDR" \
  --symbol "$SYMBOL" \
  --max_leverage "$MAX_LEVERAGE"

# Mirror the new ticker into addresses.json if it isn't already there, so
# the publisher loops + frontend SUPPORTED_SYMBOLS pick it up automatically.
echo "  ensuring '$SYMBOL' is listed in addresses.json[$NETWORK_KEY].tickers"
TMP=$(mktemp)
jq --arg net "$NETWORK_KEY" --arg sym "$SYMBOL" \
  'if .[$net].tickers | index($sym) then . else .[$net].tickers += [$sym] end' \
  "$ADDRESSES_FILE" > "$TMP"
mv "$TMP" "$ADDRESSES_FILE"

echo ""
echo "=== Done ==="
echo "  $SYMBOL is now registered on the OracleRouter and PositionManager."
echo "  Restart oracle publishers (make oracles-cex) so they pick up the new ticker."
