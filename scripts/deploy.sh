#!/usr/bin/env bash
# deploy.sh — Fresh deploy of the protocol contracts: ConfigManager,
# PositionManager, Vault and RequestRouter. Uses identities created by
# scripts/provision-keys.sh.
#
#   NETWORK_KEY=local    bash scripts/deploy.sh
#   NETWORK_KEY=testnet  PRICE_FEED_ADDR=C... bash scripts/deploy.sh
#   NETWORK_KEY=mainnet  PRICE_FEED_ADDR=C... ASSET_ADDR=C... \
#     UPGRADER_ADDR=G... PAUSER_ADDR=G... UNPAUSER_ADDR=G... \
#     ORACLE_ADDR=G... PROTOCOL_ADDR=G... bash scripts/deploy.sh
#
# External inputs:
#   PRICE_FEED_ADDR  SEP-40 feed (`lastprice`, `decimals() == 7`) from the
#                    oracles repo. Required off local; local deploys the
#                    unauthenticated mock-oracle instead.
#   ASSET_ADDR       Collateral token (USDC SAC, 7 decimals). Required on
#                    mainnet; elsewhere the mock-token faucet is deployed.
#   *_ROLE addresses Role holders. Mainnet requires every one of them, none
#                    equal to admin, and UPGRADER distinct from PAUSER (the
#                    pause key is what cancels a hostile upgrade).
#
# After deploy:
#   - addresses.json[<network>].contracts updated
#   - deployments/<network>.json regenerated
#   - .env.<network> gets the service env block
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=lib/protocol.sh
source "$ROOT/scripts/lib/protocol.sh"
ENV_FILE="${ENV_FILE:-$ROOT/.env.${NETWORK_KEY}}"
DATABASE_URL_DEFAULT="${DATABASE_URL:-}"
if [[ "$NETWORK_KEY" == "local" ]]; then
  DATABASE_URL_DEFAULT="${DATABASE_URL:-postgresql://stellars:stellars@localhost:5432/stellars}"
fi

TICKERS=()
while IFS= read -r ticker; do
  [[ -n "$ticker" ]] && TICKERS+=("$ticker")
