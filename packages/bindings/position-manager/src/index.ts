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
  2: {message:"InvalidCaller"},
  10: {message:"PositionNotFound"},
  11: {message:"MarketNotConfigured"},
  12: {message:"ActionNotFound"},
  14: {message:"TriggerNotAttached"},
  16: {message:"UpgradeNoPending"},
  20: {message:"NotInitialized"},
  21: {message:"AlreadyInitialized"},
  22: {message:"Paused"},
  23: {message:"MarketDisabled"},
  24: {message:"RiskStateBlocked"},
  25: {message:"PositionHealthy"},
  26: {message:"TooEarly"},
  27: {message:"MutationPending"},
  28: {message:"MarketNotEmpty"},
  29: {message:"StateVersionMismatch"},
  31: {message:"UpgradeTimelockNotElapsed"},
  32: {message:"InsufficientCollateral"},
  33: {message:"MarketLimitExceeded"},
  40: {message:"InvalidAmount"},
  41: {message:"InvalidConfig"},
  42: {message:"InvalidOrder"},
  46: {message:"WrongActionKind"},
  47: {message:"UpgradeHashMismatch"},
  60: {message:"PriceUnavailable"},
  61: {message:"StalePrice"},
  70: {message:"InvariantViolation"},
  80: {message:"ArithmeticError"}
}


export interface Ledger {
  action_escrow_total: i128;
  borrow_index: i128;
  borrow_index_remainder: i128;
  current_borrow_rate: i128;
  last_global_checkpoint: u64;
  open_position_count: u64;
  pending_receiver_funding_total: i128;
  position_collateral_total: i128;
  protocol_claimable_total: i128;
  restricted_market_side_count: u32;
  state_version: u32;
  total_risk_units: i128;
  unclaimed_payout_total: i128;
}

export type StorageKey = {tag: "ConfigManager", values: void} | {tag: "PriceFeed", values: void} | {tag: "Vault", values: void} | {tag: "GlobalConfig", values: void} | {tag: "Paused", values: void} | {tag: "NextPositionId", values: void} | {tag: "ActiveMarkets", values: void} | {tag: "Ledger", values: void} | {tag: "Version", values: void} | {tag: "Position", values: readonly [u64]} | {tag: "Market", values: readonly [string]} | {tag: "MarketDisabled", values: readonly [string]} | {tag: "PendingAction", values: readonly [u64]} | {tag: "NextActionId", values: void} | {tag: "UnclaimedPayout", values: readonly [string]} | {tag: "Governor", values: void};

export type FeeSource = {tag: "Opening", values: void} | {tag: "Closing", values: void} | {tag: "Borrow", values: void};

export type CloseReason = {tag: "VoluntaryClose", values: void} | {tag: "TakeProfit", values: void} | {tag: "StopLoss", values: void} | {tag: "Liquidation", values: void} | {tag: "Adl", values: void};




























export interface PriceData {
  price: i128;
  timestamp: u64;
}


export interface StampedPrice {
  observed_at: u64;
  price: i128;
}


export interface Market {
  config: MarketConfig;
  current_payer_rate: i128;
  current_payer_side: PayerSide;
  last_funding_checkpoint: u64;
  long: MarketSide;
  long_payer_remainders: RemainderGroup;
  lp_backed_index_long: i128;
  lp_backed_index_short: i128;
  pending_receiver_funding: i128;
  receiver_backed_index_long: i128;
  receiver_backed_index_short: i128;
  receiver_index_long: i128;
  receiver_index_short: i128;
  short: MarketSide;
  short_payer_remainders: RemainderGroup;
  skew_ema: i128;
}

export type Trigger = {tag: "None", values: void} | {tag: "Attached", values: readonly [TriggerInstruction]};


export interface LpConfig {
  lp_request_delay_seconds: u64;
  max_withdraw_utilization_bps: u32;
  min_deposit_nav_factor_bps: u32;
}


export interface Position {
  base_exposure: i128;
  borrow_index_snapshot: i128;
  id: u64;
  is_long: boolean;
  lp_payer_index_snapshot: i128;
  market: string;
  opened_at: u64;
  owner: string;
  pending_mutation_action_id: Option<u64>;
  receiver_index_snapshot: i128;
  receiver_payer_index_snapshot: i128;
  risk_units: i128;
  size: i128;
  stop_loss: Trigger;
  stored_collateral: i128;
  stored_minimum_borrow_fee: i128;
  take_profit: Trigger;
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

export type PayerSide = {tag: "None", values: void} | {tag: "Long", values: void} | {tag: "Short", values: void};

export type RiskState = {tag: "Normal", values: void} | {tag: "Warning", values: void} | {tag: "Adl", values: void} | {tag: "HardCap", values: void};

export type ActionKind = {tag: "MarketOpen", values: void} | {tag: "LimitOpen", values: void} | {tag: "Decrease", values: void} | {tag: "Close", values: void};


export interface MarketSide {
  base_exposure: i128;
  hard_cap_payout_factor: i128;
  hard_cap_reference_pnl: i128;
  risk_state: RiskState;
  risk_units: i128;
  size_open_interest: i128;
  stored_collateral_total: i128;
}


export interface OpenPayload {
  acceptable_price: i128;
  expires_at: u64;
  is_long: boolean;
  size: i128;
  stop_loss: i128;
  submitted_collateral: i128;
  take_profit: i128;
}


export interface ClosePayload {
  acceptable_price: i128;
  position_id: u64;
}


export interface GlobalConfig {
  base_borrow_rate_bps_day: i128;
  borrow_lp_revenue_share_bps: u32;
  config_timelock_seconds: u64;
  fee_lp_revenue_share_bps: u32;
  funding_half_life_seconds: u64;
  global_hard_cap_limit_bps: u32;
  hard_cap_relatch_band_bps: u32;
  keeper_rewards: KeeperRewards;
  max_active_markets: u32;
  max_market_order_lifetime: u64;
  max_order_lifetime_seconds: u64;
  max_price_age_seconds: u64;
  max_variable_borrow_bps_day: i128;
  min_borrow_fee_seconds: u64;
  min_collateral: i128;
  min_position_lifetime: u64;
  risk_capacity_limit_bps: u32;
}


export interface MarketConfig {
  adl_pnl_factor_bps: u32;
  close_pnl_fee_bps: u32;
  close_size_fee_bps: u32;
  hard_cap_pnl_factor_bps: u32;
  initial_margin_bps: u32;
  instant_weight_bps: u32;
  maintenance_margin_bps: u32;
  market_risk_factor_bps: u32;
  max_funding_rate_bps_day: i128;
  max_long_base_exposure: i128;
  max_long_size_open_interest: i128;
  max_short_base_exposure: i128;
  max_short_size_open_interest: i128;
  open_fee_bps: u32;
  order_execution_delay_seconds: u64;
  recovery_pnl_factor_bps: u32;
  warning_pnl_factor_bps: u32;
}

export type ActionOutcome = {tag: "Executed", values: void} | {tag: "Failed", values: void} | {tag: "Cancelled", values: void} | {tag: "Expired", values: void} | {tag: "Superseded", values: void} | {tag: "NotReady", values: void} | {tag: "Pending", values: void} | {tag: "RequiresLiquidation", values: void};

export type ActionPayload = {tag: "MarketOpen", values: readonly [OpenPayload]} | {tag: "LimitOpen", values: readonly [OpenPayload, TriggerCondition]} | {tag: "Decrease", values: readonly [DecreasePayload]} | {tag: "Close", values: readonly [ClosePayload]};

export type FailureReason = {tag: "PriceBoundExceeded", values: void} | {tag: "CapacityExceeded", values: void} | {tag: "ExposureCapExceeded", values: void} | {tag: "SideRestricted", values: void} | {tag: "InsufficientCollateral", values: void} | {tag: "UnpayableProfit", values: void} | {tag: "PositionGone", values: void} | {tag: "MarketPaused", values: void} | {tag: "SizeTooSmall", values: void};


export interface KeeperRewards {
  adl: i128;
  close: i128;
  decrease: i128;
  expiry: i128;
  limit_order: i128;
  liquidation: i128;
  lp_resolve: i128;
  open: i128;
  sl: i128;
  tp: i128;
}

export type LpRequestKind = {tag: "Deposit", values: void} | {tag: "Withdrawal", values: void};


export interface MigrationData {
  version: u32;
}


export interface PendingAction {
  action_id: u64;
  commit_observed_at: u64;
  created_at: u64;
  escrowed_collateral: i128;
  execute_after: u64;
  kind: ActionKind;
  market_id: string;
  owner: string;
  payload: ActionPayload;
}


export interface PendingUpgrade {
  eta: u64;
  wasm_hash: Buffer;
}


export interface RemainderGroup {
  distribution_remainder: i128;
  lp_payer_remainder: i128;
  receiver_liability_remainder: i128;
  receiver_payer_remainder: i128;
}


export interface DecreasePayload {
  acceptable_price: i128;
  position_id: u64;
  size_removed: i128;
}

export type LpRequestStatus = {tag: "Pending", values: void} | {tag: "Settled", values: void} | {tag: "Failed", values: void};


export interface PendingFeesView {
  borrow: i128;
  funding_paid_to_lps: i128;
  funding_paid_to_receivers: i128;
  funding_received: i128;
}


export interface PendingPriceFeed {
  effective_at: u64;
  price_feed: string;
}


export interface SettlementResult {
  amount: i128;
  reward: i128;
  status: SettlementStatus;
}

export type SettlementStatus = {tag: "Settled", values: void} | {tag: "Failed", values: void} | {tag: "NotReady", values: void};


export interface TriggerCondition {
  trigger_above: boolean;
  trigger_price: i128;
}


export interface AccountingSnapshot {
  cash_lp_equity: i128;
  cash_shortfall: i128;
  deleveraging_side_count: u32;
  free_lp_capital: i128;
  min_equity_clear_of_adl: i128;
  non_lp_claims: i128;
  open_position_count: u64;
  physical_cash: i128;
  required_risk_backing: i128;
  restricted_side_count: u32;
  total_risk_units: i128;
  vault_nav: i128;
}


export interface TriggerInstruction {
  acceptable_price: i128;
  commit_observed_at: u64;
  committed_at: u64;
  execute_after: u64;
  trigger_price: i128;
}


export interface PendingGlobalConfig {
  config: GlobalConfig;
  effective_at: u64;
}


export interface PendingMarketConfig {
  config: MarketConfig;
  effective_at: u64;
}


export interface EventHeader {
  actor: string;
  event_version: u32;
  ledger_timestamp: u64;
  market: string;
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
 * Storage keys for the data associated with `MerkleDistributor`
 */
export type MerkleDistributorStorageKey = {tag: "Root", values: void} | {tag: "Claimed", values: readonly [u32]};

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
   * Construct and simulate a governor transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  governor: (options?: MethodOptions) => Promise<AssembledTransaction<string>>

