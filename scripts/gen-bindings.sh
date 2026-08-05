#!/bin/bash
set -e

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WASM_DIR="$ROOT/target/wasm32v1-none/release"
BIND_OUT="$ROOT/packages/bindings"

CONTRACTS=(
  vault
  request-router
  position-manager
  config-manager
  oracle-router
  oracle
  mock-token
)

# --- Clean ---
echo "Cleaning old bindings..."
for contract in "${CONTRACTS[@]}"; do
  rm -rf "$BIND_OUT/$contract"
done

# --- Generate ---
for contract in "${CONTRACTS[@]}"; do
  wasm="$WASM_DIR/${contract//-/_}.optimized.wasm"
  if [ ! -f "$wasm" ]; then
    echo "Error: $wasm not found. Run 'make optimize' first."
    exit 1
  fi
  echo "Generating $contract..."
  stellar contract bindings typescript \
    --wasm "$wasm" \
    --output-dir "$BIND_OUT/$contract" \
    --overwrite
done

# --- Parent package.json ---
cat > "$BIND_OUT/package.json" <<'EOF'
{
  "name": "@win-trader/bindings",
  "version": "0.0.4",
  "type": "module",
  "exports": {
    "./vault": "./vault/dist/index.js",
    "./request-router": "./request-router/dist/index.js",
    "./position-manager": "./position-manager/dist/index.js",
    "./config-manager": "./config-manager/dist/index.js",
    "./oracle-router": "./oracle-router/dist/index.js",
    "./oracle": "./oracle/dist/index.js",
    "./mock-token": "./mock-token/dist/index.js"
  },
  "files": ["vault/dist", "request-router/dist", "position-manager/dist", "config-manager/dist", "oracle-router/dist", "oracle/dist", "mock-token/dist"],
  "publishConfig": { "access": "public" },
  "repository": { "type": "git", "url": "git+https://github.com/win-trader/contracts.git", "directory": "packages/bindings" },
  "dependencies": {
    "@stellar/stellar-sdk": "^14.1.1",
    "buffer": "6.0.3"
  },
  "devDependencies": {
    "typescript": "^5.7.0"
  }
}
EOF

# --- Ensure compiler ---
if [ ! -x "$BIND_OUT/node_modules/.bin/tsc" ]; then
  echo "Installing binding compiler dependencies..."
  pnpm install --filter @win-trader/bindings
else
  echo "Using installed binding compiler."
fi

# --- Build ---
# Run tsc from BIND_OUT (where typescript is installed) and point at each
# contract's tsconfig with -p. The contract subdirs aren't workspace
# members, so pnpm exec only resolves tsc when invoked at BIND_OUT.
for contract in "${CONTRACTS[@]}"; do
  echo "Building $contract..."
  (cd "$BIND_OUT" && pnpm exec tsc -p "$contract" 2>/dev/null || true)
  if [ -f "$BIND_OUT/$contract/dist/index.js" ]; then
    echo "  OK"
  else
    echo "  FAILED - no dist output"
    exit 1
  fi
done

echo ""
echo "All bindings generated and built:"
for contract in "${CONTRACTS[@]}"; do
  echo "  - $contract/"
done
