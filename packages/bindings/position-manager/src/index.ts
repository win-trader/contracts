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
  20: {message:"InvalidCaller"}
}



export type Key = {tag: "ConfigManager", values: void} | {tag: "OracleRouter", values: void} | {tag: "Vault", values: void} | {tag: "GlobalConfig", values: void} | {tag: "Initialized", values: void} | {tag: "Paused", values: void} | {tag: "NextPositionId", values: void} | {tag: "ActiveMarkets", values: void} | {tag: "Position", values: readonly [u64]} | {tag: "Market", values: readonly [string]} | {tag: "MarketDisabled", values: readonly [string]} | {tag: "BorrowIndex", values: void} | {tag: "BorrowIndexRemainder", values: void} | {tag: "CurrentBorrowRate", values: void} | {tag: "GlobalReceiverFlow", values: void} | {tag: "GlobalReceiverRemainder", values: void} | {tag: "LastGlobalCheckpoint", values: void} | {tag: "StoredCollateralTotal", values: void} | {tag: "PendingReceiverFundingTotal", values: void} | {tag: "ExecutionBudgetTotal", values: void} | {tag: "ProtocolClaimableTotal", values: void} | {tag: "RiskKeeperReserveTotal", values: void} | {tag: "TotalRiskUnits", values: void} | {tag: "OpenPositionCount", values: void} | {tag: "LpBlockedSideCount", values: void};


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
 * Asset units at `PRECISION`.
 */
base_exposure: i128;
  borrow_debt: i128;
  /**
 * Trader-owned collateral held by the vault.
 */
collateral: i128;
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
 * USD notional at `PRECISION`.
 */
size: i128;
  stop_loss: i128;
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

export type RiskState = {tag: "Normal", values: void} | {tag: "Warning", values: void} | {tag: "Adl", values: void} | {tag: "HardCap", values: void};


export interface MarketInfo {
  config: MarketConfig;
  current_lp_flow_per_second: i128;
  current_payer_rate: i128;
  /**
 * 1 = long pays, -1 = short pays, 0 = no payer.
 */
current_payer_side: i32;
  last_funding_checkpoint: u64;
  long: MarketSide;
  lp_backed_payer_index_long: i128;
  lp_backed_payer_index_short: i128;
  lp_payer_remainder: i128;
  receiver_flow_per_second: i128;
  receiver_flow_remainder: i128;
  receiver_index_long: i128;
  receiver_index_remainder: i128;
  receiver_index_short: i128;
  receiver_payer_remainder: i128;
  recv_payer_index_long: i128;
  recv_payer_index_short: i128;
  short: MarketSide;
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
  hard_cap_factor_limit_bps: u32;
  lp_revenue_share_bps: u32;
  max_active_markets: u32;
  max_adl_reward: i128;
  max_insolvent_touch_reward: i128;
  max_variable_borrow_bps_day: i128;
  min_collateral: i128;
  min_position_lifetime: u64;
  risk_capacity_limit_bps: u32;
  risk_keeper_revenue_share_bps: u32;
}


export interface MarketConfig {
  adl_pnl_factor_bps: u32;
  adl_reward_bps: u32;
  hard_cap_pnl_factor_bps: u32;
  liquidation_reward_bps: u32;
  maintenance_margin_bps: u32;
  market_risk_factor_bps: u32;
  max_funding_rate_bps_day: i128;
  max_long_base_exposure: i128;
  max_long_size_open_interest: i128;
  max_short_base_exposure: i128;
  max_short_size_open_interest: i128;
  open_fee_high_bps: u32;
  open_fee_low_bps: u32;
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
 * (e.g., 100 = 1%). Bounded at `shared::constants::MAX_DEVIATION_BPS_CEILING`.
 */
max_deviation_bps: i128;
  /**
 * Minimum number of source responses that must agree within
 * `max_deviation_bps` for OracleRouter to return a price. Floored at
 * `shared::constants::MIN_REQUIRED_SOURCES_FLOOR`, ceilinged at
 * `shared::constants::MAX_ORACLE_SOURCES`.
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
 * `interfaces::upgrade::pending_upgrade_key`). `upgrade` refuses to install
 * unless `pending.wasm_hash` matches the supplied hash and `now >= eta`.
 */
export interface PendingUpgrade {
  eta: u64;
  wasm_hash: Buffer;
}

export type LpRequestStatus = {tag: "Pending", values: void} | {tag: "Settled", values: void} | {tag: "Failed", values: void} | {tag: "Expired", values: void};


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
  get_market: ({market}: {market: string}, options?: MethodOptions) => Promise<AssembledTransaction<MarketInfo>>

