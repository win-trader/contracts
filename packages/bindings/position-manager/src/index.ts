import { Buffer } from "buffer";
import { Address } from "@stellar/stellar-sdk";
import {
  AssembledTransaction,
  Client as ContractClient,
  ClientOptions as ContractClientOptions,
  MethodOptions,
  Result,
  Spec as ContractSpec,
} from "@stellar/stellar-sdk/contract";
import type {
  u32,
  i32,
  u64,
  i64,
  u128,
  i128,
  u256,
  i256,
  Option,
  Timepoint,
  Duration,
} from "@stellar/stellar-sdk/contract";
export * from "@stellar/stellar-sdk";
export * as contract from "@stellar/stellar-sdk/contract";
export * as rpc from "@stellar/stellar-sdk/rpc";

if (typeof window !== "undefined") {
  //@ts-ignore Buffer exists
  window.Buffer = window.Buffer || Buffer;
}




export const PositionManagerError = {
  1: {message:"Unauthorized"},
  2: {message:"NotInitialized"},
  3: {message:"AlreadyInitialized"},
  4: {message:"InvalidAmount"},
  5: {message:"InvalidConfig"},
  6: {message:"PositionNotFound"},
  7: {message:"MarketNotConfigured"},
  8: {message:"MarketDisabled"},
  9: {message:"SlippageExceeded"},
  10: {message:"CapacityExceeded"},
  11: {message:"MarketLimitExceeded"},
  12: {message:"InsufficientCollateral"},
  13: {message:"PositionHealthy"},
  14: {message:"RiskStateBlocked"},
  15: {message:"ArithmeticError"},
  16: {message:"InvalidOracleRound"},
  17: {message:"TooEarly"},
  18: {message:"InvalidOrder"},
  19: {message:"InsufficientExecutionBudget"},
  20: {message:"InvalidCaller"},
  /**
   * The contract is operationally paused (distinct from a risk state).
   */
  21: {message:"Paused"},
  /**
   * An accounting invariant broke — e.g. a negative pending fee, which
   * means a decreasing index or corrupted debt baseline (§11.2).
   */
  22: {message:"InvariantViolation"},
  /**
   * `upgrade` called with no pending proposal.
   */
  23: {message:"UpgradeNoPending"},
  /**
   * `upgrade` called before the proposal's timelock eta.
   */
  24: {message:"UpgradeTimelockNotElapsed"},
  /**
   * `upgrade` called with a hash that differs from the proposal.
   */
  25: {message:"UpgradeHashMismatch"},
  /**
   * No entry order exists for the given id.
   */
  26: {message:"OrderNotFound"},
  /**
   * `execute_entry_order` called before the trigger price was crossed.
   */
  27: {message:"OrderNotTriggered"},
  /**
   * `register_referral_code` for a code that is already owned.
   */
  28: {message:"ReferralCodeTaken"},
  /**
   * A referral code failed the length/format bounds.
   */
  29: {message:"ReferralCodeInvalid"},
  /**
   * `set_referrer` for a code no one has registered.
   */
  30: {message:"ReferralCodeNotFound"},
  /**
   * A trader tried to set their own code as their referrer.
   */
  31: {message:"SelfReferral"}
}

/**
 * Which collection routed revenue through the split (§13).
 */
export type FeeSource = {tag: "Closing", values: void} | {tag: "Borrow", values: void};

/**
 * Why a position left the book.
 */
export type CloseReason = {tag: "Trader", values: void} | {tag: "Liquidation", values: void} | {tag: "Deleverage", values: void} | {tag: "Order", values: void};










/**
 * Why an entry order was removed before filling.
 */
export type CancelReason = {tag: "Owner", values: void} | {tag: "Expired", values: void} | {tag: "PullFailed", values: void};




















/**
 * §5.1 global state: the five non-LP claim totals, the risk counters, and
 * the global borrow accrual. The receiver-funding liability total is fed
 * per-market by `funding::accrue` (§8.3).
 */
export interface Ledger {
  borrow_index: i128;
  borrow_index_remainder: i128;
  /**
 * `INDEX_PRECISION`-scaled bps/day rate for the current interval.
 */
current_borrow_rate: i128;
  execution_budget_total: i128;
  last_global_checkpoint: u64;
  lp_blocked_side_count: u32;
  open_position_count: u64;
  pending_receiver_funding_total: i128;
  position_collateral_total: i128;
  protocol_claimable_total: i128;
  /**
 * §11.1 referral rewards accrued but not yet claimed — the aggregate
 * backing the per-referrer `ReferralBalance` map (their sum is this
 * total). Like every claim here it is a label on cash already in the
 * vault, so NAV and the solvency checks net it out automatically.
 */
referral_claimable_total: i128;
  risk_keeper_reserve_total: i128;
  total_risk_units: i128;
}

export type StorageKey = {tag: "ConfigManager", values: void} | {tag: "OracleRouter", values: void} | {tag: "Vault", values: void} | {tag: "GlobalConfig", values: void} | {tag: "Initialized", values: void} | {tag: "Paused", values: void} | {tag: "NextPositionId", values: void} | {tag: "ActiveMarkets", values: void} | {tag: "Ledger", values: void} | {tag: "Version", values: void} | {tag: "Position", values: readonly [u64]} | {tag: "Market", values: readonly [string]} | {tag: "MarketDisabled", values: readonly [string]} | {tag: "EntryOrder", values: readonly [u64]} | {tag: "NextEntryOrderId", values: void} | {tag: "ReferralCode", values: readonly [string]} | {tag: "Referrer", values: readonly [string]} | {tag: "ReferralBalance", values: readonly [string]};


/**
 * Authoritative per-market state: the side aggregates, the funding indices,
 * the funding EMA, and the market configuration.
 * 
 * Soroban limits UDT field names to 30 characters, so where a doc glossary
 * term is longer the field drops the redundant qualifier and its doc
 * comment carries the full term (e.g. `receiver_backed_index_long` is the
 * doc's `receiver_backed_payer_index` for the long side).
 */
export interface Market {
  config: MarketConfig;
  /**
 * `INDEX_PRECISION`-scaled bps/day payer rate as of the last refresh.
 */
current_payer_rate: i128;
  current_payer_side: PayerSide;
  last_funding_checkpoint: u64;
  long: MarketSide;
  /**
 * Cumulative payer fee per unit of dominant-side size that is LP
 * revenue on collection (§8.2).
 */
lp_backed_index_long: i128;
  lp_backed_index_short: i128;
  lp_payer_remainder: i128;
  /**
 * Sub-stroop carry of the receiver-liability accrual (§8.3).
 */
pending_remainder: i128;
  /**
 * Cumulative payer fee per unit of dominant-side size whose collection
 * restores cash backing an already-accrued receiver claim (§8.2).
 */
receiver_backed_index_long: i128;
  receiver_backed_index_short: i128;
  /**
 * Cumulative funding credit per unit of light-side size (§8.2).
 */
receiver_index_long: i128;
  receiver_index_remainder: i128;
  receiver_index_short: i128;
  receiver_payer_remainder: i128;
  short: MarketSide;
  /**
 * §8.1 signed EMA of the instantaneous skew: a fraction of one at
 * `INDEX_PRECISION` scale, positive when history says longs dominate.
 * Decays toward the current skew with the global half-life.
 */
skew_ema: i128;
}


export interface LpConfig {
  lp_request_delay: u64;
  max_withdraw_utilization_bps: u32;
  min_deposit_nav_factor_bps: u32;
}


/**
 * Represents a single trader's open leveraged position.
 */
export interface Position {
  /**
 * Asset units at `PRICE_PRECISION`.
 */
base_exposure: i128;
  borrow_debt: i128;
  /**
 * Cash owned by an optional-order executor.
 */
execution_budget: i128;
  funding_paid_to_lps_debt: i128;
  funding_paid_to_receivers_debt: i128;
  funding_received_debt: i128;
  id: u64;
  is_long: boolean;
  last_increased_time: u64;
  market: string;
  owner: string;
  /**
 * Fixed gross capacity assigned when risk opens.
 */
risk_units: i128;
  /**
 * USD notional at `PRICE_PRECISION`.
 */
size: i128;
  /**
 * Trigger price for the optional stop-loss order; `0` = none.
 */
stop_loss: i128;
  /**
 * Trader-owned collateral recorded in contract state (the doc's
 * "stored collateral"). Effective collateral — stored collateral after
 * pending fees and funding credits — is always derived, never stored.
 */
stored_collateral: i128;
  /**
 * Trigger price for the optional take-profit order; `0` = none.
 */
take_profit: i128;
}


export interface LpRequest {
  amount: i128;
  execute_after: u64;
  id: u64;
  kind: LpRequestKind;
  owner: string;
  request_time: u64;
  status: LpRequestStatus;
}

/**
 * Which side currently pays funding (§8.1: the side the blended integral
 * skew points at — under the EMA this can be the *lighter* side for a
 * while after the book flips). `None` when the blend is exactly zero.
 */
export type PayerSide = {tag: "None", values: void} | {tag: "Long", values: void} | {tag: "Short", values: void};

export type RiskState = {tag: "Normal", values: void} | {tag: "Warning", values: void} | {tag: "Adl", values: void} | {tag: "HardCap", values: void};


/**
 * A pending limit/stop entry order: the frozen `open_position` arguments
 * plus a trigger condition and an expiry. Placing one only writes this
 * record — no funds move. A keeper's `execute_entry_order` pulls the
 * collateral via the owner's token allowance and opens the position
 * exactly as a market open would.
 */
export interface EntryOrder {
  acceptable_price: i128;
  collateral: i128;
  execution_budget: i128;
  /**
 * Ledger timestamp after which the order is dead and swept on the next
 * touch (user-configurable max TTL).
 */
expires_at: u64;
  id: u64;
  is_long: boolean;
  market: string;
  owner: string;
  size: i128;
  stop_loss: i128;
  take_profit: i128;
  /**
 * True → fill when price ≥ trigger (stop/breakout entry); false → fill
 * when price ≤ trigger (limit/dip entry). Inferred at placement from
 * the trigger vs. the current price.
 */
trigger_above: boolean;
  /**
 * The oracle price at which the order becomes fillable.
 */
trigger_price: i128;
}


export interface MarketSide {
  base_exposure: i128;
  risk_state: RiskState;
  risk_units: i128;
  size_open_interest: i128;
  stored_collateral_total: i128;
}


export interface RoundPrice {
  price: i128;
  symbol: string;
}


export interface OracleRound {
  id: u64;
  previous_id: u64;
  previous_timestamp: u64;
  prices: Array<RoundPrice>;
  timestamp: u64;
}


export interface GlobalConfig {
  base_borrow_rate_bps_day: i128;
  /**
 * §9.2 borrow-curve exponent, bps: 20_000 = u² (legacy quadratic),
 * 10_000 = linear. Bounded to ≤ 100_000 (e ≤ 10) by validation.
 */
borrow_exponent_bps: u32;
  /**
 * §8.1 half-life of the funding skew EMA, seconds (global: one memory
 * horizon for every market).
 */
funding_half_life_seconds: u64;
  hard_cap_factor_limit_bps: u32;
  lp_revenue_share_bps: u32;
  max_active_markets: u32;
  max_adl_reward: i128;
  max_insolvent_touch_reward: i128;
  max_variable_borrow_bps_day: i128;
  /**
 * §11.2 minimum borrow charge per capitalization: an index delta at
 * `INDEX_PRECISION` scale applied to the position's risk units
 * (2e11 = 2 bps of notional at a 10% market risk factor). Zero
 * disables the floor.
 */
min_borrow_index_delta: i128;
  min_collateral: i128;
  min_position_lifetime: u64;
  /**
 * §11.1 share of a closing fee routed to the trader's referrer, carved
 * from the protocol slice (keeper and LP shares are untouched). `0`
 * disables referral accrual globally — a kill switch. Validated so
 * `lp + keeper + referral ≤ BPS`, keeping the protocol remainder ≥ 0.
 */
referral_fee_share_bps: u32;
  risk_capacity_limit_bps: u32;
  risk_keeper_revenue_share_bps: u32;
}


export interface MarketConfig {
  adl_pnl_factor_bps: u32;
  adl_reward_bps: u32;
  /**
 * §11.1 closing-fee tier when the close worsens skew.
 */
close_fee_high_bps: u32;
  /**
 * §11.1 closing-fee tier when the close improves or preserves skew.
 */
close_fee_low_bps: u32;
  hard_cap_pnl_factor_bps: u32;
  /**
 * Margin required to open or add risk (§12.3). Divides max leverage:
 * a position may not be created closer to liquidation than this.
 */
initial_margin_bps: u32;
  /**
 * §8.1 weight of the instantaneous skew in the funding blend, in bps;
 * the rest is the half-life EMA. `BPS` reproduces pure instant skew.
 */
instant_weight_bps: u32;
  liquidation_reward_bps: u32;
  /**
 * Margin below which the position is liquidatable (§12.3). Must not
 * exceed `initial_margin_bps`; the gap is the entry buffer.
 */
maintenance_margin_bps: u32;
  market_risk_factor_bps: u32;
  max_funding_rate_bps_day: i128;
  max_long_base_exposure: i128;
  max_long_size_open_interest: i128;
  max_short_base_exposure: i128;
  max_short_size_open_interest: i128;
  recovery_pnl_factor_bps: u32;
  warning_pnl_factor_bps: u32;
}


/**
 * Global safety thresholds for price validation.
 */
export interface OracleConfig {
  /**
 * How long a cached aggregated price remains valid after the router
 * fetch (in seconds). A cache hit also requires every source timestamp
 * used for the cached median to remain within `staleness_threshold`.
 * Must be > 0 and <= `staleness_threshold`.
 */
cache_duration: u64;
  /**
 * Maximum allowed spread between oracle sources in basis points
 * (e.g., 100 = 1%). Bounded at `crate::constants::MAX_DEVIATION_BPS_CEILING`.
 */
max_deviation_bps: i128;
  /**
 * Minimum number of source responses that must agree within
 * `max_deviation_bps` for OracleRouter to return a price. Floored at
 * `crate::constants::MIN_REQUIRED_SOURCES_FLOOR`, ceilinged at
 * `crate::constants::MAX_ORACLE_SOURCES`.
 */
min_required_sources: u32;
  /**
 * Maximum age of an external SEP-40 price feed before it is rejected
 * as stale (in seconds).
 */
staleness_threshold: u64;
}

export type LpRequestKind = {tag: "Deposit", values: void} | {tag: "Withdrawal", values: void};


/**
 * Data required during a WASM migration. Single definition for all contracts.
 */
export interface MigrationData {
  version: u32;
}


