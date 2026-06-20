#!/bin/bash
#
# split-deployments.sh <network>
#
# Extracts one network's config from the combined packages/config/addresses.json
# into deployments/<network>.json — the per-network artifact that off-chain
# services inject at runtime via ADDRESSES_JSON. The deploy scripts call this
# after they update addresses.json, so a `make deploy-testnet` refreshes only
# deployments/testnet.json (other networks untouched).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
NETWORK="${1:?usage: split-deployments.sh <network>}"
SRC="$ROOT/packages/config/addresses.json"
OUT_DIR="$ROOT/deployments"

mkdir -p "$OUT_DIR"
# `-e` makes jq exit non-zero if the network key is absent / null, so a typo
# fails loudly instead of writing an empty file.
jq -e --arg n "$NETWORK" '.[$n]' "$SRC" > "$OUT_DIR/$NETWORK.json"
echo "  deployments/$NETWORK.json written from addresses.json[$NETWORK]"
