#!/usr/bin/env bash
# check-sizes.sh — Fail if a protocol contract's optimized WASM exceeds the
# network's contract_max_size_bytes (131,072 on testnet and mainnet); warn when
# headroom drops below WARN_HEADROOM. Run after `make optimize`.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WASM_DIR="$ROOT/${WASM_DIR:-target/wasm32v1-none/release}"
LIMIT=131072
WARN_HEADROOM="${WARN_HEADROOM:-10240}"
CONTRACTS="${CONTRACTS:-config_manager market_governor position_manager vault request_router}"

status=0
for name in $CONTRACTS; do
  wasm="$WASM_DIR/$name.optimized.wasm"
  if [[ ! -f "$wasm" ]]; then
    echo "❌ $wasm missing — run 'make optimize'"
    status=1
    continue
  fi
  size=$(wc -c < "$wasm" | tr -d ' ')
  headroom=$((LIMIT - size))
  if (( headroom < 0 )); then
    echo "❌ $name: $size bytes, $((-headroom)) over the $LIMIT-byte limit"
    status=1
  elif (( headroom < WARN_HEADROOM )); then
    echo "⚠ $name: $size bytes, only $headroom bytes of headroom"
  else
    echo "✓ $name: $size bytes ($headroom headroom)"
  fi
done
exit $status
