#!/usr/bin/env bash
# deploy.sh — Network-agnostic full deploy of all protocol contracts. Reads
# RPC + passphrase + friendbot from $NETWORK_KEY (local/testnet/mainnet) and
# uses identities created by scripts/provision-keys.sh.
#
#   NETWORK_KEY=local    bash scripts/deploy.sh
#   NETWORK_KEY=testnet  bash scripts/deploy.sh
#
# After deploy:
#   - packages/config/addresses.json[<network>].contracts.* updated
#   - .env.<network> updated with service env (DATABASE_URL, KEEPER_SECRET,
#     etc.). Identity block from provision-keys is preserved.
#
# Pre-reqs:
#   - `stellar` CLI installed
#   - `provision-keys.sh` already run (admin + keeper identities exist & are
#     funded). On mainnet, accounts must be pre-funded by the operator.
#
# Local uses a short LP request delay. Public networks use a longer delay.
# Override it with LP_REQUEST_DELAY.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WASM_DIR="$ROOT/target/wasm32v1-none/release"
ADDRESSES_FILE="$ROOT/packages/config/addresses.json"
NETWORK_KEY="${NETWORK_KEY:-local}"
ENV_FILE="${ENV_FILE:-$ROOT/.env.${NETWORK_KEY}}"

# Network params — sane defaults per known network, all overridable.
case "$NETWORK_KEY" in
  local)
    RPC_URL="${RPC_URL:-http://localhost:8000/soroban/rpc}"
    NETWORK_PASSPHRASE="${NETWORK_PASSPHRASE:-Standalone Network ; February 2017}"
    LP_REQUEST_DELAY="${LP_REQUEST_DELAY:-60}"
    DATABASE_URL_DEFAULT="postgresql://stellars:stellars@localhost:5432/stellars"
    ;;
  testnet)
    RPC_URL="${RPC_URL:-https://soroban-testnet.stellar.org}"
    NETWORK_PASSPHRASE="${NETWORK_PASSPHRASE:-Test SDF Network ; September 2015}"
    LP_REQUEST_DELAY="${LP_REQUEST_DELAY:-3600}"
    DATABASE_URL_DEFAULT="${DATABASE_URL:-}"
    ;;
  mainnet)
    RPC_URL="${RPC_URL:-https://soroban.stellar.org}"
    NETWORK_PASSPHRASE="${NETWORK_PASSPHRASE:-Public Global Stellar Network ; September 2015}"
    LP_REQUEST_DELAY="${LP_REQUEST_DELAY:-86400}"
    DATABASE_URL_DEFAULT="${DATABASE_URL:-}"
    ;;
  *)
    echo "❌ Unknown NETWORK_KEY '$NETWORK_KEY' — set RPC_URL/NETWORK_PASSPHRASE explicitly"
    : "${RPC_URL:?required}" "${NETWORK_PASSPHRASE:?required}"
    LP_REQUEST_DELAY="${LP_REQUEST_DELAY:-3600}"
    DATABASE_URL_DEFAULT="${DATABASE_URL:-}"
    ;;
esac

if ! command -v jq >/dev/null 2>&1; then
  echo "❌ jq is required — install via 'brew install jq'"
  exit 1
fi

# Load the bounded market registry before contract construction.
TICKERS=()
while IFS= read -r ticker; do
  [[ -n "$ticker" ]] && TICKERS+=("$ticker")
