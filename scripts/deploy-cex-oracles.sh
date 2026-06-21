#!/usr/bin/env bash
# deploy-cex-oracles.sh — Deploy and register Binance + KuCoin oracle instances.
#
# Each CEX gets its own on-chain `oracle` contract (so OracleRouter can
# median across sources) and its own publisher keypair (so they don't
# contend on sequence numbers when running in parallel).
#
# Idempotent: skips deployment if addresses.json already has a non-empty
# slot for the source. Always re-grants ORACLE and re-registers router
# primaries so the script can be safely re-run after editing TICKERS.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WASM_DIR="$ROOT/target/wasm32v1-none/release"
ADDRESSES_FILE="$ROOT/packages/config/addresses.json"
NETWORK_KEY="${NETWORK_KEY:-local}"
ENV_FILE="${ENV_FILE:-$ROOT/.env.${NETWORK_KEY}}"

# Network params — mirror deploy.sh so NETWORK_KEY alone selects the right
# RPC. Without this the script silently fell through to local RPC even when
# called as `NETWORK_KEY=testnet make cex-oracles`, and the addresses.json
# slots got mixed (testnet base contracts + local oracle deploys → grant_role
# errors).
case "$NETWORK_KEY" in
  local)
    RPC_URL="${RPC_URL:-http://localhost:8000/soroban/rpc}"
    NETWORK_PASSPHRASE="${NETWORK_PASSPHRASE:-Standalone Network ; February 2017}"
    FRIENDBOT="${FRIENDBOT:-http://localhost:8000/friendbot}"
    HORIZON="${HORIZON:-http://localhost:8000}"
    ;;
  testnet)
    RPC_URL="${RPC_URL:-https://soroban-testnet.stellar.org}"
    NETWORK_PASSPHRASE="${NETWORK_PASSPHRASE:-Test SDF Network ; September 2015}"
    FRIENDBOT="${FRIENDBOT:-https://friendbot.stellar.org}"
    HORIZON="${HORIZON:-https://horizon-testnet.stellar.org}"
    ;;
  mainnet)
    RPC_URL="${RPC_URL:-https://soroban.stellar.org}"
    NETWORK_PASSPHRASE="${NETWORK_PASSPHRASE:-Public Global Stellar Network ; September 2015}"
    FRIENDBOT=""
    HORIZON="${HORIZON:-https://horizon.stellar.org}"
    ;;
  *)
    : "${RPC_URL:?required for unknown NETWORK_KEY}" "${NETWORK_PASSPHRASE:?required}"
    FRIENDBOT="${FRIENDBOT:-}"
    HORIZON="${HORIZON:-}"
    ;;
esac

if ! command -v jq >/dev/null 2>&1; then
  echo "❌ jq is required — install via 'brew install jq'"
  exit 1
fi

# Tickers come from addresses.json so the script, the oracle publishers,
# the indexer poller, and the frontend symbol picker all agree. mapfile
# would be cleaner but it's bash 4+ and macOS ships bash 3.2.
TICKERS=()
while IFS= read -r ticker; do
  [ -n "$ticker" ] && TICKERS+=("$ticker")
done < <(jq -r --arg net "$NETWORK_KEY" '.[$net].tickers[]' "$ADDRESSES_FILE")
if [ "${#TICKERS[@]}" -eq 0 ]; then
  echo "❌ No tickers configured for network '$NETWORK_KEY' in $ADDRESSES_FILE"
  exit 1
fi

current_ledger() {
  curl -sf "$RPC_URL" \
    -X POST -H 'Content-Type: application/json' \
    -d '{"jsonrpc":"2.0","id":1,"method":"getLatestLedger"}' \
    | grep -o '"sequence":[0-9]*' | head -1 | cut -d: -f2
}

invoke() {
  stellar contract invoke \
    --source admin \
    --rpc-url "$RPC_URL" \
    --network-passphrase "$NETWORK_PASSPHRASE" \
    "$@"
}

