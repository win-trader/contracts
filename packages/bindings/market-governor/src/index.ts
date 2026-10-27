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




export const MarketGovernorError = {
  500: {message:"Unauthorized"},
  510: {message:"NoPendingConfig"},
  520: {message:"ConfigTimelockNotElapsed"},
  521: {message:"ConfigProposalExpired"},
  530: {message:"InvalidConfig"},
  531: {message:"PriceUnavailable"},
  540: {message:"UpgradeNoPending"},
  541: {message:"UpgradeTimelockNotElapsed"},
  542: {message:"UpgradeHashMismatch"}
}




export type Key = {tag: "ConfigManager", values: void} | {tag: "PositionManager", values: void} | {tag: "PendingGlobalConfig", values: void} | {tag: "PendingMarketConfig", values: readonly [string]} | {tag: "PendingPriceFeed", values: void} | {tag: "Version", values: void};

/**
 * SEP-40 asset identifier. Markets are quoted as `Other(market symbol)`.
 */
export type Asset = {tag: "Stellar", values: readonly [string]} | {tag: "Other", values: readonly [string]};


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
  /**
 * The resolve reward fixed when the request was made.
 */
reward: i128;
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
   * Construct and simulate a migrate transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  migrate: ({data, operator}: {data: MigrationData, operator: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a upgrade transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  upgrade: ({new_wasm_hash, operator}: {new_wasm_hash: Buffer, operator: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a cancel_upgrade transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  cancel_upgrade: ({caller}: {caller: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a propose_upgrade transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  propose_upgrade: ({caller, wasm_hash}: {caller: string, wasm_hash: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a apply_price_feed transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  apply_price_feed: ({caller}: {caller: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a position_manager transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  position_manager: (options?: MethodOptions) => Promise<AssembledTransaction<string>>

  /**
   * Construct and simulate a cancel_price_feed transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  cancel_price_feed: ({caller}: {caller: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a deregister_market transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  deregister_market: ({caller, market_symbol}: {caller: string, market_symbol: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a propose_price_feed transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  propose_price_feed: ({caller, price_feed}: {caller: string, price_feed: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a apply_global_config transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  apply_global_config: ({caller}: {caller: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a apply_market_config transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  apply_market_config: ({caller, market_symbol}: {caller: string, market_symbol: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a cancel_global_config transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  cancel_global_config: ({caller}: {caller: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a cancel_market_config transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  cancel_market_config: ({caller, market_symbol}: {caller: string, market_symbol: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a propose_global_config transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  propose_global_config: ({caller, config}: {caller: string, config: GlobalConfig}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a propose_market_config transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  propose_market_config: ({caller, market_symbol, config}: {caller: string, market_symbol: string, config: MarketConfig}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

}
export class Client extends ContractClient {
  static async deploy<T = Client>(
        /** Constructor/Initialization Args for the contract's `__constructor` method */
        {config_manager, position_manager}: {config_manager: string, position_manager: string},
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
    return ContractClient.deploy({config_manager, position_manager}, options)
  }
  constructor(public readonly options: ContractClientOptions) {
    super(
      new ContractSpec([ "AAAABAAAAAAAAAAAAAAAE01hcmtldEdvdmVybm9yRXJyb3IAAAAACQAAAAAAAAAMVW5hdXRob3JpemVkAAAB9AAAAAAAAAAPTm9QZW5kaW5nQ29uZmlnAAAAAf4AAAAAAAAAGENvbmZpZ1RpbWVsb2NrTm90RWxhcHNlZAAAAggAAAAAAAAAFUNvbmZpZ1Byb3Bvc2FsRXhwaXJlZAAAAAAAAgkAAAAAAAAADUludmFsaWRDb25maWcAAAAAAAISAAAAAAAAABBQcmljZVVuYXZhaWxhYmxlAAACEwAAAAAAAAAQVXBncmFkZU5vUGVuZGluZwAAAhwAAAAAAAAAGVVwZ3JhZGVUaW1lbG9ja05vdEVsYXBzZWQAAAAAAAIdAAAAAAAAABNVcGdyYWRlSGFzaE1pc21hdGNoAAAAAh4=",
        "AAAABQAAAAAAAAAAAAAAEVByaWNlRmVlZFByb3Bvc2VkAAAAAAAAAQAAAAhmZWVkcHJvcAAAAAMAAAAAAAAABmhlYWRlcgAAAAAH0AAAAAtFdmVudEhlYWRlcgAAAAAAAAAAAAAAAApwcmljZV9mZWVkAAAAAAATAAAAAAAAAAAAAAAMZWZmZWN0aXZlX2F0AAAABgAAAAAAAAAB",
        "AAAABQAAAAAAAAAAAAAAEVByb3Bvc2FsQ2FuY2VsbGVkAAAAAAAAAQAAAAljZmdjYW5jZWwAAAAAAAADAAAAAAAAAAZoZWFkZXIAAAAAB9AAAAALRXZlbnRIZWFkZXIAAAAAAAAAAAAAAAAGbWFya2V0AAAAAAPoAAAAEQAAAAAAAAAAAAAACnByaWNlX2ZlZWQAAAAAAAEAAAAAAAAAAQ==",
        "AAAABQAAAAAAAAAAAAAAFUNvbmZpZ3VyYXRpb25Qcm9wb3NlZAAAAAAAAAEAAAAHY2ZncHJvcAAAAAADAAAAAAAAAAZoZWFkZXIAAAAAB9AAAAALRXZlbnRIZWFkZXIAAAAAAAAAAAAAAAAGbWFya2V0AAAAAAPoAAAAEQAAAAAAAAAAAAAADGVmZmVjdGl2ZV9hdAAAAAYAAAAAAAAAAQ==",
        "AAAAAgAAAAAAAAAAAAAAA0tleQAAAAAGAAAAAAAAAAAAAAANQ29uZmlnTWFuYWdlcgAAAAAAAAAAAAAAAAAAD1Bvc2l0aW9uTWFuYWdlcgAAAAAAAAAAAAAAABNQZW5kaW5nR2xvYmFsQ29uZmlnAAAAAAEAAAAAAAAAE1BlbmRpbmdNYXJrZXRDb25maWcAAAAAAQAAABEAAAAAAAAAAAAAABBQZW5kaW5nUHJpY2VGZWVkAAAAAAAAAAAAAAAHVmVyc2lvbgA=",
        "AAAAAAAAAAAAAAAHbWlncmF0ZQAAAAACAAAAAAAAAARkYXRhAAAH0AAAAA1NaWdyYXRpb25EYXRhAAAAAAAAAAAAAAhvcGVyYXRvcgAAABMAAAAA",
        "AAAAAAAAAAAAAAAHdXBncmFkZQAAAAACAAAAAAAAAA1uZXdfd2FzbV9oYXNoAAAAAAAD7gAAACAAAAAAAAAACG9wZXJhdG9yAAAAEwAAAAA=",
        "AAAAAAAAAAAAAAANX19jb25zdHJ1Y3RvcgAAAAAAAAIAAAAAAAAADmNvbmZpZ19tYW5hZ2VyAAAAAAATAAAAAAAAABBwb3NpdGlvbl9tYW5hZ2VyAAAAEwAAAAA=",
        "AAAAAAAAAAAAAAAOY2FuY2VsX3VwZ3JhZGUAAAAAAAEAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAA=",
        "AAAAAAAAAAAAAAAPcHJvcG9zZV91cGdyYWRlAAAAAAIAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAAAAAAJd2FzbV9oYXNoAAAAAAAD7gAAACAAAAAA",
        "AAAAAAAAAAAAAAAQYXBwbHlfcHJpY2VfZmVlZAAAAAEAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAA=",
        "AAAAAAAAAAAAAAAQcG9zaXRpb25fbWFuYWdlcgAAAAAAAAABAAAAEw==",
        "AAAAAAAAAAAAAAARY2FuY2VsX3ByaWNlX2ZlZWQAAAAAAAABAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAA",
        "AAAAAAAAAAAAAAARZGVyZWdpc3Rlcl9tYXJrZXQAAAAAAAACAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAAAAAADW1hcmtldF9zeW1ib2wAAAAAAAARAAAAAA==",
        "AAAAAAAAAAAAAAAScHJvcG9zZV9wcmljZV9mZWVkAAAAAAACAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAAAAAACnByaWNlX2ZlZWQAAAAAABMAAAAA",
        "AAAAAAAAAAAAAAATYXBwbHlfZ2xvYmFsX2NvbmZpZwAAAAABAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAA",
        "AAAAAAAAAAAAAAATYXBwbHlfbWFya2V0X2NvbmZpZwAAAAACAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAAAAAADW1hcmtldF9zeW1ib2wAAAAAAAARAAAAAA==",
        "AAAAAAAAAAAAAAAUY2FuY2VsX2dsb2JhbF9jb25maWcAAAABAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAA",
        "AAAAAAAAAAAAAAAUY2FuY2VsX21hcmtldF9jb25maWcAAAACAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAAAAAADW1hcmtldF9zeW1ib2wAAAAAAAARAAAAAA==",
        "AAAAAAAAAAAAAAAVcHJvcG9zZV9nbG9iYWxfY29uZmlnAAAAAAAAAgAAAAAAAAAGY2FsbGVyAAAAAAATAAAAAAAAAAZjb25maWcAAAAAB9AAAAAMR2xvYmFsQ29uZmlnAAAAAA==",
        "AAAAAAAAAAAAAAAVcHJvcG9zZV9tYXJrZXRfY29uZmlnAAAAAAAAAwAAAAAAAAAGY2FsbGVyAAAAAAATAAAAAAAAAA1tYXJrZXRfc3ltYm9sAAAAAAAAEQAAAAAAAAAGY29uZmlnAAAAAAfQAAAADE1hcmtldENvbmZpZwAAAAA=",
        "AAAAAgAAAEZTRVAtNDAgYXNzZXQgaWRlbnRpZmllci4gTWFya2V0cyBhcmUgcXVvdGVkIGFzIGBPdGhlcihtYXJrZXQgc3ltYm9sKWAuAAAAAAAAAAAABUFzc2V0AAAAAAAAAgAAAAEAAAAAAAAAB1N0ZWxsYXIAAAAAAQAAABMAAAABAAAAAAAAAAVPdGhlcgAAAAAAAAEAAAAR",
        "AAAAAQAAAAAAAAAAAAAACVByaWNlRGF0YQAAAAAAAAIAAAAAAAAABXByaWNlAAAAAAAACwAAAAAAAAAJdGltZXN0YW1wAAAAAAAABg==",
        "AAAAAQAAAAAAAAAAAAAADFN0YW1wZWRQcmljZQAAAAIAAAAAAAAAC29ic2VydmVkX2F0AAAAAAYAAAAAAAAABXByaWNlAAAAAAAACw==",
        "AAAAAQAAAAAAAAAAAAAABk1hcmtldAAAAAAAEAAAAAAAAAAGY29uZmlnAAAAAAfQAAAADE1hcmtldENvbmZpZwAAAAAAAAASY3VycmVudF9wYXllcl9yYXRlAAAAAAALAAAAAAAAABJjdXJyZW50X3BheWVyX3NpZGUAAAAAB9AAAAAJUGF5ZXJTaWRlAAAAAAAAAAAAABdsYXN0X2Z1bmRpbmdfY2hlY2twb2ludAAAAAAGAAAAAAAAAARsb25nAAAH0AAAAApNYXJrZXRTaWRlAAAAAAAAAAAAFWxvbmdfcGF5ZXJfcmVtYWluZGVycwAAAAAAB9AAAAAOUmVtYWluZGVyR3JvdXAAAAAAAAAAAAAUbHBfYmFja2VkX2luZGV4X2xvbmcAAAALAAAAAAAAABVscF9iYWNrZWRfaW5kZXhfc2hvcnQAAAAAAAALAAAAAAAAABhwZW5kaW5nX3JlY2VpdmVyX2Z1bmRpbmcAAAALAAAAAAAAABpyZWNlaXZlcl9iYWNrZWRfaW5kZXhfbG9uZwAAAAAACwAAAAAAAAAbcmVjZWl2ZXJfYmFja2VkX2luZGV4X3Nob3J0AAAAAAsAAAAAAAAAE3JlY2VpdmVyX2luZGV4X2xvbmcAAAAACwAAAAAAAAAUcmVjZWl2ZXJfaW5kZXhfc2hvcnQAAAALAAAAAAAAAAVzaG9ydAAAAAAAB9AAAAAKTWFya2V0U2lkZQAAAAAAAAAAABZzaG9ydF9wYXllcl9yZW1haW5kZXJzAAAAAAfQAAAADlJlbWFpbmRlckdyb3VwAAAAAAAAAAAACHNrZXdfZW1hAAAACw==",
        "AAAAAgAAAAAAAAAAAAAAB1RyaWdnZXIAAAAAAgAAAAAAAAAAAAAABE5vbmUAAAABAAAAAAAAAAhBdHRhY2hlZAAAAAEAAAfQAAAAElRyaWdnZXJJbnN0cnVjdGlvbgAA",
        "AAAAAQAAAAAAAAAAAAAACExwQ29uZmlnAAAAAwAAAAAAAAAYbHBfcmVxdWVzdF9kZWxheV9zZWNvbmRzAAAABgAAAAAAAAAcbWF4X3dpdGhkcmF3X3V0aWxpemF0aW9uX2JwcwAAAAQAAAAAAAAAGm1pbl9kZXBvc2l0X25hdl9mYWN0b3JfYnBzAAAAAAAE",
        "AAAAAQAAAAAAAAAAAAAACFBvc2l0aW9uAAAAEQAAAAAAAAANYmFzZV9leHBvc3VyZQAAAAAAAAsAAAAAAAAAFWJvcnJvd19pbmRleF9zbmFwc2hvdAAAAAAAAAsAAAAAAAAAAmlkAAAAAAAGAAAAAAAAAAdpc19sb25nAAAAAAEAAAAAAAAAF2xwX3BheWVyX2luZGV4X3NuYXBzaG90AAAAAAsAAAAAAAAABm1hcmtldAAAAAAAEQAAAAAAAAAJb3BlbmVkX2F0AAAAAAAABgAAAAAAAAAFb3duZXIAAAAAAAATAAAAAAAAABpwZW5kaW5nX211dGF0aW9uX2FjdGlvbl9pZAAAAAAD6AAAAAYAAAAAAAAAF3JlY2VpdmVyX2luZGV4X3NuYXBzaG90AAAAAAsAAAAAAAAAHXJlY2VpdmVyX3BheWVyX2luZGV4X3NuYXBzaG90AAAAAAAACwAAAAAAAAAKcmlza191bml0cwAAAAAACwAAAAAAAAAEc2l6ZQAAAAsAAAAAAAAACXN0b3BfbG9zcwAAAAAAB9AAAAAHVHJpZ2dlcgAAAAAAAAAAEXN0b3JlZF9jb2xsYXRlcmFsAAAAAAAACwAAAAAAAAAZc3RvcmVkX21pbmltdW1fYm9ycm93X2ZlZQAAAAAAAAsAAAAAAAAAC3Rha2VfcHJvZml0AAAAB9AAAAAHVHJpZ2dlcgA=",
        "AAAAAQAAAAAAAAAAAAAACUxwUmVxdWVzdAAAAAAAAAgAAAAAAAAABmFtb3VudAAAAAAACwAAAAAAAAANZXhlY3V0ZV9hZnRlcgAAAAAAAAYAAAAAAAAAAmlkAAAAAAAGAAAAAAAAAARraW5kAAAH0AAAAA1McFJlcXVlc3RLaW5kAAAAAAAAAAAAAAVvd25lcgAAAAAAABMAAAAAAAAADHJlcXVlc3RfdGltZQAAAAYAAAAzVGhlIHJlc29sdmUgcmV3YXJkIGZpeGVkIHdoZW4gdGhlIHJlcXVlc3Qgd2FzIG1hZGUuAAAAAAZyZXdhcmQAAAAAAAsAAAAAAAAABnN0YXR1cwAAAAAH0AAAAA9McFJlcXVlc3RTdGF0dXMA",
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
        "AAAABQAAAEVBIG9uZS1zaG90IGNyb3NzLWNvbnRyYWN0IHdpcmluZyAoYHNldF92YXVsdGAsIGBzZXRfcmVxdWVzdF9yb3V0ZXJgKS4AAAAAAAAAAAAABVdpcmVkAAAAAAAAAQAAAAV3aXJlZAAAAAAAAAMAAAAAAAAABnRhcmdldAAAAAAAEQAAAAAAAAAAAAAAB2FkZHJlc3MAAAAAEwAAAAAAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAAAAAAB",
        "AAAABQAAACNBIGNvbXBsZXRlZCBwb3N0LXVwZ3JhZGUgbWlncmF0aW9uLgAAAAAAAAAACE1pZ3JhdGVkAAAAAQAAAAhtaWdyYXRlZAAAAAIAAAAAAAAAB3ZlcnNpb24AAAAABAAAAAAAAAAAAAAACG9wZXJhdG9yAAAAEwAAAAAAAAAB",
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
    migrate: this.txFromJSON<null>,
        upgrade: this.txFromJSON<null>,
        cancel_upgrade: this.txFromJSON<null>,
        propose_upgrade: this.txFromJSON<null>,
        apply_price_feed: this.txFromJSON<null>,
        position_manager: this.txFromJSON<string>,
        cancel_price_feed: this.txFromJSON<null>,
        deregister_market: this.txFromJSON<null>,
        propose_price_feed: this.txFromJSON<null>,
        apply_global_config: this.txFromJSON<null>,
        apply_market_config: this.txFromJSON<null>,
        cancel_global_config: this.txFromJSON<null>,
        cancel_market_config: this.txFromJSON<null>,
        propose_global_config: this.txFromJSON<null>,
        propose_market_config: this.txFromJSON<null>
  }
}