  /**
   * Construct and simulate a is_paused transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  is_paused: (options?: MethodOptions) => Promise<AssembledTransaction<boolean>>

  /**
   * Construct and simulate a set_vault transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  set_vault: ({caller, vault}: {caller: string, vault: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a get_market transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_market: ({market}: {market: string}, options?: MethodOptions) => Promise<AssembledTransaction<Market>>

  /**
   * Construct and simulate a price_feed transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  price_feed: (options?: MethodOptions) => Promise<AssembledTransaction<string>>

  /**
   * Construct and simulate a execute_adl transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  execute_adl: ({keeper, position_id}: {keeper: string, position_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<ActionOutcome>>

  /**
   * Construct and simulate a claim_payout transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  claim_payout: ({owner}: {owner: string}, options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a create_close transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  create_close: ({position_id, acceptable_price}: {position_id: u64, acceptable_price: i128}, options?: MethodOptions) => Promise<AssembledTransaction<u64>>

  /**
   * Construct and simulate a get_position transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_position: ({position_id}: {position_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Position>>

  /**
   * Construct and simulate a recapitalize transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  recapitalize: ({contributor, amount}: {contributor: string, amount: i128}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a settle_close transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  settle_close: ({keeper, action_id}: {keeper: string, action_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<ActionOutcome>>

  /**
   * Construct and simulate a enable_market transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  enable_market: ({caller, market}: {caller: string, market: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a global_config transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  global_config: (options?: MethodOptions) => Promise<AssembledTransaction<GlobalConfig>>

  /**
   * Construct and simulate a non_lp_claims transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  non_lp_claims: (options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a set_stop_loss transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  set_stop_loss: ({position_id, trigger_price, acceptable_price}: {position_id: u64, trigger_price: i128, acceptable_price: i128}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a active_markets transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  active_markets: (options?: MethodOptions) => Promise<AssembledTransaction<Array<string>>>

  /**
   * Construct and simulate a add_collateral transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  add_collateral: ({position_id, amount}: {position_id: u64, amount: i128}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a cancel_upgrade transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  cancel_upgrade: ({caller}: {caller: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a claim_protocol transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  claim_protocol: ({caller, recipient, amount}: {caller: string, recipient: string, amount: i128}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a disable_market transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  disable_market: ({caller, market}: {caller: string, market: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a update_indices transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  update_indices: ({caller, market_symbol}: {caller: string, market_symbol: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a clear_stop_loss transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  clear_stop_loss: ({position_id}: {position_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a create_decrease transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  create_decrease: ({position_id, size_removed, acceptable_price}: {position_id: u64, size_removed: i128, acceptable_price: i128}, options?: MethodOptions) => Promise<AssembledTransaction<u64>>

  /**
   * Construct and simulate a propose_upgrade transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  propose_upgrade: ({caller, wasm_hash}: {caller: string, wasm_hash: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a set_take_profit transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  set_take_profit: ({position_id, trigger_price, acceptable_price}: {position_id: u64, trigger_price: i128, acceptable_price: i128}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a settle_decrease transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  settle_decrease: ({keeper, action_id}: {keeper: string, action_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<ActionOutcome>>

  /**
   * Construct and simulate a unclaimed_payout transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  unclaimed_payout: ({owner}: {owner: string}, options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a cancel_limit_open transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  cancel_limit_open: ({action_id}: {action_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a clear_take_profit transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  clear_take_profit: ({position_id}: {position_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a create_limit_open transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  create_limit_open: ({owner, market, request, trigger_price}: {owner: string, market: string, request: OpenPayload, trigger_price: i128}, options?: MethodOptions) => Promise<AssembledTransaction<u64>>

  /**
   * Construct and simulate a deregister_market transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  deregister_market: ({caller, actor, market_symbol}: {caller: string, actor: string, market_symbol: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a execute_stop_loss transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  execute_stop_loss: ({keeper, position_id}: {keeper: string, position_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<ActionOutcome>>

  /**
   * Construct and simulate a settle_limit_open transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  settle_limit_open: ({keeper, action_id}: {keeper: string, action_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<ActionOutcome>>

  /**
   * Construct and simulate a create_market_open transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  create_market_open: ({owner, market, request}: {owner: string, market: string, request: OpenPayload}, options?: MethodOptions) => Promise<AssembledTransaction<u64>>

  /**
   * Construct and simulate a get_pending_action transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_pending_action: ({action_id}: {action_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<PendingAction>>

  /**
   * Construct and simulate a install_price_feed transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  install_price_feed: ({caller, actor, price_feed}: {caller: string, actor: string, price_feed: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a is_market_disabled transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  is_market_disabled: ({market}: {market: string}, options?: MethodOptions) => Promise<AssembledTransaction<boolean>>

  /**
   * Construct and simulate a liquidate_position transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  liquidate_position: ({keeper, position_id}: {keeper: string, position_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a settle_market_open transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  settle_market_open: ({keeper, action_id}: {keeper: string, action_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<ActionOutcome>>

  /**
   * Construct and simulate a accounting_snapshot transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  accounting_snapshot: ({physical}: {physical: i128}, options?: MethodOptions) => Promise<AssembledTransaction<AccountingSnapshot>>

  /**
   * Construct and simulate a clean_expired_entry transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  clean_expired_entry: ({keeper, action_id}: {keeper: string, action_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a execute_take_profit transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  execute_take_profit: ({keeper, position_id}: {keeper: string, position_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<ActionOutcome>>

  /**
   * Construct and simulate a prepare_lp_snapshot transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  prepare_lp_snapshot: ({caller, physical}: {caller: string, physical: i128}, options?: MethodOptions) => Promise<AssembledTransaction<AccountingSnapshot>>

  /**
   * Construct and simulate a refresh_borrow_rate transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  refresh_borrow_rate: ({caller, physical}: {caller: string, physical: i128}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a install_global_config transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  install_global_config: ({caller, actor, config}: {caller: string, actor: string, config: GlobalConfig}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a install_market_config transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  install_market_config: ({caller, actor, market_symbol, config}: {caller: string, actor: string, market_symbol: string, config: MarketConfig}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

}
export class Client extends ContractClient {
  static async deploy<T = Client>(
        /** Constructor/Initialization Args for the contract's `__constructor` method */
        {config_manager, governor, price_feed, config}: {config_manager: string, governor: string, price_feed: string, config: GlobalConfig},
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
    return ContractClient.deploy({config_manager, governor, price_feed, config}, options)
  }
  constructor(public readonly options: ContractClientOptions) {
    super(
      new ContractSpec([ "AAAABAAAAAAAAAAAAAAAFFBvc2l0aW9uTWFuYWdlckVycm9yAAAAHQAAAAAAAAAMVW5hdXRob3JpemVkAAAAAQAAAAAAAAANSW52YWxpZENhbGxlcgAAAAAAAAIAAAAAAAAAEFBvc2l0aW9uTm90Rm91bmQAAAAKAAAAAAAAABNNYXJrZXROb3RDb25maWd1cmVkAAAAAAsAAAAAAAAADkFjdGlvbk5vdEZvdW5kAAAAAAAMAAAAAAAAABJUcmlnZ2VyTm90QXR0YWNoZWQAAAAAAA4AAAAAAAAAEFVwZ3JhZGVOb1BlbmRpbmcAAAAQAAAAAAAAAA5Ob3RJbml0aWFsaXplZAAAAAAAFAAAAAAAAAASQWxyZWFkeUluaXRpYWxpemVkAAAAAAAVAAAAAAAAAAZQYXVzZWQAAAAAABYAAAAAAAAADk1hcmtldERpc2FibGVkAAAAAAAXAAAAAAAAABBSaXNrU3RhdGVCbG9ja2VkAAAAGAAAAAAAAAAPUG9zaXRpb25IZWFsdGh5AAAAABkAAAAAAAAACFRvb0Vhcmx5AAAAGgAAAAAAAAAPTXV0YXRpb25QZW5kaW5nAAAAABsAAAAAAAAADk1hcmtldE5vdEVtcHR5AAAAAAAcAAAAAAAAABRTdGF0ZVZlcnNpb25NaXNtYXRjaAAAAB0AAAAAAAAAGVVwZ3JhZGVUaW1lbG9ja05vdEVsYXBzZWQAAAAAAAAfAAAAAAAAABZJbnN1ZmZpY2llbnRDb2xsYXRlcmFsAAAAAAAgAAAAAAAAABNNYXJrZXRMaW1pdEV4Y2VlZGVkAAAAACEAAAAAAAAADUludmFsaWRBbW91bnQAAAAAAAAoAAAAAAAAAA1JbnZhbGlkQ29uZmlnAAAAAAAAKQAAAAAAAAAMSW52YWxpZE9yZGVyAAAAKgAAAAAAAAAPV3JvbmdBY3Rpb25LaW5kAAAAAC4AAAAAAAAAE1VwZ3JhZGVIYXNoTWlzbWF0Y2gAAAAALwAAAAAAAAAQUHJpY2VVbmF2YWlsYWJsZQAAADwAAAAAAAAAClN0YWxlUHJpY2UAAAAAAD0AAAAAAAAAEkludmFyaWFudFZpb2xhdGlvbgAAAAAARgAAAAAAAAAPQXJpdGhtZXRpY0Vycm9yAAAAAFA=",
        "AAAAAQAAAAAAAAAAAAAABkxlZGdlcgAAAAAADQAAAAAAAAATYWN0aW9uX2VzY3Jvd190b3RhbAAAAAALAAAAAAAAAAxib3Jyb3dfaW5kZXgAAAALAAAAAAAAABZib3Jyb3dfaW5kZXhfcmVtYWluZGVyAAAAAAALAAAAAAAAABNjdXJyZW50X2JvcnJvd19yYXRlAAAAAAsAAAAAAAAAFmxhc3RfZ2xvYmFsX2NoZWNrcG9pbnQAAAAAAAYAAAAAAAAAE29wZW5fcG9zaXRpb25fY291bnQAAAAABgAAAAAAAAAecGVuZGluZ19yZWNlaXZlcl9mdW5kaW5nX3RvdGFsAAAAAAALAAAAAAAAABlwb3NpdGlvbl9jb2xsYXRlcmFsX3RvdGFsAAAAAAAACwAAAAAAAAAYcHJvdG9jb2xfY2xhaW1hYmxlX3RvdGFsAAAACwAAAAAAAAAccmVzdHJpY3RlZF9tYXJrZXRfc2lkZV9jb3VudAAAAAQAAAAAAAAADXN0YXRlX3ZlcnNpb24AAAAAAAAEAAAAAAAAABB0b3RhbF9yaXNrX3VuaXRzAAAACwAAAAAAAAAWdW5jbGFpbWVkX3BheW91dF90b3RhbAAAAAAACw==",
        "AAAAAgAAAAAAAAAAAAAAClN0b3JhZ2VLZXkAAAAAABAAAAAAAAAAAAAAAA1Db25maWdNYW5hZ2VyAAAAAAAAAAAAAAAAAAAJUHJpY2VGZWVkAAAAAAAAAAAAAAAAAAAFVmF1bHQAAAAAAAAAAAAAAAAAAAxHbG9iYWxDb25maWcAAAAAAAAAAAAAAAZQYXVzZWQAAAAAAAAAAAAAAAAADk5leHRQb3NpdGlvbklkAAAAAAAAAAAAAAAAAA1BY3RpdmVNYXJrZXRzAAAAAAAAAAAAAAAAAAAGTGVkZ2VyAAAAAAAAAAAAAAAAAAdWZXJzaW9uAAAAAAEAAAAAAAAACFBvc2l0aW9uAAAAAQAAAAYAAAABAAAAAAAAAAZNYXJrZXQAAAAAAAEAAAARAAAAAQAAAAAAAAAOTWFya2V0RGlzYWJsZWQAAAAAAAEAAAARAAAAAQAAAAAAAAANUGVuZGluZ0FjdGlvbgAAAAAAAAEAAAAGAAAAAAAAAAAAAAAMTmV4dEFjdGlvbklkAAAAAQAAAAAAAAAPVW5jbGFpbWVkUGF5b3V0AAAAAAEAAAATAAAAAAAAAAAAAAAIR292ZXJub3I=",
        "AAAAAgAAAAAAAAAAAAAACUZlZVNvdXJjZQAAAAAAAAMAAAAAAAAAAAAAAAdPcGVuaW5nAAAAAAAAAAAAAAAAB0Nsb3NpbmcAAAAAAAAAAAAAAAAGQm9ycm93AAA=",
        "AAAAAgAAAAAAAAAAAAAAC0Nsb3NlUmVhc29uAAAAAAUAAAAAAAAAAAAAAA5Wb2x1bnRhcnlDbG9zZQAAAAAAAAAAAAAAAAAKVGFrZVByb2ZpdAAAAAAAAAAAAAAAAAAIU3RvcExvc3MAAAAAAAAAAAAAAAtMaXF1aWRhdGlvbgAAAAAAAAAAAAAAAANBZGwA",
        "AAAABQAAAAAAAAAAAAAADEFjdGlvbkZhaWxlZAAAAAEAAAAHYWN0ZmFpbAAAAAAIAAAAAAAAAAlhY3Rpb25faWQAAAAAAAAGAAAAAQAAAAAAAAAGaGVhZGVyAAAAAAfQAAAAC0V2ZW50SGVhZGVyAAAAAAAAAAAAAAAABW93bmVyAAAAAAAAEwAAAAAAAAAAAAAABGtpbmQAAAfQAAAACkFjdGlvbktpbmQAAAAAAAAAAAAAAAAABnJlYXNvbgAAAAAH0AAAAA1GYWlsdXJlUmVhc29uAAAAAAAAAAAAAAAAAAAGa2VlcGVyAAAAAAATAAAAAAAAAAAAAAAGcmV3YXJkAAAAAAALAAAAAAAAAAAAAAAGcmVmdW5kAAAAAAALAAAAAAAAAAI=",
        "AAAABQAAAAAAAAAAAAAADFBhdXNlQ2hhbmdlZAAAAAEAAAAFcGF1c2UAAAAAAAACAAAAAAAAAAZoZWFkZXIAAAAAB9AAAAALRXZlbnRIZWFkZXIAAAAAAAAAAAAAAAAGcGF1c2VkAAAAAAABAAAAAAAAAAE=",
        "AAAABQAAAAAAAAAAAAAADVJlY2FwaXRhbGl6ZWQAAAAAAAABAAAABXJlY2FwAAAAAAAAAwAAAAAAAAAGaGVhZGVyAAAAAAfQAAAAC0V2ZW50SGVhZGVyAAAAAAAAAAAAAAAAC2NvbnRyaWJ1dG9yAAAAABMAAAAAAAAAAAAAAAZhbW91bnQAAAAAAAsAAAAAAAAAAQ==",
        "AAAABQAAAAAAAAAAAAAADFJldmVudWVTcGxpdAAAAAEAAAAIcmV2c3BsaXQAAAAGAAAAAAAAAAZoZWFkZXIAAAAAB9AAAAALRXZlbnRIZWFkZXIAAAAAAAAAAAAAAAALcG9zaXRpb25faWQAAAAABgAAAAAAAAAAAAAABnNvdXJjZQAAAAAH0AAAAAlGZWVTb3VyY2UAAAAAAAAAAAAAAAAAAAljb2xsZWN0ZWQAAAAAAAALAAAAAAAAAAAAAAAIbHBfc2hhcmUAAAALAAAAAAAAAAAAAAAOcHJvdG9jb2xfc2hhcmUAAAAAAAsAAAAAAAAAAQ==",
        "AAAABQAAAAAAAAAAAAAAC1RwU2xVcGRhdGVkAAAAAAEAAAAEdHBzbAAAAAUAAAAAAAAAC3Bvc2l0aW9uX2lkAAAAAAYAAAABAAAAAAAAAAZoZWFkZXIAAAAAB9AAAAALRXZlbnRIZWFkZXIAAAAAAAAAAAAAAAAFb3duZXIAAAAAAAATAAAAAAAAAAAAAAALdGFrZV9wcm9maXQAAAAACwAAAAAAAAAAAAAACXN0b3BfbG9zcwAAAAAAAAsAAAAAAAAAAg==",
        "AAAABQAAAAAAAAAAAAAADUFjdGlvbkV4cGlyZWQAAAAAAAABAAAACWFjdGV4cGlyZQAAAAAAAAYAAAAAAAAACWFjdGlvbl9pZAAAAAAAAAYAAAABAAAAAAAAAAZoZWFkZXIAAAAAB9AAAAALRXZlbnRIZWFkZXIAAAAAAAAAAAAAAAAFb3duZXIAAAAAAAATAAAAAAAAAAAAAAAGa2VlcGVyAAAAAAATAAAAAAAAAAAAAAAGcmV3YXJkAAAAAAALAAAAAAAAAAAAAAAGcmVmdW5kAAAAAAALAAAAAAAAAAI=",
        "AAAABQAAAAAAAAAAAAAADUFjdGlvblNldHRsZWQAAAAAAAABAAAACWFjdHNldHRsZQAAAAAAAAgAAAAAAAAACWFjdGlvbl9pZAAAAAAAAAYAAAABAAAAAAAAAAZoZWFkZXIAAAAAB9AAAAALRXZlbnRIZWFkZXIAAAAAAAAAAAAAAAAFb3duZXIAAAAAAAATAAAAAAAAAAAAAAAEa2luZAAAB9AAAAAKQWN0aW9uS2luZAAAAAAAAAAAAAAAAAALcG9zaXRpb25faWQAAAAABgAAAAAAAAAAAAAACmZpbGxfcHJpY2UAAAAAAAsAAAAAAAAAAAAAABBmaWxsX29ic2VydmVkX2F0AAAABgAAAAAAAAAAAAAADWtlZXBlcl9yZXdhcmQAAAAAAAALAAAAAAAAAAI=",
        "AAAABQAAAAAAAAAAAAAADVBheW91dENsYWltZWQAAAAAAAABAAAACHBheWNsYWltAAAAAwAAAAAAAAAGaGVhZGVyAAAAAAfQAAAAC0V2ZW50SGVhZGVyAAAAAAAAAAAAAAAABW93bmVyAAAAAAAAEwAAAAAAAAAAAAAABmFtb3VudAAAAAAACwAAAAAAAAAB",
        "AAAABQAAAAAAAAAAAAAADlBheW91dERlZmVycmVkAAAAAAABAAAACHBheWRlZmVyAAAAAwAAAAAAAAAGaGVhZGVyAAAAAAfQAAAAC0V2ZW50SGVhZGVyAAAAAAAAAAAAAAAABW93bmVyAAAAAAAAEwAAAAAAAAAAAAAABmFtb3VudAAAAAAACwAAAAAAAAAB",
        "AAAABQAAAAAAAAAAAAAADlBvc2l0aW9uQ2xvc2VkAAAAAAABAAAACHBvc2Nsb3NlAAAAFwAAAAAAAAALcG9zaXRpb25faWQAAAAABgAAAAEAAAAAAAAABmhlYWRlcgAAAAAH0AAAAAtFdmVudEhlYWRlcgAAAAAAAAAAAAAAAAVvd25lcgAAAAAAABMAAAAAAAAAAAAAAAZyZWFzb24AAAAAB9AAAAALQ2xvc2VSZWFzb24AAAAAAAAAAAAAAAAEc2l6ZQAAAAsAAAAAAAAAAAAAAAVwcmljZQAAAAAAAAsAAAAAAAAAAAAAAAdyYXdfcG5sAAAAAAsAAAAAAAAAAAAAAAtwYXlhYmxlX3BubAAAAAALAAAAAAAAAAAAAAARY29sbGF0ZXJhbF9wYXlvdXQAAAAAAAALAAAAAAAAAAAAAAAIYmFkX2RlYnQAAAALAAAAAAAAAAAAAAANdW5wYWlkX3Byb2ZpdAAAAAAAAAsAAAAAAAAAAAAAAAtjbG9zaW5nX2ZlZQAAAAALAAAAAAAAAAAAAAANa2VlcGVyX3Jld2FyZAAAAAAAAAsAAAAAAAAAAAAAABdrZWVwZXJfZnJvbV9scF9iYWNrc3RvcAAAAAALAAAAAAAAAAAAAAANa2VlcGVyX3VucGFpZAAAAAAAAAsAAAAAAAAAAAAAABRlZmZlY3RpdmVfY29sbGF0ZXJhbAAAAAsAAAAAAAAAAAAAABVsaXF1aWRhdGlvbl90aHJlc2hvbGQAAAAAAAALAAAAAAAAAAAAAAANcGF5b3V0X2ZhY3RvcgAAAAAAAAsAAAAAAAAAAAAAABVyZWNlaXZlcl9mdW5kaW5nX3BhaWQAAAAAAAALAAAAAAAAAAAAAAAPbHBfZnVuZGluZ19wYWlkAAAAAAsAAAAAAAAAAAAAAAtib3Jyb3dfcGFpZAAAAAALAAAAAAAAAAAAAAAQZnVuZGluZ19yZWNlaXZlZAAAAAsAAAAAAAAAAAAAAA5sb3NzX2NvbGxlY3RlZAAAAAAACwAAAAAAAAAC",
        "AAAABQAAAAAAAAAAAAAADlBvc2l0aW9uT3BlbmVkAAAAAAABAAAAB3Bvc29wZW4AAAAACgAAAAAAAAALcG9zaXRpb25faWQAAAAABgAAAAEAAAAAAAAABmhlYWRlcgAAAAAH0AAAAAtFdmVudEhlYWRlcgAAAAAAAAAAAAAAAAVvd25lcgAAAAAAABMAAAAAAAAAAAAAAAdpc19sb25nAAAAAAEAAAAAAAAAAAAAAARzaXplAAAACwAAAAAAAAAAAAAADWJhc2VfZXhwb3N1cmUAAAAAAAALAAAAAAAAAAAAAAARc3RvcmVkX2NvbGxhdGVyYWwAAAAAAAALAAAAAAAAAAAAAAAFcHJpY2UAAAAAAAALAAAAAAAAAAAAAAALdGFrZV9wcm9maXQAAAAACwAAAAAAAAAAAAAACXN0b3BfbG9zcwAAAAAAAAsAAAAAAAAAAg==",
        "AAAABQAAAAAAAAAAAAAAD0FjdGlvbkNhbmNlbGxlZAAAAAABAAAACWFjdGNhbmNlbAAAAAAAAAQAAAAAAAAACWFjdGlvbl9pZAAAAAAAAAYAAAABAAAAAAAAAAZoZWFkZXIAAAAAB9AAAAALRXZlbnRIZWFkZXIAAAAAAAAAAAAAAAAFb3duZXIAAAAAAAATAAAAAAAAAAAAAAAGcmVmdW5kAAAAAAALAAAAAAAAAAI=",
        "AAAABQAAAAAAAAAAAAAAD0FjdGlvbkNvbW1pdHRlZAAAAAABAAAACWFjdGNvbW1pdAAAAAAAAAkAAAAAAAAACWFjdGlvbl9pZAAAAAAAAAYAAAABAAAAAAAAAAZoZWFkZXIAAAAAB9AAAAALRXZlbnRIZWFkZXIAAAAAAAAAAAAAAAAFb3duZXIAAAAAAAATAAAAAAAAAAAAAAAEa2luZAAAB9AAAAAKQWN0aW9uS2luZAAAAAAAAAAAAAAAAAAKY3JlYXRlZF9hdAAAAAAABgAAAAAAAAAAAAAADWV4ZWN1dGVfYWZ0ZXIAAAAAAAAGAAAAAAAAAAAAAAASY29tbWl0X29ic2VydmVkX2F0AAAAAAAGAAAAAAAAAAAAAAATZXNjcm93ZWRfY29sbGF0ZXJhbAAAAAALAAAAAAAAAAAAAAAKZXhwaXJlc19hdAAAAAAABgAAAAAAAAAC",
        "AAAABQAAAAAAAAAAAAAAD0NvbGxhdGVyYWxBZGRlZAAAAAABAAAAB2NvbGxhZGQAAAAABQAAAAAAAAALcG9zaXRpb25faWQAAAAABgAAAAEAAAAAAAAABmhlYWRlcgAAAAAH0AAAAAtFdmVudEhlYWRlcgAAAAAAAAAAAAAAAAVvd25lcgAAAAAAABMAAAAAAAAAAAAAAAZhbW91bnQAAAAAAAsAAAAAAAAAAAAAABFzdG9yZWRfY29sbGF0ZXJhbAAAAAAAAAsAAAAAAAAAAg==",
        "AAAABQAAAAAAAAAAAAAAD1Byb3RvY29sQ2xhaW1lZAAAAAABAAAACXByb3RjbGFpbQAAAAAAAAMAAAAAAAAABmhlYWRlcgAAAAAH0AAAAAtFdmVudEhlYWRlcgAAAAAAAAAAAAAAAAlyZWNpcGllbnQAAAAAAAATAAAAAAAAAAAAAAAGYW1vdW50AAAAAAALAAAAAAAAAAE=",
        "AAAABQAAAAAAAAAAAAAAEEFjdGlvblN1cGVyc2VkZWQAAAABAAAACGFjdHN1cGVyAAAABQAAAAAAAAAJYWN0aW9uX2lkAAAAAAAABgAAAAEAAAAAAAAABmhlYWRlcgAAAAAH0AAAAAtFdmVudEhlYWRlcgAAAAAAAAAAAAAAAAtwb3NpdGlvbl9pZAAAAAAGAAAAAAAAAAAAAAAFb3duZXIAAAAAAAATAAAAAAAAAAAAAAAGcmVmdW5kAAAAAAALAAAAAAAAAAI=",
        "AAAABQAAAAAAAAAAAAAAEEJvcnJvd0NoZWNrcG9pbnQAAAABAAAACWJvcnJvd2NoawAAAAAAAAUAAAAAAAAABmhlYWRlcgAAAAAH0AAAAAtFdmVudEhlYWRlcgAAAAAAAAAAAAAAAAdlbGFwc2VkAAAAAAYAAAAAAAAAAAAAAAtpbmRleF9kZWx0YQAAAAALAAAAAAAAAAAAAAAMcmF0ZV9hcHBsaWVkAAAACwAAAAAAAAAAAAAAC2luZGV4X2FmdGVyAAAAAAsAAAAAAAAAAQ==",
        "AAAABQAAAAAAAAAAAAAAEE1hcmtldENoZWNrcG9pbnQAAAABAAAABm1rdGNoawAAAAAADgAAAAAAAAAGbWFya2V0AAAAAAARAAAAAQAAAAAAAAAGaGVhZGVyAAAAAAfQAAAAC0V2ZW50SGVhZGVyAAAAAAAAAAAAAAAAGnJlY2VpdmVyX2JhY2tlZF9pbmRleF9sb25nAAAAAAALAAAAAAAAAAAAAAAbcmVjZWl2ZXJfYmFja2VkX2luZGV4X3Nob3J0AAAAAAsAAAAAAAAAAAAAABRscF9iYWNrZWRfaW5kZXhfbG9uZwAAAAsAAAAAAAAAAAAAABVscF9iYWNrZWRfaW5kZXhfc2hvcnQAAAAAAAALAAAAAAAAAAAAAAATcmVjZWl2ZXJfaW5kZXhfbG9uZwAAAAALAAAAAAAAAAAAAAAUcmVjZWl2ZXJfaW5kZXhfc2hvcnQAAAALAAAAAAAAAAAAAAASY3VycmVudF9wYXllcl9zaWRlAAAAAAfQAAAACVBheWVyU2lkZQAAAAAAAAAAAAAAAAAAEmN1cnJlbnRfcGF5ZXJfcmF0ZQAAAAAACwAAAAAAAAAAAAAACHNrZXdfZW1hAAAACwAAAAAAAAAAAAAADGJvcnJvd19pbmRleAAAAAsAAAAAAAAAAAAAABNjdXJyZW50X2JvcnJvd19yYXRlAAAAAAsAAAAAAAAAAAAAAAl0aW1lc3RhbXAAAAAAAAAGAAAAAAAAAAI=",
        "AAAABQAAAAAAAAAAAAAAEUZ1bmRpbmdDaGVja3BvaW50AAAAAAAAAQAAAAdmdW5kY2hrAAAAAAoAAAAAAAAABm1hcmtldAAAAAAAEQAAAAEAAAAAAAAABmhlYWRlcgAAAAAH0AAAAAtFdmVudEhlYWRlcgAAAAAAAAAAAAAAAAdzZWdtZW50AAAAAAQAAAAAAAAAAAAAAApwYXllcl9zaWRlAAAAAAfQAAAACVBheWVyU2lkZQAAAAAAAAAAAAAAAAAAFXJlY2VpdmVyX2JhY2tlZF9kZWx0YQAAAAAAAAsAAAAAAAAAAAAAAA9scF9iYWNrZWRfZGVsdGEAAAAACwAAAAAAAAAAAAAADnJlY2VpdmVyX2RlbHRhAAAAAAALAAAAAAAAAAAAAAAPbGlhYmlsaXR5X2RlbHRhAAAAAAsAAAAAAAAAAAAAAAllbWFfYWZ0ZXIAAAAAAAALAAAAAAAAAAAAAAAHZWxhcHNlZAAAAAAGAAAAAAAAAAI=",
        "AAAABQAAAAAAAAAAAAAAEVBvc2l0aW9uRGVjcmVhc2VkAAAAAAAAAQAAAAZwb3NkZWMAAAAAAA8AAAAAAAAAC3Bvc2l0aW9uX2lkAAAAAAYAAAABAAAAAAAAAAZoZWFkZXIAAAAAB9AAAAALRXZlbnRIZWFkZXIAAAAAAAAAAAAAAAAFb3duZXIAAAAAAAATAAAAAAAAAAAAAAAMc2l6ZV9yZW1vdmVkAAAACwAAAAAAAAAAAAAABXByaWNlAAAAAAAACwAAAAAAAAAAAAAAB3Jhd19wbmwAAAAACwAAAAAAAAAAAAAAC3BheWFibGVfcG5sAAAAAAsAAAAAAAAAAAAAABFzdG9yZWRfY29sbGF0ZXJhbAAAAAAAAAsAAAAAAAAAAAAAAAtjbG9zaW5nX2ZlZQAAAAALAAAAAAAAAAAAAAANa2VlcGVyX3Jld2FyZAAAAAAAAAsAAAAAAAAAAAAAABVyZWNlaXZlcl9mdW5kaW5nX3BhaWQAAAAAAAALAAAAAAAAAAAAAAAPbHBfZnVuZGluZ19wYWlkAAAAAAsAAAAAAAAAAAAAAAtib3Jyb3dfcGFpZAAAAAALAAAAAAAAAAAAAAAQZnVuZGluZ19yZWNlaXZlZAAAAAsAAAAAAAAAAAAAAA5sb3NzX2NvbGxlY3RlZAAAAAAACwAAAAAAAAAC",
        "AAAABQAAAAAAAAAAAAAAEFByaWNlRmVlZENoYW5nZWQAAAABAAAAB2ZlZWRzZXQAAAAAAgAAAAAAAAAGaGVhZGVyAAAAAAfQAAAAC0V2ZW50SGVhZGVyAAAAAAAAAAAAAAAACnByaWNlX2ZlZWQAAAAAABMAAAAAAAAAAQ==",
        "AAAABQAAAAAAAAAAAAAAEFJpc2tTdGF0ZUNoYW5nZWQAAAABAAAACXJpc2tzdGF0ZQAAAAAAAAUAAAAAAAAABmhlYWRlcgAAAAAH0AAAAAtFdmVudEhlYWRlcgAAAAAAAAAAAAAAAAdpc19sb25nAAAAAAEAAAAAAAAAAAAAAA5wcmV2aW91c19zdGF0ZQAAAAAH0AAAAAlSaXNrU3RhdGUAAAAAAAAAAAAAAAAAAApuZXh0X3N0YXRlAAAAAAfQAAAACVJpc2tTdGF0ZQAAAAAAAAAAAAAAAAAADnBubF9mYWN0b3JfYnBzAAAAAAALAAAAAAAAAAE=",
        "AAAABQAAAAAAAAAAAAAAE0dsb2JhbENvbmZpZ1VwZGF0ZWQAAAAAAQAAAAljZmdnbG9iYWwAAAAAAAACAAAAAAAAAAZoZWFkZXIAAAAAB9AAAAALRXZlbnRIZWFkZXIAAAAAAAAAAAAAAAAGY29uZmlnAAAAAAfQAAAADEdsb2JhbENvbmZpZwAAAAAAAAAC",
        "AAAABQAAAAAAAAAAAAAAE01hcmtldENvbmZpZ1VwZGF0ZWQAAAAAAQAAAAljZmdtYXJrZXQAAAAAAAADAAAAAAAAAAZtYXJrZXQAAAAAABEAAAABAAAAAAAAAAZoZWFkZXIAAAAAB9AAAAALRXZlbnRIZWFkZXIAAAAAAAAAAAAAAAAGY29uZmlnAAAAAAfQAAAADE1hcmtldENvbmZpZwAAAAAAAAAC",
        "AAAABQAAAAAAAAAAAAAAE01hcmtldFN0YXR1c0NoYW5nZWQAAAAAAQAAAAlta3RzdGF0dXMAAAAAAAACAAAAAAAAAAZoZWFkZXIAAAAAB9AAAAALRXZlbnRIZWFkZXIAAAAAAAAAAAAAAAAIZGlzYWJsZWQAAAABAAAAAAAAAAE=",
        "AAAABQAAAAAAAAAAAAAAB0JhZERlYnQAAAAAAQAAAAdiYWRkZWJ0AAAAAAMAAAAAAAAABmhlYWRlcgAAAAAH0AAAAAtFdmVudEhlYWRlcgAAAAAAAAAAAAAAAAtwb3NpdGlvbl9pZAAAAAAGAAAAAAAAAAAAAAAGYW1vdW50AAAAAAALAAAAAAAAAAE=",
        "AAAAAAAAAAAAAAAFcGF1c2UAAAAAAAABAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAA",
        "AAAAAAAAAAAAAAAHbWlncmF0ZQAAAAACAAAAAAAAAA5taWdyYXRpb25fZGF0YQAAAAAH0AAAAA1NaWdyYXRpb25EYXRhAAAAAAAAAAAAAAhvcGVyYXRvcgAAABMAAAAA",
        "AAAAAAAAAAAAAAAHdW5wYXVzZQAAAAABAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAA",
        "AAAAAAAAAAAAAAAHdXBncmFkZQAAAAACAAAAAAAAAA1uZXdfd2FzbV9oYXNoAAAAAAAD7gAAACAAAAAAAAAACG9wZXJhdG9yAAAAEwAAAAA=",
        "AAAAAAAAAAAAAAAIZ292ZXJub3IAAAAAAAAAAQAAABM=",
        "AAAAAAAAAAAAAAAJaXNfcGF1c2VkAAAAAAAAAAAAAAEAAAAB",
        "AAAAAAAAAAAAAAAJc2V0X3ZhdWx0AAAAAAAAAgAAAAAAAAAGY2FsbGVyAAAAAAATAAAAAAAAAAV2YXVsdAAAAAAAABMAAAAA",
        "AAAAAAAAAAAAAAAKZ2V0X21hcmtldAAAAAAAAQAAAAAAAAAGbWFya2V0AAAAAAARAAAAAQAAB9AAAAAGTWFya2V0AAA=",
        "AAAAAAAAAAAAAAAKcHJpY2VfZmVlZAAAAAAAAAAAAAEAAAAT",
        "AAAAAAAAAAAAAAALZXhlY3V0ZV9hZGwAAAAAAgAAAAAAAAAGa2VlcGVyAAAAAAATAAAAAAAAAAtwb3NpdGlvbl9pZAAAAAAGAAAAAQAAB9AAAAANQWN0aW9uT3V0Y29tZQAAAA==",
        "AAAAAAAAAAAAAAAMY2xhaW1fcGF5b3V0AAAAAQAAAAAAAAAFb3duZXIAAAAAAAATAAAAAQAAAAs=",
        "AAAAAAAAAAAAAAAMY3JlYXRlX2Nsb3NlAAAAAgAAAAAAAAALcG9zaXRpb25faWQAAAAABgAAAAAAAAAQYWNjZXB0YWJsZV9wcmljZQAAAAsAAAABAAAABg==",
        "AAAAAAAAAAAAAAAMZ2V0X3Bvc2l0aW9uAAAAAQAAAAAAAAALcG9zaXRpb25faWQAAAAABgAAAAEAAAfQAAAACFBvc2l0aW9u",
        "AAAAAAAAAAAAAAAMcmVjYXBpdGFsaXplAAAAAgAAAAAAAAALY29udHJpYnV0b3IAAAAAEwAAAAAAAAAGYW1vdW50AAAAAAALAAAAAA==",
        "AAAAAAAAAAAAAAAMc2V0dGxlX2Nsb3NlAAAAAgAAAAAAAAAGa2VlcGVyAAAAAAATAAAAAAAAAAlhY3Rpb25faWQAAAAAAAAGAAAAAQAAB9AAAAANQWN0aW9uT3V0Y29tZQAAAA==",
        "AAAAAAAAAAAAAAANX19jb25zdHJ1Y3RvcgAAAAAAAAQAAAAAAAAADmNvbmZpZ19tYW5hZ2VyAAAAAAATAAAAAAAAAAhnb3Zlcm5vcgAAABMAAAAAAAAACnByaWNlX2ZlZWQAAAAAABMAAAAAAAAABmNvbmZpZwAAAAAH0AAAAAxHbG9iYWxDb25maWcAAAAA",
        "AAAAAAAAAAAAAAANZW5hYmxlX21hcmtldAAAAAAAAAIAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAAAAAAGbWFya2V0AAAAAAARAAAAAA==",
        "AAAAAAAAAAAAAAANZ2xvYmFsX2NvbmZpZwAAAAAAAAAAAAABAAAH0AAAAAxHbG9iYWxDb25maWc=",
        "AAAAAAAAAAAAAAANbm9uX2xwX2NsYWltcwAAAAAAAAAAAAABAAAACw==",
        "AAAAAAAAAAAAAAANc2V0X3N0b3BfbG9zcwAAAAAAAAMAAAAAAAAAC3Bvc2l0aW9uX2lkAAAAAAYAAAAAAAAADXRyaWdnZXJfcHJpY2UAAAAAAAALAAAAAAAAABBhY2NlcHRhYmxlX3ByaWNlAAAACwAAAAA=",
        "AAAAAAAAAAAAAAAOYWN0aXZlX21hcmtldHMAAAAAAAAAAAABAAAD6gAAABE=",
        "AAAAAAAAAAAAAAAOYWRkX2NvbGxhdGVyYWwAAAAAAAIAAAAAAAAAC3Bvc2l0aW9uX2lkAAAAAAYAAAAAAAAABmFtb3VudAAAAAAACwAAAAA=",
        "AAAAAAAAAAAAAAAOY2FuY2VsX3VwZ3JhZGUAAAAAAAEAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAA=",
        "AAAAAAAAAAAAAAAOY2xhaW1fcHJvdG9jb2wAAAAAAAMAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAAAAAAJcmVjaXBpZW50AAAAAAAAEwAAAAAAAAAGYW1vdW50AAAAAAALAAAAAA==",
        "AAAAAAAAAAAAAAAOZGlzYWJsZV9tYXJrZXQAAAAAAAIAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAAAAAAGbWFya2V0AAAAAAARAAAAAA==",
        "AAAAAAAAAAAAAAAOdXBkYXRlX2luZGljZXMAAAAAAAIAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAAAAAANbWFya2V0X3N5bWJvbAAAAAAAABEAAAAA",
        "AAAAAAAAAAAAAAAPY2xlYXJfc3RvcF9sb3NzAAAAAAEAAAAAAAAAC3Bvc2l0aW9uX2lkAAAAAAYAAAAA",
        "AAAAAAAAAAAAAAAPY3JlYXRlX2RlY3JlYXNlAAAAAAMAAAAAAAAAC3Bvc2l0aW9uX2lkAAAAAAYAAAAAAAAADHNpemVfcmVtb3ZlZAAAAAsAAAAAAAAAEGFjY2VwdGFibGVfcHJpY2UAAAALAAAAAQAAAAY=",
        "AAAAAAAAAAAAAAAPcHJvcG9zZV91cGdyYWRlAAAAAAIAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAAAAAAJd2FzbV9oYXNoAAAAAAAD7gAAACAAAAAA",
        "AAAAAAAAAAAAAAAPc2V0X3Rha2VfcHJvZml0AAAAAAMAAAAAAAAAC3Bvc2l0aW9uX2lkAAAAAAYAAAAAAAAADXRyaWdnZXJfcHJpY2UAAAAAAAALAAAAAAAAABBhY2NlcHRhYmxlX3ByaWNlAAAACwAAAAA=",
        "AAAAAAAAAAAAAAAPc2V0dGxlX2RlY3JlYXNlAAAAAAIAAAAAAAAABmtlZXBlcgAAAAAAEwAAAAAAAAAJYWN0aW9uX2lkAAAAAAAABgAAAAEAAAfQAAAADUFjdGlvbk91dGNvbWUAAAA=",
        "AAAAAAAAAAAAAAAQdW5jbGFpbWVkX3BheW91dAAAAAEAAAAAAAAABW93bmVyAAAAAAAAEwAAAAEAAAAL",
        "AAAAAAAAAAAAAAARY2FuY2VsX2xpbWl0X29wZW4AAAAAAAABAAAAAAAAAAlhY3Rpb25faWQAAAAAAAAGAAAAAQAAAAs=",
        "AAAAAAAAAAAAAAARY2xlYXJfdGFrZV9wcm9maXQAAAAAAAABAAAAAAAAAAtwb3NpdGlvbl9pZAAAAAAGAAAAAA==",
        "AAAAAAAAAAAAAAARY3JlYXRlX2xpbWl0X29wZW4AAAAAAAAEAAAAAAAAAAVvd25lcgAAAAAAABMAAAAAAAAABm1hcmtldAAAAAAAEQAAAAAAAAAHcmVxdWVzdAAAAAfQAAAAC09wZW5QYXlsb2FkAAAAAAAAAAANdHJpZ2dlcl9wcmljZQAAAAAAAAsAAAABAAAABg==",
        "AAAAAAAAAAAAAAARZGVyZWdpc3Rlcl9tYXJrZXQAAAAAAAADAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAAAAAABWFjdG9yAAAAAAAAEwAAAAAAAAANbWFya2V0X3N5bWJvbAAAAAAAABEAAAAA",
        "AAAAAAAAAAAAAAARZXhlY3V0ZV9zdG9wX2xvc3MAAAAAAAACAAAAAAAAAAZrZWVwZXIAAAAAABMAAAAAAAAAC3Bvc2l0aW9uX2lkAAAAAAYAAAABAAAH0AAAAA1BY3Rpb25PdXRjb21lAAAA",
        "AAAAAAAAAAAAAAARc2V0dGxlX2xpbWl0X29wZW4AAAAAAAACAAAAAAAAAAZrZWVwZXIAAAAAABMAAAAAAAAACWFjdGlvbl9pZAAAAAAAAAYAAAABAAAH0AAAAA1BY3Rpb25PdXRjb21lAAAA",
        "AAAAAAAAAAAAAAASY3JlYXRlX21hcmtldF9vcGVuAAAAAAADAAAAAAAAAAVvd25lcgAAAAAAABMAAAAAAAAABm1hcmtldAAAAAAAEQAAAAAAAAAHcmVxdWVzdAAAAAfQAAAAC09wZW5QYXlsb2FkAAAAAAEAAAAG",
        "AAAAAAAAAAAAAAASZ2V0X3BlbmRpbmdfYWN0aW9uAAAAAAABAAAAAAAAAAlhY3Rpb25faWQAAAAAAAAGAAAAAQAAB9AAAAANUGVuZGluZ0FjdGlvbgAAAA==",
        "AAAAAAAAAAAAAAASaW5zdGFsbF9wcmljZV9mZWVkAAAAAAADAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAAAAAABWFjdG9yAAAAAAAAEwAAAAAAAAAKcHJpY2VfZmVlZAAAAAAAEwAAAAA=",
        "AAAAAAAAAAAAAAASaXNfbWFya2V0X2Rpc2FibGVkAAAAAAABAAAAAAAAAAZtYXJrZXQAAAAAABEAAAABAAAAAQ==",
        "AAAAAAAAAAAAAAASbGlxdWlkYXRlX3Bvc2l0aW9uAAAAAAACAAAAAAAAAAZrZWVwZXIAAAAAABMAAAAAAAAAC3Bvc2l0aW9uX2lkAAAAAAYAAAAA",
        "AAAAAAAAAAAAAAASc2V0dGxlX21hcmtldF9vcGVuAAAAAAACAAAAAAAAAAZrZWVwZXIAAAAAABMAAAAAAAAACWFjdGlvbl9pZAAAAAAAAAYAAAABAAAH0AAAAA1BY3Rpb25PdXRjb21lAAAA",
        "AAAAAAAAAAAAAAATYWNjb3VudGluZ19zbmFwc2hvdAAAAAABAAAAAAAAAAhwaHlzaWNhbAAAAAsAAAABAAAH0AAAABJBY2NvdW50aW5nU25hcHNob3QAAA==",
        "AAAAAAAAAAAAAAATY2xlYW5fZXhwaXJlZF9lbnRyeQAAAAACAAAAAAAAAAZrZWVwZXIAAAAAABMAAAAAAAAACWFjdGlvbl9pZAAAAAAAAAYAAAAA",
        "AAAAAAAAAAAAAAATZXhlY3V0ZV90YWtlX3Byb2ZpdAAAAAACAAAAAAAAAAZrZWVwZXIAAAAAABMAAAAAAAAAC3Bvc2l0aW9uX2lkAAAAAAYAAAABAAAH0AAAAA1BY3Rpb25PdXRjb21lAAAA",
        "AAAAAAAAAAAAAAATcHJlcGFyZV9scF9zbmFwc2hvdAAAAAACAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAAAAAACHBoeXNpY2FsAAAACwAAAAEAAAfQAAAAEkFjY291bnRpbmdTbmFwc2hvdAAA",
        "AAAAAAAAAAAAAAATcmVmcmVzaF9ib3Jyb3dfcmF0ZQAAAAACAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAAAAAACHBoeXNpY2FsAAAACwAAAAA=",
        "AAAAAAAAAAAAAAAVaW5zdGFsbF9nbG9iYWxfY29uZmlnAAAAAAAAAwAAAAAAAAAGY2FsbGVyAAAAAAATAAAAAAAAAAVhY3RvcgAAAAAAABMAAAAAAAAABmNvbmZpZwAAAAAH0AAAAAxHbG9iYWxDb25maWcAAAAA",
        "AAAAAAAAAAAAAAAVaW5zdGFsbF9tYXJrZXRfY29uZmlnAAAAAAAABAAAAAAAAAAGY2FsbGVyAAAAAAATAAAAAAAAAAVhY3RvcgAAAAAAABMAAAAAAAAADW1hcmtldF9zeW1ib2wAAAAAAAARAAAAAAAAAAZjb25maWcAAAAAB9AAAAAMTWFya2V0Q29uZmlnAAAAAA==",
        "AAAAAQAAAAAAAAAAAAAACVByaWNlRGF0YQAAAAAAAAIAAAAAAAAABXByaWNlAAAAAAAACwAAAAAAAAAJdGltZXN0YW1wAAAAAAAABg==",
        "AAAAAQAAAAAAAAAAAAAADFN0YW1wZWRQcmljZQAAAAIAAAAAAAAAC29ic2VydmVkX2F0AAAAAAYAAAAAAAAABXByaWNlAAAAAAAACw==",
        "AAAAAQAAAAAAAAAAAAAABk1hcmtldAAAAAAAEAAAAAAAAAAGY29uZmlnAAAAAAfQAAAADE1hcmtldENvbmZpZwAAAAAAAAASY3VycmVudF9wYXllcl9yYXRlAAAAAAALAAAAAAAAABJjdXJyZW50X3BheWVyX3NpZGUAAAAAB9AAAAAJUGF5ZXJTaWRlAAAAAAAAAAAAABdsYXN0X2Z1bmRpbmdfY2hlY2twb2ludAAAAAAGAAAAAAAAAARsb25nAAAH0AAAAApNYXJrZXRTaWRlAAAAAAAAAAAAFWxvbmdfcGF5ZXJfcmVtYWluZGVycwAAAAAAB9AAAAAOUmVtYWluZGVyR3JvdXAAAAAAAAAAAAAUbHBfYmFja2VkX2luZGV4X2xvbmcAAAALAAAAAAAAABVscF9iYWNrZWRfaW5kZXhfc2hvcnQAAAAAAAALAAAAAAAAABhwZW5kaW5nX3JlY2VpdmVyX2Z1bmRpbmcAAAALAAAAAAAAABpyZWNlaXZlcl9iYWNrZWRfaW5kZXhfbG9uZwAAAAAACwAAAAAAAAAbcmVjZWl2ZXJfYmFja2VkX2luZGV4X3Nob3J0AAAAAAsAAAAAAAAAE3JlY2VpdmVyX2luZGV4X2xvbmcAAAAACwAAAAAAAAAUcmVjZWl2ZXJfaW5kZXhfc2hvcnQAAAALAAAAAAAAAAVzaG9ydAAAAAAAB9AAAAAKTWFya2V0U2lkZQAAAAAAAAAAABZzaG9ydF9wYXllcl9yZW1haW5kZXJzAAAAAAfQAAAADlJlbWFpbmRlckdyb3VwAAAAAAAAAAAACHNrZXdfZW1hAAAACw==",
        "AAAAAgAAAAAAAAAAAAAAB1RyaWdnZXIAAAAAAgAAAAAAAAAAAAAABE5vbmUAAAABAAAAAAAAAAhBdHRhY2hlZAAAAAEAAAfQAAAAElRyaWdnZXJJbnN0cnVjdGlvbgAA",
        "AAAAAQAAAAAAAAAAAAAACExwQ29uZmlnAAAAAwAAAAAAAAAYbHBfcmVxdWVzdF9kZWxheV9zZWNvbmRzAAAABgAAAAAAAAAcbWF4X3dpdGhkcmF3X3V0aWxpemF0aW9uX2JwcwAAAAQAAAAAAAAAGm1pbl9kZXBvc2l0X25hdl9mYWN0b3JfYnBzAAAAAAAE",
        "AAAAAQAAAAAAAAAAAAAACFBvc2l0aW9uAAAAEQAAAAAAAAANYmFzZV9leHBvc3VyZQAAAAAAAAsAAAAAAAAAFWJvcnJvd19pbmRleF9zbmFwc2hvdAAAAAAAAAsAAAAAAAAAAmlkAAAAAAAGAAAAAAAAAAdpc19sb25nAAAAAAEAAAAAAAAAF2xwX3BheWVyX2luZGV4X3NuYXBzaG90AAAAAAsAAAAAAAAABm1hcmtldAAAAAAAEQAAAAAAAAAJb3BlbmVkX2F0AAAAAAAABgAAAAAAAAAFb3duZXIAAAAAAAATAAAAAAAAABpwZW5kaW5nX211dGF0aW9uX2FjdGlvbl9pZAAAAAAD6AAAAAYAAAAAAAAAF3JlY2VpdmVyX2luZGV4X3NuYXBzaG90AAAAAAsAAAAAAAAAHXJlY2VpdmVyX3BheWVyX2luZGV4X3NuYXBzaG90AAAAAAAACwAAAAAAAAAKcmlza191bml0cwAAAAAACwAAAAAAAAAEc2l6ZQAAAAsAAAAAAAAACXN0b3BfbG9zcwAAAAAAB9AAAAAHVHJpZ2dlcgAAAAAAAAAAEXN0b3JlZF9jb2xsYXRlcmFsAAAAAAAACwAAAAAAAAAZc3RvcmVkX21pbmltdW1fYm9ycm93X2ZlZQAAAAAAAAsAAAAAAAAAC3Rha2VfcHJvZml0AAAAB9AAAAAHVHJpZ2dlcgA=",
        "AAAAAQAAAAAAAAAAAAAACUxwUmVxdWVzdAAAAAAAAAcAAAAAAAAABmFtb3VudAAAAAAACwAAAAAAAAANZXhlY3V0ZV9hZnRlcgAAAAAAAAYAAAAAAAAAAmlkAAAAAAAGAAAAAAAAAARraW5kAAAH0AAAAA1McFJlcXVlc3RLaW5kAAAAAAAAAAAAAAVvd25lcgAAAAAAABMAAAAAAAAADHJlcXVlc3RfdGltZQAAAAYAAAAAAAAABnN0YXR1cwAAAAAH0AAAAA9McFJlcXVlc3RTdGF0dXMA",
        "AAAAAgAAAAAAAAAAAAAACVBheWVyU2lkZQAAAAAAAAMAAAAAAAAAAAAAAAROb25lAAAAAAAAAAAAAAAETG9uZwAAAAAAAAAAAAAABVNob3J0AAAA",
        "AAAAAgAAAAAAAAAAAAAACVJpc2tTdGF0ZQAAAAAAAAQAAAAAAAAAAAAAAAZOb3JtYWwAAAAAAAAAAAAAAAAAB1dhcm5pbmcAAAAAAAAAAAAAAAADQWRsAAAAAAAAAAAAAAAAB0hhcmRDYXAA",
        "AAAAAgAAAAAAAAAAAAAACkFjdGlvbktpbmQAAAAAAAQAAAAAAAAAAAAAAApNYXJrZXRPcGVuAAAAAAAAAAAAAAAAAAlMaW1pdE9wZW4AAAAAAAAAAAAAAAAAAAhEZWNyZWFzZQAAAAAAAAAAAAAABUNsb3NlAAAA",
        "AAAAAQAAAAAAAAAAAAAACk1hcmtldFNpZGUAAAAAAAcAAAAAAAAADWJhc2VfZXhwb3N1cmUAAAAAAAALAAAAAAAAABZoYXJkX2NhcF9wYXlvdXRfZmFjdG9yAAAAAAALAAAAAAAAABZoYXJkX2NhcF9yZWZlcmVuY2VfcG5sAAAAAAALAAAAAAAAAApyaXNrX3N0YXRlAAAAAAfQAAAACVJpc2tTdGF0ZQAAAAAAAAAAAAAKcmlza191bml0cwAAAAAACwAAAAAAAAASc2l6ZV9vcGVuX2ludGVyZXN0AAAAAAALAAAAAAAAABdzdG9yZWRfY29sbGF0ZXJhbF90b3RhbAAAAAAL",
        "AAAAAQAAAAAAAAAAAAAAC09wZW5QYXlsb2FkAAAAAAcAAAAAAAAAEGFjY2VwdGFibGVfcHJpY2UAAAALAAAAAAAAAApleHBpcmVzX2F0AAAAAAAGAAAAAAAAAAdpc19sb25nAAAAAAEAAAAAAAAABHNpemUAAAALAAAAAAAAAAlzdG9wX2xvc3MAAAAAAAALAAAAAAAAABRzdWJtaXR0ZWRfY29sbGF0ZXJhbAAAAAsAAAAAAAAAC3Rha2VfcHJvZml0AAAAAAs=",
        "AAAAAQAAAAAAAAAAAAAADENsb3NlUGF5bG9hZAAAAAIAAAAAAAAAEGFjY2VwdGFibGVfcHJpY2UAAAALAAAAAAAAAAtwb3NpdGlvbl9pZAAAAAAG",
        "AAAAAQAAAAAAAAAAAAAADEdsb2JhbENvbmZpZwAAABEAAAAAAAAAGGJhc2VfYm9ycm93X3JhdGVfYnBzX2RheQAAAAsAAAAAAAAAG2JvcnJvd19scF9yZXZlbnVlX3NoYXJlX2JwcwAAAAAEAAAAAAAAABdjb25maWdfdGltZWxvY2tfc2Vjb25kcwAAAAAGAAAAAAAAABhmZWVfbHBfcmV2ZW51ZV9zaGFyZV9icHMAAAAEAAAAAAAAABlmdW5kaW5nX2hhbGZfbGlmZV9zZWNvbmRzAAAAAAAABgAAAAAAAAAZZ2xvYmFsX2hhcmRfY2FwX2xpbWl0X2JwcwAAAAAAAAQAAAAAAAAAGWhhcmRfY2FwX3JlbGF0Y2hfYmFuZF9icHMAAAAAAAAEAAAAAAAAAA5rZWVwZXJfcmV3YXJkcwAAAAAH0AAAAA1LZWVwZXJSZXdhcmRzAAAAAAAAAAAAABJtYXhfYWN0aXZlX21hcmtldHMAAAAAAAQAAAAAAAAAGW1heF9tYXJrZXRfb3JkZXJfbGlmZXRpbWUAAAAAAAAGAAAAAAAAABptYXhfb3JkZXJfbGlmZXRpbWVfc2Vjb25kcwAAAAAABgAAAAAAAAAVbWF4X3ByaWNlX2FnZV9zZWNvbmRzAAAAAAAABgAAAAAAAAAbbWF4X3ZhcmlhYmxlX2JvcnJvd19icHNfZGF5AAAAAAsAAAAAAAAAFm1pbl9ib3Jyb3dfZmVlX3NlY29uZHMAAAAAAAYAAAAAAAAADm1pbl9jb2xsYXRlcmFsAAAAAAALAAAAAAAAABVtaW5fcG9zaXRpb25fbGlmZXRpbWUAAAAAAAAGAAAAAAAAABdyaXNrX2NhcGFjaXR5X2xpbWl0X2JwcwAAAAAE",
        "AAAAAQAAAAAAAAAAAAAADE1hcmtldENvbmZpZwAAABEAAAAAAAAAEmFkbF9wbmxfZmFjdG9yX2JwcwAAAAAABAAAAAAAAAARY2xvc2VfcG5sX2ZlZV9icHMAAAAAAAAEAAAAAAAAABJjbG9zZV9zaXplX2ZlZV9icHMAAAAAAAQAAAAAAAAAF2hhcmRfY2FwX3BubF9mYWN0b3JfYnBzAAAAAAQAAAAAAAAAEmluaXRpYWxfbWFyZ2luX2JwcwAAAAAABAAAAAAAAAASaW5zdGFudF93ZWlnaHRfYnBzAAAAAAAEAAAAAAAAABZtYWludGVuYW5jZV9tYXJnaW5fYnBzAAAAAAAEAAAAAAAAABZtYXJrZXRfcmlza19mYWN0b3JfYnBzAAAAAAAEAAAAAAAAABhtYXhfZnVuZGluZ19yYXRlX2Jwc19kYXkAAAALAAAAAAAAABZtYXhfbG9uZ19iYXNlX2V4cG9zdXJlAAAAAAALAAAAAAAAABttYXhfbG9uZ19zaXplX29wZW5faW50ZXJlc3QAAAAACwAAAAAAAAAXbWF4X3Nob3J0X2Jhc2VfZXhwb3N1cmUAAAAACwAAAAAAAAAcbWF4X3Nob3J0X3NpemVfb3Blbl9pbnRlcmVzdAAAAAsAAAAAAAAADG9wZW5fZmVlX2JwcwAAAAQAAAAAAAAAHW9yZGVyX2V4ZWN1dGlvbl9kZWxheV9zZWNvbmRzAAAAAAAABgAAAAAAAAAXcmVjb3ZlcnlfcG5sX2ZhY3Rvcl9icHMAAAAABAAAAAAAAAAWd2FybmluZ19wbmxfZmFjdG9yX2JwcwAAAAAABA==",
        "AAAAAgAAAAAAAAAAAAAADUFjdGlvbk91dGNvbWUAAAAAAAAIAAAAAAAAAAAAAAAIRXhlY3V0ZWQAAAAAAAAAAAAAAAZGYWlsZWQAAAAAAAAAAAAAAAAACUNhbmNlbGxlZAAAAAAAAAAAAAAAAAAAB0V4cGlyZWQAAAAAAAAAAAAAAAAKU3VwZXJzZWRlZAAAAAAAAAAAAAAAAAAITm90UmVhZHkAAAAAAAAAAAAAAAdQZW5kaW5nAAAAAAAAAAAAAAAAE1JlcXVpcmVzTGlxdWlkYXRpb24A",
        "AAAAAgAAAAAAAAAAAAAADUFjdGlvblBheWxvYWQAAAAAAAAEAAAAAQAAAAAAAAAKTWFya2V0T3BlbgAAAAAAAQAAB9AAAAALT3BlblBheWxvYWQAAAAAAQAAAAAAAAAJTGltaXRPcGVuAAAAAAAAAgAAB9AAAAALT3BlblBheWxvYWQAAAAH0AAAABBUcmlnZ2VyQ29uZGl0aW9uAAAAAQAAAAAAAAAIRGVjcmVhc2UAAAABAAAH0AAAAA9EZWNyZWFzZVBheWxvYWQAAAAAAQAAAAAAAAAFQ2xvc2UAAAAAAAABAAAH0AAAAAxDbG9zZVBheWxvYWQ=",
        "AAAAAgAAAAAAAAAAAAAADUZhaWx1cmVSZWFzb24AAAAAAAAJAAAAAAAAAAAAAAASUHJpY2VCb3VuZEV4Y2VlZGVkAAAAAAAAAAAAAAAAABBDYXBhY2l0eUV4Y2VlZGVkAAAAAAAAAAAAAAATRXhwb3N1cmVDYXBFeGNlZWRlZAAAAAAAAAAAAAAAAA5TaWRlUmVzdHJpY3RlZAAAAAAAAAAAAAAAAAAWSW5zdWZmaWNpZW50Q29sbGF0ZXJhbAAAAAAAAAAAAAAAAAAPVW5wYXlhYmxlUHJvZml0AAAAAAAAAAAAAAAADFBvc2l0aW9uR29uZQAAAAAAAAAAAAAADE1hcmtldFBhdXNlZAAAAAAAAAAAAAAADFNpemVUb29TbWFsbA==",
        "AAAAAQAAAAAAAAAAAAAADUtlZXBlclJld2FyZHMAAAAAAAAKAAAAAAAAAANhZGwAAAAACwAAAAAAAAAFY2xvc2UAAAAAAAALAAAAAAAAAAhkZWNyZWFzZQAAAAsAAAAAAAAABmV4cGlyeQAAAAAACwAAAAAAAAALbGltaXRfb3JkZXIAAAAACwAAAAAAAAALbGlxdWlkYXRpb24AAAAACwAAAAAAAAAKbHBfcmVzb2x2ZQAAAAAACwAAAAAAAAAEb3BlbgAAAAsAAAAAAAAAAnNsAAAAAAALAAAAAAAAAAJ0cAAAAAAACw==",
        "AAAAAgAAAAAAAAAAAAAADUxwUmVxdWVzdEtpbmQAAAAAAAACAAAAAAAAAAAAAAAHRGVwb3NpdAAAAAAAAAAAAAAAAApXaXRoZHJhd2FsAAA=",
        "AAAAAQAAAAAAAAAAAAAADU1pZ3JhdGlvbkRhdGEAAAAAAAABAAAAAAAAAAd2ZXJzaW9uAAAAAAQ=",
        "AAAAAQAAAAAAAAAAAAAADVBlbmRpbmdBY3Rpb24AAAAAAAAJAAAAAAAAAAlhY3Rpb25faWQAAAAAAAAGAAAAAAAAABJjb21taXRfb2JzZXJ2ZWRfYXQAAAAAAAYAAAAAAAAACmNyZWF0ZWRfYXQAAAAAAAYAAAAAAAAAE2VzY3Jvd2VkX2NvbGxhdGVyYWwAAAAACwAAAAAAAAANZXhlY3V0ZV9hZnRlcgAAAAAAAAYAAAAAAAAABGtpbmQAAAfQAAAACkFjdGlvbktpbmQAAAAAAAAAAAAJbWFya2V0X2lkAAAAAAAAEQAAAAAAAAAFb3duZXIAAAAAAAATAAAAAAAAAAdwYXlsb2FkAAAAB9AAAAANQWN0aW9uUGF5bG9hZAAAAA==",
        "AAAAAQAAAAAAAAAAAAAADlBlbmRpbmdVcGdyYWRlAAAAAAACAAAAAAAAAANldGEAAAAABgAAAAAAAAAJd2FzbV9oYXNoAAAAAAAD7gAAACA=",
        "AAAAAQAAAAAAAAAAAAAADlJlbWFpbmRlckdyb3VwAAAAAAAEAAAAAAAAABZkaXN0cmlidXRpb25fcmVtYWluZGVyAAAAAAALAAAAAAAAABJscF9wYXllcl9yZW1haW5kZXIAAAAAAAsAAAAAAAAAHHJlY2VpdmVyX2xpYWJpbGl0eV9yZW1haW5kZXIAAAALAAAAAAAAABhyZWNlaXZlcl9wYXllcl9yZW1haW5kZXIAAAAL",
        "AAAAAQAAAAAAAAAAAAAAD0RlY3JlYXNlUGF5bG9hZAAAAAADAAAAAAAAABBhY2NlcHRhYmxlX3ByaWNlAAAACwAAAAAAAAALcG9zaXRpb25faWQAAAAABgAAAAAAAAAMc2l6ZV9yZW1vdmVkAAAACw==",
        "AAAAAgAAAAAAAAAAAAAAD0xwUmVxdWVzdFN0YXR1cwAAAAADAAAAAAAAAAAAAAAHUGVuZGluZwAAAAAAAAAAAAAAAAdTZXR0bGVkAAAAAAAAAAAAAAAABkZhaWxlZAAA",
        "AAAAAQAAAAAAAAAAAAAAD1BlbmRpbmdGZWVzVmlldwAAAAAEAAAAAAAAAAZib3Jyb3cAAAAAAAsAAAAAAAAAE2Z1bmRpbmdfcGFpZF90b19scHMAAAAACwAAAAAAAAAZZnVuZGluZ19wYWlkX3RvX3JlY2VpdmVycwAAAAAAAAsAAAAAAAAAEGZ1bmRpbmdfcmVjZWl2ZWQAAAAL",
        "AAAAAQAAAAAAAAAAAAAAEFBlbmRpbmdQcmljZUZlZWQAAAACAAAAAAAAAAxlZmZlY3RpdmVfYXQAAAAGAAAAAAAAAApwcmljZV9mZWVkAAAAAAAT",
        "AAAAAQAAAAAAAAAAAAAAEFNldHRsZW1lbnRSZXN1bHQAAAADAAAAAAAAAAZhbW91bnQAAAAAAAsAAAAAAAAABnJld2FyZAAAAAAACwAAAAAAAAAGc3RhdHVzAAAAAAfQAAAAEFNldHRsZW1lbnRTdGF0dXM=",
        "AAAAAgAAAAAAAAAAAAAAEFNldHRsZW1lbnRTdGF0dXMAAAADAAAAAAAAAAAAAAAHU2V0dGxlZAAAAAAAAAAAAAAAAAZGYWlsZWQAAAAAAAAAAAAAAAAACE5vdFJlYWR5",
        "AAAAAQAAAAAAAAAAAAAAEFRyaWdnZXJDb25kaXRpb24AAAACAAAAAAAAAA10cmlnZ2VyX2Fib3ZlAAAAAAAAAQAAAAAAAAANdHJpZ2dlcl9wcmljZQAAAAAAAAs=",
        "AAAAAQAAAAAAAAAAAAAAEkFjY291bnRpbmdTbmFwc2hvdAAAAAAADAAAAAAAAAAOY2FzaF9scF9lcXVpdHkAAAAAAAsAAAAAAAAADmNhc2hfc2hvcnRmYWxsAAAAAAALAAAAAAAAABdkZWxldmVyYWdpbmdfc2lkZV9jb3VudAAAAAAEAAAAAAAAAA9mcmVlX2xwX2NhcGl0YWwAAAAACwAAAAAAAAAXbWluX2VxdWl0eV9jbGVhcl9vZl9hZGwAAAAACwAAAAAAAAANbm9uX2xwX2NsYWltcwAAAAAAAAsAAAAAAAAAE29wZW5fcG9zaXRpb25fY291bnQAAAAABgAAAAAAAAANcGh5c2ljYWxfY2FzaAAAAAAAAAsAAAAAAAAAFXJlcXVpcmVkX3Jpc2tfYmFja2luZwAAAAAAAAsAAAAAAAAAFXJlc3RyaWN0ZWRfc2lkZV9jb3VudAAAAAAAAAQAAAAAAAAAEHRvdGFsX3Jpc2tfdW5pdHMAAAALAAAAAAAAAAl2YXVsdF9uYXYAAAAAAAAL",
        "AAAAAQAAAAAAAAAAAAAAElRyaWdnZXJJbnN0cnVjdGlvbgAAAAAABQAAAAAAAAAQYWNjZXB0YWJsZV9wcmljZQAAAAsAAAAAAAAAEmNvbW1pdF9vYnNlcnZlZF9hdAAAAAAABgAAAAAAAAAMY29tbWl0dGVkX2F0AAAABgAAAAAAAAANZXhlY3V0ZV9hZnRlcgAAAAAAAAYAAAAAAAAADXRyaWdnZXJfcHJpY2UAAAAAAAAL",
        "AAAAAQAAAAAAAAAAAAAAE1BlbmRpbmdHbG9iYWxDb25maWcAAAAAAgAAAAAAAAAGY29uZmlnAAAAAAfQAAAADEdsb2JhbENvbmZpZwAAAAAAAAAMZWZmZWN0aXZlX2F0AAAABg==",
        "AAAAAQAAAAAAAAAAAAAAE1BlbmRpbmdNYXJrZXRDb25maWcAAAAAAgAAAAAAAAAGY29uZmlnAAAAAAfQAAAADE1hcmtldENvbmZpZwAAAAAAAAAMZWZmZWN0aXZlX2F0AAAABg==",
        "AAAAAQAAAAAAAAAAAAAAC0V2ZW50SGVhZGVyAAAAAAQAAAAAAAAABWFjdG9yAAAAAAAAEwAAAAAAAAANZXZlbnRfdmVyc2lvbgAAAAAAAAQAAAAAAAAAEGxlZGdlcl90aW1lc3RhbXAAAAAGAAAAAAAAAAZtYXJrZXQAAAAAABE=",
        "AAAABQAAAAAAAAAAAAAAD1VwZ3JhZGVQcm9wb3NlZAAAAAABAAAABnVwZ3BycAAAAAAAAgAAAAAAAAAJd2FzbV9oYXNoAAAAAAAD7gAAACAAAAAAAAAAAAAAAANldGEAAAAABgAAAAAAAAAB",
        "AAAABQAAAAAAAAAAAAAAEFVwZ3JhZGVDYW5jZWxsZWQAAAABAAAABnVwZ2NhbgAAAAAAAQAAAAAAAAAGY2FsbGVyAAAAAAATAAAAAAAAAAE=",
        "AAAABAAAAAAAAAAAAAAAEFVwZ3JhZGVhYmxlRXJyb3IAAAABAAAAQVdoZW4gbWlncmF0aW9uIGlzIGF0dGVtcHRlZCBidXQgbm90IGFsbG93ZWQgZHVlIHRvIHVwZ3JhZGUgc3RhdGUuAAAAAAAAE01pZ3JhdGlvbk5vdEFsbG93ZWQAAAAETA==",
        "AAAABQAAACpFdmVudCBlbWl0dGVkIHdoZW4gdGhlIG1lcmtsZSByb290IGlzIHNldC4AAAAAAAAAAAAHU2V0Um9vdAAAAAABAAAACHNldF9yb290AAAAAQAAAAAAAAAEcm9vdAAAAA4AAAAAAAAAAg==",
        "AAAABQAAACdFdmVudCBlbWl0dGVkIHdoZW4gYW4gaW5kZXggaXMgY2xhaW1lZC4AAAAAAAAAAApTZXRDbGFpbWVkAAAAAAABAAAAC3NldF9jbGFpbWVkAAAAAAEAAAAAAAAABWluZGV4AAAAAAAAAAAAAAAAAAAC",
        "AAAABAAAAAAAAAAAAAAAFk1lcmtsZURpc3RyaWJ1dG9yRXJyb3IAAAAAAAMAAAAbVGhlIG1lcmtsZSByb290IGlzIG5vdCBzZXQuAAAAAApSb290Tm90U2V0AAAAAAUUAAAAJ1RoZSBwcm92aWRlZCBpbmRleCB3YXMgYWxyZWFkeSBjbGFpbWVkLgAAAAATSW5kZXhBbHJlYWR5Q2xhaW1lZAAAAAUVAAAAFVRoZSBwcm9vZiBpcyBpbnZhbGlkLgAAAAAAAAxJbnZhbGlkUHJvb2YAAAUW",
        "AAAAAgAAACpSb3VuZGluZyBkaXJlY3Rpb24gZm9yIGRpdmlzaW9uIG9wZXJhdGlvbnMAAAAAAAAAAAAIUm91bmRpbmcAAAADAAAAAAAAACVSb3VuZCB0b3dhcmQgbmVnYXRpdmUgaW5maW5pdHkgKGRvd24pAAAAAAAABUZsb29yAAAAAAAAAAAAACNSb3VuZCB0b3dhcmQgcG9zaXRpdmUgaW5maW5pdHkgKHVwKQAAAAAEQ2VpbAAAAAAAAAAeUm91bmQgdG93YXJkIHplcm8gKHRydW5jYXRpb24pAAAAAAAIVHJ1bmNhdGU=",
        "AAAABAAAAAAAAAAAAAAAFlNvcm9iYW5GaXhlZFBvaW50RXJyb3IAAAAAAAIAAAAcQXJpdGhtZXRpYyBvdmVyZmxvdyBvY2N1cnJlZAAAAAhPdmVyZmxvdwAABdwAAAAQRGl2aXNpb24gYnkgemVybwAAAA5EaXZpc2lvbkJ5WmVybwAAAAAF3Q==",
        "AAAABQAAACpFdmVudCBlbWl0dGVkIHdoZW4gdGhlIGNvbnRyYWN0IGlzIHBhdXNlZC4AAAAAAAAAAAAGUGF1c2VkAAAAAAABAAAABnBhdXNlZAAAAAAAAAAAAAI=",
        "AAAABQAAACxFdmVudCBlbWl0dGVkIHdoZW4gdGhlIGNvbnRyYWN0IGlzIHVucGF1c2VkLgAAAAAAAAAIVW5wYXVzZWQAAAABAAAACHVucGF1c2VkAAAAAAAAAAI=",
        "AAAABAAAAAAAAAAAAAAADVBhdXNhYmxlRXJyb3IAAAAAAAACAAAANFRoZSBvcGVyYXRpb24gZmFpbGVkIGJlY2F1c2UgdGhlIGNvbnRyYWN0IGlzIHBhdXNlZC4AAAANRW5mb3JjZWRQYXVzZQAAAAAAA+gAAAA4VGhlIG9wZXJhdGlvbiBmYWlsZWQgYmVjYXVzZSB0aGUgY29udHJhY3QgaXMgbm90IHBhdXNlZC4AAAANRXhwZWN0ZWRQYXVzZQAAAAAAA+k=",
        "AAAAAgAAAD1TdG9yYWdlIGtleXMgZm9yIHRoZSBkYXRhIGFzc29jaWF0ZWQgd2l0aCBgTWVya2xlRGlzdHJpYnV0b3JgAAAAAAAAAAAAABtNZXJrbGVEaXN0cmlidXRvclN0b3JhZ2VLZXkAAAAAAgAAAAAAAAAoVGhlIE1lcmtsZSByb290IG9mIHRoZSBkaXN0cmlidXRpb24gdHJlZQAAAARSb290AAAAAQAAACNNYXBzIGFuIGluZGV4IHRvIGl0cyBjbGFpbWVkIHN0YXR1cwAAAAAHQ2xhaW1lZAAAAAABAAAABA==",
        "AAAABAAAAAAAAAAAAAAAC0NyeXB0b0Vycm9yAAAAAAMAAAApVGhlIG1lcmtsZSBwcm9vZiBsZW5ndGggaXMgb3V0IG9mIGJvdW5kcy4AAAAAAAAWTWVya2xlUHJvb2ZPdXRPZkJvdW5kcwAAAAAFeAAAACdUaGUgaW5kZXggb2YgdGhlIGxlYWYgaXMgb3V0IG9mIGJvdW5kcy4AAAAAFk1lcmtsZUluZGV4T3V0T2ZCb3VuZHMAAAAABXkAAAAYTm8gZGF0YSBpbiBoYXNoZXIgc3RhdGUuAAAAEEhhc2hlckVtcHR5U3RhdGUAAAV6",
        "AAAAAgAAACJTdG9yYWdlIGtleSBmb3IgdGhlIHBhdXNhYmxlIHN0YXRlAAAAAAAAAAAAElBhdXNhYmxlU3RvcmFnZUtleQAAAAAAAQAAAAAAAAAySW5kaWNhdGVzIHdoZXRoZXIgdGhlIGNvbnRyYWN0IGlzIGluIHBhdXNlZCBzdGF0ZS4AAAAAAAZQYXVzZWQAAA==" ]),
      options
    )
  }
  public readonly fromJSON = {
    pause: this.txFromJSON<null>,
        migrate: this.txFromJSON<null>,
        unpause: this.txFromJSON<null>,
        upgrade: this.txFromJSON<null>,
        governor: this.txFromJSON<string>,
        is_paused: this.txFromJSON<boolean>,
        set_vault: this.txFromJSON<null>,
        get_market: this.txFromJSON<Market>,
        price_feed: this.txFromJSON<string>,
        execute_adl: this.txFromJSON<ActionOutcome>,
        claim_payout: this.txFromJSON<i128>,
        create_close: this.txFromJSON<u64>,
        get_position: this.txFromJSON<Position>,
        recapitalize: this.txFromJSON<null>,
        settle_close: this.txFromJSON<ActionOutcome>,
        enable_market: this.txFromJSON<null>,
        global_config: this.txFromJSON<GlobalConfig>,
        non_lp_claims: this.txFromJSON<i128>,
        set_stop_loss: this.txFromJSON<null>,
        active_markets: this.txFromJSON<Array<string>>,
        add_collateral: this.txFromJSON<null>,
        cancel_upgrade: this.txFromJSON<null>,
        claim_protocol: this.txFromJSON<null>,
        disable_market: this.txFromJSON<null>,
        update_indices: this.txFromJSON<null>,
        clear_stop_loss: this.txFromJSON<null>,
        create_decrease: this.txFromJSON<u64>,
        propose_upgrade: this.txFromJSON<null>,
        set_take_profit: this.txFromJSON<null>,
        settle_decrease: this.txFromJSON<ActionOutcome>,
        unclaimed_payout: this.txFromJSON<i128>,
        cancel_limit_open: this.txFromJSON<i128>,
        clear_take_profit: this.txFromJSON<null>,
        create_limit_open: this.txFromJSON<u64>,
        deregister_market: this.txFromJSON<null>,
        execute_stop_loss: this.txFromJSON<ActionOutcome>,
        settle_limit_open: this.txFromJSON<ActionOutcome>,
        create_market_open: this.txFromJSON<u64>,
        get_pending_action: this.txFromJSON<PendingAction>,
        install_price_feed: this.txFromJSON<null>,
        is_market_disabled: this.txFromJSON<boolean>,
        liquidate_position: this.txFromJSON<null>,
        settle_market_open: this.txFromJSON<ActionOutcome>,
        accounting_snapshot: this.txFromJSON<AccountingSnapshot>,
        clean_expired_entry: this.txFromJSON<null>,
        execute_take_profit: this.txFromJSON<ActionOutcome>,
        prepare_lp_snapshot: this.txFromJSON<AccountingSnapshot>,
        refresh_borrow_rate: this.txFromJSON<null>,
        install_global_config: this.txFromJSON<null>,
        install_market_config: this.txFromJSON<null>
  }
}