done < <(jq -r --arg net "$NETWORK_KEY" '.[$net].tickers[]' "$ADDRESSES_FILE")
if [[ ${#TICKERS[@]} -eq 0 ]]; then
  echo "❌ No tickers configured for network '$NETWORK_KEY' in $ADDRESSES_FILE"
  exit 1
fi

MAX_ACTIVE_MARKETS="${MAX_ACTIVE_MARKETS:-8}"
if (( ${#TICKERS[@]} > MAX_ACTIVE_MARKETS )); then
  echo "❌ ${#TICKERS[@]} configured markets exceed MAX_ACTIVE_MARKETS=$MAX_ACTIVE_MARKETS"
  exit 1
fi

# Explicit constructor parameters. They are deployment policy, not hidden
# contract defaults. Values use 1e7 USD/base precision where applicable.
GLOBAL_CONFIG="{\"min_collateral\":\"${MIN_COLLATERAL:-10000000}\",\"min_position_lifetime\":${MIN_POSITION_LIFETIME:-60},\"funding_half_life_seconds\":${FUNDING_HALF_LIFE_SECONDS:-43200},\"risk_capacity_limit_bps\":${RISK_CAPACITY_LIMIT_BPS:-8500},\"base_borrow_rate_bps_day\":\"${BASE_BORROW_RATE_BPS_DAY:-25}\",\"max_variable_borrow_bps_day\":\"${MAX_VARIABLE_BORROW_BPS_DAY:-250}\",\"lp_revenue_share_bps\":${LP_REVENUE_SHARE_BPS:-9000},\"risk_keeper_revenue_share_bps\":${RISK_KEEPER_REVENUE_SHARE_BPS:-500},\"hard_cap_factor_limit_bps\":${HARD_CAP_FACTOR_LIMIT_BPS:-10000},\"max_adl_reward\":\"${MAX_ADL_REWARD:-50000000}\",\"max_insolvent_touch_reward\":\"${MAX_INSOLVENT_TOUCH_REWARD:-50000000}\",\"max_active_markets\":${MAX_ACTIVE_MARKETS}}"
LP_CONFIG="{\"max_withdraw_utilization_bps\":${MAX_WITHDRAW_UTILIZATION_BPS:-8000},\"min_deposit_nav_factor_bps\":${MIN_DEPOSIT_NAV_FACTOR_BPS:-8000},\"lp_request_delay\":${LP_REQUEST_DELAY}}"
MARKET_CONFIG="{\"close_fee_low_bps\":${CLOSE_FEE_LOW_BPS:-50},\"close_fee_high_bps\":${CLOSE_FEE_HIGH_BPS:-150},\"max_funding_rate_bps_day\":\"${MAX_FUNDING_RATE_BPS_DAY:-80}\",\"instant_weight_bps\":${INSTANT_WEIGHT_BPS:-3000},\"market_risk_factor_bps\":${MARKET_RISK_FACTOR_BPS:-1000},\"max_long_size_open_interest\":\"${MAX_MARKET_SIZE_OPEN_INTEREST:-1000000000000000}\",\"max_short_size_open_interest\":\"${MAX_MARKET_SIZE_OPEN_INTEREST:-1000000000000000}\",\"max_long_base_exposure\":\"${MAX_MARKET_BASE_EXPOSURE:-1000000000000000000}\",\"max_short_base_exposure\":\"${MAX_MARKET_BASE_EXPOSURE:-1000000000000000000}\",\"recovery_pnl_factor_bps\":${RECOVERY_PNL_FACTOR_BPS:-250},\"warning_pnl_factor_bps\":${WARNING_PNL_FACTOR_BPS:-400},\"adl_pnl_factor_bps\":${ADL_PNL_FACTOR_BPS:-500},\"hard_cap_pnl_factor_bps\":${HARD_CAP_PNL_FACTOR_BPS:-600},\"initial_margin_bps\":${INITIAL_MARGIN_BPS:-500},\"maintenance_margin_bps\":${MAINTENANCE_MARGIN_BPS:-250},\"liquidation_reward_bps\":${LIQUIDATION_REWARD_BPS:-100},\"adl_reward_bps\":${ADL_REWARD_BPS:-5}}"

# Mainnet guardrails.
#   1. Typed confirmation prompt — no accidental `NETWORK_KEY=mainnet make deploy`
#      run-throughs.
#   2. Idempotency: refuse if addresses.json already has mainnet contracts.
#      Use scripts/upgrade.sh for re-deploys.
#   3. Role separation: ADMIN must not be deploying with UPGRADER + PAUSER
#      bundled in. The caller must explicitly supply UPGRADER_ADDR and
#      PAUSER_ADDR as separate accounts.
if [[ "$NETWORK_KEY" == "mainnet" ]]; then
  read -r -p "type MAINNET to confirm deploy to mainnet: " _confirm
  if [[ "$_confirm" != "MAINNET" ]]; then
    echo "❌ Aborted — confirmation string did not match."
    exit 1
  fi
  existing=$(jq -r '.mainnet.contracts.vault.address // empty' "$ADDRESSES_FILE" 2>/dev/null || true)
  if [[ -n "$existing" ]]; then
    echo "❌ Mainnet already has a vault deployed at $existing."
    echo "    Use scripts/upgrade.sh for redeploys, or hand-edit addresses.json if rotating."
    exit 1
  fi
  if [[ -z "${UPGRADER_ADDR:-}" ]] || [[ -z "${PAUSER_ADDR:-}" ]]; then
    echo "❌ Mainnet deploy requires UPGRADER_ADDR and PAUSER_ADDR to be distinct from ADMIN."
    echo "    Set both env vars before running."
    exit 1
  fi
fi

# Backup addresses.json before mutation so a botched run can be reverted
# with a single mv.
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

# Make sure the CLI knows about this network (idempotent).
if ! stellar network ls 2>/dev/null | grep -qE "^${NETWORK_KEY}\b"; then
  echo "Configuring '${NETWORK_KEY}' network in Stellar CLI..."
  stellar network add "$NETWORK_KEY" \
    --rpc-url "$RPC_URL" \
    --network-passphrase "$NETWORK_PASSPHRASE"
fi

# Identities must already exist — provision-keys.sh handles creation/funding.
require_identity() {
  local name=$1
  if ! stellar keys address "$name" >/dev/null 2>&1; then
    echo "❌ Identity '$name' not found. Run: NETWORK_KEY=$NETWORK_KEY bash scripts/provision-keys.sh"
    exit 1
  fi
}
require_identity admin
require_identity keeper
ADMIN_ADDR=$(stellar keys address admin)
KEEPER_ADDR=$(stellar keys address keeper)
UPGRADER_ROLE_ADDR="${UPGRADER_ADDR:-$ADMIN_ADDR}"
PAUSER_ROLE_ADDR="${PAUSER_ADDR:-$ADMIN_ADDR}"
if [[ "$NETWORK_KEY" == "mainnet" ]] \
  && { [[ "$UPGRADER_ROLE_ADDR" == "$ADMIN_ADDR" ]] || [[ "$PAUSER_ROLE_ADDR" == "$ADMIN_ADDR" ]]; }; then
  echo "❌ UPGRADER_ADDR and PAUSER_ADDR must NOT equal the admin address."
  exit 1
fi
echo "Admin:  $ADMIN_ADDR"
echo "Keeper: $KEEPER_ADDR"

# ---------- Build ----------
# Explicitly run the optimize target so the script resolves `.optimized.wasm`
# below. The Makefile's `deploy` target also depends on `optimize` but
# operators sometimes invoke this script directly; this keeps the
# prerequisite local.
echo ""
echo "=== Building + optimizing WASMs ==="
(cd "$ROOT" && make optimize)

# ---------- Helper ----------
deploy() {
  local name=$1
  shift
  # Any remaining args are the contract's constructor arguments, forwarded
  # after `--`. Protocol contracts initialize via their constructor (atomic
  # with deploy); the mocks have no constructor and are deployed with none.
  local wasm="$WASM_DIR/$(echo "$name" | tr '-' '_').optimized.wasm"
  if [[ ! -f "$wasm" ]]; then
    echo "❌ Optimized WASM missing: $wasm" >&2
    echo "    Run 'make optimize' first." >&2
    exit 1
  fi
  echo "Deploying $name (optimized)..." >&2
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
  # Post-deploy WASM hash verification. Confirm the on-chain bytecode matches
  # the file we built — catches a registry/proxy-injection where the deployed
  # code differs from what we sha256'd.
  local expected_hash actual_hash
  expected_hash=$(shasum -a 256 "$wasm" | awk '{print $1}')
  actual_hash=$(stellar contract info build-meta --id "$contract_id" \
    --rpc-url "$RPC_URL" \
    --network-passphrase "$NETWORK_PASSPHRASE" 2>/dev/null \
    | grep -oE 'sha256[: ]+[0-9a-f]+' | awk -F'[: ]+' '{print $2}' || true)
  if [[ -n "$actual_hash" ]] && [[ "$expected_hash" != "$actual_hash" ]]; then
    echo "❌ WASM hash mismatch for $name: expected=$expected_hash actual=$actual_hash" >&2
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

# ---------- Deploy contracts ----------
echo ""
echo "=== Deploying contracts ==="

CM_ID=$(deploy config-manager --admin "$ADMIN_ADDR")
CM_LEDGER=$(current_ledger)
echo "  config-manager : $CM_ID  (ledger $CM_LEDGER)"

OR_ID=$(deploy oracle-router --config_manager_address "$CM_ID")
OR_LEDGER=$(current_ledger)
echo "  oracle-router  : $OR_ID  (ledger $OR_LEDGER)"

ORACLE_ID=$(deploy oracle --config_manager "$CM_ID" --publisher "$ADMIN_ADDR")
ORACLE_LEDGER=$(current_ledger)
echo "  oracle (mock)  : $ORACLE_ID  (ledger $ORACLE_LEDGER)"

# OracleRouter requires a two-source quorum. Keep a second admin-published
# oracle in local deploys so the initial config is valid. Production replaces
# these with mock+binance+kucoin via deploy-cex-oracles.sh.
ORACLE2_ID=$(deploy oracle --config_manager "$CM_ID" --publisher "$ADMIN_ADDR")
ORACLE2_LEDGER=$(current_ledger)
echo "  oracle 2 (mock): $ORACLE2_ID  (ledger $ORACLE2_LEDGER)"

MOCK_TOKEN_ID=$(deploy mock-token)
MOCK_TOKEN_LEDGER=$(current_ledger)
echo "  mock-token     : $MOCK_TOKEN_ID  (ledger $MOCK_TOKEN_LEDGER)"

echo "  mock-token.initialize(admin, 7, USDC, USDC)"
invoke --id "$MOCK_TOKEN_ID" -- initialize \
  --admin "$ADMIN_ADDR" \
  --decimals 7 \
  --name "USDC" \
  --symbol "USDC"

# PositionManager is deployed BEFORE the Vault: the Vault binds its trusted
# PositionManager atomically in its own constructor, so PM must already exist.
# PM's constructor omits the Vault; the reference cycle is closed by the
# one-shot `set_vault` call below.
PM_ID=$(deploy position-manager \
  --config_manager "$CM_ID" \
  --oracle_router "$OR_ID" \
  --config "$GLOBAL_CONFIG")
PM_LEDGER=$(current_ledger)
echo "  position-mgr   : $PM_ID  (ledger $PM_LEDGER)"

VAULT_ID=$(deploy vault \
  --asset_address "$MOCK_TOKEN_ID" \
  --config_manager "$CM_ID" \
  --position_manager "$PM_ID" \
  --lp_config "$LP_CONFIG")
VAULT_LEDGER=$(current_ledger)
echo "  vault          : $VAULT_ID  (ledger $VAULT_LEDGER)"

REQUEST_ROUTER_ID=$(deploy request-router \
  --asset_address "$MOCK_TOKEN_ID" \
  --vault_address "$VAULT_ID" \
  --oracle_router "$OR_ID" \
  --config_manager_address "$CM_ID")
REQUEST_ROUTER_LEDGER=$(current_ledger)
echo "  request-router : $REQUEST_ROUTER_ID  (ledger $REQUEST_ROUTER_LEDGER)"

# ---------- Wire contracts ----------
echo ""
echo "=== Wiring contracts ==="

# Every protocol contract initializes via its constructor, atomically at
# deploy (the --constructor args above). The remaining post-deploy wiring call
# closes the Vault↔PositionManager reference cycle via set_vault.

echo "  position-manager.set_vault(admin, vault)"
invoke --id "$PM_ID" -- set_vault \
  --caller "$ADMIN_ADDR" \
  --vault "$VAULT_ID"

echo "  vault.set_request_router(admin, request-router)"
invoke --id "$VAULT_ID" -- set_request_router \
  --caller "$ADMIN_ADDR" \
  --request_router "$REQUEST_ROUTER_ID"

echo "  oracle-router.set_position_manager(admin, position-manager)"
invoke --id "$OR_ID" -- set_position_manager \
  --caller "$ADMIN_ADDR" \
  --position_manager "$PM_ID"

echo "  mock-token.configure_protocol(admin, vault, position-manager)"
invoke --id "$MOCK_TOKEN_ID" -- configure_protocol \
  --admin "$ADMIN_ADDR" \
  --vault "$VAULT_ID" \
  --position_manager "$PM_ID"

echo "  mock-token.set_protocol_contract(admin, request-router)"
invoke --id "$MOCK_TOKEN_ID" -- set_protocol_contract \
  --admin "$ADMIN_ADDR" \
  --contract "$REQUEST_ROUTER_ID" \
  --allowed true

# ---------- Grant roles ----------
echo ""
echo "=== Granting roles ==="

echo "  grant KEEPER to keeper"
invoke --id "$CM_ID" -- grant_role \
  --caller "$ADMIN_ADDR" \
  --role KEEPER \
  --account "$KEEPER_ADDR"

echo "  grant KEEPER to admin (sim/oracle seeding)"
invoke --id "$CM_ID" -- grant_role \
  --caller "$ADMIN_ADDR" \
  --role KEEPER \
  --account "$ADMIN_ADDR"

echo "  grant PAUSER"
invoke --id "$CM_ID" -- grant_role \
  --caller "$ADMIN_ADDR" \
  --role PAUSER \
  --account "$PAUSER_ROLE_ADDR"

# UPGRADER — needed for `upgrade.sh` to push new WASMs without an extra
# manual grant step. Fine on a single-admin deployment; production multi-sig
# would gate this with a separate operator key.
echo "  grant UPGRADER"
invoke --id "$CM_ID" -- grant_role \
  --caller "$ADMIN_ADDR" \
  --role UPGRADER \
  --account "$UPGRADER_ROLE_ADDR"

# ---------- Wire oracle ----------
echo ""
echo "=== Configuring oracle ==="

for ticker in "${TICKERS[@]}"; do
  echo "  set oracle sources: $ticker → [oracle, oracle2] (replace via deploy-cex-oracles)"
  invoke --id "$OR_ID" -- set_oracle_sources \
    --caller "$ADMIN_ADDR" \
    --symbol "$ticker" \
    --sources '["'"$ORACLE_ID"'","'"$ORACLE2_ID"'"]'
done

echo "  set oracle config"
invoke --id "$OR_ID" -- set_oracle_config \
  --caller "$ADMIN_ADDR" \
  --config '{"max_deviation_bps":"500","staleness_threshold":600,"cache_duration":10,"min_required_sources":2}'

# ---------- Configure markets ----------
echo ""
echo "=== Configuring PositionManager markets ==="
for ticker in "${TICKERS[@]}"; do
  echo "  set_market_config($ticker)"
  invoke --id "$PM_ID" -- set_market_config \
    --caller "$ADMIN_ADDR" \
    --market_symbol "$ticker" \
    --config "$MARKET_CONFIG"
done

# ---------- Write addresses.json ----------
echo ""
echo "=== Writing $ADDRESSES_FILE [$NETWORK_KEY] ==="
TMP_ADDR=$(mktemp)
jq \
  --arg net "$NETWORK_KEY" \
  --arg vault "$VAULT_ID"           --argjson vaultL    "${VAULT_LEDGER:-0}" \
  --arg rr "$REQUEST_ROUTER_ID"      --argjson rrL       "${REQUEST_ROUTER_LEDGER:-0}" \
  --arg pm "$PM_ID"                 --argjson pmL       "${PM_LEDGER:-0}" \
  --arg cm "$CM_ID"                 --argjson cmL       "${CM_LEDGER:-0}" \
  --arg or "$OR_ID"                 --argjson orL       "${OR_LEDGER:-0}" \
  --arg oracle "$ORACLE_ID"         --argjson oracleL   "${ORACLE_LEDGER:-0}" \
  --arg oracle2 "$ORACLE2_ID"       --argjson oracle2L  "${ORACLE2_LEDGER:-0}" \
  --arg mockToken "$MOCK_TOKEN_ID"  --argjson mockTokenL "${MOCK_TOKEN_LEDGER:-0}" \
  '.[$net].contracts.vault            = {address: $vault,     startLedger: $vaultL}
   | .[$net].contracts.requestRouter   = {address: $rr,        startLedger: $rrL}
   | .[$net].contracts.positionManager = {address: $pm,        startLedger: $pmL}
   | .[$net].contracts.configManager   = {address: $cm,        startLedger: $cmL}
   | .[$net].contracts.oracleRouter    = {address: $or,        startLedger: $orL}
   | .[$net].contracts.oracle          = {address: $oracle,    startLedger: $oracleL}
   | .[$net].contracts.oracle2         = {address: $oracle2,   startLedger: $oracle2L}
   | .[$net].contracts.mockToken       = {address: $mockToken, startLedger: $mockTokenL}' \
  "$ADDRESSES_FILE" > "$TMP_ADDR"
mv "$TMP_ADDR" "$ADDRESSES_FILE"

# ---------- Append service env ----------
# provision-keys.sh wrote the identity block; we extend it with the runtime
# env the off-chain services consume. Strip any prior service block so
# re-runs don't accumulate duplicates.
KEEPER_SECRET=$(stellar keys show keeper)
SERVICE_BLOCK_MARKER="# --- service env (deploy.sh) ---"
if [[ -f "$ENV_FILE" ]]; then
  TMP_ENV=$(mktemp)
  awk -v marker="$SERVICE_BLOCK_MARKER" '$0==marker{stop=1} !stop' "$ENV_FILE" > "$TMP_ENV"
  mv "$TMP_ENV" "$ENV_FILE"
fi
{
  echo "$SERVICE_BLOCK_MARKER"
  if [[ -n "$DATABASE_URL_DEFAULT" ]]; then
    echo "DATABASE_URL=$DATABASE_URL_DEFAULT"
  fi
  echo "POLL_INTERVAL_MS=${POLL_INTERVAL_MS:-3000}"
  echo "HEALTH_PORT=${HEALTH_PORT:-3001}"
  echo "KEEPER_SECRET=$KEEPER_SECRET"
  echo "ORACLE_CONTRACT=$ORACLE_ID"
  echo "MOCK_TOKEN_CONTRACT=$MOCK_TOKEN_ID"
  echo "ADMIN_ADDRESS=$ADMIN_ADDR"
  echo "KEEPER_ADDRESS=$KEEPER_ADDR"
} >> "$ENV_FILE"
chmod 600 "$ENV_FILE"

# Refresh the per-network deployment artifact services inject via ADDRESSES_JSON.
bash "$ROOT/scripts/split-deployments.sh" "$NETWORK_KEY"

echo ""
echo "=== Done ==="
echo "  Network   : $NETWORK_KEY"
echo "  Addresses → $ADDRESSES_FILE"
echo "  Deployment → deployments/$NETWORK_KEY.json"
echo "  Service env → $ENV_FILE"
echo ""
echo "Next: 'NETWORK_KEY=$NETWORK_KEY bash scripts/deploy-cex-oracles.sh' to wire CEX oracle publishers."
