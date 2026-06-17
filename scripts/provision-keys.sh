#!/usr/bin/env bash
# provision-keys.sh — Generate and (where the network supports it) fund the
# Stellar identities the protocol services need:
#
#   admin           — deploy + admin operations (KEEPER/PAUSER/ORACLE/UPGRADER)
#   keeper          — automated keeper bot (TP/SL, liquidations, ADL)
#   binance-oracle  — Binance price publisher
#   kucoin-oracle   — KuCoin price publisher
#
# Idempotent: an existing identity is left in place; only missing ones are
# created. Funding is best-effort — testnet/local friendbots can be flaky and
# we never let a 400 ("already funded") block the run.
#
# After this script the deploy pipeline assumes the named identities exist;
# scripts/deploy*.sh and deploy-cex-oracles.sh no longer try to generate
# their own keys, so secrets land in exactly one place.
#
# Usage:
#   bash scripts/provision-keys.sh                # local
#   NETWORK_KEY=testnet bash scripts/provision-keys.sh
#   NETWORK_KEY=mainnet bash scripts/provision-keys.sh   # generates only — no friendbot
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
NETWORK_KEY="${NETWORK_KEY:-local}"
ENV_FILE="${ENV_FILE:-$ROOT/.env.${NETWORK_KEY}}"

# Default identities, override via $IDENTITIES (space-separated).
IDENTITIES_DEFAULT="admin keeper binance-oracle kucoin-oracle"
IDENTITIES="${IDENTITIES:-$IDENTITIES_DEFAULT}"

# Friendbot endpoints per network. Empty string means "manual funding only"
# (mainnet, or anything we don't recognise).
case "$NETWORK_KEY" in
  local)
    FRIENDBOT="${FRIENDBOT:-http://localhost:8000/friendbot}"
    HORIZON="${HORIZON:-http://localhost:8000}"
    ;;
  testnet)
    FRIENDBOT="${FRIENDBOT:-https://friendbot.stellar.org}"
    HORIZON="${HORIZON:-https://horizon-testnet.stellar.org}"
    ;;
  mainnet)
    FRIENDBOT=""
    HORIZON="${HORIZON:-https://horizon.stellar.org}"
    ;;
  *)
    FRIENDBOT="${FRIENDBOT:-}"
    HORIZON="${HORIZON:-}"
    ;;
esac

if ! command -v stellar >/dev/null 2>&1; then
  echo "❌ stellar CLI not on PATH — see https://developers.stellar.org/docs/tools/developer-tools/cli/install-cli"
  exit 1
fi

# `stellar keys generate` writes to ~/.config/soroban/identity by default;
# that's the same store the deploy/upgrade scripts read from via `--source`,
# so we don't need to thread paths around.
ensure_key() {
  local name=$1
  # Status messages go to stderr so callers that capture stdout see only
  # the address. (Bash command substitution captures stdout only.)
  if stellar keys address "$name" >/dev/null 2>&1; then
    echo "✓ $name already exists" >&2
  else
    echo "+ generating $name" >&2
    # --no-fund: we fund explicitly below so the loop is uniform across
    # networks (and so a flaky friendbot fails LOUDLY rather than silently).
    stellar keys generate --no-fund "$name" >/dev/null
  fi
  stellar keys address "$name"
}

# Horizon /accounts/<addr> returns 200 once the funded account is visible.
# Polling that is the only reliable confirmation — friendbot returns 200/400
# regardless of whether the account has propagated to the API yet.
account_exists() {
  local addr=$1
  [[ -z "$HORIZON" ]] && return 1
  local code
  code=$(curl -s -o /dev/null -w "%{http_code}" --max-time 5 "$HORIZON/accounts/$addr") || code="000"
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
    curl -sf "$FRIENDBOT?addr=$addr" >/dev/null 2>&1 || true
    if account_exists "$addr"; then
      echo "  $label ($addr) — funded"
      return 0
    fi
    sleep 2
  done
  echo "❌ Failed to fund $label ($addr) after ~60s — re-run, or fund manually"
  return 1
}

echo "=== Provisioning identities for network '$NETWORK_KEY' ==="
declare -a ADDRS_OUT
for name in $IDENTITIES; do
  addr=$(ensure_key "$name")
  ADDRS_OUT+=("$name=$addr")
done

if [[ -n "$FRIENDBOT" ]]; then
  echo ""
  echo "=== Funding via $FRIENDBOT ==="
  for name in $IDENTITIES; do
    addr=$(stellar keys address "$name")
    fund_account "$addr" "$name" || true
  done
fi

# Persist a network-scoped env snapshot so server bundles don't have to
# call the CLI at runtime. The block is bracketed by markers so we can
# re-run provision-keys without clobbering the service block that deploy.sh
# (or any human edit) appends afterward.
echo ""
echo "=== Writing $ENV_FILE (identity block) ==="
IDENT_BEGIN="# --- identities (provision-keys) ---"
IDENT_END="# --- end identities ---"

# Strip any existing identity block from the file. awk's bracket-skip is
# robust when the markers happen to be the only content (fresh file) AND
# when other blocks live below.
if [[ -f "$ENV_FILE" ]]; then
  TMP_ENV=$(mktemp)
  awk -v b="$IDENT_BEGIN" -v e="$IDENT_END" '
    $0==b {skip=1; next}
    skip && $0==e {skip=0; next}
    !skip {print}
  ' "$ENV_FILE" > "$TMP_ENV"
  mv "$TMP_ENV" "$ENV_FILE"
fi

# Prepend the fresh identity block. Keep NETWORK at top so anything that
# greps `^NETWORK=` finds it without scanning past identity lines.
TMP_ENV=$(mktemp)
{
  echo "$IDENT_BEGIN"
  echo "# Updated $(date -u +%Y-%m-%dT%H:%M:%SZ) by scripts/provision-keys.sh"
  echo "NETWORK=$NETWORK_KEY"
  for name in $IDENTITIES; do
    upper=$(echo "$name" | tr '[:lower:]-' '[:upper:]_')
    echo "${upper}_ADDRESS=$(stellar keys address "$name")"
    echo "${upper}_SECRET=$(stellar keys show "$name")"
  done
  echo "$IDENT_END"
  if [[ -f "$ENV_FILE" ]]; then
    cat "$ENV_FILE"
  fi
} > "$TMP_ENV"
mv "$TMP_ENV" "$ENV_FILE"
chmod 600 "$ENV_FILE"

echo ""
echo "=== Done ==="
for line in "${ADDRS_OUT[@]}"; do echo "  $line"; done
echo ""
echo "Secrets written to $ENV_FILE (mode 600)."
echo "Next: NETWORK_KEY=$NETWORK_KEY bash scripts/deploy.sh"
