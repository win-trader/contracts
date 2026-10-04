#!/usr/bin/env bash
# protocol.sh — Shared by deploy.sh, add-market.sh and upgrade.sh: network
# parameters and the constructor/config JSON, built from the same field set as
# `shared::types` so the scripts cannot drift from the contracts again.
#
# Every value defaults to `contracts/shared/src/defaults.rs` and can be
# overridden through the environment. i128 fields are JSON strings; u32/u64
# fields are JSON numbers.

ROOT="${ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
WASM_DIR="$ROOT/target/wasm32v1-none/release"
ADDRESSES_FILE="${ADDRESSES_FILE:-$ROOT/packages/config/addresses.json}"
NETWORK_KEY="${NETWORK_KEY:-local}"

case "$NETWORK_KEY" in
  local)
    RPC_URL="${RPC_URL:-http://localhost:8000/soroban/rpc}"
    NETWORK_PASSPHRASE="${NETWORK_PASSPHRASE:-Standalone Network ; February 2017}"
    LP_REQUEST_DELAY="${LP_REQUEST_DELAY:-60}"
    ;;
  testnet)
    RPC_URL="${RPC_URL:-https://soroban-testnet.stellar.org}"
    NETWORK_PASSPHRASE="${NETWORK_PASSPHRASE:-Test SDF Network ; September 2015}"
    LP_REQUEST_DELAY="${LP_REQUEST_DELAY:-3600}"
    ;;
  mainnet)
    RPC_URL="${RPC_URL:-https://soroban.stellar.org}"
    NETWORK_PASSPHRASE="${NETWORK_PASSPHRASE:-Public Global Stellar Network ; September 2015}"
    LP_REQUEST_DELAY="${LP_REQUEST_DELAY:-86400}"
    ;;
  *)
    : "${RPC_URL:?required for an unknown NETWORK_KEY}" "${NETWORK_PASSPHRASE:?required for an unknown NETWORK_KEY}"
    LP_REQUEST_DELAY="${LP_REQUEST_DELAY:-3600}"
    ;;
esac

if ! command -v jq >/dev/null 2>&1; then
  echo "❌ jq is required — install via 'brew install jq'" >&2
  exit 1
fi

KEEPER_REWARD="${KEEPER_REWARD:-2500000}"

GLOBAL_CONFIG=$(jq -nc \
  --arg min_collateral "${MIN_COLLATERAL:-10000000}" \
  --argjson min_position_lifetime "${MIN_POSITION_LIFETIME:-60}" \
  --argjson max_order_lifetime_seconds "${MAX_ORDER_LIFETIME_SECONDS:-604800}" \
  --argjson max_market_order_lifetime "${MAX_MARKET_ORDER_LIFETIME:-300}" \
  --argjson min_borrow_fee_seconds "${MIN_BORROW_FEE_SECONDS:-900}" \
  --argjson funding_half_life_seconds "${FUNDING_HALF_LIFE_SECONDS:-43200}" \
  --argjson max_price_age_seconds "${MAX_PRICE_AGE_SECONDS:-60}" \
  --argjson risk_capacity_limit_bps "${RISK_CAPACITY_LIMIT_BPS:-8500}" \
  --arg base_borrow_rate_bps_day "${BASE_BORROW_RATE_BPS_DAY:-25}" \
  --arg max_variable_borrow_bps_day "${MAX_VARIABLE_BORROW_BPS_DAY:-250}" \
  --argjson fee_lp_revenue_share_bps "${FEE_LP_REVENUE_SHARE_BPS:-9000}" \
  --argjson borrow_lp_revenue_share_bps "${BORROW_LP_REVENUE_SHARE_BPS:-9000}" \
  --argjson config_timelock_seconds "${CONFIG_TIMELOCK_SECONDS:-172800}" \
  --argjson max_active_markets "${MAX_ACTIVE_MARKETS:-8}" \
  --argjson global_hard_cap_limit_bps "${GLOBAL_HARD_CAP_LIMIT_BPS:-10000}" \
  --argjson hard_cap_relatch_band_bps "${HARD_CAP_RELATCH_BAND_BPS:-2500}" \
  --arg reward "$KEEPER_REWARD" \
  '$ARGS.named
   | del(.reward)
   | .keeper_rewards = ({open:0, limit_order:0, increase:0, decrease:0, close:0,
                          tp:0, sl:0, expiry:0, liquidation:0, adl:0, lp_resolve:0}
                         | map_values($reward))')

MARKET_CONFIG=$(jq -nc \
  --argjson open_fee_bps "${OPEN_FEE_BPS:-0}" \
  --argjson close_size_fee_bps "${CLOSE_SIZE_FEE_BPS:-5}" \
  --argjson close_pnl_fee_bps "${CLOSE_PNL_FEE_BPS:-1000}" \
  --arg max_funding_rate_bps_day "${MAX_FUNDING_RATE_BPS_DAY:-80}" \
  --argjson instant_weight_bps "${INSTANT_WEIGHT_BPS:-3000}" \
  --argjson market_risk_factor_bps "${MARKET_RISK_FACTOR_BPS:-1000}" \
  --argjson initial_margin_bps "${INITIAL_MARGIN_BPS:-500}" \
  --argjson maintenance_margin_bps "${MAINTENANCE_MARGIN_BPS:-250}" \
  --argjson recovery_pnl_factor_bps "${RECOVERY_PNL_FACTOR_BPS:-250}" \
  --argjson warning_pnl_factor_bps "${WARNING_PNL_FACTOR_BPS:-400}" \
  --argjson adl_pnl_factor_bps "${ADL_PNL_FACTOR_BPS:-500}" \
  --argjson hard_cap_pnl_factor_bps "${HARD_CAP_PNL_FACTOR_BPS:-600}" \
  --arg max_long_size_open_interest "${MAX_MARKET_SIZE_OPEN_INTEREST:-1000000000000000}" \
  --arg max_short_size_open_interest "${MAX_MARKET_SIZE_OPEN_INTEREST:-1000000000000000}" \
  --arg max_long_base_exposure "${MAX_MARKET_BASE_EXPOSURE:-1000000000000000000}" \
  --arg max_short_base_exposure "${MAX_MARKET_BASE_EXPOSURE:-1000000000000000000}" \
  --argjson order_execution_delay_seconds "${ORDER_EXECUTION_DELAY_SECONDS:-5}" \
  '$ARGS.named')

LP_CONFIG=$(jq -nc \
  --argjson max_withdraw_utilization_bps "${MAX_WITHDRAW_UTILIZATION_BPS:-8000}" \
  --argjson min_deposit_nav_factor_bps "${MIN_DEPOSIT_NAV_FACTOR_BPS:-1000}" \
  --argjson lp_request_delay_seconds "$LP_REQUEST_DELAY" \
  '$ARGS.named')

contract_addr() {
  jq -r --arg net "$NETWORK_KEY" --arg k "$1" '.[$net].contracts[$k].address // ""' "$ADDRESSES_FILE"
}

confirm_mainnet() {
  if [[ "$NETWORK_KEY" == "mainnet" ]]; then
    read -r -p "type MAINNET to confirm $1 on mainnet: " _confirm
    if [[ "$_confirm" != "MAINNET" ]]; then
      echo "❌ Aborted — confirmation string did not match." >&2
      exit 1
    fi
  fi
}
