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




export const OracleRouterError = {
  1: {message:"AlreadyInitialized"},
  2: {message:"NotInitialized"},
  3: {message:"Unauthorized"},
  /**
   * Every oracle source returned data older than `staleness_threshold`,
   * or returned invalid (zero/negative) prices, or a future timestamp.
   */
  4: {message:"StalePrice"},
  /**
   * Spread between source prices exceeds `max_deviation_bps`.
   */
  5: {message:"PriceDeviationTooHigh"},
  /**
   * No SEP-40 oracle sources are configured for the requested symbol.
   */
  6: {message:"NoPriceSources"},
  /**
   * Cross-contract call to an oracle source failed.
   */
  7: {message:"PriceFetchFailed"},
  /**
   * Oracle configuration field is invalid (e.g., zero threshold, out-of-range bps).
   */
  8: {message:"InvalidConfig"},
  /**
   * Fewer than `min_required_sources` valid prices were returned.
   */
  9: {message:"InsufficientSources"},
  /**
   * `set_oracle_sources` called with more than `MAX_ORACLE_SOURCES` entries.
   */
  10: {message:"TooManySources"},
  /**
   * Deviation math would overflow on the supplied prices.
   */
  11: {message:"DeviationOverflow"},
  /**
   * `upgrade` rejected — no `propose_upgrade` was made before commit.
   */
  12: {message:"NoPendingUpgrade"},
  /**
   * `upgrade` rejected — timelock has not elapsed yet.
   */
  13: {message:"UpgradeTimelockNotElapsed"},
  /**
   * `upgrade` rejected — `new_wasm_hash` does not match the proposed
   * `PendingUpgrade.wasm_hash`.
   */
  14: {message:"UpgradeHashMismatch"},
  /**
   * A source's `decimals()` differs from `shared::constants::PRICE_DECIMALS`,
   * or the source could not be queried for its scale.
   */
  15: {message:"InvalidSourceDecimals"},
  /**
   * Even-count median averaging would overflow on the supplied prices.
   */
  16: {message:"MedianOverflow"},
  17: {message:"PositionManagerAlreadySet"},
  18: {message:"PositionManagerNotSet"},
  19: {message:"RoundNotFound"}
}




export type StorageKey = {tag: "Initialized", values: void} | {tag: "ConfigManager", values: void} | {tag: "OracleConfig", values: void} | {tag: "ConfigVersion", values: void} | {tag: "Sources", values: readonly [string]} | {tag: "CachedPrice", values: readonly [string]} | {tag: "CachedPriceV2", values: readonly [string, u64]} | {tag: "Version", values: void} | {tag: "PositionManager", values: void} | {tag: "LatestRoundId", values: void} | {tag: "Round", values: readonly [u64]};


/**
 * Cached aggregated median price for a symbol. `fetched_at` bounds router
 * cache duration; `oldest_source_update` ensures the cached median is not
 * served after any source response used to compute it has gone stale.
 */