  /**
   * Construct and simulate a get_position transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_position: ({position_id}: {position_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Position>>

  /**
   * Construct and simulate a recapitalize transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  recapitalize: ({contributor, amount}: {contributor: string, amount: i128}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

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
   * Construct and simulate a disable_market transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  disable_market: ({caller, market}: {caller: string, market: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a update_indices transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  update_indices: ({caller, market_symbol}: {caller: string, market_symbol: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a propose_upgrade transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  propose_upgrade: ({caller, wasm_hash}: {caller: string, wasm_hash: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a decrease_position transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  decrease_position: ({position_id, size_removed, collateral_withdrawn, acceptable_price}: {position_id: u64, size_removed: i128, collateral_withdrawn: i128, acceptable_price: i128}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a increase_position transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  increase_position: ({position_id, size_added, collateral_added, acceptable_price}: {position_id: u64, size_added: i128, collateral_added: i128, acceptable_price: i128}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a set_global_config transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  set_global_config: ({caller, config}: {caller: string, config: GlobalConfig}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a set_market_config transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  set_market_config: ({caller, market_symbol, config}: {caller: string, market_symbol: string, config: MarketConfig}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

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
   * Construct and simulate a prepare_lp_snapshot transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  prepare_lp_snapshot: ({caller, round, physical}: {caller: string, round: OracleRound, physical: i128}, options?: MethodOptions) => Promise<AssembledTransaction<AccountingSnapshot>>

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
   * Construct and simulate a protocol_claimable_total transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  protocol_claimable_total: (options?: MethodOptions) => Promise<AssembledTransaction<i128>>

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
      new ContractSpec([ "AAAABAAAAAAAAAAAAAAAFFBvc2l0aW9uTWFuYWdlckVycm9yAAAAFAAAAAAAAAAMVW5hdXRob3JpemVkAAAAAQAAAAAAAAAOTm90SW5pdGlhbGl6ZWQAAAAAAAIAAAAAAAAAEkFscmVhZHlJbml0aWFsaXplZAAAAAAAAwAAAAAAAAANSW52YWxpZEFtb3VudAAAAAAAAAQAAAAAAAAADUludmFsaWRDb25maWcAAAAAAAAFAAAAAAAAABBQb3NpdGlvbk5vdEZvdW5kAAAABgAAAAAAAAATTWFya2V0Tm90Q29uZmlndXJlZAAAAAAHAAAAAAAAAA5NYXJrZXREaXNhYmxlZAAAAAAACAAAAAAAAAAQU2xpcHBhZ2VFeGNlZWRlZAAAAAkAAAAAAAAAEENhcGFjaXR5RXhjZWVkZWQAAAAKAAAAAAAAABNNYXJrZXRMaW1pdEV4Y2VlZGVkAAAAAAsAAAAAAAAAFkluc3VmZmljaWVudENvbGxhdGVyYWwAAAAAAAwAAAAAAAAAD1Bvc2l0aW9uSGVhbHRoeQAAAAANAAAAAAAAABBSaXNrU3RhdGVCbG9ja2VkAAAADgAAAAAAAAAPQXJpdGhtZXRpY0Vycm9yAAAAAA8AAAAAAAAAEkludmFsaWRPcmFjbGVSb3VuZAAAAAAAEAAAAAAAAAAIVG9vRWFybHkAAAARAAAAAAAAAAxJbnZhbGlkT3JkZXIAAAASAAAAAAAAABtJbnN1ZmZpY2llbnRFeGVjdXRpb25CdWRnZXQAAAAAEwAAAAAAAAANSW52YWxpZENhbGxlcgAAAAAAABQ=",
        "AAAABQAAAAAAAAAAAAAAB0JhZERlYnQAAAAAAQAAAAdiYWRkZWJ0AAAAAAIAAAAAAAAAC3Bvc2l0aW9uX2lkAAAAAAYAAAAAAAAAAAAAAAZhbW91bnQAAAAAAAsAAAAAAAAAAQ==",
        "AAAABQAAAAAAAAAAAAAADlBvc2l0aW9uT3BlbmVkAAAAAAABAAAAB3Bvc29wZW4AAAAAAwAAAAAAAAALcG9zaXRpb25faWQAAAAABgAAAAAAAAAAAAAABW93bmVyAAAAAAAAEwAAAAAAAAAAAAAABm1hcmtldAAAAAAAEQAAAAAAAAAB",
        "AAAAAgAAAAAAAAAAAAAAA0tleQAAAAAZAAAAAAAAAAAAAAANQ29uZmlnTWFuYWdlcgAAAAAAAAAAAAAAAAAADE9yYWNsZVJvdXRlcgAAAAAAAAAAAAAABVZhdWx0AAAAAAAAAAAAAAAAAAAMR2xvYmFsQ29uZmlnAAAAAAAAAAAAAAALSW5pdGlhbGl6ZWQAAAAAAAAAAAAAAAAGUGF1c2VkAAAAAAAAAAAAAAAAAA5OZXh0UG9zaXRpb25JZAAAAAAAAAAAAAAAAAANQWN0aXZlTWFya2V0cwAAAAAAAAEAAAAAAAAACFBvc2l0aW9uAAAAAQAAAAYAAAABAAAAAAAAAAZNYXJrZXQAAAAAAAEAAAARAAAAAQAAAAAAAAAOTWFya2V0RGlzYWJsZWQAAAAAAAEAAAARAAAAAAAAAAAAAAALQm9ycm93SW5kZXgAAAAAAAAAAAAAAAAUQm9ycm93SW5kZXhSZW1haW5kZXIAAAAAAAAAAAAAABFDdXJyZW50Qm9ycm93UmF0ZQAAAAAAAAAAAAAAAAAAEkdsb2JhbFJlY2VpdmVyRmxvdwAAAAAAAAAAAAAAAAAXR2xvYmFsUmVjZWl2ZXJSZW1haW5kZXIAAAAAAAAAAAAAAAAUTGFzdEdsb2JhbENoZWNrcG9pbnQAAAAAAAAAAAAAABVTdG9yZWRDb2xsYXRlcmFsVG90YWwAAAAAAAAAAAAAAAAAABtQZW5kaW5nUmVjZWl2ZXJGdW5kaW5nVG90YWwAAAAAAAAAAAAAAAAURXhlY3V0aW9uQnVkZ2V0VG90YWwAAAAAAAAAAAAAABZQcm90b2NvbENsYWltYWJsZVRvdGFsAAAAAAAAAAAAAAAAABZSaXNrS2VlcGVyUmVzZXJ2ZVRvdGFsAAAAAAAAAAAAAAAAAA5Ub3RhbFJpc2tVbml0cwAAAAAAAAAAAAAAAAART3BlblBvc2l0aW9uQ291bnQAAAAAAAAAAAAAAAAAABJMcEJsb2NrZWRTaWRlQ291bnQAAA==",
        "AAAAAAAAAAAAAAAFcGF1c2UAAAAAAAABAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAA",
        "AAAAAAAAAAAAAAAHbWlncmF0ZQAAAAACAAAAAAAAAA5taWdyYXRpb25fZGF0YQAAAAAH0AAAAA1NaWdyYXRpb25EYXRhAAAAAAAAAAAAAAhvcGVyYXRvcgAAABMAAAAA",
        "AAAAAAAAAAAAAAAHdW5wYXVzZQAAAAABAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAA",
        "AAAAAAAAAAAAAAAHdXBncmFkZQAAAAACAAAAAAAAAA1uZXdfd2FzbV9oYXNoAAAAAAAD7gAAACAAAAAAAAAACG9wZXJhdG9yAAAAEwAAAAA=",
        "AAAAAAAAAAAAAAAJc2V0X3RwX3NsAAAAAAAAAwAAAAAAAAALcG9zaXRpb25faWQAAAAABgAAAAAAAAALdGFrZV9wcm9maXQAAAAACwAAAAAAAAAJc3RvcF9sb3NzAAAAAAAACwAAAAA=",
        "AAAAAAAAAAAAAAAJc2V0X3ZhdWx0AAAAAAAAAgAAAAAAAAAGY2FsbGVyAAAAAAATAAAAAAAAAAV2YXVsdAAAAAAAABMAAAAA",
        "AAAAAAAAAAAAAAAKZ2V0X21hcmtldAAAAAAAAQAAAAAAAAAGbWFya2V0AAAAAAARAAAAAQAAB9AAAAAKTWFya2V0SW5mbwAA",
        "AAAAAAAAAAAAAAAMZ2V0X3Bvc2l0aW9uAAAAAQAAAAAAAAALcG9zaXRpb25faWQAAAAABgAAAAEAAAfQAAAACFBvc2l0aW9u",
        "AAAAAAAAAAAAAAAMcmVjYXBpdGFsaXplAAAAAgAAAAAAAAALY29udHJpYnV0b3IAAAAAEwAAAAAAAAAGYW1vdW50AAAAAAALAAAAAA==",
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
        "AAAAAAAAAAAAAAAOZGlzYWJsZV9tYXJrZXQAAAAAAAIAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAAAAAAGbWFya2V0AAAAAAARAAAAAA==",
        "AAAAAAAAAAAAAAAOdXBkYXRlX2luZGljZXMAAAAAAAIAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAAAAAANbWFya2V0X3N5bWJvbAAAAAAAABEAAAAA",
        "AAAAAAAAAAAAAAAPcHJvcG9zZV91cGdyYWRlAAAAAAIAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAAAAAAJd2FzbV9oYXNoAAAAAAAD7gAAACAAAAAA",
        "AAAAAAAAAAAAAAARZGVjcmVhc2VfcG9zaXRpb24AAAAAAAAEAAAAAAAAAAtwb3NpdGlvbl9pZAAAAAAGAAAAAAAAAAxzaXplX3JlbW92ZWQAAAALAAAAAAAAABRjb2xsYXRlcmFsX3dpdGhkcmF3bgAAAAsAAAAAAAAAEGFjY2VwdGFibGVfcHJpY2UAAAALAAAAAA==",
        "AAAAAAAAAAAAAAARaW5jcmVhc2VfcG9zaXRpb24AAAAAAAAEAAAAAAAAAAtwb3NpdGlvbl9pZAAAAAAGAAAAAAAAAApzaXplX2FkZGVkAAAAAAALAAAAAAAAABBjb2xsYXRlcmFsX2FkZGVkAAAACwAAAAAAAAAQYWNjZXB0YWJsZV9wcmljZQAAAAsAAAAA",
        "AAAAAAAAAAAAAAARc2V0X2dsb2JhbF9jb25maWcAAAAAAAACAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAAAAAABmNvbmZpZwAAAAAH0AAAAAxHbG9iYWxDb25maWcAAAAA",
        "AAAAAAAAAAAAAAARc2V0X21hcmtldF9jb25maWcAAAAAAAADAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAAAAAADW1hcmtldF9zeW1ib2wAAAAAAAARAAAAAAAAAAZjb25maWcAAAAAB9AAAAAMTWFya2V0Q29uZmlnAAAAAA==",
        "AAAAAAAAAAAAAAASaXNfbWFya2V0X2Rpc2FibGVkAAAAAAABAAAAAAAAAAZtYXJrZXQAAAAAABEAAAABAAAAAQ==",
        "AAAAAAAAAAAAAAASbGlxdWlkYXRlX3Bvc2l0aW9uAAAAAAACAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAAAAAAC3Bvc2l0aW9uX2lkAAAAAAYAAAAA",
        "AAAAAAAAAAAAAAATYWNjb3VudGluZ19zbmFwc2hvdAAAAAACAAAAAAAAAAVyb3VuZAAAAAAAB9AAAAALT3JhY2xlUm91bmQAAAAAAAAAAAhwaHlzaWNhbAAAAAsAAAABAAAH0AAAABJBY2NvdW50aW5nU25hcHNob3QAAA==",
        "AAAAAAAAAAAAAAATZGVsZXZlcmFnZV9wb3NpdGlvbgAAAAACAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAAAAAAC3Bvc2l0aW9uX2lkAAAAAAYAAAAA",
        "AAAAAAAAAAAAAAATcHJlcGFyZV9scF9zbmFwc2hvdAAAAAADAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAAAAAABXJvdW5kAAAAAAAH0AAAAAtPcmFjbGVSb3VuZAAAAAAAAAAACHBoeXNpY2FsAAAACwAAAAEAAAfQAAAAEkFjY291bnRpbmdTbmFwc2hvdAAA",
        "AAAAAAAAAAAAAAATcmVmcmVzaF9ib3Jyb3dfcmF0ZQAAAAACAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAAAAAACHBoeXNpY2FsAAAACwAAAAA=",
        "AAAAAAAAAAAAAAAVY2FuX2NyZWF0ZV9scF9yZXF1ZXN0AAAAAAAAAgAAAAAAAAAGY2FsbGVyAAAAAAATAAAAAAAAAAhwaHlzaWNhbAAAAAsAAAABAAAAAQ==",
        "AAAAAAAAAAAAAAAVZnVuZF9leGVjdXRpb25fYnVkZ2V0AAAAAAAAAgAAAAAAAAALcG9zaXRpb25faWQAAAAABgAAAAAAAAAGYW1vdW50AAAAAAALAAAAAA==",
        "AAAAAAAAAAAAAAAYcHJvdG9jb2xfY2xhaW1hYmxlX3RvdGFsAAAAAAAAAAEAAAAL",
        "AAAAAAAAAAAAAAAZcmlza19rZWVwZXJfcmVzZXJ2ZV90b3RhbAAAAAAAAAAAAAABAAAACw==",
        "AAAAAAAAAAAAAAAZd2l0aGRyYXdfZXhlY3V0aW9uX2J1ZGdldAAAAAAAAAIAAAAAAAAAC3Bvc2l0aW9uX2lkAAAAAAYAAAAAAAAABmFtb3VudAAAAAAACwAAAAA=",
        "AAAAAAAAAAAAAAAecGVuZGluZ19yZWNlaXZlcl9mdW5kaW5nX3RvdGFsAAAAAAAAAAAAAQAAAAs=",
        "AAAAAQAAAAAAAAAAAAAACExwQ29uZmlnAAAAAwAAAAAAAAAQbHBfcmVxdWVzdF9kZWxheQAAAAYAAAAAAAAAHG1heF93aXRoZHJhd191dGlsaXphdGlvbl9icHMAAAAEAAAAAAAAABptaW5fZGVwb3NpdF9uYXZfZmFjdG9yX2JwcwAAAAAABA==",
        "AAAAAQAAADVSZXByZXNlbnRzIGEgc2luZ2xlIHRyYWRlcidzIG9wZW4gbGV2ZXJhZ2VkIHBvc2l0aW9uLgAAAAAAAAAAAAAIUG9zaXRpb24AAAAQAAAAG0Fzc2V0IHVuaXRzIGF0IGBQUkVDSVNJT05gLgAAAAANYmFzZV9leHBvc3VyZQAAAAAAAAsAAAAAAAAAC2JvcnJvd19kZWJ0AAAAAAsAAAAqVHJhZGVyLW93bmVkIGNvbGxhdGVyYWwgaGVsZCBieSB0aGUgdmF1bHQuAAAAAAAKY29sbGF0ZXJhbAAAAAAACwAAAClDYXNoIG93bmVkIGJ5IGFuIG9wdGlvbmFsLW9yZGVyIGV4ZWN1dG9yLgAAAAAAABBleGVjdXRpb25fYnVkZ2V0AAAACwAAAAAAAAAYZnVuZGluZ19wYWlkX3RvX2xwc19kZWJ0AAAACwAAAAAAAAAeZnVuZGluZ19wYWlkX3RvX3JlY2VpdmVyc19kZWJ0AAAAAAALAAAAAAAAABVmdW5kaW5nX3JlY2VpdmVkX2RlYnQAAAAAAAALAAAAAAAAAAJpZAAAAAAABgAAAAAAAAAHaXNfbG9uZwAAAAABAAAAAAAAABNsYXN0X2luY3JlYXNlZF90aW1lAAAAAAYAAAAAAAAABm1hcmtldAAAAAAAEQAAAAAAAAAFb3duZXIAAAAAAAATAAAALkZpeGVkIGdyb3NzIGNhcGFjaXR5IGFzc2lnbmVkIHdoZW4gcmlzayBvcGVucy4AAAAAAApyaXNrX3VuaXRzAAAAAAALAAAAHFVTRCBub3Rpb25hbCBhdCBgUFJFQ0lTSU9OYC4AAAAEc2l6ZQAAAAsAAAAAAAAACXN0b3BfbG9zcwAAAAAAAAsAAAAAAAAAC3Rha2VfcHJvZml0AAAAAAs=",
        "AAAAAQAAAAAAAAAAAAAACUxwUmVxdWVzdAAAAAAAAAcAAAAAAAAABmFtb3VudAAAAAAACwAAAAAAAAANZXhlY3V0ZV9hZnRlcgAAAAAAAAYAAAAAAAAAAmlkAAAAAAAGAAAAAAAAAARraW5kAAAH0AAAAA1McFJlcXVlc3RLaW5kAAAAAAAAAAAAAAVvd25lcgAAAAAAABMAAAAAAAAADHJlcXVlc3RfdGltZQAAAAYAAAAAAAAABnN0YXR1cwAAAAAH0AAAAA9McFJlcXVlc3RTdGF0dXMA",
        "AAAAAgAAAAAAAAAAAAAACVJpc2tTdGF0ZQAAAAAAAAQAAAAAAAAAAAAAAAZOb3JtYWwAAAAAAAAAAAAAAAAAB1dhcm5pbmcAAAAAAAAAAAAAAAADQWRsAAAAAAAAAAAAAAAAB0hhcmRDYXAA",
        "AAAAAQAAAAAAAAAAAAAACk1hcmtldEluZm8AAAAAABIAAAAAAAAABmNvbmZpZwAAAAAH0AAAAAxNYXJrZXRDb25maWcAAAAAAAAAGmN1cnJlbnRfbHBfZmxvd19wZXJfc2Vjb25kAAAAAAALAAAAAAAAABJjdXJyZW50X3BheWVyX3JhdGUAAAAAAAsAAAAtMSA9IGxvbmcgcGF5cywgLTEgPSBzaG9ydCBwYXlzLCAwID0gbm8gcGF5ZXIuAAAAAAAAEmN1cnJlbnRfcGF5ZXJfc2lkZQAAAAAABQAAAAAAAAAXbGFzdF9mdW5kaW5nX2NoZWNrcG9pbnQAAAAABgAAAAAAAAAEbG9uZwAAB9AAAAAKTWFya2V0U2lkZQAAAAAAAAAAABpscF9iYWNrZWRfcGF5ZXJfaW5kZXhfbG9uZwAAAAAACwAAAAAAAAAbbHBfYmFja2VkX3BheWVyX2luZGV4X3Nob3J0AAAAAAsAAAAAAAAAEmxwX3BheWVyX3JlbWFpbmRlcgAAAAAACwAAAAAAAAAYcmVjZWl2ZXJfZmxvd19wZXJfc2Vjb25kAAAACwAAAAAAAAAXcmVjZWl2ZXJfZmxvd19yZW1haW5kZXIAAAAACwAAAAAAAAATcmVjZWl2ZXJfaW5kZXhfbG9uZwAAAAALAAAAAAAAABhyZWNlaXZlcl9pbmRleF9yZW1haW5kZXIAAAALAAAAAAAAABRyZWNlaXZlcl9pbmRleF9zaG9ydAAAAAsAAAAAAAAAGHJlY2VpdmVyX3BheWVyX3JlbWFpbmRlcgAAAAsAAAAAAAAAFXJlY3ZfcGF5ZXJfaW5kZXhfbG9uZwAAAAAAAAsAAAAAAAAAFnJlY3ZfcGF5ZXJfaW5kZXhfc2hvcnQAAAAAAAsAAAAAAAAABXNob3J0AAAAAAAH0AAAAApNYXJrZXRTaWRlAAA=",
        "AAAAAQAAAAAAAAAAAAAACk1hcmtldFNpZGUAAAAAAAUAAAAAAAAADWJhc2VfZXhwb3N1cmUAAAAAAAALAAAAAAAAAApyaXNrX3N0YXRlAAAAAAfQAAAACVJpc2tTdGF0ZQAAAAAAAAAAAAAKcmlza191bml0cwAAAAAACwAAAAAAAAASc2l6ZV9vcGVuX2ludGVyZXN0AAAAAAALAAAAAAAAABdzdG9yZWRfY29sbGF0ZXJhbF90b3RhbAAAAAAL",
        "AAAAAQAAAAAAAAAAAAAAClJvdW5kUHJpY2UAAAAAAAIAAAAAAAAABXByaWNlAAAAAAAACwAAAAAAAAAGc3ltYm9sAAAAAAAR",
        "AAAAAQAAAAAAAAAAAAAAC09yYWNsZVJvdW5kAAAAAAUAAAAAAAAAAmlkAAAAAAAGAAAAAAAAAAtwcmV2aW91c19pZAAAAAAGAAAAAAAAABJwcmV2aW91c190aW1lc3RhbXAAAAAAAAYAAAAAAAAABnByaWNlcwAAAAAD6gAAB9AAAAAKUm91bmRQcmljZQAAAAAAAAAAAAl0aW1lc3RhbXAAAAAAAAAG",
        "AAAAAQAAAAAAAAAAAAAADEdsb2JhbENvbmZpZwAAAAsAAAAAAAAAGGJhc2VfYm9ycm93X3JhdGVfYnBzX2RheQAAAAsAAAAAAAAAGWhhcmRfY2FwX2ZhY3Rvcl9saW1pdF9icHMAAAAAAAAEAAAAAAAAABRscF9yZXZlbnVlX3NoYXJlX2JwcwAAAAQAAAAAAAAAEm1heF9hY3RpdmVfbWFya2V0cwAAAAAABAAAAAAAAAAObWF4X2FkbF9yZXdhcmQAAAAAAAsAAAAAAAAAGm1heF9pbnNvbHZlbnRfdG91Y2hfcmV3YXJkAAAAAAALAAAAAAAAABttYXhfdmFyaWFibGVfYm9ycm93X2Jwc19kYXkAAAAACwAAAAAAAAAObWluX2NvbGxhdGVyYWwAAAAAAAsAAAAAAAAAFW1pbl9wb3NpdGlvbl9saWZldGltZQAAAAAAAAYAAAAAAAAAF3Jpc2tfY2FwYWNpdHlfbGltaXRfYnBzAAAAAAQAAAAAAAAAHXJpc2tfa2VlcGVyX3JldmVudWVfc2hhcmVfYnBzAAAAAAAABA==",
        "AAAAAQAAAAAAAAAAAAAADE1hcmtldENvbmZpZwAAAA8AAAAAAAAAEmFkbF9wbmxfZmFjdG9yX2JwcwAAAAAABAAAAAAAAAAOYWRsX3Jld2FyZF9icHMAAAAAAAQAAAAAAAAAF2hhcmRfY2FwX3BubF9mYWN0b3JfYnBzAAAAAAQAAAAAAAAAFmxpcXVpZGF0aW9uX3Jld2FyZF9icHMAAAAAAAQAAAAAAAAAFm1haW50ZW5hbmNlX21hcmdpbl9icHMAAAAAAAQAAAAAAAAAFm1hcmtldF9yaXNrX2ZhY3Rvcl9icHMAAAAAAAQAAAAAAAAAGG1heF9mdW5kaW5nX3JhdGVfYnBzX2RheQAAAAsAAAAAAAAAFm1heF9sb25nX2Jhc2VfZXhwb3N1cmUAAAAAAAsAAAAAAAAAG21heF9sb25nX3NpemVfb3Blbl9pbnRlcmVzdAAAAAALAAAAAAAAABdtYXhfc2hvcnRfYmFzZV9leHBvc3VyZQAAAAALAAAAAAAAABxtYXhfc2hvcnRfc2l6ZV9vcGVuX2ludGVyZXN0AAAACwAAAAAAAAARb3Blbl9mZWVfaGlnaF9icHMAAAAAAAAEAAAAAAAAABBvcGVuX2ZlZV9sb3dfYnBzAAAABAAAAAAAAAAXcmVjb3ZlcnlfcG5sX2ZhY3Rvcl9icHMAAAAABAAAAAAAAAAWd2FybmluZ19wbmxfZmFjdG9yX2JwcwAAAAAABA==",
        "AAAAAQAAAC5HbG9iYWwgc2FmZXR5IHRocmVzaG9sZHMgZm9yIHByaWNlIHZhbGlkYXRpb24uAAAAAAAAAAAADE9yYWNsZUNvbmZpZwAAAAQAAADzSG93IGxvbmcgYSBjYWNoZWQgYWdncmVnYXRlZCBwcmljZSByZW1haW5zIHZhbGlkIGFmdGVyIHRoZSByb3V0ZXIKZmV0Y2ggKGluIHNlY29uZHMpLiBBIGNhY2hlIGhpdCBhbHNvIHJlcXVpcmVzIGV2ZXJ5IHNvdXJjZSB0aW1lc3RhbXAKdXNlZCBmb3IgdGhlIGNhY2hlZCBtZWRpYW4gdG8gcmVtYWluIHdpdGhpbiBgc3RhbGVuZXNzX3RocmVzaG9sZGAuCk11c3QgYmUgPiAwIGFuZCA8PSBgc3RhbGVuZXNzX3RocmVzaG9sZGAuAAAAAA5jYWNoZV9kdXJhdGlvbgAAAAAABgAAAIpNYXhpbXVtIGFsbG93ZWQgc3ByZWFkIGJldHdlZW4gb3JhY2xlIHNvdXJjZXMgaW4gYmFzaXMgcG9pbnRzCihlLmcuLCAxMDAgPSAxJSkuIEJvdW5kZWQgYXQgYHNoYXJlZDo6Y29uc3RhbnRzOjpNQVhfREVWSUFUSU9OX0JQU19DRUlMSU5HYC4AAAAAABFtYXhfZGV2aWF0aW9uX2JwcwAAAAAAAAsAAADjTWluaW11bSBudW1iZXIgb2Ygc291cmNlIHJlc3BvbnNlcyB0aGF0IG11c3QgYWdyZWUgd2l0aGluCmBtYXhfZGV2aWF0aW9uX2Jwc2AgZm9yIE9yYWNsZVJvdXRlciB0byByZXR1cm4gYSBwcmljZS4gRmxvb3JlZCBhdApgc2hhcmVkOjpjb25zdGFudHM6Ok1JTl9SRVFVSVJFRF9TT1VSQ0VTX0ZMT09SYCwgY2VpbGluZ2VkIGF0CmBzaGFyZWQ6OmNvbnN0YW50czo6TUFYX09SQUNMRV9TT1VSQ0VTYC4AAAAAFG1pbl9yZXF1aXJlZF9zb3VyY2VzAAAABAAAAFlNYXhpbXVtIGFnZSBvZiBhbiBleHRlcm5hbCBTRVAtNDAgcHJpY2UgZmVlZCBiZWZvcmUgaXQgaXMgcmVqZWN0ZWQKYXMgc3RhbGUgKGluIHNlY29uZHMpLgAAAAAAABNzdGFsZW5lc3NfdGhyZXNob2xkAAAAAAY=",
        "AAAAAgAAAAAAAAAAAAAADUxwUmVxdWVzdEtpbmQAAAAAAAACAAAAAAAAAAAAAAAHRGVwb3NpdAAAAAAAAAAAAAAAAApXaXRoZHJhd2FsAAA=",
        "AAAAAQAAAEtEYXRhIHJlcXVpcmVkIGR1cmluZyBhIFdBU00gbWlncmF0aW9uLiBTaW5nbGUgZGVmaW5pdGlvbiBmb3IgYWxsIGNvbnRyYWN0cy4AAAAAAAAAAA1NaWdyYXRpb25EYXRhAAAAAAAAAQAAAAAAAAAHdmVyc2lvbgAAAAAE",
        "AAAAAQAAAbVQZW5kaW5nIFdBU00gdXBncmFkZSDigJQgc2V0IGJ5IGBwcm9wb3NlX3VwZ3JhZGVgLCBjb25zdW1lZCBieSBgdXBncmFkZWAKKGNsZWFyZWQgYXRvbWljYWxseSBvbiBhIHN1Y2Nlc3NmdWwgaW5zdGFsbCksIG9yIGNsZWFyZWQgYnkgYGNhbmNlbF91cGdyYWRlYC4KU2luZ2xlIHNoYXBlIGFjcm9zcyBldmVyeSBwcm90b2NvbCBjb250cmFjdC4gQ29udHJhY3RzIHN0b3JlIGl0IGF0CnRoZSBzaGFyZWQgYHBlbmRpbmdfdXBncmFkZWAgU3ltYm9sIGtleSBpbiB0aGVpciBvd24gaW5zdGFuY2Ugc3RvcmFnZSAoc2VlCmBpbnRlcmZhY2VzOjp1cGdyYWRlOjpwZW5kaW5nX3VwZ3JhZGVfa2V5YCkuIGB1cGdyYWRlYCByZWZ1c2VzIHRvIGluc3RhbGwKdW5sZXNzIGBwZW5kaW5nLndhc21faGFzaGAgbWF0Y2hlcyB0aGUgc3VwcGxpZWQgaGFzaCBhbmQgYG5vdyA+PSBldGFgLgAAAAAAAAAAAAAOUGVuZGluZ1VwZ3JhZGUAAAAAAAIAAAAAAAAAA2V0YQAAAAAGAAAAAAAAAAl3YXNtX2hhc2gAAAAAAAPuAAAAIA==",
        "AAAAAgAAAAAAAAAAAAAAD0xwUmVxdWVzdFN0YXR1cwAAAAAEAAAAAAAAAAAAAAAHUGVuZGluZwAAAAAAAAAAAAAAAAdTZXR0bGVkAAAAAAAAAAAAAAAABkZhaWxlZAAAAAAAAAAAAAAAAAAHRXhwaXJlZAA=",
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
        get_market: this.txFromJSON<MarketInfo>,
        get_position: this.txFromJSON<Position>,
        recapitalize: this.txFromJSON<null>,
        bump_position: this.txFromJSON<null>,
        enable_market: this.txFromJSON<null>,
        execute_order: this.txFromJSON<null>,
        global_config: this.txFromJSON<GlobalConfig>,
        non_lp_claims: this.txFromJSON<i128>,
        open_position: this.txFromJSON<u64>,
        active_markets: this.txFromJSON<Array<string>>,
        cancel_upgrade: this.txFromJSON<null>,
        claim_protocol: this.txFromJSON<null>,
        disable_market: this.txFromJSON<null>,
        update_indices: this.txFromJSON<null>,
        propose_upgrade: this.txFromJSON<null>,
        decrease_position: this.txFromJSON<null>,
        increase_position: this.txFromJSON<null>,
        set_global_config: this.txFromJSON<null>,
        set_market_config: this.txFromJSON<null>,
        is_market_disabled: this.txFromJSON<boolean>,
        liquidate_position: this.txFromJSON<null>,
        accounting_snapshot: this.txFromJSON<AccountingSnapshot>,
        deleverage_position: this.txFromJSON<null>,
        prepare_lp_snapshot: this.txFromJSON<AccountingSnapshot>,
        refresh_borrow_rate: this.txFromJSON<null>,
        can_create_lp_request: this.txFromJSON<boolean>,
        fund_execution_budget: this.txFromJSON<null>,
        protocol_claimable_total: this.txFromJSON<i128>,
        risk_keeper_reserve_total: this.txFromJSON<i128>,
        withdraw_execution_budget: this.txFromJSON<null>,
        pending_receiver_funding_total: this.txFromJSON<i128>
  }
}