/**
 * Pending WASM upgrade — set by `propose_upgrade`, consumed by `upgrade`
 * (cleared atomically on a successful install), or cleared by `cancel_upgrade`.
 * Single shape across every protocol contract. Contracts store it at
 * the shared `pending_upgrade` Symbol key in their own instance storage (see
 * `crate::upgrade::pending_upgrade_key`). `upgrade` refuses to install
 * unless `pending.wasm_hash` matches the supplied hash and `now >= eta`.
 */
export interface PendingUpgrade {
  eta: u64;
  wasm_hash: Buffer;
}

export type LpRequestStatus = {tag: "Pending", values: void} | {tag: "Settled", values: void} | {tag: "Failed", values: void} | {tag: "Expired", values: void};


/**
 * The caller-supplied fields of a `place_entry_order` request, bundled so
 * the entry point stays within Soroban's parameter limit. `owner` and
 * `market` are passed alongside; `id` and `trigger_above` are derived at
 * placement.
 */
export interface EntryOrderParams {
  acceptable_price: i128;
  collateral: i128;
  execution_budget: i128;
  expires_at: u64;
  is_long: boolean;
  size: i128;
  stop_loss: i128;
  take_profit: i128;
  trigger_price: i128;
}


export interface SettlementResult {
  /**
 * Shares minted for a deposit or assets paid for a withdrawal.
 */
amount: i128;
  status: SettlementStatus;
}

export type SettlementStatus = {tag: "Settled", values: void} | {tag: "Failed", values: void};


export interface AccountingSnapshot {
  cash_lp_equity: i128;
  cash_shortfall: i128;
  free_lp_capital: i128;
  lp_blocked_side_count: u32;
  non_lp_claims: i128;
  open_position_count: u64;
  physical_cash: i128;
  required_risk_backing: i128;
  total_risk_units: i128;
  vault_nav: i128;
}



export const UpgradeableError = {
  /**
   * When migration is attempted but not allowed due to upgrade state.
   */
  1100: {message:"MigrationNotAllowed"}
}



export const MerkleDistributorError = {
  /**
   * The merkle root is not set.
   */
  1300: {message:"RootNotSet"},
  /**
   * The provided index was already claimed.
   */
  1301: {message:"IndexAlreadyClaimed"},
  /**
   * The proof is invalid.
   */
  1302: {message:"InvalidProof"}
}

/**
 * Storage keys for the data associated with `MerkleDistributor`
 */
export type MerkleDistributorStorageKey = {tag: "Root", values: void} | {tag: "Claimed", values: readonly [u32]};

/**
 * Rounding direction for division operations
 */
export type Rounding = {tag: "Floor", values: void} | {tag: "Ceil", values: void} | {tag: "Truncate", values: void};

export const SorobanFixedPointError = {
  /**
   * Arithmetic overflow occurred
   */
  1500: {message:"Overflow"},
  /**
   * Division by zero
   */
  1501: {message:"DivisionByZero"}
}

export const CryptoError = {
  /**
   * The merkle proof length is out of bounds.
   */
  1400: {message:"MerkleProofOutOfBounds"},
  /**
   * The index of the leaf is out of bounds.
   */
  1401: {message:"MerkleIndexOutOfBounds"},
  /**
   * No data in hasher state.
   */
  1402: {message:"HasherEmptyState"}
}



export const PausableError = {
  /**
   * The operation failed because the contract is paused.
   */
  1000: {message:"EnforcedPause"},
  /**
   * The operation failed because the contract is not paused.
   */
  1001: {message:"ExpectedPause"}
}

/**
 * Storage key for the pausable state
 */
export type PausableStorageKey = {tag: "Paused", values: void};

