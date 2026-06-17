#!/usr/bin/env bash
# deploy-local.sh — Compatibility shim. The deploy logic moved to
# scripts/deploy.sh which is network-agnostic; this script keeps the old
# entrypoint working for muscle-memory. Identity creation moved to
# scripts/provision-keys.sh.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

# Make sure identities exist before deploy.sh tries to use them. Cheap
# call — provision-keys is idempotent.
NETWORK_KEY=local bash "$ROOT/scripts/provision-keys.sh"
NETWORK_KEY=local bash "$ROOT/scripts/deploy.sh"