deploy() {
  local name=$1
  shift
  # Trailing args are constructor arguments, forwarded after `--`.
  local wasm="$WASM_DIR/$(echo "$name" | tr '-' '_').optimized.wasm"
  echo "Deploying $name..." >&2
  local contract_id
  if [[ $# -gt 0 ]]; then
    contract_id=$(stellar contract deploy \
      --wasm "$wasm" \
      --source admin \
      --rpc-url "$RPC_URL" \
      --network-passphrase "$NETWORK_PASSPHRASE" \
      -- "$@")
  else
    contract_id=$(stellar contract deploy \
      --wasm "$wasm" \
      --source admin \
      --rpc-url "$RPC_URL" \
      --network-passphrase "$NETWORK_PASSPHRASE")
  fi
  if [[ -z "$contract_id" ]]; then
    echo "❌ Deploy for $name returned an empty contract id" >&2
    exit 1
  fi
  echo "$contract_id"
}

# ---------- Pull required base addresses ----------
CM_ID=$(jq -r ".[\"$NETWORK_KEY\"].contracts.configManager.address" "$ADDRESSES_FILE")
OR_ID=$(jq -r ".[\"$NETWORK_KEY\"].contracts.oracleRouter.address" "$ADDRESSES_FILE")
MOCK_ORACLE_ID=$(jq -r ".[\"$NETWORK_KEY\"].contracts.oracle.address" "$ADDRESSES_FILE")
ADMIN_ADDR=$(stellar keys address admin 2>/dev/null || true)

if [[ -z "$CM_ID" || "$CM_ID" == "null" || -z "$OR_ID" || "$OR_ID" == "null" || -z "$ADMIN_ADDR" ]]; then
  echo "❌ Base contracts not deployed. Run 'make deploy' first."
  exit 1
fi

# ---------- Build (only oracle wasm needed) ----------
echo ""
echo "=== Building + optimizing oracle wasm ==="
(cd "$ROOT" && cargo build --target wasm32v1-none --release -p oracle)
stellar contract optimize --wasm "$WASM_DIR/oracle.wasm"

# ---------- Load per-source keypairs ----------
require_identity() {
  local name=$1
  if ! stellar keys address "$name" >/dev/null 2>&1; then
    echo "❌ Identity '$name' not found. Run: NETWORK_KEY=$NETWORK_KEY bash scripts/provision-keys.sh"
    exit 1
  fi
  stellar keys address "$name"
}

BINANCE_KEY_ADDR=$(require_identity binance-oracle)
KUCOIN_KEY_ADDR=$(require_identity kucoin-oracle)

# Friendbot returns 200 on first fund and 400 on subsequent ("already funded").
# Verify against Horizon after each attempt so a curl failure cannot silently
# leave the publisher account unusable.
account_exists() {
  local addr=$1
  [[ -z "$HORIZON" ]] && return 1
  local code
  code=$(curl -s -o /dev/null -w "%{http_code}" --max-time 5 "$HORIZON/accounts/${addr}") || code="000"
  [[ "$code" == "200" ]]
}

fund_account() {
  local addr=$1
  local label=$2
  if [[ -z "$FRIENDBOT" ]]; then
    echo "  $label ($addr) — skipped (no friendbot for $NETWORK_KEY)"
    return 0
  fi
  if account_exists "$addr"; then
    echo "  $label ($addr) — already funded"
    return 0
  fi
  for attempt in $(seq 1 30); do
    curl -sf "$FRIENDBOT?addr=${addr}" >/dev/null 2>&1 || true
    if account_exists "$addr"; then
      echo "  $label funded ($addr)"
      return 0
    fi
    sleep 2
  done
  echo "❌ Failed to fund $label ($addr) after ~60s"
  echo "   Re-run manually: curl '$FRIENDBOT?addr=$addr'"
  exit 1
}

echo "Funding publisher keypairs..."
fund_account "$BINANCE_KEY_ADDR" binance-oracle
fund_account "$KUCOIN_KEY_ADDR" kucoin-oracle

# ---------- Deploy oracle instances ----------
# Always deploy fresh. The previous "reuse if addresses.json has an entry"
# optimization was unsafe across `make reset` — the chain wipe leaves stale
# addresses in addresses.json that point to dead contracts, and downstream
# init/set_price calls fail with confusing "Contract not found" errors.
echo ""
echo "=== Deploying oracle instances ==="
# Each oracle instance binds its ConfigManager and publisher in its
# constructor (atomic with deploy) — set_price is gated on caller ==
# publisher, not on an ORACLE role, so each CEX publisher key is passed as
# --publisher here and no separate role grant or initialize step is needed.
BINANCE_ID=$(deploy oracle --config_manager "$CM_ID" --publisher "$BINANCE_KEY_ADDR")
BINANCE_LEDGER=$(current_ledger)
echo "  binance-oracle : $BINANCE_ID  (ledger $BINANCE_LEDGER)"

KUCOIN_ID=$(deploy oracle --config_manager "$CM_ID" --publisher "$KUCOIN_KEY_ADDR")
KUCOIN_LEDGER=$(current_ledger)
echo "  kucoin-oracle  : $KUCOIN_ID  (ledger $KUCOIN_LEDGER)"

# ---------- Register sources on the OracleRouter ----------
# Strategy: rewrite primaries to [mock_oracle, binance, kucoin] so the
# existing simulation (which pushes through mock_oracle) keeps working
# alongside the live CEX feeds. Router takes the median across all three.
echo ""
echo "=== Registering router sources (${TICKERS[*]}) ==="
# Flat source list — mock + binance + kucoin. Quorum is enforced via
# OracleConfig.min_required_sources; this script doesn't touch the config,
# so whatever min_required_sources was set during initial deploy stays.
SOURCES_JSON=$(jq -nc \
  --arg m "$MOCK_ORACLE_ID" \
  --arg b "$BINANCE_ID" \
  --arg k "$KUCOIN_ID" \
  '[$m, $b, $k]')

for ticker in "${TICKERS[@]}"; do
  echo "  set_oracle_sources($ticker)"
  invoke --id "$OR_ID" -- set_oracle_sources \
    --caller "$ADMIN_ADDR" \
    --symbol "$ticker" \
    --sources "$SOURCES_JSON"
done

# ---------- Persist addresses.json ----------
echo ""
echo "=== Updating $ADDRESSES_FILE ==="
TMP=$(mktemp)
jq \
  --arg net "$NETWORK_KEY" \
  --arg b "$BINANCE_ID"   --argjson bL "${BINANCE_LEDGER:-0}" \
  --arg k "$KUCOIN_ID"    --argjson kL "${KUCOIN_LEDGER:-0}" \
  '.[$net].contracts.binanceOracle = {address: $b, startLedger: $bL}
   | .[$net].contracts.kucoinOracle  = {address: $k, startLedger: $kL}' \
  "$ADDRESSES_FILE" > "$TMP"
mv "$TMP" "$ADDRESSES_FILE"

# ---------- Append publisher secrets to .env.<network> ----------
BINANCE_SECRET=$(stellar keys show binance-oracle)
KUCOIN_SECRET=$(stellar keys show kucoin-oracle)

echo ""
echo "=== Updating $ENV_FILE with publisher secrets ==="
# Strip any prior CEX-publisher block before re-appending, so re-running the
# script doesn't grow the env file with duplicates.
if [[ -f "$ENV_FILE" ]]; then
  TMP_ENV=$(mktemp)
  awk '/^# --- CEX oracle publishers ---$/{stop=1} !stop' "$ENV_FILE" > "$TMP_ENV"
  mv "$TMP_ENV" "$ENV_FILE"
fi
cat >> "$ENV_FILE" <<EOF
# --- CEX oracle publishers ---
BINANCE_ORACLE_SECRET=$BINANCE_SECRET
KUCOIN_ORACLE_SECRET=$KUCOIN_SECRET
EOF

# Refresh the per-network deployment artifact services inject via ADDRESSES_JSON.
bash "$ROOT/scripts/split-deployments.sh" "$NETWORK_KEY"

echo ""
echo "=== Done ==="
echo "  binance-oracle : $BINANCE_ID  (publisher $BINANCE_KEY_ADDR)"
echo "  kucoin-oracle  : $KUCOIN_ID  (publisher $KUCOIN_KEY_ADDR)"
echo ""
echo "Next: copy deployments/$NETWORK_KEY.json plus publisher secrets to the oracles repo/VPS and run:"
echo "  docker compose --env-file .env.$NETWORK_KEY -f compose.oracles.yml up -d --build"