export interface Client {
  /**
   * Construct and simulate a pause transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  pause: ({caller}: {caller: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a migrate transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  migrate: ({migration_data, operator}: {migration_data: MigrationData, operator: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a unpause transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  unpause: ({caller}: {caller: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a upgrade transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  upgrade: ({new_wasm_hash, operator}: {new_wasm_hash: Buffer, operator: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a set_tp_sl transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  set_tp_sl: ({position_id, take_profit, stop_loss}: {position_id: u64, take_profit: i128, stop_loss: i128}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a set_vault transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  set_vault: ({caller, vault}: {caller: string, vault: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a get_market transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_market: ({market}: {market: string}, options?: MethodOptions) => Promise<AssembledTransaction<Market>>

  /**
   * Construct and simulate a get_position transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_position: ({position_id}: {position_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Position>>

  /**
   * Construct and simulate a get_referrer transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_referrer: ({trader}: {trader: string}, options?: MethodOptions) => Promise<AssembledTransaction<Option<string>>>

  /**
   * Construct and simulate a recapitalize transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  recapitalize: ({contributor, amount}: {contributor: string, amount: i128}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a set_referrer transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  set_referrer: ({trader, code}: {trader: string, code: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a bump_position transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  bump_position: ({position_id}: {position_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a enable_market transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  enable_market: ({caller, market}: {caller: string, market: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a execute_order transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  execute_order: ({caller, position_id}: {caller: string, position_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a global_config transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  global_config: (options?: MethodOptions) => Promise<AssembledTransaction<GlobalConfig>>

  /**
   * Construct and simulate a non_lp_claims transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  non_lp_claims: (options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a open_position transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  open_position: ({owner, market_symbol, is_long, size, collateral, execution_budget, take_profit, stop_loss, acceptable_price}: {owner: string, market_symbol: string, is_long: boolean, size: i128, collateral: i128, execution_budget: i128, take_profit: i128, stop_loss: i128, acceptable_price: i128}, options?: MethodOptions) => Promise<AssembledTransaction<u64>>

  /**
   * Construct and simulate a active_markets transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  active_markets: (options?: MethodOptions) => Promise<AssembledTransaction<Array<string>>>

  /**
   * Construct and simulate a cancel_upgrade transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  cancel_upgrade: ({caller}: {caller: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a claim_protocol transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  claim_protocol: ({caller, recipient, amount}: {caller: string, recipient: string, amount: i128}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a claim_referral transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  claim_referral: ({referrer}: {referrer: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a disable_market transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  disable_market: ({caller, market}: {caller: string, market: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a update_indices transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  update_indices: ({caller, market_symbol}: {caller: string, market_symbol: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a get_entry_order transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_entry_order: ({order_id}: {order_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<EntryOrder>>

  /**
   * Construct and simulate a propose_upgrade transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  propose_upgrade: ({caller, wasm_hash}: {caller: string, wasm_hash: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a referral_balance transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  referral_balance: ({referrer}: {referrer: string}, options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a decrease_position transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  decrease_position: ({position_id, size_removed, collateral_withdrawn, acceptable_price}: {position_id: u64, size_removed: i128, collateral_withdrawn: i128, acceptable_price: i128}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a increase_position transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  increase_position: ({position_id, size_added, collateral_added, acceptable_price}: {position_id: u64, size_added: i128, collateral_added: i128, acceptable_price: i128}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a place_entry_order transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  place_entry_order: ({owner, market, params}: {owner: string, market: string, params: EntryOrderParams}, options?: MethodOptions) => Promise<AssembledTransaction<u64>>

  /**
   * Construct and simulate a set_global_config transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  set_global_config: ({caller, config}: {caller: string, config: GlobalConfig}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a set_market_config transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  set_market_config: ({caller, market_symbol, config}: {caller: string, market_symbol: string, config: MarketConfig}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a cancel_entry_order transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  cancel_entry_order: ({order_id}: {order_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a is_market_disabled transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  is_market_disabled: ({market}: {market: string}, options?: MethodOptions) => Promise<AssembledTransaction<boolean>>

  /**
   * Construct and simulate a liquidate_position transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  liquidate_position: ({caller, position_id}: {caller: string, position_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a accounting_snapshot transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  accounting_snapshot: ({round, physical}: {round: OracleRound, physical: i128}, options?: MethodOptions) => Promise<AssembledTransaction<AccountingSnapshot>>

  /**
   * Construct and simulate a deleverage_position transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  deleverage_position: ({caller, position_id}: {caller: string, position_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a execute_entry_order transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  execute_entry_order: ({caller, order_id}: {caller: string, order_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a prepare_lp_snapshot transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  prepare_lp_snapshot: ({caller, round, physical}: {caller: string, round: OracleRound, physical: i128}, options?: MethodOptions) => Promise<AssembledTransaction<AccountingSnapshot>>

  /**
   * Construct and simulate a referral_code_owner transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  referral_code_owner: ({code}: {code: string}, options?: MethodOptions) => Promise<AssembledTransaction<Option<string>>>

  /**
   * Construct and simulate a refresh_borrow_rate transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  refresh_borrow_rate: ({caller, physical}: {caller: string, physical: i128}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a can_create_lp_request transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  can_create_lp_request: ({caller, physical}: {caller: string, physical: i128}, options?: MethodOptions) => Promise<AssembledTransaction<boolean>>

  /**
   * Construct and simulate a fund_execution_budget transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  fund_execution_budget: ({position_id, amount}: {position_id: u64, amount: i128}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a register_referral_code transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  register_referral_code: ({owner, code}: {owner: string, code: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a protocol_claimable_total transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  protocol_claimable_total: (options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a referral_claimable_total transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  referral_claimable_total: (options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a risk_keeper_reserve_total transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  risk_keeper_reserve_total: (options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a withdraw_execution_budget transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  withdraw_execution_budget: ({position_id, amount}: {position_id: u64, amount: i128}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a pending_receiver_funding_total transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  pending_receiver_funding_total: (options?: MethodOptions) => Promise<AssembledTransaction<i128>>

}
export class Client extends ContractClient {
  static async deploy<T = Client>(
        /** Constructor/Initialization Args for the contract's `__constructor` method */
        {config_manager, oracle_router, config}: {config_manager: string, oracle_router: string, config: GlobalConfig},
    /** Options for initializing a Client as well as for calling a method, with extras specific to deploying. */
    options: MethodOptions &
      Omit<ContractClientOptions, "contractId"> & {
        /** The hash of the Wasm blob, which must already be installed on-chain. */
        wasmHash: Buffer | string;
        /** Salt used to generate the contract's ID. Passed through to {@link Operation.createCustomContract}. Default: random. */
        salt?: Buffer | Uint8Array;
        /** The format used to decode `wasmHash`, if it's provided as a string. */
        format?: "hex" | "base64";
      }
  ): Promise<AssembledTransaction<T>> {
    return ContractClient.deploy({config_manager, oracle_router, config}, options)
  }
  constructor(public readonly options: ContractClientOptions) {
    super(
      new ContractSpec([ "AAAABAAAAAAAAAAAAAAAFFBvc2l0aW9uTWFuYWdlckVycm9yAAAAHwAAAAAAAAAMVW5hdXRob3JpemVkAAAAAQAAAAAAAAAOTm90SW5pdGlhbGl6ZWQAAAAAAAIAAAAAAAAAEkFscmVhZHlJbml0aWFsaXplZAAAAAAAAwAAAAAAAAANSW52YWxpZEFtb3VudAAAAAAAAAQAAAAAAAAADUludmFsaWRDb25maWcAAAAAAAAFAAAAAAAAABBQb3NpdGlvbk5vdEZvdW5kAAAABgAAAAAAAAATTWFya2V0Tm90Q29uZmlndXJlZAAAAAAHAAAAAAAAAA5NYXJrZXREaXNhYmxlZAAAAAAACAAAAAAAAAAQU2xpcHBhZ2VFeGNlZWRlZAAAAAkAAAAAAAAAEENhcGFjaXR5RXhjZWVkZWQAAAAKAAAAAAAAABNNYXJrZXRMaW1pdEV4Y2VlZGVkAAAAAAsAAAAAAAAAFkluc3VmZmljaWVudENvbGxhdGVyYWwAAAAAAAwAAAAAAAAAD1Bvc2l0aW9uSGVhbHRoeQAAAAANAAAAAAAAABBSaXNrU3RhdGVCbG9ja2VkAAAADgAAAAAAAAAPQXJpdGhtZXRpY0Vycm9yAAAAAA8AAAAAAAAAEkludmFsaWRPcmFjbGVSb3VuZAAAAAAAEAAAAAAAAAAIVG9vRWFybHkAAAARAAAAAAAAAAxJbnZhbGlkT3JkZXIAAAASAAAAAAAAABtJbnN1ZmZpY2llbnRFeGVjdXRpb25CdWRnZXQAAAAAEwAAAAAAAAANSW52YWxpZENhbGxlcgAAAAAAABQAAABCVGhlIGNvbnRyYWN0IGlzIG9wZXJhdGlvbmFsbHkgcGF1c2VkIChkaXN0aW5jdCBmcm9tIGEgcmlzayBzdGF0ZSkuAAAAAAAGUGF1c2VkAAAAAAAVAAAAgkFuIGFjY291bnRpbmcgaW52YXJpYW50IGJyb2tlIOKAlCBlLmcuIGEgbmVnYXRpdmUgcGVuZGluZyBmZWUsIHdoaWNoCm1lYW5zIGEgZGVjcmVhc2luZyBpbmRleCBvciBjb3JydXB0ZWQgZGVidCBiYXNlbGluZSAowqcxMS4yKS4AAAAAABJJbnZhcmlhbnRWaW9sYXRpb24AAAAAABYAAAAqYHVwZ3JhZGVgIGNhbGxlZCB3aXRoIG5vIHBlbmRpbmcgcHJvcG9zYWwuAAAAAAAQVXBncmFkZU5vUGVuZGluZwAAABcAAAA0YHVwZ3JhZGVgIGNhbGxlZCBiZWZvcmUgdGhlIHByb3Bvc2FsJ3MgdGltZWxvY2sgZXRhLgAAABlVcGdyYWRlVGltZWxvY2tOb3RFbGFwc2VkAAAAAAAAGAAAADxgdXBncmFkZWAgY2FsbGVkIHdpdGggYSBoYXNoIHRoYXQgZGlmZmVycyBmcm9tIHRoZSBwcm9wb3NhbC4AAAATVXBncmFkZUhhc2hNaXNtYXRjaAAAAAAZAAAAJ05vIGVudHJ5IG9yZGVyIGV4aXN0cyBmb3IgdGhlIGdpdmVuIGlkLgAAAAANT3JkZXJOb3RGb3VuZAAAAAAAABoAAABCYGV4ZWN1dGVfZW50cnlfb3JkZXJgIGNhbGxlZCBiZWZvcmUgdGhlIHRyaWdnZXIgcHJpY2Ugd2FzIGNyb3NzZWQuAAAAAAART3JkZXJOb3RUcmlnZ2VyZWQAAAAAAAAbAAAAOmByZWdpc3Rlcl9yZWZlcnJhbF9jb2RlYCBmb3IgYSBjb2RlIHRoYXQgaXMgYWxyZWFkeSBvd25lZC4AAAAAABFSZWZlcnJhbENvZGVUYWtlbgAAAAAAABwAAAAwQSByZWZlcnJhbCBjb2RlIGZhaWxlZCB0aGUgbGVuZ3RoL2Zvcm1hdCBib3VuZHMuAAAAE1JlZmVycmFsQ29kZUludmFsaWQAAAAAHQAAADBgc2V0X3JlZmVycmVyYCBmb3IgYSBjb2RlIG5vIG9uZSBoYXMgcmVnaXN0ZXJlZC4AAAAUUmVmZXJyYWxDb2RlTm90Rm91bmQAAAAeAAAAN0EgdHJhZGVyIHRyaWVkIHRvIHNldCB0aGVpciBvd24gY29kZSBhcyB0aGVpciByZWZlcnJlci4AAAAADFNlbGZSZWZlcnJhbAAAAB8=",
        "AAAAAgAAADlXaGljaCBjb2xsZWN0aW9uIHJvdXRlZCByZXZlbnVlIHRocm91Z2ggdGhlIHNwbGl0ICjCpzEzKS4AAAAAAAAAAAAACUZlZVNvdXJjZQAAAAAAAAIAAAAAAAAAAAAAAAdDbG9zaW5nAAAAAAAAAAAAAAAABkJvcnJvdwAA",
        "AAAAAgAAAB1XaHkgYSBwb3NpdGlvbiBsZWZ0IHRoZSBib29rLgAAAAAAAAAAAAALQ2xvc2VSZWFzb24AAAAABAAAAAAAAAAcVGhlIG93bmVyIGRlY3JlYXNlZCB0byB6ZXJvLgAAAAZUcmFkZXIAAAAAAAAAAAA5QSBrZWVwZXIgb3IgdGhpcmQgcGFydHkgbGlxdWlkYXRlZCBhbiB1bmhlYWx0aHkgcG9zaXRpb24uAAAAAAAAC0xpcXVpZGF0aW9uAAAAAAAAAAAxRnVuZGVkIGF1dG8tZGVsZXZlcmFnaW5nIG9uIGFuIEFETC9oYXJkLWNhcCBzaWRlLgAAAAAAAApEZWxldmVyYWdlAAAAAAAAAAAALEEgdGFrZS1wcm9maXQgb3Igc3RvcC1sb3NzIHRyaWdnZXIgZXhlY3V0ZWQuAAAABU9yZGVyAAAA",
        "AAAABQAAAHhBbiBlbnRyeSBvcmRlciB3YXMgZmlsbGVkOiB0aGUgY29sbGF0ZXJhbCB3YXMgcHVsbGVkIGFuZCBhIHBvc2l0aW9uCm9wZW5lZCBhdCBgZmlsbF9wcmljZWAuIFRoZSBvcmRlciByZWNvcmQgaXMgcmVtb3ZlZC4AAAAAAAAAC09yZGVyRmlsbGVkAAAAAAEAAAAHb3JkZmlsbAAAAAAEAAAAAAAAAAhvcmRlcl9pZAAAAAYAAAABAAAAAAAAAAtwb3NpdGlvbl9pZAAAAAAGAAAAAAAAAAAAAAAIZXhlY3V0b3IAAAATAAAAAAAAAAAAAAAKZmlsbF9wcmljZQAAAAAACwAAAAAAAAAC",
        "AAAABQAAAEZBIGxpbWl0L3N0b3AgZW50cnkgb3JkZXIgd2FzIHBsYWNlZCAoc3RvcmFnZSBvbmx5IOKAlCBubyBmdW5kcyBtb3ZlZCkuAAAAAAAAAAAAC09yZGVyUGxhY2VkAAAAAAEAAAAIb3JkcGxhY2UAAAANAAAAAAAAAAhvcmRlcl9pZAAAAAYAAAABAAAAAAAAAAVvd25lcgAAAAAAABMAAAAAAAAAAAAAAAZtYXJrZXQAAAAAABEAAAAAAAAAAAAAAAdpc19sb25nAAAAAAEAAAAAAAAAAAAAAARzaXplAAAACwAAAAAAAAAAAAAACmNvbGxhdGVyYWwAAAAAAAsAAAAAAAAAAAAAABBleGVjdXRpb25fYnVkZ2V0AAAACwAAAAAAAAAAAAAAC3Rha2VfcHJvZml0AAAAAAsAAAAAAAAAAAAAAAlzdG9wX2xvc3MAAAAAAAALAAAAAAAAAAAAAAAQYWNjZXB0YWJsZV9wcmljZQAAAAsAAAAAAAAAAAAAAA10cmlnZ2VyX3ByaWNlAAAAAAAACwAAAAAAAAAAAAAADXRyaWdnZXJfYWJvdmUAAAAAAAABAAAAAAAAAAAAAAAKZXhwaXJlc19hdAAAAAAABgAAAAAAAAAC",
        "AAAABQAAADpBIHRyYWRlciAocmUtKXBvaW50ZWQgdGhlbXNlbHZlcyBhdCBhIHJlZmVycmVyIHZpYSBhIGNvZGUuAAAAAAAAAAAAC1JlZmVycmVyU2V0AAAAAAEAAAAGcmVmc2V0AAAAAAADAAAAAAAAAAZ0cmFkZXIAAAAAABMAAAAAAAAAAAAAAAhyZWZlcnJlcgAAABMAAAAAAAAAAAAAAARjb2RlAAAAEQAAAAAAAAAB",
        "AAAABQAAAAAAAAAAAAAADFBhdXNlQ2hhbmdlZAAAAAEAAAAFcGF1c2UAAAAAAAABAAAAAAAAAAZwYXVzZWQAAAAAAAEAAAAAAAAAAQ==",
        "AAAABQAAAD5DYXNoIGFkZGVkIGR1cmluZyBhIHNob3J0ZmFsbCB3aXRob3V0IG1pbnRpbmcgc2hhcmVzICjCpzE1LjIpLgAAAAAAAAAAAA1SZWNhcGl0YWxpemVkAAAAAAAAAQAAAAVyZWNhcAAAAAAAAAIAAAAAAAAAC2NvbnRyaWJ1dG9yAAAAABMAAAAAAAAAAAAAAAZhbW91bnQAAAAAAAsAAAAAAAAAAQ==",
        "AAAABQAAAQpBIGNvbGxlY3RlZCBmZWUgc3BsaXQgaW50byBpdHMgcmV2ZW51ZSBzaGFyZXMuIGBscF9zaGFyZWAgc3RheXMgaW4gdGhlCnZhdWx0IGFzIExQIGNhc2g7IHRoZSBvdGhlcnMgYWNjcnVlIHRvIHRoZWlyIGNsYWltIHRvdGFscy4gYHJlZmVycmFsYCBpcwpjYXJ2ZWQgZnJvbSB0aGUgcHJvdG9jb2wgc2xpY2UgKG9ubHkgbm9uemVybyBvbiBhIHJlZmVycmVkIGNsb3NpbmcgZmVlKTsKYGtlZXBlciArIGxwICsgcHJvdG9jb2wgKyByZWZlcnJhbCA9PSBjb2xsZWN0ZWRgLgAAAAAAAAAAAAxSZXZlbnVlU3BsaXQAAAABAAAACHJldnNwbGl0AAAABwAAAAAAAAALcG9zaXRpb25faWQAAAAABgAAAAAAAAAAAAAABnNvdXJjZQAAAAAH0AAAAAlGZWVTb3VyY2UAAAAAAAAAAAAAAAAAAAljb2xsZWN0ZWQAAAAAAAALAAAAAAAAAAAAAAAMa2VlcGVyX3NoYXJlAAAACwAAAAAAAAAAAAAACGxwX3NoYXJlAAAACwAAAAAAAAAAAAAADnByb3RvY29sX3NoYXJlAAAAAAALAAAAAAAAAAAAAAAOcmVmZXJyYWxfc2hhcmUAAAAAAAsAAAAAAAAAAQ==",
        "AAAABQAAAFhUYWtlLXByb2ZpdCAvIHN0b3AtbG9zcyB0cmlnZ2VycyBjaGFuZ2VkIG9uIGFuIG9wZW4gcG9zaXRpb24uIFplcm8gbWVhbnMKbm8gdHJpZ2dlciBzZXQuAAAAAAAAAAtUcFNsVXBkYXRlZAAAAAABAAAABHRwc2wAAAAFAAAAAAAAAAtwb3NpdGlvbl9pZAAAAAAGAAAAAQAAAAAAAAAFb3duZXIAAAAAAAATAAAAAAAAAAAAAAAGbWFya2V0AAAAAAARAAAAAAAAAAAAAAALdGFrZV9wcm9maXQAAAAACwAAAAAAAAAAAAAACXN0b3BfbG9zcwAAAAAAAAsAAAAAAAAAAg==",
        "AAAABQAAAAAAAAAAAAAADU9yZGVyRXhlY3V0ZWQAAAAAAAABAAAAB29yZGV4ZWMAAAAAAwAAAAAAAAALcG9zaXRpb25faWQAAAAABgAAAAAAAAAAAAAACGV4ZWN1dG9yAAAAEwAAAAAAAAAAAAAAC2J1ZGdldF9wYWlkAAAAAAsAAAAAAAAAAQ==",
        "AAAABQAAADRBREwgcmV3YXJkIHBhaWQgZnJvbSB0aGUgcmlzay1rZWVwZXIgcmVzZXJ2ZSAowqcxNCkuAAAAAAAAAA1BZGxSZXdhcmRQYWlkAAAAAAAAAQAAAAlhZGxyZXdhcmQAAAAAAAADAAAAAAAAAAtwb3NpdGlvbl9pZAAAAAAGAAAAAAAAAAAAAAAGa2VlcGVyAAAAAAATAAAAAAAAAAAAAAAGYW1vdW50AAAAAAALAAAAAAAAAAE=",
        "AAAAAgAAAC5XaHkgYW4gZW50cnkgb3JkZXIgd2FzIHJlbW92ZWQgYmVmb3JlIGZpbGxpbmcuAAAAAAAAAAAADENhbmNlbFJlYXNvbgAAAAMAAAAAAAAAF1RoZSBvd25lciBjYW5jZWxsZWQgaXQuAAAAAAVPd25lcgAAAAAAAAAAAAAjSXQgcGFzc2VkIGl0cyBleHBpcnkgYW5kIHdhcyBzd2VwdC4AAAAAB0V4cGlyZWQAAAAAAAAAAEZBIGZpbGwgYXR0ZW1wdCBjb3VsZCBub3QgcHVsbCB0aGUgY29sbGF0ZXJhbCAoYWxsb3dhbmNlL2JhbGFuY2UgZ29uZSkuAAAAAAAKUHVsbEZhaWxlZAAA",
        "AAAABQAAACpBbiBlbnRyeSBvcmRlciB3YXMgcmVtb3ZlZCBiZWZvcmUgZmlsbGluZy4AAAAAAAAAAAAOT3JkZXJDYW5jZWxsZWQAAAAAAAEAAAAJb3JkY2FuY2VsAAAAAAAAAgAAAAAAAAAIb3JkZXJfaWQAAAAGAAAAAAAAAAAAAAAGcmVhc29uAAAAAAfQAAAADENhbmNlbFJlYXNvbgAAAAAAAAAB",
        "AAAABQAAAGpBIGZ1bGwgY2xvc2UgdmlhIGFueSBwYXRoIOKAlCBgcmVhc29uYCBkaXN0aW5ndWlzaGVzIHRyYWRlciBjbG9zZSwKbGlxdWlkYXRpb24sIEFETCwgYW5kIHRyaWdnZXJlZCBvcmRlcnMuAAAAAAAAAAAADlBvc2l0aW9uQ2xvc2VkAAAAAAABAAAACHBvc2Nsb3NlAAAAEgAAAAAAAAALcG9zaXRpb25faWQAAAAABgAAAAEAAAAAAAAABW93bmVyAAAAAAAAEwAAAAAAAAAAAAAABm1hcmtldAAAAAAAEQAAAAAAAAAAAAAABnJlYXNvbgAAAAAH0AAAAAtDbG9zZVJlYXNvbgAAAAAAAAAAAAAAAARzaXplAAAACwAAAAAAAAAAAAAABXByaWNlAAAAAAAACwAAAAAAAAAAAAAAB3Jhd19wbmwAAAAACwAAAAAAAAAAAAAAC3BheWFibGVfcG5sAAAAAAsAAAAAAAAAOlJlc2lkdWFsIGNvbGxhdGVyYWwgcGFpZCB0byB0aGUgb3duZXIgYWZ0ZXIgdGhlIHdhdGVyZmFsbC4AAAAAABFjb2xsYXRlcmFsX3BheW91dAAAAAAAAAsAAAAAAAAAAAAAAAhiYWRfZGVidAAAAAsAAAAAAAAAAAAAABJsaXF1aWRhdGlvbl9yZXdhcmQAAAAAAAsAAAAAAAAAAAAAABlleGVjdXRpb25fYnVkZ2V0X3JlZnVuZGVkAAAAAAAACwAAAAAAAAA6wqcxMS4xIGNsb3NpbmcgZmVlIGNvbGxlY3RlZCBvdXQgb2YgdGhlIHJlYWxpemVkIHdpbm5pbmdzLgAAAAAAC2Nsb3NpbmdfZmVlAAAAAAsAAAAAAAAAAAAAABVyZWNlaXZlcl9mdW5kaW5nX3BhaWQAAAAAAAALAAAAAAAAAAAAAAAPbHBfZnVuZGluZ19wYWlkAAAAAAsAAAAAAAAAAAAAAAtib3Jyb3dfcGFpZAAAAAALAAAAAAAAAAAAAAAQZnVuZGluZ19yZWNlaXZlZAAAAAsAAAAAAAAAfE5lZ2F0aXZlIHByaWNlIFBuTCBjb2xsZWN0ZWQgZnJvbSBjb2xsYXRlcmFsOyB3aXRoIGBiYWRfZGVidGAgdGhpcwpkaXNhbWJpZ3VhdGVzIHRoZSBsb3NzLXZzLWZ1bmRpbmcgc3BsaXQgb2YgdGhlIHdhdGVyZmFsbC4AAAAObG9zc19jb2xsZWN0ZWQAAAAAAAsAAAAAAAAAAg==",
        "AAAABQAAAAAAAAAAAAAADlBvc2l0aW9uT3BlbmVkAAAAAAABAAAAB3Bvc29wZW4AAAAACwAAAAAAAAALcG9zaXRpb25faWQAAAAABgAAAAEAAAAAAAAABW93bmVyAAAAAAAAEwAAAAAAAAAAAAAABm1hcmtldAAAAAAAEQAAAAAAAAAAAAAAB2lzX2xvbmcAAAAAAQAAAAAAAAAAAAAABHNpemUAAAALAAAAAAAAAAAAAAANYmFzZV9leHBvc3VyZQAAAAAAAAsAAAAAAAAAAAAAABFzdG9yZWRfY29sbGF0ZXJhbAAAAAAAAAsAAAAAAAAAAAAAABBleGVjdXRpb25fYnVkZ2V0AAAACwAAAAAAAAAAAAAABXByaWNlAAAAAAAACwAAAAAAAAAaWmVybyBtZWFucyBubyB0cmlnZ2VyIHNldC4AAAAAAAt0YWtlX3Byb2ZpdAAAAAALAAAAAAAAAAAAAAAJc3RvcF9sb3NzAAAAAAAACwAAAAAAAAAC",
        "AAAABQAAAAAAAAAAAAAAD1Byb3RvY29sQ2xhaW1lZAAAAAABAAAACXByb3RjbGFpbQAAAAAAAAIAAAAAAAAACXJlY2lwaWVudAAAAAAAABMAAAAAAAAAAAAAAAZhbW91bnQAAAAAAAsAAAAAAAAAAQ==",
        "AAAABQAAAI9BIHJlZmVycmFsIHJld2FyZCB3YXMgYWNjcnVlZCB0byBhIHJlZmVycmVyIG91dCBvZiBhIHBvc2l0aW9uJ3MgY2xvc2luZwpmZWUgKGF0dHJpYnV0aW9uIGZvciB0aGUgaW5kZXhlcjsgdGhlIG1vbmV5IG1vdmUgaXMgaW4gYFJldmVudWVTcGxpdGApLgAAAAAAAAAAD1JlZmVycmFsQWNjcnVlZAAAAAABAAAAB3JlZmFjY3IAAAAAAwAAAAAAAAAIcmVmZXJyZXIAAAATAAAAAAAAAAAAAAALcG9zaXRpb25faWQAAAAABgAAAAAAAAAAAAAABmFtb3VudAAAAAAACwAAAAAAAAAB",
        "AAAABQAAADNBIHJlZmVycmVyIHdpdGhkcmV3IHRoZWlyIGFjY3J1ZWQgcmVmZXJyYWwgcmV3YXJkcy4AAAAAAAAAAA9SZWZlcnJhbENsYWltZWQAAAAAAQAAAAhyZWZjbGFpbQAAAAIAAAAAAAAACHJlZmVycmVyAAAAEwAAAAAAAAAAAAAABmFtb3VudAAAAAAACwAAAAAAAAAB",
        "AAAABQAAAT9GdW5kaW5nL2JvcnJvdyBpbmRpY2VzIGFuZCBjdXJyZW50IHJhdGVzIGFmdGVyIGEga2VlcGVyIGNoZWNrcG9pbnQKKGB1cGRhdGVfaW5kaWNlc2ApLiBUaGUgb2ZmLWNoYWluIGZlZSBwcm9qZWN0aW9uIGFuZCBzdGFsZW5lc3MgbW9uaXRvcnMKa2V5IG9uIHRoaXMgZXZlbnQuIFZhbHVlcyBhcmUgZXhhY3QgYXQgYHRpbWVzdGFtcGA7IHBvc2l0aW9uIGFjdGlvbnMKYmV0d2VlbiBrZWVwZXIgcnVucyBjaGFuZ2UgZmxvd3MgYW5kIHJhdGVzIHdpdGhvdXQgZW1pdHRpbmcgb25lLCBzbwpwcm9qZWN0aW9ucyBjYXJyeSBrZWVwZXItY2FkZW5jZSBzdGFsZW5lc3MuAAAAAAAAAAAQTWFya2V0Q2hlY2twb2ludAAAAAEAAAAGbWt0Y2hrAAAAAAANAAAAAAAAAAZtYXJrZXQAAAAAABEAAAABAAAAAAAAABpyZWNlaXZlcl9iYWNrZWRfaW5kZXhfbG9uZwAAAAAACwAAAAAAAAAAAAAAG3JlY2VpdmVyX2JhY2tlZF9pbmRleF9zaG9ydAAAAAALAAAAAAAAAAAAAAAUbHBfYmFja2VkX2luZGV4X2xvbmcAAAALAAAAAAAAAAAAAAAVbHBfYmFja2VkX2luZGV4X3Nob3J0AAAAAAAACwAAAAAAAAAAAAAAE3JlY2VpdmVyX2luZGV4X2xvbmcAAAAACwAAAAAAAAAAAAAAFHJlY2VpdmVyX2luZGV4X3Nob3J0AAAACwAAAAAAAAAAAAAAEmN1cnJlbnRfcGF5ZXJfc2lkZQAAAAAH0AAAAAlQYXllclNpZGUAAAAAAAAAAAAAAAAAABJjdXJyZW50X3BheWVyX3JhdGUAAAAAAAsAAAAAAAAAAAAAAAhza2V3X2VtYQAAAAsAAAAAAAAAAAAAAAxib3Jyb3dfaW5kZXgAAAALAAAAAAAAAAAAAAATY3VycmVudF9ib3Jyb3dfcmF0ZQAAAAALAAAAAAAAAAAAAAAJdGltZXN0YW1wAAAAAAAABgAAAAAAAAAC",
        "AAAABQAAAKxBIHBhcnRpYWwgY2xvc2UgKMKnMTIuMikuIEZlZSBmaWVsZHMgYXJlIHRoZSBhbW91bnRzIGFjdHVhbGx5IGNvbGxlY3RlZCBpbgp0aGlzIHNldHRsZW1lbnQ7IGBmdW5kaW5nX3JlY2VpdmVkYCBpcyB0aGUgY3JlZGl0IGNhcGl0YWxpemVkIGZyb20gdGhlCmd1YXJhbnRlZWQgcmVjZWl2ZXIgY2xhaW0uAAAAAAAAABFQb3NpdGlvbkRlY3JlYXNlZAAAAAAAAAEAAAAGcG9zZGVjAAAAAAAPAAAAAAAAAAtwb3NpdGlvbl9pZAAAAAAGAAAAAQAAAAAAAAAFb3duZXIAAAAAAAATAAAAAAAAAAAAAAAGbWFya2V0AAAAAAARAAAAAAAAAAAAAAAMc2l6ZV9yZW1vdmVkAAAACwAAAAAAAAAAAAAABXByaWNlAAAAAAAACwAAAAAAAAAAAAAAB3Jhd19wbmwAAAAACwAAAAAAAAAAAAAAC3BheWFibGVfcG5sAAAAAAsAAAAAAAAAAAAAAA9yZWFsaXplZF9wYXlvdXQAAAAACwAAAAAAAAAAAAAAFGNvbGxhdGVyYWxfd2l0aGRyYXduAAAACwAAAAAAAAA6wqcxMS4xIGNsb3NpbmcgZmVlIGNvbGxlY3RlZCBvdXQgb2YgdGhlIHJlYWxpemVkIHdpbm5pbmdzLgAAAAAAC2Nsb3NpbmdfZmVlAAAAAAsAAAAAAAAAAAAAABVyZWNlaXZlcl9mdW5kaW5nX3BhaWQAAAAAAAALAAAAAAAAAAAAAAAPbHBfZnVuZGluZ19wYWlkAAAAAAsAAAAAAAAAAAAAAAtib3Jyb3dfcGFpZAAAAAALAAAAAAAAAAAAAAAQZnVuZGluZ19yZWNlaXZlZAAAAAsAAAAAAAAAAAAAAA5sb3NzX2NvbGxlY3RlZAAAAAAACwAAAAAAAAAC",
        "AAAABQAAAAAAAAAAAAAAEVBvc2l0aW9uSW5jcmVhc2VkAAAAAAAAAQAAAAZwb3NpbmMAAAAAAAwAAAAAAAAAC3Bvc2l0aW9uX2lkAAAAAAYAAAABAAAAAAAAAAVvd25lcgAAAAAAABMAAAAAAAAAAAAAAAZtYXJrZXQAAAAAABEAAAAAAAAAAAAAAApzaXplX2FkZGVkAAAAAAALAAAAAAAAAAAAAAAKYmFzZV9hZGRlZAAAAAAACwAAAAAAAAAAAAAAEGNvbGxhdGVyYWxfYWRkZWQAAAALAAAAAAAAAAAAAAAFcHJpY2UAAAAAAAALAAAAAAAAACdTdG9yZWQgY29sbGF0ZXJhbCBhZnRlciBjYXBpdGFsaXphdGlvbi4AAAAAEXN0b3JlZF9jb2xsYXRlcmFsAAAAAAAACwAAAAAAAAB7QWNjcnVlZCBhbW91bnRzIHRoZSBpbmNyZWFzZSBjYXBpdGFsaXplZCBiZWZvcmUgYWRkaW5nIG5ldyBzaXplIOKAlAp0aGUgc2FtZSBkZWNvbXBvc2l0aW9uIHRoZSBkZWNyZWFzZS9jbG9zZSBldmVudHMgY2FycnkuAAAAABVyZWNlaXZlcl9mdW5kaW5nX3BhaWQAAAAAAAALAAAAAAAAAAAAAAAPbHBfZnVuZGluZ19wYWlkAAAAAAsAAAAAAAAAAAAAAAtib3Jyb3dfcGFpZAAAAAALAAAAAAAAAAAAAAAQZnVuZGluZ19yZWNlaXZlZAAAAAsAAAAAAAAAAg==",
        "AAAABQAAAI1BIHNpZGUgZW50ZXJlZCBvciBsZWZ0IGEgcmVzdHJpY3RlZCByaXNrIHN0YXRlICjCpzE0KS4gRW1pdHRlZCBvbmx5IG9uCmFjdHVhbCB0cmFuc2l0aW9ucyDigJQgdGhlIGtlZXBlcidzIHB1c2ggc2lnbmFsIGZvciBBREwvaGFyZC1jYXAgZHV0eS4AAAAAAAAAAAAAEFJpc2tTdGF0ZUNoYW5nZWQAAAABAAAACXJpc2tzdGF0ZQAAAAAAAAMAAAAAAAAABm1hcmtldAAAAAAAEQAAAAAAAAAAAAAAB2lzX2xvbmcAAAAAAQAAAAAAAAAAAAAABXN0YXRlAAAAAAAH0AAAAAlSaXNrU3RhdGUAAAAAAAAAAAAAAQ==",
        "AAAABQAAAAAAAAAAAAAAE0dsb2JhbENvbmZpZ1VwZGF0ZWQAAAAAAQAAAAljZmdnbG9iYWwAAAAAAAABAAAAAAAAAAZjb25maWcAAAAAB9AAAAAMR2xvYmFsQ29uZmlnAAAAAAAAAAI=",
        "AAAABQAAAAAAAAAAAAAAE01hcmtldENvbmZpZ1VwZGF0ZWQAAAAAAQAAAAljZmdtYXJrZXQAAAAAAAACAAAAAAAAAAZtYXJrZXQAAAAAABEAAAABAAAAAAAAAAZjb25maWcAAAAAB9AAAAAMTWFya2V0Q29uZmlnAAAAAAAAAAI=",
        "AAAABQAAAAAAAAAAAAAAE01hcmtldFN0YXR1c0NoYW5nZWQAAAAAAQAAAAlta3RzdGF0dXMAAAAAAAACAAAAAAAAAAZtYXJrZXQAAAAAABEAAAAAAAAAAAAAAAhkaXNhYmxlZAAAAAEAAAAAAAAAAQ==",
        "AAAABQAAAFdSZXdhcmQgZm9yIHJldmVhbGluZyBhbiBpbnNvbHZlbnQgcG9zaXRpb24sIHBhaWQgZnJvbSB0aGUgcmlzay1rZWVwZXIKcmVzZXJ2ZSAowqcxMi4zKS4AAAAAAAAAABRJbnNvbHZlbmN5UmV3YXJkUGFpZAAAAAEAAAAJaW5zcmV3YXJkAAAAAAAAAwAAAAAAAAALcG9zaXRpb25faWQAAAAABgAAAAAAAAAAAAAABmtlZXBlcgAAAAAAEwAAAAAAAAAAAAAABmFtb3VudAAAAAAACwAAAAAAAAAB",
        "AAAABQAAAAAAAAAAAAAAFUV4ZWN1dGlvbkJ1ZGdldEZ1bmRlZAAAAAAAAAEAAAAIYnVkZ2V0aW4AAAACAAAAAAAAAAtwb3NpdGlvbl9pZAAAAAAGAAAAAAAAAAAAAAAGYW1vdW50AAAAAAALAAAAAAAAAAE=",
        "AAAABQAAAEhBIHJlZmVycmVyIGNsYWltZWQgb3duZXJzaGlwIG9mIGEgcmVmZXJyYWwgY29kZSAob3duZXIgaW1tdXRhYmxlIGFmdGVyKS4AAAAAAAAAFlJlZmVycmFsQ29kZVJlZ2lzdGVyZWQAAAAAAAEAAAAGcmVmcmVnAAAAAAACAAAAAAAAAAVvd25lcgAAAAAAABMAAAAAAAAAAAAAAARjb2RlAAAAEQAAAAAAAAAB",
        "AAAABQAAAAAAAAAAAAAAGEV4ZWN1dGlvbkJ1ZGdldFdpdGhkcmF3bgAAAAEAAAAJYnVkZ2V0b3V0AAAAAAAAAgAAAAAAAAALcG9zaXRpb25faWQAAAAABgAAAAAAAAAAAAAABmFtb3VudAAAAAAACwAAAAAAAAAB",
        "AAAABQAAAAAAAAAAAAAAB0JhZERlYnQAAAAAAQAAAAdiYWRkZWJ0AAAAAAIAAAAAAAAAC3Bvc2l0aW9uX2lkAAAAAAYAAAAAAAAAAAAAAAZhbW91bnQAAAAAAAsAAAAAAAAAAQ==",
        "AAAAAQAAALjCpzUuMSBnbG9iYWwgc3RhdGU6IHRoZSBmaXZlIG5vbi1MUCBjbGFpbSB0b3RhbHMsIHRoZSByaXNrIGNvdW50ZXJzLCBhbmQKdGhlIGdsb2JhbCBib3Jyb3cgYWNjcnVhbC4gVGhlIHJlY2VpdmVyLWZ1bmRpbmcgbGlhYmlsaXR5IHRvdGFsIGlzIGZlZApwZXItbWFya2V0IGJ5IGBmdW5kaW5nOjphY2NydWVgICjCpzguMykuAAAAAAAAAAZMZWRnZXIAAAAAAA0AAAAAAAAADGJvcnJvd19pbmRleAAAAAsAAAAAAAAAFmJvcnJvd19pbmRleF9yZW1haW5kZXIAAAAAAAsAAAA/YElOREVYX1BSRUNJU0lPTmAtc2NhbGVkIGJwcy9kYXkgcmF0ZSBmb3IgdGhlIGN1cnJlbnQgaW50ZXJ2YWwuAAAAABNjdXJyZW50X2JvcnJvd19yYXRlAAAAAAsAAAAAAAAAFmV4ZWN1dGlvbl9idWRnZXRfdG90YWwAAAAAAAsAAAAAAAAAFmxhc3RfZ2xvYmFsX2NoZWNrcG9pbnQAAAAAAAYAAAAAAAAAFWxwX2Jsb2NrZWRfc2lkZV9jb3VudAAAAAAAAAQAAAAAAAAAE29wZW5fcG9zaXRpb25fY291bnQAAAAABgAAAAAAAAAecGVuZGluZ19yZWNlaXZlcl9mdW5kaW5nX3RvdGFsAAAAAAALAAAAAAAAABlwb3NpdGlvbl9jb2xsYXRlcmFsX3RvdGFsAAAAAAAACwAAAAAAAAAYcHJvdG9jb2xfY2xhaW1hYmxlX3RvdGFsAAAACwAAAQrCpzExLjEgcmVmZXJyYWwgcmV3YXJkcyBhY2NydWVkIGJ1dCBub3QgeWV0IGNsYWltZWQg4oCUIHRoZSBhZ2dyZWdhdGUKYmFja2luZyB0aGUgcGVyLXJlZmVycmVyIGBSZWZlcnJhbEJhbGFuY2VgIG1hcCAodGhlaXIgc3VtIGlzIHRoaXMKdG90YWwpLiBMaWtlIGV2ZXJ5IGNsYWltIGhlcmUgaXQgaXMgYSBsYWJlbCBvbiBjYXNoIGFscmVhZHkgaW4gdGhlCnZhdWx0LCBzbyBOQVYgYW5kIHRoZSBzb2x2ZW5jeSBjaGVja3MgbmV0IGl0IG91dCBhdXRvbWF0aWNhbGx5LgAAAAAAGHJlZmVycmFsX2NsYWltYWJsZV90b3RhbAAAAAsAAAAAAAAAGXJpc2tfa2VlcGVyX3Jlc2VydmVfdG90YWwAAAAAAAALAAAAAAAAABB0b3RhbF9yaXNrX3VuaXRzAAAACw==",
        "AAAAAgAAAAAAAAAAAAAAClN0b3JhZ2VLZXkAAAAAABIAAAAAAAAAAAAAAA1Db25maWdNYW5hZ2VyAAAAAAAAAAAAAAAAAAAMT3JhY2xlUm91dGVyAAAAAAAAAAAAAAAFVmF1bHQAAAAAAAAAAAAAAAAAAAxHbG9iYWxDb25maWcAAAAAAAAAAAAAAAtJbml0aWFsaXplZAAAAAAAAAAAAAAAAAZQYXVzZWQAAAAAAAAAAAAAAAAADk5leHRQb3NpdGlvbklkAAAAAAAAAAAAAAAAAA1BY3RpdmVNYXJrZXRzAAAAAAAAAAAAAAAAAAAGTGVkZ2VyAAAAAAAAAAAAAAAAAAdWZXJzaW9uAAAAAAEAAAAAAAAACFBvc2l0aW9uAAAAAQAAAAYAAAABAAAAAAAAAAZNYXJrZXQAAAAAAAEAAAARAAAAAQAAAAAAAAAOTWFya2V0RGlzYWJsZWQAAAAAAAEAAAARAAAAAQAAAAAAAAAKRW50cnlPcmRlcgAAAAAAAQAAAAYAAAAAAAAAAAAAABBOZXh0RW50cnlPcmRlcklkAAAAAQAAAEVSZWZlcnJhbCBjb2RlIOKGkiBvd25pbmcgcmVmZXJyZXIgYWRkcmVzcyAob3duZXIgaW1tdXRhYmxlIG9uY2Ugc2V0KS4AAAAAAAAMUmVmZXJyYWxDb2RlAAAAAQAAABEAAAABAAAAQFRyYWRlciDihpIgdGhlaXIgcmVmZXJyZXIgYWRkcmVzcyAoZnJlZWx5IHJlLXNldCBieSB0aGUgdHJhZGVyKS4AAAAIUmVmZXJyZXIAAAABAAAAEwAAAAEAAABzUmVmZXJyZXIg4oaSIGFjY3J1ZWQgdW5jbGFpbWVkIHJlZmVycmFsIHJld2FyZHMuIFRoZSBwZXItcmVmZXJyZXIKYWxsb2NhdGlvbiBvZiBgTGVkZ2VyOjpyZWZlcnJhbF9jbGFpbWFibGVfdG90YWxgLgAAAAAPUmVmZXJyYWxCYWxhbmNlAAAAAAEAAAAT",
        "AAAAAAAAAAAAAAAFcGF1c2UAAAAAAAABAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAA",
        "AAAAAAAAAAAAAAAHbWlncmF0ZQAAAAACAAAAAAAAAA5taWdyYXRpb25fZGF0YQAAAAAH0AAAAA1NaWdyYXRpb25EYXRhAAAAAAAAAAAAAAhvcGVyYXRvcgAAABMAAAAA",
        "AAAAAAAAAAAAAAAHdW5wYXVzZQAAAAABAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAA",
        "AAAAAAAAAAAAAAAHdXBncmFkZQAAAAACAAAAAAAAAA1uZXdfd2FzbV9oYXNoAAAAAAAD7gAAACAAAAAAAAAACG9wZXJhdG9yAAAAEwAAAAA=",
        "AAAAAAAAAAAAAAAJc2V0X3RwX3NsAAAAAAAAAwAAAAAAAAALcG9zaXRpb25faWQAAAAABgAAAAAAAAALdGFrZV9wcm9maXQAAAAACwAAAAAAAAAJc3RvcF9sb3NzAAAAAAAACwAAAAA=",
        "AAAAAAAAAAAAAAAJc2V0X3ZhdWx0AAAAAAAAAgAAAAAAAAAGY2FsbGVyAAAAAAATAAAAAAAAAAV2YXVsdAAAAAAAABMAAAAA",
        "AAAAAAAAAAAAAAAKZ2V0X21hcmtldAAAAAAAAQAAAAAAAAAGbWFya2V0AAAAAAARAAAAAQAAB9AAAAAGTWFya2V0AAA=",
        "AAAAAAAAAAAAAAAMZ2V0X3Bvc2l0aW9uAAAAAQAAAAAAAAALcG9zaXRpb25faWQAAAAABgAAAAEAAAfQAAAACFBvc2l0aW9u",
        "AAAAAAAAAAAAAAAMZ2V0X3JlZmVycmVyAAAAAQAAAAAAAAAGdHJhZGVyAAAAAAATAAAAAQAAA+gAAAAT",
        "AAAAAAAAAAAAAAAMcmVjYXBpdGFsaXplAAAAAgAAAAAAAAALY29udHJpYnV0b3IAAAAAEwAAAAAAAAAGYW1vdW50AAAAAAALAAAAAA==",
        "AAAAAAAAAAAAAAAMc2V0X3JlZmVycmVyAAAAAgAAAAAAAAAGdHJhZGVyAAAAAAATAAAAAAAAAARjb2RlAAAAEQAAAAA=",
        "AAAAAAAAAAAAAAANX19jb25zdHJ1Y3RvcgAAAAAAAAMAAAAAAAAADmNvbmZpZ19tYW5hZ2VyAAAAAAATAAAAAAAAAA1vcmFjbGVfcm91dGVyAAAAAAAAEwAAAAAAAAAGY29uZmlnAAAAAAfQAAAADEdsb2JhbENvbmZpZwAAAAA=",
        "AAAAAAAAAAAAAAANYnVtcF9wb3NpdGlvbgAAAAAAAAEAAAAAAAAAC3Bvc2l0aW9uX2lkAAAAAAYAAAAA",
        "AAAAAAAAAAAAAAANZW5hYmxlX21hcmtldAAAAAAAAAIAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAAAAAAGbWFya2V0AAAAAAARAAAAAA==",
        "AAAAAAAAAAAAAAANZXhlY3V0ZV9vcmRlcgAAAAAAAAIAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAAAAAALcG9zaXRpb25faWQAAAAABgAAAAA=",
        "AAAAAAAAAAAAAAANZ2xvYmFsX2NvbmZpZwAAAAAAAAAAAAABAAAH0AAAAAxHbG9iYWxDb25maWc=",
        "AAAAAAAAAAAAAAANbm9uX2xwX2NsYWltcwAAAAAAAAAAAAABAAAACw==",
        "AAAAAAAAAAAAAAANb3Blbl9wb3NpdGlvbgAAAAAAAAkAAAAAAAAABW93bmVyAAAAAAAAEwAAAAAAAAANbWFya2V0X3N5bWJvbAAAAAAAABEAAAAAAAAAB2lzX2xvbmcAAAAAAQAAAAAAAAAEc2l6ZQAAAAsAAAAAAAAACmNvbGxhdGVyYWwAAAAAAAsAAAAAAAAAEGV4ZWN1dGlvbl9idWRnZXQAAAALAAAAAAAAAAt0YWtlX3Byb2ZpdAAAAAALAAAAAAAAAAlzdG9wX2xvc3MAAAAAAAALAAAAAAAAABBhY2NlcHRhYmxlX3ByaWNlAAAACwAAAAEAAAAG",
        "AAAAAAAAAAAAAAAOYWN0aXZlX21hcmtldHMAAAAAAAAAAAABAAAD6gAAABE=",
        "AAAAAAAAAAAAAAAOY2FuY2VsX3VwZ3JhZGUAAAAAAAEAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAA=",
        "AAAAAAAAAAAAAAAOY2xhaW1fcHJvdG9jb2wAAAAAAAMAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAAAAAAJcmVjaXBpZW50AAAAAAAAEwAAAAAAAAAGYW1vdW50AAAAAAALAAAAAA==",
        "AAAAAAAAAAAAAAAOY2xhaW1fcmVmZXJyYWwAAAAAAAEAAAAAAAAACHJlZmVycmVyAAAAEwAAAAA=",
        "AAAAAAAAAAAAAAAOZGlzYWJsZV9tYXJrZXQAAAAAAAIAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAAAAAAGbWFya2V0AAAAAAARAAAAAA==",
        "AAAAAAAAAAAAAAAOdXBkYXRlX2luZGljZXMAAAAAAAIAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAAAAAANbWFya2V0X3N5bWJvbAAAAAAAABEAAAAA",
        "AAAAAAAAAAAAAAAPZ2V0X2VudHJ5X29yZGVyAAAAAAEAAAAAAAAACG9yZGVyX2lkAAAABgAAAAEAAAfQAAAACkVudHJ5T3JkZXIAAA==",
        "AAAAAAAAAAAAAAAPcHJvcG9zZV91cGdyYWRlAAAAAAIAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAAAAAAJd2FzbV9oYXNoAAAAAAAD7gAAACAAAAAA",
        "AAAAAAAAAAAAAAAQcmVmZXJyYWxfYmFsYW5jZQAAAAEAAAAAAAAACHJlZmVycmVyAAAAEwAAAAEAAAAL",
        "AAAAAAAAAAAAAAARZGVjcmVhc2VfcG9zaXRpb24AAAAAAAAEAAAAAAAAAAtwb3NpdGlvbl9pZAAAAAAGAAAAAAAAAAxzaXplX3JlbW92ZWQAAAALAAAAAAAAABRjb2xsYXRlcmFsX3dpdGhkcmF3bgAAAAsAAAAAAAAAEGFjY2VwdGFibGVfcHJpY2UAAAALAAAAAA==",
        "AAAAAAAAAAAAAAARaW5jcmVhc2VfcG9zaXRpb24AAAAAAAAEAAAAAAAAAAtwb3NpdGlvbl9pZAAAAAAGAAAAAAAAAApzaXplX2FkZGVkAAAAAAALAAAAAAAAABBjb2xsYXRlcmFsX2FkZGVkAAAACwAAAAAAAAAQYWNjZXB0YWJsZV9wcmljZQAAAAsAAAAA",
        "AAAAAAAAAAAAAAARcGxhY2VfZW50cnlfb3JkZXIAAAAAAAADAAAAAAAAAAVvd25lcgAAAAAAABMAAAAAAAAABm1hcmtldAAAAAAAEQAAAAAAAAAGcGFyYW1zAAAAAAfQAAAAEEVudHJ5T3JkZXJQYXJhbXMAAAABAAAABg==",
        "AAAAAAAAAAAAAAARc2V0X2dsb2JhbF9jb25maWcAAAAAAAACAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAAAAAABmNvbmZpZwAAAAAH0AAAAAxHbG9iYWxDb25maWcAAAAA",
        "AAAAAAAAAAAAAAARc2V0X21hcmtldF9jb25maWcAAAAAAAADAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAAAAAADW1hcmtldF9zeW1ib2wAAAAAAAARAAAAAAAAAAZjb25maWcAAAAAB9AAAAAMTWFya2V0Q29uZmlnAAAAAA==",
        "AAAAAAAAAAAAAAASY2FuY2VsX2VudHJ5X29yZGVyAAAAAAABAAAAAAAAAAhvcmRlcl9pZAAAAAYAAAAA",
        "AAAAAAAAAAAAAAASaXNfbWFya2V0X2Rpc2FibGVkAAAAAAABAAAAAAAAAAZtYXJrZXQAAAAAABEAAAABAAAAAQ==",
        "AAAAAAAAAAAAAAASbGlxdWlkYXRlX3Bvc2l0aW9uAAAAAAACAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAAAAAAC3Bvc2l0aW9uX2lkAAAAAAYAAAAA",
        "AAAAAAAAAAAAAAATYWNjb3VudGluZ19zbmFwc2hvdAAAAAACAAAAAAAAAAVyb3VuZAAAAAAAB9AAAAALT3JhY2xlUm91bmQAAAAAAAAAAAhwaHlzaWNhbAAAAAsAAAABAAAH0AAAABJBY2NvdW50aW5nU25hcHNob3QAAA==",
        "AAAAAAAAAAAAAAATZGVsZXZlcmFnZV9wb3NpdGlvbgAAAAACAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAAAAAAC3Bvc2l0aW9uX2lkAAAAAAYAAAAA",
        "AAAAAAAAAAAAAAATZXhlY3V0ZV9lbnRyeV9vcmRlcgAAAAACAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAAAAAACG9yZGVyX2lkAAAABgAAAAA=",
        "AAAAAAAAAAAAAAATcHJlcGFyZV9scF9zbmFwc2hvdAAAAAADAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAAAAAABXJvdW5kAAAAAAAH0AAAAAtPcmFjbGVSb3VuZAAAAAAAAAAACHBoeXNpY2FsAAAACwAAAAEAAAfQAAAAEkFjY291bnRpbmdTbmFwc2hvdAAA",
        "AAAAAAAAAAAAAAATcmVmZXJyYWxfY29kZV9vd25lcgAAAAABAAAAAAAAAARjb2RlAAAAEQAAAAEAAAPoAAAAEw==",
        "AAAAAAAAAAAAAAATcmVmcmVzaF9ib3Jyb3dfcmF0ZQAAAAACAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAAAAAACHBoeXNpY2FsAAAACwAAAAA=",
        "AAAAAAAAAAAAAAAVY2FuX2NyZWF0ZV9scF9yZXF1ZXN0AAAAAAAAAgAAAAAAAAAGY2FsbGVyAAAAAAATAAAAAAAAAAhwaHlzaWNhbAAAAAsAAAABAAAAAQ==",
        "AAAAAAAAAAAAAAAVZnVuZF9leGVjdXRpb25fYnVkZ2V0AAAAAAAAAgAAAAAAAAALcG9zaXRpb25faWQAAAAABgAAAAAAAAAGYW1vdW50AAAAAAALAAAAAA==",
        "AAAAAAAAAAAAAAAWcmVnaXN0ZXJfcmVmZXJyYWxfY29kZQAAAAAAAgAAAAAAAAAFb3duZXIAAAAAAAATAAAAAAAAAARjb2RlAAAAEQAAAAA=",
        "AAAAAAAAAAAAAAAYcHJvdG9jb2xfY2xhaW1hYmxlX3RvdGFsAAAAAAAAAAEAAAAL",
        "AAAAAAAAAAAAAAAYcmVmZXJyYWxfY2xhaW1hYmxlX3RvdGFsAAAAAAAAAAEAAAAL",
        "AAAAAAAAAAAAAAAZcmlza19rZWVwZXJfcmVzZXJ2ZV90b3RhbAAAAAAAAAAAAAABAAAACw==",
        "AAAAAAAAAAAAAAAZd2l0aGRyYXdfZXhlY3V0aW9uX2J1ZGdldAAAAAAAAAIAAAAAAAAAC3Bvc2l0aW9uX2lkAAAAAAYAAAAAAAAABmFtb3VudAAAAAAACwAAAAA=",
        "AAAAAAAAAAAAAAAecGVuZGluZ19yZWNlaXZlcl9mdW5kaW5nX3RvdGFsAAAAAAAAAAAAAQAAAAs=",
        "AAAAAQAAAYVBdXRob3JpdGF0aXZlIHBlci1tYXJrZXQgc3RhdGU6IHRoZSBzaWRlIGFnZ3JlZ2F0ZXMsIHRoZSBmdW5kaW5nIGluZGljZXMsCnRoZSBmdW5kaW5nIEVNQSwgYW5kIHRoZSBtYXJrZXQgY29uZmlndXJhdGlvbi4KClNvcm9iYW4gbGltaXRzIFVEVCBmaWVsZCBuYW1lcyB0byAzMCBjaGFyYWN0ZXJzLCBzbyB3aGVyZSBhIGRvYyBnbG9zc2FyeQp0ZXJtIGlzIGxvbmdlciB0aGUgZmllbGQgZHJvcHMgdGhlIHJlZHVuZGFudCBxdWFsaWZpZXIgYW5kIGl0cyBkb2MKY29tbWVudCBjYXJyaWVzIHRoZSBmdWxsIHRlcm0gKGUuZy4gYHJlY2VpdmVyX2JhY2tlZF9pbmRleF9sb25nYCBpcyB0aGUKZG9jJ3MgYHJlY2VpdmVyX2JhY2tlZF9wYXllcl9pbmRleGAgZm9yIHRoZSBsb25nIHNpZGUpLgAAAAAAAAAAAAAGTWFya2V0AAAAAAARAAAAAAAAAAZjb25maWcAAAAAB9AAAAAMTWFya2V0Q29uZmlnAAAAQ2BJTkRFWF9QUkVDSVNJT05gLXNjYWxlZCBicHMvZGF5IHBheWVyIHJhdGUgYXMgb2YgdGhlIGxhc3QgcmVmcmVzaC4AAAAAEmN1cnJlbnRfcGF5ZXJfcmF0ZQAAAAAACwAAAAAAAAASY3VycmVudF9wYXllcl9zaWRlAAAAAAfQAAAACVBheWVyU2lkZQAAAAAAAAAAAAAXbGFzdF9mdW5kaW5nX2NoZWNrcG9pbnQAAAAABgAAAAAAAAAEbG9uZwAAB9AAAAAKTWFya2V0U2lkZQAAAAAAXUN1bXVsYXRpdmUgcGF5ZXIgZmVlIHBlciB1bml0IG9mIGRvbWluYW50LXNpZGUgc2l6ZSB0aGF0IGlzIExQCnJldmVudWUgb24gY29sbGVjdGlvbiAowqc4LjIpLgAAAAAAABRscF9iYWNrZWRfaW5kZXhfbG9uZwAAAAsAAAAAAAAAFWxwX2JhY2tlZF9pbmRleF9zaG9ydAAAAAAAAAsAAAAAAAAAEmxwX3BheWVyX3JlbWFpbmRlcgAAAAAACwAAADtTdWItc3Ryb29wIGNhcnJ5IG9mIHRoZSByZWNlaXZlci1saWFiaWxpdHkgYWNjcnVhbCAowqc4LjMpLgAAAAARcGVuZGluZ19yZW1haW5kZXIAAAAAAAALAAAAhUN1bXVsYXRpdmUgcGF5ZXIgZmVlIHBlciB1bml0IG9mIGRvbWluYW50LXNpZGUgc2l6ZSB3aG9zZSBjb2xsZWN0aW9uCnJlc3RvcmVzIGNhc2ggYmFja2luZyBhbiBhbHJlYWR5LWFjY3J1ZWQgcmVjZWl2ZXIgY2xhaW0gKMKnOC4yKS4AAAAAAAAacmVjZWl2ZXJfYmFja2VkX2luZGV4X2xvbmcAAAAAAAsAAAAAAAAAG3JlY2VpdmVyX2JhY2tlZF9pbmRleF9zaG9ydAAAAAALAAAAPkN1bXVsYXRpdmUgZnVuZGluZyBjcmVkaXQgcGVyIHVuaXQgb2YgbGlnaHQtc2lkZSBzaXplICjCpzguMikuAAAAAAATcmVjZWl2ZXJfaW5kZXhfbG9uZwAAAAALAAAAAAAAABhyZWNlaXZlcl9pbmRleF9yZW1haW5kZXIAAAALAAAAAAAAABRyZWNlaXZlcl9pbmRleF9zaG9ydAAAAAsAAAAAAAAAGHJlY2VpdmVyX3BheWVyX3JlbWFpbmRlcgAAAAsAAAAAAAAABXNob3J0AAAAAAAH0AAAAApNYXJrZXRTaWRlAAAAAAC+wqc4LjEgc2lnbmVkIEVNQSBvZiB0aGUgaW5zdGFudGFuZW91cyBza2V3OiBhIGZyYWN0aW9uIG9mIG9uZSBhdApgSU5ERVhfUFJFQ0lTSU9OYCBzY2FsZSwgcG9zaXRpdmUgd2hlbiBoaXN0b3J5IHNheXMgbG9uZ3MgZG9taW5hdGUuCkRlY2F5cyB0b3dhcmQgdGhlIGN1cnJlbnQgc2tldyB3aXRoIHRoZSBnbG9iYWwgaGFsZi1saWZlLgAAAAAACHNrZXdfZW1hAAAACw==",
        "AAAAAQAAAAAAAAAAAAAACExwQ29uZmlnAAAAAwAAAAAAAAAQbHBfcmVxdWVzdF9kZWxheQAAAAYAAAAAAAAAHG1heF93aXRoZHJhd191dGlsaXphdGlvbl9icHMAAAAEAAAAAAAAABptaW5fZGVwb3NpdF9uYXZfZmFjdG9yX2JwcwAAAAAABA==",
        "AAAAAQAAADVSZXByZXNlbnRzIGEgc2luZ2xlIHRyYWRlcidzIG9wZW4gbGV2ZXJhZ2VkIHBvc2l0aW9uLgAAAAAAAAAAAAAIUG9zaXRpb24AAAAQAAAAIUFzc2V0IHVuaXRzIGF0IGBQUklDRV9QUkVDSVNJT05gLgAAAAAAAA1iYXNlX2V4cG9zdXJlAAAAAAAACwAAAAAAAAALYm9ycm93X2RlYnQAAAAACwAAAClDYXNoIG93bmVkIGJ5IGFuIG9wdGlvbmFsLW9yZGVyIGV4ZWN1dG9yLgAAAAAAABBleGVjdXRpb25fYnVkZ2V0AAAACwAAAAAAAAAYZnVuZGluZ19wYWlkX3RvX2xwc19kZWJ0AAAACwAAAAAAAAAeZnVuZGluZ19wYWlkX3RvX3JlY2VpdmVyc19kZWJ0AAAAAAALAAAAAAAAABVmdW5kaW5nX3JlY2VpdmVkX2RlYnQAAAAAAAALAAAAAAAAAAJpZAAAAAAABgAAAAAAAAAHaXNfbG9uZwAAAAABAAAAAAAAABNsYXN0X2luY3JlYXNlZF90aW1lAAAAAAYAAAAAAAAABm1hcmtldAAAAAAAEQAAAAAAAAAFb3duZXIAAAAAAAATAAAALkZpeGVkIGdyb3NzIGNhcGFjaXR5IGFzc2lnbmVkIHdoZW4gcmlzayBvcGVucy4AAAAAAApyaXNrX3VuaXRzAAAAAAALAAAAIlVTRCBub3Rpb25hbCBhdCBgUFJJQ0VfUFJFQ0lTSU9OYC4AAAAAAARzaXplAAAACwAAADtUcmlnZ2VyIHByaWNlIGZvciB0aGUgb3B0aW9uYWwgc3RvcC1sb3NzIG9yZGVyOyBgMGAgPSBub25lLgAAAAAJc3RvcF9sb3NzAAAAAAAACwAAAMpUcmFkZXItb3duZWQgY29sbGF0ZXJhbCByZWNvcmRlZCBpbiBjb250cmFjdCBzdGF0ZSAodGhlIGRvYydzCiJzdG9yZWQgY29sbGF0ZXJhbCIpLiBFZmZlY3RpdmUgY29sbGF0ZXJhbCDigJQgc3RvcmVkIGNvbGxhdGVyYWwgYWZ0ZXIKcGVuZGluZyBmZWVzIGFuZCBmdW5kaW5nIGNyZWRpdHMg4oCUIGlzIGFsd2F5cyBkZXJpdmVkLCBuZXZlciBzdG9yZWQuAAAAAAARc3RvcmVkX2NvbGxhdGVyYWwAAAAAAAALAAAAPVRyaWdnZXIgcHJpY2UgZm9yIHRoZSBvcHRpb25hbCB0YWtlLXByb2ZpdCBvcmRlcjsgYDBgID0gbm9uZS4AAAAAAAALdGFrZV9wcm9maXQAAAAACw==",
        "AAAAAQAAAAAAAAAAAAAACUxwUmVxdWVzdAAAAAAAAAcAAAAAAAAABmFtb3VudAAAAAAACwAAAAAAAAANZXhlY3V0ZV9hZnRlcgAAAAAAAAYAAAAAAAAAAmlkAAAAAAAGAAAAAAAAAARraW5kAAAH0AAAAA1McFJlcXVlc3RLaW5kAAAAAAAAAAAAAAVvd25lcgAAAAAAABMAAAAAAAAADHJlcXVlc3RfdGltZQAAAAYAAAAAAAAABnN0YXR1cwAAAAAH0AAAAA9McFJlcXVlc3RTdGF0dXMA",
        "AAAAAgAAANFXaGljaCBzaWRlIGN1cnJlbnRseSBwYXlzIGZ1bmRpbmcgKMKnOC4xOiB0aGUgc2lkZSB0aGUgYmxlbmRlZCBpbnRlZ3JhbApza2V3IHBvaW50cyBhdCDigJQgdW5kZXIgdGhlIEVNQSB0aGlzIGNhbiBiZSB0aGUgKmxpZ2h0ZXIqIHNpZGUgZm9yIGEKd2hpbGUgYWZ0ZXIgdGhlIGJvb2sgZmxpcHMpLiBgTm9uZWAgd2hlbiB0aGUgYmxlbmQgaXMgZXhhY3RseSB6ZXJvLgAAAAAAAAAAAAAJUGF5ZXJTaWRlAAAAAAAAAwAAAAAAAAAAAAAABE5vbmUAAAAAAAAAAAAAAARMb25nAAAAAAAAAAAAAAAFU2hvcnQAAAA=",
        "AAAAAgAAAAAAAAAAAAAACVJpc2tTdGF0ZQAAAAAAAAQAAAAAAAAAAAAAAAZOb3JtYWwAAAAAAAAAAAAAAAAAB1dhcm5pbmcAAAAAAAAAAAAAAAADQWRsAAAAAAAAAAAAAAAAB0hhcmRDYXAA",
        "AAAAAQAAATJBIHBlbmRpbmcgbGltaXQvc3RvcCBlbnRyeSBvcmRlcjogdGhlIGZyb3plbiBgb3Blbl9wb3NpdGlvbmAgYXJndW1lbnRzCnBsdXMgYSB0cmlnZ2VyIGNvbmRpdGlvbiBhbmQgYW4gZXhwaXJ5LiBQbGFjaW5nIG9uZSBvbmx5IHdyaXRlcyB0aGlzCnJlY29yZCDigJQgbm8gZnVuZHMgbW92ZS4gQSBrZWVwZXIncyBgZXhlY3V0ZV9lbnRyeV9vcmRlcmAgcHVsbHMgdGhlCmNvbGxhdGVyYWwgdmlhIHRoZSBvd25lcidzIHRva2VuIGFsbG93YW5jZSBhbmQgb3BlbnMgdGhlIHBvc2l0aW9uCmV4YWN0bHkgYXMgYSBtYXJrZXQgb3BlbiB3b3VsZC4AAAAAAAAAAAAKRW50cnlPcmRlcgAAAAAADQAAAAAAAAAQYWNjZXB0YWJsZV9wcmljZQAAAAsAAAAAAAAACmNvbGxhdGVyYWwAAAAAAAsAAAAAAAAAEGV4ZWN1dGlvbl9idWRnZXQAAAALAAAAZ0xlZGdlciB0aW1lc3RhbXAgYWZ0ZXIgd2hpY2ggdGhlIG9yZGVyIGlzIGRlYWQgYW5kIHN3ZXB0IG9uIHRoZSBuZXh0CnRvdWNoICh1c2VyLWNvbmZpZ3VyYWJsZSBtYXggVFRMKS4AAAAACmV4cGlyZXNfYXQAAAAAAAYAAAAAAAAAAmlkAAAAAAAGAAAAAAAAAAdpc19sb25nAAAAAAEAAAAAAAAABm1hcmtldAAAAAAAEQAAAAAAAAAFb3duZXIAAAAAAAATAAAAAAAAAARzaXplAAAACwAAAAAAAAAJc3RvcF9sb3NzAAAAAAAACwAAAAAAAAALdGFrZV9wcm9maXQAAAAACwAAALJUcnVlIOKGkiBmaWxsIHdoZW4gcHJpY2Ug4omlIHRyaWdnZXIgKHN0b3AvYnJlYWtvdXQgZW50cnkpOyBmYWxzZSDihpIgZmlsbAp3aGVuIHByaWNlIOKJpCB0cmlnZ2VyIChsaW1pdC9kaXAgZW50cnkpLiBJbmZlcnJlZCBhdCBwbGFjZW1lbnQgZnJvbQp0aGUgdHJpZ2dlciB2cy4gdGhlIGN1cnJlbnQgcHJpY2UuAAAAAAANdHJpZ2dlcl9hYm92ZQAAAAAAAAEAAAA1VGhlIG9yYWNsZSBwcmljZSBhdCB3aGljaCB0aGUgb3JkZXIgYmVjb21lcyBmaWxsYWJsZS4AAAAAAAANdHJpZ2dlcl9wcmljZQAAAAAAAAs=",
        "AAAAAQAAAAAAAAAAAAAACk1hcmtldFNpZGUAAAAAAAUAAAAAAAAADWJhc2VfZXhwb3N1cmUAAAAAAAALAAAAAAAAAApyaXNrX3N0YXRlAAAAAAfQAAAACVJpc2tTdGF0ZQAAAAAAAAAAAAAKcmlza191bml0cwAAAAAACwAAAAAAAAASc2l6ZV9vcGVuX2ludGVyZXN0AAAAAAALAAAAAAAAABdzdG9yZWRfY29sbGF0ZXJhbF90b3RhbAAAAAAL",
        "AAAAAQAAAAAAAAAAAAAAClJvdW5kUHJpY2UAAAAAAAIAAAAAAAAABXByaWNlAAAAAAAACwAAAAAAAAAGc3ltYm9sAAAAAAAR",
        "AAAAAQAAAAAAAAAAAAAAC09yYWNsZVJvdW5kAAAAAAUAAAAAAAAAAmlkAAAAAAAGAAAAAAAAAAtwcmV2aW91c19pZAAAAAAGAAAAAAAAABJwcmV2aW91c190aW1lc3RhbXAAAAAAAAYAAAAAAAAABnByaWNlcwAAAAAD6gAAB9AAAAAKUm91bmRQcmljZQAAAAAAAAAAAAl0aW1lc3RhbXAAAAAAAAAG",
        "AAAAAQAAAAAAAAAAAAAADEdsb2JhbENvbmZpZwAAAA8AAAAAAAAAGGJhc2VfYm9ycm93X3JhdGVfYnBzX2RheQAAAAsAAACEwqc5LjIgYm9ycm93LWN1cnZlIGV4cG9uZW50LCBicHM6IDIwXzAwMCA9IHXCsiAobGVnYWN5IHF1YWRyYXRpYyksCjEwXzAwMCA9IGxpbmVhci4gQm91bmRlZCB0byDiiaQgMTAwXzAwMCAoZSDiiaQgMTApIGJ5IHZhbGlkYXRpb24uAAAAE2JvcnJvd19leHBvbmVudF9icHMAAAAABAAAAF/CpzguMSBoYWxmLWxpZmUgb2YgdGhlIGZ1bmRpbmcgc2tldyBFTUEsIHNlY29uZHMgKGdsb2JhbDogb25lIG1lbW9yeQpob3Jpem9uIGZvciBldmVyeSBtYXJrZXQpLgAAAAAZZnVuZGluZ19oYWxmX2xpZmVfc2Vjb25kcwAAAAAAAAYAAAAAAAAAGWhhcmRfY2FwX2ZhY3Rvcl9saW1pdF9icHMAAAAAAAAEAAAAAAAAABRscF9yZXZlbnVlX3NoYXJlX2JwcwAAAAQAAAAAAAAAEm1heF9hY3RpdmVfbWFya2V0cwAAAAAABAAAAAAAAAAObWF4X2FkbF9yZXdhcmQAAAAAAAsAAAAAAAAAGm1heF9pbnNvbHZlbnRfdG91Y2hfcmV3YXJkAAAAAAALAAAAAAAAABttYXhfdmFyaWFibGVfYm9ycm93X2Jwc19kYXkAAAAACwAAANDCpzExLjIgbWluaW11bSBib3Jyb3cgY2hhcmdlIHBlciBjYXBpdGFsaXphdGlvbjogYW4gaW5kZXggZGVsdGEgYXQKYElOREVYX1BSRUNJU0lPTmAgc2NhbGUgYXBwbGllZCB0byB0aGUgcG9zaXRpb24ncyByaXNrIHVuaXRzCigyZTExID0gMiBicHMgb2Ygbm90aW9uYWwgYXQgYSAxMCUgbWFya2V0IHJpc2sgZmFjdG9yKS4gWmVybwpkaXNhYmxlcyB0aGUgZmxvb3IuAAAAFm1pbl9ib3Jyb3dfaW5kZXhfZGVsdGEAAAAAAAsAAAAAAAAADm1pbl9jb2xsYXRlcmFsAAAAAAALAAAAAAAAABVtaW5fcG9zaXRpb25fbGlmZXRpbWUAAAAAAAAGAAABEsKnMTEuMSBzaGFyZSBvZiBhIGNsb3NpbmcgZmVlIHJvdXRlZCB0byB0aGUgdHJhZGVyJ3MgcmVmZXJyZXIsIGNhcnZlZApmcm9tIHRoZSBwcm90b2NvbCBzbGljZSAoa2VlcGVyIGFuZCBMUCBzaGFyZXMgYXJlIHVudG91Y2hlZCkuIGAwYApkaXNhYmxlcyByZWZlcnJhbCBhY2NydWFsIGdsb2JhbGx5IOKAlCBhIGtpbGwgc3dpdGNoLiBWYWxpZGF0ZWQgc28KYGxwICsga2VlcGVyICsgcmVmZXJyYWwg4omkIEJQU2AsIGtlZXBpbmcgdGhlIHByb3RvY29sIHJlbWFpbmRlciDiiaUgMC4AAAAAABZyZWZlcnJhbF9mZWVfc2hhcmVfYnBzAAAAAAAEAAAAAAAAABdyaXNrX2NhcGFjaXR5X2xpbWl0X2JwcwAAAAAEAAAAAAAAAB1yaXNrX2tlZXBlcl9yZXZlbnVlX3NoYXJlX2JwcwAAAAAAAAQ=",
        "AAAAAQAAAAAAAAAAAAAADE1hcmtldENvbmZpZwAAABEAAAAAAAAAEmFkbF9wbmxfZmFjdG9yX2JwcwAAAAAABAAAAAAAAAAOYWRsX3Jld2FyZF9icHMAAAAAAAQAAAA0wqcxMS4xIGNsb3NpbmctZmVlIHRpZXIgd2hlbiB0aGUgY2xvc2Ugd29yc2VucyBza2V3LgAAABJjbG9zZV9mZWVfaGlnaF9icHMAAAAAAAQAAABCwqcxMS4xIGNsb3NpbmctZmVlIHRpZXIgd2hlbiB0aGUgY2xvc2UgaW1wcm92ZXMgb3IgcHJlc2VydmVzIHNrZXcuAAAAAAARY2xvc2VfZmVlX2xvd19icHMAAAAAAAAEAAAAAAAAABdoYXJkX2NhcF9wbmxfZmFjdG9yX2JwcwAAAAAEAAAAgk1hcmdpbiByZXF1aXJlZCB0byBvcGVuIG9yIGFkZCByaXNrICjCpzEyLjMpLiBEaXZpZGVzIG1heCBsZXZlcmFnZToKYSBwb3NpdGlvbiBtYXkgbm90IGJlIGNyZWF0ZWQgY2xvc2VyIHRvIGxpcXVpZGF0aW9uIHRoYW4gdGhpcy4AAAAAABJpbml0aWFsX21hcmdpbl9icHMAAAAAAAQAAACHwqc4LjEgd2VpZ2h0IG9mIHRoZSBpbnN0YW50YW5lb3VzIHNrZXcgaW4gdGhlIGZ1bmRpbmcgYmxlbmQsIGluIGJwczsKdGhlIHJlc3QgaXMgdGhlIGhhbGYtbGlmZSBFTUEuIGBCUFNgIHJlcHJvZHVjZXMgcHVyZSBpbnN0YW50IHNrZXcuAAAAABJpbnN0YW50X3dlaWdodF9icHMAAAAAAAQAAAAAAAAAFmxpcXVpZGF0aW9uX3Jld2FyZF9icHMAAAAAAAQAAAB8TWFyZ2luIGJlbG93IHdoaWNoIHRoZSBwb3NpdGlvbiBpcyBsaXF1aWRhdGFibGUgKMKnMTIuMykuIE11c3Qgbm90CmV4Y2VlZCBgaW5pdGlhbF9tYXJnaW5fYnBzYDsgdGhlIGdhcCBpcyB0aGUgZW50cnkgYnVmZmVyLgAAABZtYWludGVuYW5jZV9tYXJnaW5fYnBzAAAAAAAEAAAAAAAAABZtYXJrZXRfcmlza19mYWN0b3JfYnBzAAAAAAAEAAAAAAAAABhtYXhfZnVuZGluZ19yYXRlX2Jwc19kYXkAAAALAAAAAAAAABZtYXhfbG9uZ19iYXNlX2V4cG9zdXJlAAAAAAALAAAAAAAAABttYXhfbG9uZ19zaXplX29wZW5faW50ZXJlc3QAAAAACwAAAAAAAAAXbWF4X3Nob3J0X2Jhc2VfZXhwb3N1cmUAAAAACwAAAAAAAAAcbWF4X3Nob3J0X3NpemVfb3Blbl9pbnRlcmVzdAAAAAsAAAAAAAAAF3JlY292ZXJ5X3BubF9mYWN0b3JfYnBzAAAAAAQAAAAAAAAAFndhcm5pbmdfcG5sX2ZhY3Rvcl9icHMAAAAAAAQ=",
        "AAAAAQAAAC5HbG9iYWwgc2FmZXR5IHRocmVzaG9sZHMgZm9yIHByaWNlIHZhbGlkYXRpb24uAAAAAAAAAAAADE9yYWNsZUNvbmZpZwAAAAQAAADzSG93IGxvbmcgYSBjYWNoZWQgYWdncmVnYXRlZCBwcmljZSByZW1haW5zIHZhbGlkIGFmdGVyIHRoZSByb3V0ZXIKZmV0Y2ggKGluIHNlY29uZHMpLiBBIGNhY2hlIGhpdCBhbHNvIHJlcXVpcmVzIGV2ZXJ5IHNvdXJjZSB0aW1lc3RhbXAKdXNlZCBmb3IgdGhlIGNhY2hlZCBtZWRpYW4gdG8gcmVtYWluIHdpdGhpbiBgc3RhbGVuZXNzX3RocmVzaG9sZGAuCk11c3QgYmUgPiAwIGFuZCA8PSBgc3RhbGVuZXNzX3RocmVzaG9sZGAuAAAAAA5jYWNoZV9kdXJhdGlvbgAAAAAABgAAAIlNYXhpbXVtIGFsbG93ZWQgc3ByZWFkIGJldHdlZW4gb3JhY2xlIHNvdXJjZXMgaW4gYmFzaXMgcG9pbnRzCihlLmcuLCAxMDAgPSAxJSkuIEJvdW5kZWQgYXQgYGNyYXRlOjpjb25zdGFudHM6Ok1BWF9ERVZJQVRJT05fQlBTX0NFSUxJTkdgLgAAAAAAABFtYXhfZGV2aWF0aW9uX2JwcwAAAAAAAAsAAADhTWluaW11bSBudW1iZXIgb2Ygc291cmNlIHJlc3BvbnNlcyB0aGF0IG11c3QgYWdyZWUgd2l0aGluCmBtYXhfZGV2aWF0aW9uX2Jwc2AgZm9yIE9yYWNsZVJvdXRlciB0byByZXR1cm4gYSBwcmljZS4gRmxvb3JlZCBhdApgY3JhdGU6OmNvbnN0YW50czo6TUlOX1JFUVVJUkVEX1NPVVJDRVNfRkxPT1JgLCBjZWlsaW5nZWQgYXQKYGNyYXRlOjpjb25zdGFudHM6Ok1BWF9PUkFDTEVfU09VUkNFU2AuAAAAAAAAFG1pbl9yZXF1aXJlZF9zb3VyY2VzAAAABAAAAFlNYXhpbXVtIGFnZSBvZiBhbiBleHRlcm5hbCBTRVAtNDAgcHJpY2UgZmVlZCBiZWZvcmUgaXQgaXMgcmVqZWN0ZWQKYXMgc3RhbGUgKGluIHNlY29uZHMpLgAAAAAAABNzdGFsZW5lc3NfdGhyZXNob2xkAAAAAAY=",
        "AAAAAgAAAAAAAAAAAAAADUxwUmVxdWVzdEtpbmQAAAAAAAACAAAAAAAAAAAAAAAHRGVwb3NpdAAAAAAAAAAAAAAAAApXaXRoZHJhd2FsAAA=",
        "AAAAAQAAAEtEYXRhIHJlcXVpcmVkIGR1cmluZyBhIFdBU00gbWlncmF0aW9uLiBTaW5nbGUgZGVmaW5pdGlvbiBmb3IgYWxsIGNvbnRyYWN0cy4AAAAAAAAAAA1NaWdyYXRpb25EYXRhAAAAAAAAAQAAAAAAAAAHdmVyc2lvbgAAAAAE",
        "AAAAAQAAAbBQZW5kaW5nIFdBU00gdXBncmFkZSDigJQgc2V0IGJ5IGBwcm9wb3NlX3VwZ3JhZGVgLCBjb25zdW1lZCBieSBgdXBncmFkZWAKKGNsZWFyZWQgYXRvbWljYWxseSBvbiBhIHN1Y2Nlc3NmdWwgaW5zdGFsbCksIG9yIGNsZWFyZWQgYnkgYGNhbmNlbF91cGdyYWRlYC4KU2luZ2xlIHNoYXBlIGFjcm9zcyBldmVyeSBwcm90b2NvbCBjb250cmFjdC4gQ29udHJhY3RzIHN0b3JlIGl0IGF0CnRoZSBzaGFyZWQgYHBlbmRpbmdfdXBncmFkZWAgU3ltYm9sIGtleSBpbiB0aGVpciBvd24gaW5zdGFuY2Ugc3RvcmFnZSAoc2VlCmBjcmF0ZTo6dXBncmFkZTo6cGVuZGluZ191cGdyYWRlX2tleWApLiBgdXBncmFkZWAgcmVmdXNlcyB0byBpbnN0YWxsCnVubGVzcyBgcGVuZGluZy53YXNtX2hhc2hgIG1hdGNoZXMgdGhlIHN1cHBsaWVkIGhhc2ggYW5kIGBub3cgPj0gZXRhYC4AAAAAAAAADlBlbmRpbmdVcGdyYWRlAAAAAAACAAAAAAAAAANldGEAAAAABgAAAAAAAAAJd2FzbV9oYXNoAAAAAAAD7gAAACA=",
        "AAAAAgAAAAAAAAAAAAAAD0xwUmVxdWVzdFN0YXR1cwAAAAAEAAAAAAAAAAAAAAAHUGVuZGluZwAAAAAAAAAAAAAAAAdTZXR0bGVkAAAAAAAAAAAAAAAABkZhaWxlZAAAAAAAAAAAAAAAAAAHRXhwaXJlZAA=",
        "AAAAAQAAAN1UaGUgY2FsbGVyLXN1cHBsaWVkIGZpZWxkcyBvZiBhIGBwbGFjZV9lbnRyeV9vcmRlcmAgcmVxdWVzdCwgYnVuZGxlZCBzbwp0aGUgZW50cnkgcG9pbnQgc3RheXMgd2l0aGluIFNvcm9iYW4ncyBwYXJhbWV0ZXIgbGltaXQuIGBvd25lcmAgYW5kCmBtYXJrZXRgIGFyZSBwYXNzZWQgYWxvbmdzaWRlOyBgaWRgIGFuZCBgdHJpZ2dlcl9hYm92ZWAgYXJlIGRlcml2ZWQgYXQKcGxhY2VtZW50LgAAAAAAAAAAAAAQRW50cnlPcmRlclBhcmFtcwAAAAkAAAAAAAAAEGFjY2VwdGFibGVfcHJpY2UAAAALAAAAAAAAAApjb2xsYXRlcmFsAAAAAAALAAAAAAAAABBleGVjdXRpb25fYnVkZ2V0AAAACwAAAAAAAAAKZXhwaXJlc19hdAAAAAAABgAAAAAAAAAHaXNfbG9uZwAAAAABAAAAAAAAAARzaXplAAAACwAAAAAAAAAJc3RvcF9sb3NzAAAAAAAACwAAAAAAAAALdGFrZV9wcm9maXQAAAAACwAAAAAAAAANdHJpZ2dlcl9wcmljZQAAAAAAAAs=",
        "AAAAAQAAAAAAAAAAAAAAEFNldHRsZW1lbnRSZXN1bHQAAAACAAAAPFNoYXJlcyBtaW50ZWQgZm9yIGEgZGVwb3NpdCBvciBhc3NldHMgcGFpZCBmb3IgYSB3aXRoZHJhd2FsLgAAAAZhbW91bnQAAAAAAAsAAAAAAAAABnN0YXR1cwAAAAAH0AAAABBTZXR0bGVtZW50U3RhdHVz",
        "AAAAAgAAAAAAAAAAAAAAEFNldHRsZW1lbnRTdGF0dXMAAAACAAAAAAAAAAAAAAAHU2V0dGxlZAAAAAAAAAAAAAAAAAZGYWlsZWQAAA==",
        "AAAAAQAAAAAAAAAAAAAAEkFjY291bnRpbmdTbmFwc2hvdAAAAAAACgAAAAAAAAAOY2FzaF9scF9lcXVpdHkAAAAAAAsAAAAAAAAADmNhc2hfc2hvcnRmYWxsAAAAAAALAAAAAAAAAA9mcmVlX2xwX2NhcGl0YWwAAAAACwAAAAAAAAAVbHBfYmxvY2tlZF9zaWRlX2NvdW50AAAAAAAABAAAAAAAAAANbm9uX2xwX2NsYWltcwAAAAAAAAsAAAAAAAAAE29wZW5fcG9zaXRpb25fY291bnQAAAAABgAAAAAAAAANcGh5c2ljYWxfY2FzaAAAAAAAAAsAAAAAAAAAFXJlcXVpcmVkX3Jpc2tfYmFja2luZwAAAAAAAAsAAAAAAAAAEHRvdGFsX3Jpc2tfdW5pdHMAAAALAAAAAAAAAAl2YXVsdF9uYXYAAAAAAAAL",
        "AAAABQAAALVFbWl0dGVkIGJ5IGBwcm9wb3NlX3VwZ3JhZGVgLiBPZmYtY2hhaW4gbW9uaXRvcmluZyByZWNvcmRzIHRoZSBwcm9wb3NlZApgd2FzbV9oYXNoYCArIGBldGFgIGFuZCBmbGFncyBhbnkgc3Vic2VxdWVudCBgdXBncmFkZSgpYCBjYWxsIHdob3NlIGhhc2gKZGl2ZXJnZXMgb3IgdGhhdCBmaXJlcyBiZWZvcmUgYGV0YWAuAAAAAAAAAAAAAA9VcGdyYWRlUHJvcG9zZWQAAAAAAQAAAAZ1cGdwcnAAAAAAAAIAAAAAAAAACXdhc21faGFzaAAAAAAAA+4AAAAgAAAAAAAAAAAAAAADZXRhAAAAAAYAAAAAAAAAAQ==",
        "AAAABQAAAC9FbWl0dGVkIGJ5IGBjYW5jZWxfdXBncmFkZWAgKFBBVVNFUiB2ZXRvIHBhdGgpLgAAAAAAAAAAEFVwZ3JhZGVDYW5jZWxsZWQAAAABAAAABnVwZ2NhbgAAAAAAAQAAAAAAAAAGY2FsbGVyAAAAAAATAAAAAAAAAAE=",
        "AAAABAAAAAAAAAAAAAAAEFVwZ3JhZGVhYmxlRXJyb3IAAAABAAAAQVdoZW4gbWlncmF0aW9uIGlzIGF0dGVtcHRlZCBidXQgbm90IGFsbG93ZWQgZHVlIHRvIHVwZ3JhZGUgc3RhdGUuAAAAAAAAE01pZ3JhdGlvbk5vdEFsbG93ZWQAAAAETA==",
        "AAAABQAAACpFdmVudCBlbWl0dGVkIHdoZW4gdGhlIG1lcmtsZSByb290IGlzIHNldC4AAAAAAAAAAAAHU2V0Um9vdAAAAAABAAAACHNldF9yb290AAAAAQAAAAAAAAAEcm9vdAAAAA4AAAAAAAAAAg==",
        "AAAABQAAACdFdmVudCBlbWl0dGVkIHdoZW4gYW4gaW5kZXggaXMgY2xhaW1lZC4AAAAAAAAAAApTZXRDbGFpbWVkAAAAAAABAAAAC3NldF9jbGFpbWVkAAAAAAEAAAAAAAAABWluZGV4AAAAAAAAAAAAAAAAAAAC",
        "AAAABAAAAAAAAAAAAAAAFk1lcmtsZURpc3RyaWJ1dG9yRXJyb3IAAAAAAAMAAAAbVGhlIG1lcmtsZSByb290IGlzIG5vdCBzZXQuAAAAAApSb290Tm90U2V0AAAAAAUUAAAAJ1RoZSBwcm92aWRlZCBpbmRleCB3YXMgYWxyZWFkeSBjbGFpbWVkLgAAAAATSW5kZXhBbHJlYWR5Q2xhaW1lZAAAAAUVAAAAFVRoZSBwcm9vZiBpcyBpbnZhbGlkLgAAAAAAAAxJbnZhbGlkUHJvb2YAAAUW",
        "AAAAAgAAAD1TdG9yYWdlIGtleXMgZm9yIHRoZSBkYXRhIGFzc29jaWF0ZWQgd2l0aCBgTWVya2xlRGlzdHJpYnV0b3JgAAAAAAAAAAAAABtNZXJrbGVEaXN0cmlidXRvclN0b3JhZ2VLZXkAAAAAAgAAAAAAAAAoVGhlIE1lcmtsZSByb290IG9mIHRoZSBkaXN0cmlidXRpb24gdHJlZQAAAARSb290AAAAAQAAACNNYXBzIGFuIGluZGV4IHRvIGl0cyBjbGFpbWVkIHN0YXR1cwAAAAAHQ2xhaW1lZAAAAAABAAAABA==",
        "AAAAAgAAACpSb3VuZGluZyBkaXJlY3Rpb24gZm9yIGRpdmlzaW9uIG9wZXJhdGlvbnMAAAAAAAAAAAAIUm91bmRpbmcAAAADAAAAAAAAACVSb3VuZCB0b3dhcmQgbmVnYXRpdmUgaW5maW5pdHkgKGRvd24pAAAAAAAABUZsb29yAAAAAAAAAAAAACNSb3VuZCB0b3dhcmQgcG9zaXRpdmUgaW5maW5pdHkgKHVwKQAAAAAEQ2VpbAAAAAAAAAAeUm91bmQgdG93YXJkIHplcm8gKHRydW5jYXRpb24pAAAAAAAIVHJ1bmNhdGU=",
        "AAAABAAAAAAAAAAAAAAAFlNvcm9iYW5GaXhlZFBvaW50RXJyb3IAAAAAAAIAAAAcQXJpdGhtZXRpYyBvdmVyZmxvdyBvY2N1cnJlZAAAAAhPdmVyZmxvdwAABdwAAAAQRGl2aXNpb24gYnkgemVybwAAAA5EaXZpc2lvbkJ5WmVybwAAAAAF3Q==",
        "AAAABAAAAAAAAAAAAAAAC0NyeXB0b0Vycm9yAAAAAAMAAAApVGhlIG1lcmtsZSBwcm9vZiBsZW5ndGggaXMgb3V0IG9mIGJvdW5kcy4AAAAAAAAWTWVya2xlUHJvb2ZPdXRPZkJvdW5kcwAAAAAFeAAAACdUaGUgaW5kZXggb2YgdGhlIGxlYWYgaXMgb3V0IG9mIGJvdW5kcy4AAAAAFk1lcmtsZUluZGV4T3V0T2ZCb3VuZHMAAAAABXkAAAAYTm8gZGF0YSBpbiBoYXNoZXIgc3RhdGUuAAAAEEhhc2hlckVtcHR5U3RhdGUAAAV6",
        "AAAABQAAACpFdmVudCBlbWl0dGVkIHdoZW4gdGhlIGNvbnRyYWN0IGlzIHBhdXNlZC4AAAAAAAAAAAAGUGF1c2VkAAAAAAABAAAABnBhdXNlZAAAAAAAAAAAAAI=",
        "AAAABQAAACxFdmVudCBlbWl0dGVkIHdoZW4gdGhlIGNvbnRyYWN0IGlzIHVucGF1c2VkLgAAAAAAAAAIVW5wYXVzZWQAAAABAAAACHVucGF1c2VkAAAAAAAAAAI=",
        "AAAABAAAAAAAAAAAAAAADVBhdXNhYmxlRXJyb3IAAAAAAAACAAAANFRoZSBvcGVyYXRpb24gZmFpbGVkIGJlY2F1c2UgdGhlIGNvbnRyYWN0IGlzIHBhdXNlZC4AAAANRW5mb3JjZWRQYXVzZQAAAAAAA+gAAAA4VGhlIG9wZXJhdGlvbiBmYWlsZWQgYmVjYXVzZSB0aGUgY29udHJhY3QgaXMgbm90IHBhdXNlZC4AAAANRXhwZWN0ZWRQYXVzZQAAAAAAA+k=",
        "AAAAAgAAACJTdG9yYWdlIGtleSBmb3IgdGhlIHBhdXNhYmxlIHN0YXRlAAAAAAAAAAAAElBhdXNhYmxlU3RvcmFnZUtleQAAAAAAAQAAAAAAAAAySW5kaWNhdGVzIHdoZXRoZXIgdGhlIGNvbnRyYWN0IGlzIGluIHBhdXNlZCBzdGF0ZS4AAAAAAAZQYXVzZWQAAA==" ]),
      options
    )
  }
  public readonly fromJSON = {
    pause: this.txFromJSON<null>,
        migrate: this.txFromJSON<null>,
        unpause: this.txFromJSON<null>,
        upgrade: this.txFromJSON<null>,
        set_tp_sl: this.txFromJSON<null>,
        set_vault: this.txFromJSON<null>,
        get_market: this.txFromJSON<Market>,
        get_position: this.txFromJSON<Position>,
        get_referrer: this.txFromJSON<Option<string>>,
        recapitalize: this.txFromJSON<null>,
        set_referrer: this.txFromJSON<null>,
        bump_position: this.txFromJSON<null>,
        enable_market: this.txFromJSON<null>,
        execute_order: this.txFromJSON<null>,
        global_config: this.txFromJSON<GlobalConfig>,
        non_lp_claims: this.txFromJSON<i128>,
        open_position: this.txFromJSON<u64>,
        active_markets: this.txFromJSON<Array<string>>,
        cancel_upgrade: this.txFromJSON<null>,
        claim_protocol: this.txFromJSON<null>,
        claim_referral: this.txFromJSON<null>,
        disable_market: this.txFromJSON<null>,
        update_indices: this.txFromJSON<null>,
        get_entry_order: this.txFromJSON<EntryOrder>,
        propose_upgrade: this.txFromJSON<null>,
        referral_balance: this.txFromJSON<i128>,
        decrease_position: this.txFromJSON<null>,
        increase_position: this.txFromJSON<null>,
        place_entry_order: this.txFromJSON<u64>,
        set_global_config: this.txFromJSON<null>,
        set_market_config: this.txFromJSON<null>,
        cancel_entry_order: this.txFromJSON<null>,
        is_market_disabled: this.txFromJSON<boolean>,
        liquidate_position: this.txFromJSON<null>,
        accounting_snapshot: this.txFromJSON<AccountingSnapshot>,
        deleverage_position: this.txFromJSON<null>,
        execute_entry_order: this.txFromJSON<null>,
        prepare_lp_snapshot: this.txFromJSON<AccountingSnapshot>,
        referral_code_owner: this.txFromJSON<Option<string>>,
        refresh_borrow_rate: this.txFromJSON<null>,
        can_create_lp_request: this.txFromJSON<boolean>,
        fund_execution_budget: this.txFromJSON<null>,
        register_referral_code: this.txFromJSON<null>,
        protocol_claimable_total: this.txFromJSON<i128>,
        referral_claimable_total: this.txFromJSON<i128>,
        risk_keeper_reserve_total: this.txFromJSON<i128>,
        withdraw_execution_budget: this.txFromJSON<null>,
        pending_receiver_funding_total: this.txFromJSON<i128>
  }
}