export interface CachedPrice {
  fetched_at: u64;
  oldest_source_update: u64;
  price: i128;
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
 * `shared::upgrade::pending_upgrade_key`). `upgrade` refuses to install
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
   * Construct and simulate a migrate transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  migrate: ({migration_data, operator}: {migration_data: MigrationData, operator: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a upgrade transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  upgrade: ({new_wasm_hash, operator}: {new_wasm_hash: Buffer, operator: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a get_price transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_price: ({symbol}: {symbol: string}, options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a get_round transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_round: ({round_id}: {round_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<OracleRound>>

  /**
   * Construct and simulate a publish_round transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  publish_round: ({caller}: {caller: string}, options?: MethodOptions) => Promise<AssembledTransaction<u64>>

  /**
   * Construct and simulate a cancel_upgrade transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  cancel_upgrade: ({caller}: {caller: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a latest_round_id transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  latest_round_id: (options?: MethodOptions) => Promise<AssembledTransaction<u64>>

  /**
   * Construct and simulate a propose_upgrade transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  propose_upgrade: ({caller, wasm_hash}: {caller: string, wasm_hash: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a bump_oracle_state transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  bump_oracle_state: (options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a get_oracle_config transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_oracle_config: (options?: MethodOptions) => Promise<AssembledTransaction<OracleConfig>>

  /**
   * Construct and simulate a set_oracle_config transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  set_oracle_config: ({caller, config}: {caller: string, config: OracleConfig}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a set_oracle_sources transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  set_oracle_sources: ({caller, symbol, sources}: {caller: string, symbol: string, sources: Array<string>}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a set_position_manager transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  set_position_manager: ({caller, position_manager}: {caller: string, position_manager: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

}
export class Client extends ContractClient {
  static async deploy<T = Client>(
        /** Constructor/Initialization Args for the contract's `__constructor` method */
        {config_manager_address}: {config_manager_address: string},
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
    return ContractClient.deploy({config_manager_address}, options)
  }
  constructor(public readonly options: ContractClientOptions) {
    super(
      new ContractSpec([ "AAAABAAAAAAAAAAAAAAAEU9yYWNsZVJvdXRlckVycm9yAAAAAAAAEwAAAAAAAAASQWxyZWFkeUluaXRpYWxpemVkAAAAAAABAAAAAAAAAA5Ob3RJbml0aWFsaXplZAAAAAAAAgAAAAAAAAAMVW5hdXRob3JpemVkAAAAAwAAAIZFdmVyeSBvcmFjbGUgc291cmNlIHJldHVybmVkIGRhdGEgb2xkZXIgdGhhbiBgc3RhbGVuZXNzX3RocmVzaG9sZGAsCm9yIHJldHVybmVkIGludmFsaWQgKHplcm8vbmVnYXRpdmUpIHByaWNlcywgb3IgYSBmdXR1cmUgdGltZXN0YW1wLgAAAAAAClN0YWxlUHJpY2UAAAAAAAQAAAA5U3ByZWFkIGJldHdlZW4gc291cmNlIHByaWNlcyBleGNlZWRzIGBtYXhfZGV2aWF0aW9uX2Jwc2AuAAAAAAAAFVByaWNlRGV2aWF0aW9uVG9vSGlnaAAAAAAAAAUAAABBTm8gU0VQLTQwIG9yYWNsZSBzb3VyY2VzIGFyZSBjb25maWd1cmVkIGZvciB0aGUgcmVxdWVzdGVkIHN5bWJvbC4AAAAAAAAOTm9QcmljZVNvdXJjZXMAAAAAAAYAAAAvQ3Jvc3MtY29udHJhY3QgY2FsbCB0byBhbiBvcmFjbGUgc291cmNlIGZhaWxlZC4AAAAAEFByaWNlRmV0Y2hGYWlsZWQAAAAHAAAAT09yYWNsZSBjb25maWd1cmF0aW9uIGZpZWxkIGlzIGludmFsaWQgKGUuZy4sIHplcm8gdGhyZXNob2xkLCBvdXQtb2YtcmFuZ2UgYnBzKS4AAAAADUludmFsaWRDb25maWcAAAAAAAAIAAAAPUZld2VyIHRoYW4gYG1pbl9yZXF1aXJlZF9zb3VyY2VzYCB2YWxpZCBwcmljZXMgd2VyZSByZXR1cm5lZC4AAAAAAAATSW5zdWZmaWNpZW50U291cmNlcwAAAAAJAAAASGBzZXRfb3JhY2xlX3NvdXJjZXNgIGNhbGxlZCB3aXRoIG1vcmUgdGhhbiBgTUFYX09SQUNMRV9TT1VSQ0VTYCBlbnRyaWVzLgAAAA5Ub29NYW55U291cmNlcwAAAAAACgAAADVEZXZpYXRpb24gbWF0aCB3b3VsZCBvdmVyZmxvdyBvbiB0aGUgc3VwcGxpZWQgcHJpY2VzLgAAAAAAABFEZXZpYXRpb25PdmVyZmxvdwAAAAAAAAsAAABDYHVwZ3JhZGVgIHJlamVjdGVkIOKAlCBubyBgcHJvcG9zZV91cGdyYWRlYCB3YXMgbWFkZSBiZWZvcmUgY29tbWl0LgAAAAAQTm9QZW5kaW5nVXBncmFkZQAAAAwAAAA0YHVwZ3JhZGVgIHJlamVjdGVkIOKAlCB0aW1lbG9jayBoYXMgbm90IGVsYXBzZWQgeWV0LgAAABlVcGdyYWRlVGltZWxvY2tOb3RFbGFwc2VkAAAAAAAADQAAAF5gdXBncmFkZWAgcmVqZWN0ZWQg4oCUIGBuZXdfd2FzbV9oYXNoYCBkb2VzIG5vdCBtYXRjaCB0aGUgcHJvcG9zZWQKYFBlbmRpbmdVcGdyYWRlLndhc21faGFzaGAuAAAAAAATVXBncmFkZUhhc2hNaXNtYXRjaAAAAAAOAAAAe0Egc291cmNlJ3MgYGRlY2ltYWxzKClgIGRpZmZlcnMgZnJvbSBgc2hhcmVkOjpjb25zdGFudHM6OlBSSUNFX0RFQ0lNQUxTYCwKb3IgdGhlIHNvdXJjZSBjb3VsZCBub3QgYmUgcXVlcmllZCBmb3IgaXRzIHNjYWxlLgAAAAAVSW52YWxpZFNvdXJjZURlY2ltYWxzAAAAAAAADwAAAEJFdmVuLWNvdW50IG1lZGlhbiBhdmVyYWdpbmcgd291bGQgb3ZlcmZsb3cgb24gdGhlIHN1cHBsaWVkIHByaWNlcy4AAAAAAA5NZWRpYW5PdmVyZmxvdwAAAAAAEAAAAAAAAAAZUG9zaXRpb25NYW5hZ2VyQWxyZWFkeVNldAAAAAAAABEAAAAAAAAAFVBvc2l0aW9uTWFuYWdlck5vdFNldAAAAAAAABIAAAAAAAAADVJvdW5kTm90Rm91bmQAAAAAAAAT",
        "AAAABQAAAAAAAAAAAAAAClByaWNlRmV0Y2gAAAAAAAEAAAAFcHJpY2UAAAAAAAADAAAAAAAAAAZzeW1ib2wAAAAAABEAAAABAAAAAAAAAAVwcmljZQAAAAAAAAsAAAAAAAAAAAAAAAl0aW1lc3RhbXAAAAAAAAAGAAAAAAAAAAE=",
        "AAAABQAAAIdFbWl0dGVkIGJ5IGBzZXRfb3JhY2xlX2NvbmZpZ2Agd2hlbmV2ZXIgdGhlIGdsb2JhbCBzYWZldHkgdGhyZXNob2xkcwpjaGFuZ2UuIE1pcnJvcnMgZXZlcnkgZmllbGQgb2YgdGhlIG9uLWNoYWluIGBPcmFjbGVDb25maWdgIHN0cnVjdC4AAAAAAAAAABJPcmFjbGVDb25maWdVcGRhdGUAAAAAAAEAAAAGb3JjY2ZnAAAAAAAEAAAAAAAAAAlzdGFsZW5lc3MAAAAAAAAGAAAAAAAAAAAAAAAJZGV2aWF0aW9uAAAAAAAACwAAAAAAAAAAAAAADmNhY2hlX2R1cmF0aW9uAAAAAAAGAAAAAAAAAAAAAAAUbWluX3JlcXVpcmVkX3NvdXJjZXMAAAAEAAAAAAAAAAE=",
        "AAAABQAAAGRFbWl0dGVkIGJ5IGBzZXRfb3JhY2xlX3NvdXJjZXNgIHNvIG9mZi1jaGFpbiBtb25pdG9yaW5nIGNhbiBkZXRlY3QgZXZlcnkKcm90YXRpb24gb2YgdGhlIHNvdXJjZSBzZXQuAAAAAAAAABNPcmFjbGVTb3VyY2VzVXBkYXRlAAAAAAEAAAAGb3Jjc3JjAAAAAAACAAAAAAAAAAZzeW1ib2wAAAAAABEAAAABAAAAAAAAAAdzb3VyY2VzAAAAA+oAAAATAAAAAAAAAAE=",
        "AAAAAgAAAAAAAAAAAAAAClN0b3JhZ2VLZXkAAAAAAAsAAAAAAAAAFEluaXRpYWxpemF0aW9uIGZsYWcuAAAAC0luaXRpYWxpemVkAAAAAAAAAAAdTGlua2VkIENvbmZpZ01hbmFnZXIgYWRkcmVzcy4AAAAAAAANQ29uZmlnTWFuYWdlcgAAAAAAAAAAAAAcR2xvYmFsIG9yYWNsZSBjb25maWd1cmF0aW9uLgAAAAxPcmFjbGVDb25maWcAAAAAAAAAlk1vbm90b25pYyB2ZXJzaW9uIGJ1bXBlZCBvbiBldmVyeSBjb25maWcgdXBkYXRlLiBDYWNoZSBrZXlzIGluY2x1ZGUKdGhpcyB2YWx1ZSBzbyBjb25maWcgY2hhbmdlcyBpbnZhbGlkYXRlIG9sZCBtZWRpYW5zIHdpdGhvdXQgc2Nhbm5pbmcKZXZlcnkgc3ltYm9sLgAAAAAADUNvbmZpZ1ZlcnNpb24AAAAAAAABAAAAO1Blci1zeW1ib2wgZmxhdCBzb3VyY2UgbGlzdCAobm8gcHJpbWFyeS9zZWNvbmRhcnkgdGllcmluZykuAAAAAAdTb3VyY2VzAAAAAAEAAAARAAAAAQAAAIBMZWdhY3kgcGVyLXN5bWJvbCBjYWNoZWQgYWdncmVnYXRlZCBwcmljZSBrZXkuIEtlcHQgc28gdXBncmFkZWQKZGVwbG95bWVudHMgZG8gbm90IHRyeSB0byBkZWNvZGUgb2xkIGNhY2hlIGVudHJpZXMgYXMgVjIgdmFsdWVzLgAAAAtDYWNoZWRQcmljZQAAAAABAAAAEQAAAAEAAABBUGVyLXN5bWJvbCBjYWNoZWQgYWdncmVnYXRlZCBwcmljZSBmb3IgdGhlIGFjdGl2ZSBjb25maWcgdmVyc2lvbi4AAAAAAAANQ2FjaGVkUHJpY2VWMgAAAAAAAAIAAAARAAAABgAAAAAAAABIQ3VycmVudCBjb250cmFjdCB2ZXJzaW9uIOKAlCB3cml0dGVuIGJ5IGBfbWlncmF0ZWAgYWZ0ZXIgYSBXQVNNIHVwZ3JhZGUuAAAAB1ZlcnNpb24AAAAAAAAAAAAAAAAPUG9zaXRpb25NYW5hZ2VyAAAAAAAAAAAAAAAADUxhdGVzdFJvdW5kSWQAAAAAAAABAAAAAAAAAAVSb3VuZAAAAAAAAAEAAAAG",
        "AAAAAQAAANNDYWNoZWQgYWdncmVnYXRlZCBtZWRpYW4gcHJpY2UgZm9yIGEgc3ltYm9sLiBgZmV0Y2hlZF9hdGAgYm91bmRzIHJvdXRlcgpjYWNoZSBkdXJhdGlvbjsgYG9sZGVzdF9zb3VyY2VfdXBkYXRlYCBlbnN1cmVzIHRoZSBjYWNoZWQgbWVkaWFuIGlzIG5vdApzZXJ2ZWQgYWZ0ZXIgYW55IHNvdXJjZSByZXNwb25zZSB1c2VkIHRvIGNvbXB1dGUgaXQgaGFzIGdvbmUgc3RhbGUuAAAAAAAAAAALQ2FjaGVkUHJpY2UAAAAAAwAAAAAAAAAKZmV0Y2hlZF9hdAAAAAAABgAAAAAAAAAUb2xkZXN0X3NvdXJjZV91cGRhdGUAAAAGAAAAAAAAAAVwcmljZQAAAAAAAAs=",
        "AAAAAAAAAAAAAAAHbWlncmF0ZQAAAAACAAAAAAAAAA5taWdyYXRpb25fZGF0YQAAAAAH0AAAAA1NaWdyYXRpb25EYXRhAAAAAAAAAAAAAAhvcGVyYXRvcgAAABMAAAAA",
        "AAAAAAAAAAAAAAAHdXBncmFkZQAAAAACAAAAAAAAAA1uZXdfd2FzbV9oYXNoAAAAAAAD7gAAACAAAAAAAAAACG9wZXJhdG9yAAAAEwAAAAA=",
        "AAAAAAAAAAAAAAAJZ2V0X3ByaWNlAAAAAAAAAQAAAAAAAAAGc3ltYm9sAAAAAAARAAAAAQAAAAs=",
        "AAAAAAAAAAAAAAAJZ2V0X3JvdW5kAAAAAAAAAQAAAAAAAAAIcm91bmRfaWQAAAAGAAAAAQAAB9AAAAALT3JhY2xlUm91bmQA",
        "AAAAAAAAATZBdG9taWMtd2l0aC1kZXBsb3kgaW5pdGlhbGl6YXRpb24gKFNvcm9iYW4gY29uc3RydWN0b3IpLiBCaW5kcyB0aGUKbGlua2VkIENvbmZpZ01hbmFnZXIgb25jZSwgaW5zaWRlIHRoZSBkZXBsb3kgdHJhbnNhY3Rpb24sIHNvIG5vIHRoaXJkCnBhcnR5IGNhbiBmcm9udC1ydW4gaW5pdCBhbmQgcG9pbnQgdGhlIHJvdXRlciBhdCBhIG1hbGljaW91cyByb2xlCmF1dGhvcml0eS4gVGhlIHJvdXRlciBzdG9yZXMgbm8gYWRtaW4gb2YgaXRzIG93biDigJQgZXZlcnkgcm9sZSBjaGVjawpjcm9zcy1jYWxscyB0aGUgbGlua2VkIENvbmZpZ01hbmFnZXIuAAAAAAANX19jb25zdHJ1Y3RvcgAAAAAAAAEAAAAAAAAAFmNvbmZpZ19tYW5hZ2VyX2FkZHJlc3MAAAAAABMAAAAA",
        "AAAAAAAAAAAAAAANcHVibGlzaF9yb3VuZAAAAAAAAAEAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAEAAAAG",
        "AAAAAAAAAAAAAAAOY2FuY2VsX3VwZ3JhZGUAAAAAAAEAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAA=",
        "AAAAAAAAAAAAAAAPbGF0ZXN0X3JvdW5kX2lkAAAAAAAAAAABAAAABg==",
        "AAAAAAAAAAAAAAAPcHJvcG9zZV91cGdyYWRlAAAAAAIAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAAAAAAJd2FzbV9oYXNoAAAAAAAD7gAAACAAAAAA",
        "AAAAAAAAAAAAAAARYnVtcF9vcmFjbGVfc3RhdGUAAAAAAAAAAAAAAA==",
        "AAAAAAAAAAAAAAARZ2V0X29yYWNsZV9jb25maWcAAAAAAAAAAAAAAQAAB9AAAAAMT3JhY2xlQ29uZmln",
        "AAAAAAAAAAAAAAARc2V0X29yYWNsZV9jb25maWcAAAAAAAACAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAAAAAABmNvbmZpZwAAAAAH0AAAAAxPcmFjbGVDb25maWcAAAAA",
        "AAAAAAAAAAAAAAASc2V0X29yYWNsZV9zb3VyY2VzAAAAAAADAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAAAAAABnN5bWJvbAAAAAAAEQAAAAAAAAAHc291cmNlcwAAAAPqAAAAEwAAAAA=",
        "AAAAAAAAAAAAAAAUc2V0X3Bvc2l0aW9uX21hbmFnZXIAAAACAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAAAAAAEHBvc2l0aW9uX21hbmFnZXIAAAATAAAAAA==",
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
    migrate: this.txFromJSON<null>,
        upgrade: this.txFromJSON<null>,
        get_price: this.txFromJSON<i128>,
        get_round: this.txFromJSON<OracleRound>,
        publish_round: this.txFromJSON<u64>,
        cancel_upgrade: this.txFromJSON<null>,
        latest_round_id: this.txFromJSON<u64>,
        propose_upgrade: this.txFromJSON<null>,
        bump_oracle_state: this.txFromJSON<null>,
        get_oracle_config: this.txFromJSON<OracleConfig>,
        set_oracle_config: this.txFromJSON<null>,
        set_oracle_sources: this.txFromJSON<null>,
        set_position_manager: this.txFromJSON<null>
  }
}