done < <(jq -r --arg net "$NETWORK_KEY" '.[$net].tickers[]' "$ADDRESSES_FILE")
if [[ ${#TICKERS[@]} -eq 0 ]]; then
  echo "❌ No tickers configured for network '$NETWORK_KEY' in $ADDRESSES_FILE"
  exit 1
fi
if (( ${#TICKERS[@]} > ${MAX_ACTIVE_MARKETS:-8} )); then
  echo "❌ ${#TICKERS[@]} configured markets exceed MAX_ACTIVE_MARKETS=${MAX_ACTIVE_MARKETS:-8}"
  exit 1
fi

require_identity() {
  if ! stellar keys address "$1" >/dev/null 2>&1; then
    echo "❌ Identity '$1' not found. Run: NETWORK_KEY=$NETWORK_KEY bash scripts/provision-keys.sh"
    exit 1
  fi
}
require_identity admin
require_identity keeper
ADMIN_ADDR=$(stellar keys address admin)
KEEPER_ADDR=$(stellar keys address keeper)

PRICE_FEED_ADDR="${PRICE_FEED_ADDR:-}"
ASSET_ADDR="${ASSET_ADDR:-}"
ROLES=(UPGRADER PAUSER UNPAUSER ORACLE PROTOCOL)
holder() { local v="${1}_ADDR"; echo "${!v:-}"; }

# ---------- Guardrails ----------
if [[ -z "$PRICE_FEED_ADDR" && "$NETWORK_KEY" != "local" ]]; then
  echo "❌ PRICE_FEED_ADDR is required on '$NETWORK_KEY'. Deploy the feed from the oracles repo first."
  echo "    The mock-oracle accepts prices from anyone and is only deployed on local."
  exit 1
fi
if [[ "$NETWORK_KEY" == "mainnet" ]]; then
  if [[ -z "$ASSET_ADDR" ]]; then
    echo "❌ ASSET_ADDR (USDC SAC) is required on mainnet. The mock token is never deployed there."
    exit 1
  fi
  for role in "${ROLES[@]}"; do
    if [[ -z "$(holder "$role")" || "$(holder "$role")" == "$ADMIN_ADDR" ]]; then
      echo "❌ ${role}_ADDR must be set and must not be the admin address on mainnet."
      exit 1
    fi
  done
  if [[ "$(holder UPGRADER)" == "$(holder PAUSER)" ]]; then
    echo "❌ UPGRADER_ADDR and PAUSER_ADDR must differ: the pause key cancels upgrades."
    exit 1
  fi
  if [[ -n "$(contract_addr vault)" ]]; then
    echo "❌ Mainnet already has a vault at $(contract_addr vault). This script only does fresh deploys."
    exit 1
  fi
fi
for role in "${ROLES[@]}"; do
  [[ -z "$(holder "$role")" ]] && printf -v "${role}_ADDR" '%s' "$ADMIN_ADDR"
done
confirm_mainnet "deploy"

echo "Admin:  $ADMIN_ADDR"
echo "Keeper: $KEEPER_ADDR"

if [[ -f "$ADDRESSES_FILE" ]]; then
  backup="$ADDRESSES_FILE.bak.$(date +%s)"
  cp "$ADDRESSES_FILE" "$backup"
  echo "Backed up addresses.json → $backup"
fi

current_ledger() {
  curl -sf "$RPC_URL" \
    -X POST -H 'Content-Type: application/json' \
    -d '{"jsonrpc":"2.0","id":1,"method":"getLatestLedger"}' \
    | grep -o '"sequence":[0-9]*' | head -1 | cut -d: -f2
}

if ! stellar network ls 2>/dev/null | grep -qE "^${NETWORK_KEY}\b"; then
  stellar network add "$NETWORK_KEY" \
    --rpc-url "$RPC_URL" \
    --network-passphrase "$NETWORK_PASSPHRASE"
fi

# ---------- Build ----------
echo ""
echo "=== Building + optimizing WASMs ==="
(cd "$ROOT" && make optimize)

# ---------- Helpers ----------
deploy() {
  local name=$1
  shift
  local wasm="$WASM_DIR/$(echo "$name" | tr '-' '_').optimized.wasm"
  if [[ ! -f "$wasm" ]]; then
    echo "❌ Optimized WASM missing: $wasm" >&2
    exit 1
  fi
  echo "Deploying $name..." >&2
  local args=(--wasm "$wasm" --source admin --rpc-url "$RPC_URL" --network-passphrase "$NETWORK_PASSPHRASE")
  local contract_id
  if [[ $# -gt 0 ]]; then
    contract_id=$(stellar contract deploy "${args[@]}" -- "$@")
  else
    contract_id=$(stellar contract deploy "${args[@]}")
  fi
  if [[ -z "$contract_id" ]]; then
    echo "❌ Deploy for $name returned an empty contract id" >&2
    exit 1
  fi
  # Fail closed: the on-chain code must be the file we built.
  local expected actual
  expected=$(stellar contract info hash --wasm "$wasm")
  actual=$(stellar contract info hash --id "$contract_id" \
    --rpc-url "$RPC_URL" --network-passphrase "$NETWORK_PASSPHRASE")
  if [[ -z "$actual" || "$expected" != "$actual" ]]; then
    echo "❌ WASM hash mismatch for $name: expected=$expected actual=${actual:-<none>}" >&2
    exit 1
  fi
  echo "$contract_id"
}

invoke() {
  stellar contract invoke \
    --source admin \
    --rpc-url "$RPC_URL" \
    --network-passphrase "$NETWORK_PASSPHRASE" \
    "$@"
}

expect_eq() {
  if [[ "$2" != "$3" ]]; then
    echo "❌ Wiring check failed: $1 is $2, expected $3"
    exit 1
  fi
  echo "  ✓ $1"
}

# ---------- Deploy contracts ----------
echo ""
echo "=== Deploying contracts ==="

MOCK_TOKEN_ID=""
MOCK_ORACLE_ID=""
if [[ -z "$ASSET_ADDR" ]]; then
  MOCK_TOKEN_ID=$(deploy mock-token)
  MOCK_TOKEN_LEDGER=$(current_ledger)
  invoke --id "$MOCK_TOKEN_ID" -- initialize \
    --admin "$ADMIN_ADDR" --decimals 7 --name "USDC" --symbol "USDC"
  ASSET_ADDR="$MOCK_TOKEN_ID"
  echo "  mock-token     : $MOCK_TOKEN_ID"
fi
if [[ -z "$PRICE_FEED_ADDR" ]]; then
  MOCK_ORACLE_ID=$(deploy mock-oracle)
  MOCK_ORACLE_LEDGER=$(current_ledger)
  PRICE_FEED_ADDR="$MOCK_ORACLE_ID"
  echo "  mock-oracle    : $MOCK_ORACLE_ID"
fi

CM_ID=$(deploy config-manager --admin "$ADMIN_ADDR")
CM_LEDGER=$(current_ledger)
echo "  config-manager : $CM_ID"

# The Vault binds its PositionManager in its constructor, so PM comes first;
# `set_vault` closes the cycle.
PM_ID=$(deploy position-manager \
  --config_manager "$CM_ID" \
  --price_feed "$PRICE_FEED_ADDR" \
  --config "$GLOBAL_CONFIG")
PM_LEDGER=$(current_ledger)
echo "  position-mgr   : $PM_ID"

VAULT_ID=$(deploy vault \
  --asset_address "$ASSET_ADDR" \
  --config_manager "$CM_ID" \
  --position_manager "$PM_ID" \
  --lp_config "$LP_CONFIG")
VAULT_LEDGER=$(current_ledger)
echo "  vault          : $VAULT_ID"

REQUEST_ROUTER_ID=$(deploy request-router \
  --asset_address "$ASSET_ADDR" \
  --vault_address "$VAULT_ID" \
  --config_manager_address "$CM_ID")
REQUEST_ROUTER_LEDGER=$(current_ledger)
echo "  request-router : $REQUEST_ROUTER_ID"

# ---------- Wire contracts ----------
echo ""
echo "=== Wiring contracts ==="
invoke --id "$PM_ID" -- set_vault --caller "$ADMIN_ADDR" --vault "$VAULT_ID"
invoke --id "$VAULT_ID" -- set_request_router --caller "$ADMIN_ADDR" --request_router "$REQUEST_ROUTER_ID"

if [[ -n "$MOCK_TOKEN_ID" ]]; then
  invoke --id "$MOCK_TOKEN_ID" -- configure_protocol \
    --admin "$ADMIN_ADDR" --vault "$VAULT_ID" --position_manager "$PM_ID"
  invoke --id "$MOCK_TOKEN_ID" -- set_public_mint_cap \
    --admin "$ADMIN_ADDR" --cap_usd "${PUBLIC_MINT_CAP_USD:-600}"
  invoke --id "$MOCK_TOKEN_ID" -- set_protocol_contract \
    --admin "$ADMIN_ADDR" --contract "$REQUEST_ROUTER_ID" --allowed true
fi

# ---------- Grant roles ----------
echo ""
echo "=== Granting roles ==="
for role in "${ROLES[@]}"; do
  echo "  $role → $(holder "$role")"
  invoke --id "$CM_ID" -- grant_role \
    --caller "$ADMIN_ADDR" --role "$role" --account "$(holder "$role")"
done

# ---------- Register markets ----------
# A first registration applies immediately; later changes go through the timelock.
echo ""
echo "=== Registering markets ==="
for ticker in "${TICKERS[@]}"; do
  echo "  propose_market_config($ticker)"
  invoke --id "$PM_ID" -- propose_market_config \
    --caller "$ADMIN_ADDR" --market_symbol "$ticker" --config "$MARKET_CONFIG"
done

# ---------- Verify ----------
echo ""
echo "=== Verifying deployment ==="
unquote() { tr -d '"'; }
expect_eq "position-manager.price_feed" \
  "$(invoke --send=no --id "$PM_ID" -- price_feed | unquote)" "$PRICE_FEED_ADDR"
expect_eq "vault.query_asset" \
  "$(invoke --send=no --id "$VAULT_ID" -- query_asset | unquote)" "$ASSET_ADDR"
expect_eq "active market count" \
  "$(invoke --send=no --id "$PM_ID" -- active_markets | jq length)" "${#TICKERS[@]}"
# Reaches the vault through PM's stored address, so it proves `set_vault`.
invoke --send=no --id "$PM_ID" -- update_indices --caller "$ADMIN_ADDR" --market_symbol "${TICKERS[0]}" >/dev/null
echo "  ✓ position-manager → vault"
for role in "${ROLES[@]}"; do
  expect_eq "has_role($role)" \
    "$(invoke --send=no --id "$CM_ID" -- has_role --role "$role" --account "$(holder "$role")")" "true"
done
# LP settlement needs a fresh price for every registered market (THREAT_MODEL T-02).
for ticker in "${TICKERS[@]}"; do
  if [[ "$(invoke --send=no --id "$PRICE_FEED_ADDR" -- lastprice --symbol "$ticker")" == "null" ]]; then
    echo "  ⚠ price feed has no price for $ticker yet — LP requests cannot resolve until it does"
  fi
done

# ---------- Write addresses.json ----------
# The feed keeps the registry's `oracleRouter` key so @win-trader/config consumers
# are unchanged; on local the mock oracle is also recorded as `oracle`.
echo ""
echo "=== Writing $ADDRESSES_FILE [$NETWORK_KEY] ==="
TMP_ADDR=$(mktemp)
jq \
  --arg net "$NETWORK_KEY" \
  --arg vault "$VAULT_ID"          --argjson vaultL "${VAULT_LEDGER:-0}" \
  --arg rr "$REQUEST_ROUTER_ID"     --argjson rrL    "${REQUEST_ROUTER_LEDGER:-0}" \
  --arg pm "$PM_ID"                --argjson pmL    "${PM_LEDGER:-0}" \
  --arg cm "$CM_ID"                --argjson cmL    "${CM_LEDGER:-0}" \
  --arg feed "$PRICE_FEED_ADDR" \
  --arg mockToken "$MOCK_TOKEN_ID"   --argjson mockTokenL  "${MOCK_TOKEN_LEDGER:-0}" \
  --arg mockOracle "$MOCK_ORACLE_ID" --argjson mockOracleL "${MOCK_ORACLE_LEDGER:-0}" \
  '.[$net].contracts += {
       vault:           {address: $vault,      startLedger: $vaultL},
       requestRouter:   {address: $rr,         startLedger: $rrL},
       positionManager: {address: $pm,         startLedger: $pmL},
       configManager:   {address: $cm,         startLedger: $cmL},
       oracleRouter:    {address: $feed,       startLedger: 0},
       mockToken:       {address: $mockToken,  startLedger: $mockTokenL}}
   | if $mockOracle == "" then . else .[$net].contracts.oracle = {address: $mockOracle, startLedger: $mockOracleL} end' \
  "$ADDRESSES_FILE" > "$TMP_ADDR"
mv "$TMP_ADDR" "$ADDRESSES_FILE"

# ---------- Service env ----------
KEEPER_SECRET=$(stellar keys show keeper)
SERVICE_BLOCK_MARKER="# --- service env (deploy.sh) ---"
if [[ -f "$ENV_FILE" ]]; then
  TMP_ENV=$(mktemp)
  awk -v marker="$SERVICE_BLOCK_MARKER" '$0==marker{stop=1} !stop' "$ENV_FILE" > "$TMP_ENV"
  mv "$TMP_ENV" "$ENV_FILE"
fi
{
  echo "$SERVICE_BLOCK_MARKER"
  [[ -n "$DATABASE_URL_DEFAULT" ]] && echo "DATABASE_URL=$DATABASE_URL_DEFAULT"
  echo "POLL_INTERVAL_MS=${POLL_INTERVAL_MS:-3000}"
  echo "HEALTH_PORT=${HEALTH_PORT:-3001}"
  echo "KEEPER_SECRET=$KEEPER_SECRET"
  echo "ORACLE_CONTRACT=$PRICE_FEED_ADDR"
  [[ -n "$MOCK_TOKEN_ID" ]] && echo "MOCK_TOKEN_CONTRACT=$MOCK_TOKEN_ID"
  echo "ADMIN_ADDRESS=$ADMIN_ADDR"
  echo "KEEPER_ADDRESS=$KEEPER_ADDR"
} >> "$ENV_FILE"
chmod 600 "$ENV_FILE"

bash "$ROOT/scripts/split-deployments.sh" "$NETWORK_KEY"

echo ""
echo "=== Done ==="
echo "  Network    : $NETWORK_KEY"
echo "  Addresses  → $ADDRESSES_FILE"
echo "  Deployment → deployments/$NETWORK_KEY.json"
echo "  Service env → $ENV_FILE"
