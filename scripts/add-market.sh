#!/usr/bin/env bash
# add-market.sh — Register a new market on a live PositionManager. A first
# registration applies immediately; re-running for an existing market proposes
# a change that applies after `config_timelock_seconds`.
#
# The price feed must already serve the symbol: every registered market is
# priced on each LP settlement, so an unpriced market blocks the LP queue.
#
# Usage:
#   bash scripts/add-market.sh XLMUSD
#   NETWORK_KEY=testnet bash scripts/add-market.sh XLMUSD
set -euo pipefail

SYMBOL="${1:-}"
if [[ -z "$SYMBOL" ]]; then
  echo "❌ usage: $0 <SYMBOL>  (e.g. XLMUSD)"
  exit 1
fi

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=lib/protocol.sh
source "$ROOT/scripts/lib/protocol.sh"
SOURCE="${SOURCE:-admin}"

if ! stellar keys address "$SOURCE" >/dev/null 2>&1; then
  echo "❌ Source identity '$SOURCE' not found — run scripts/provision-keys.sh"
  exit 1
fi
ADMIN_ADDR=$(stellar keys address "$SOURCE")
PM_ID=$(contract_addr positionManager)
FEED_ID=$(contract_addr oracleRouter)
if [[ -z "$PM_ID" || -z "$FEED_ID" ]]; then
  echo "❌ PositionManager / price feed addresses missing for '$NETWORK_KEY' in $ADDRESSES_FILE"
  exit 1
fi

invoke() {
  stellar contract invoke \
    --source "$SOURCE" \
    --rpc-url "$RPC_URL" \
    --network-passphrase "$NETWORK_PASSPHRASE" \
    "$@"
}

if [[ "$(invoke --send=no --id "$FEED_ID" -- lastprice --symbol "$SYMBOL")" == "null" ]]; then
  echo "❌ The price feed has no price for $SYMBOL. Publish one before registering the market."
  exit 1
fi
confirm_mainnet "add market $SYMBOL"

echo "=== Registering market '$SYMBOL' on '$NETWORK_KEY' ==="
invoke --id "$PM_ID" -- propose_market_config \
  --caller "$ADMIN_ADDR" \
  --market_symbol "$SYMBOL" \
  --config "$MARKET_CONFIG"

TMP=$(mktemp)
jq --arg net "$NETWORK_KEY" --arg sym "$SYMBOL" \
  'if .[$net].tickers | index($sym) then . else .[$net].tickers += [$sym] end' \
  "$ADDRESSES_FILE" > "$TMP"
mv "$TMP" "$ADDRESSES_FILE"

bash "$ROOT/scripts/split-deployments.sh" "$NETWORK_KEY"

echo ""
echo "=== Done: $SYMBOL registered (or a change proposed) on the PositionManager ==="
