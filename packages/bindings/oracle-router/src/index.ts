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
  16: {message:"MedianOverflow"}
}




export type StorageKey = {tag: "Initialized", values: void} | {tag: "ConfigManager", values: void} | {tag: "OracleConfig", values: void} | {tag: "ConfigVersion", values: void} | {tag: "Sources", values: readonly [string]} | {tag: "CachedPrice", values: readonly [string]} | {tag: "CachedPriceV2", values: readonly [string, u64]} | {tag: "Version", values: void};


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


/**
 * Represents a single trader's open leveraged position.
 */
export interface Position {
  /**
 * Additive base quantity at `EXPOSURE_PRECISION`; authoritative for PnL.
 */
base_exposure: i128;
  /**
 * Baseline for `size * acc_borrow_index / INDEX_PRECISION`.
 */
borrow_fee_debt: i128;
  /**
 * USDC collateral deposited by the trader.
 */
collateral: i128;
  /**
 * Display-only harmonic entry price derived from size/base exposure.
 */
entry_price: i128;
  /**
 * Flat USDC fee escrowed when TP or SL is set. Paid to executor on trigger, refunded on user close / ADL, forfeited to revenue on liquidation.
 */
execution_fee_escrow: i128;
  /**
 * True for a long position, false for a short.
 */
is_long: boolean;
  /**
 * Block timestamp when the position was last increased (anti-front-running lock).
 */
last_increased_time: u64;
  /**
 * Notional size of the position in USDC.
 */
size: i128;
  /**
 * Baseline for `size * side_skew_index / INDEX_PRECISION`.
 */
skew_fee_debt: i128;
  /**
 * Stop-loss price (scaled by 1e7). 0 = not set.
 */
stop_loss: i128;
  /**
 * Take-profit price (scaled by 1e7). 0 = not set.
 */
take_profit: i128;
}


/**
 * Global market state for a single tradeable asset symbol.
 */
export interface MarketInfo {
  /**
 * Cumulative borrow fee index (grows monotonically with time).
 */
acc_borrow_index: i128;
  /**
 * Dominant-long skew carrying-fee index.
 */
acc_long_skew_index: i128;
  /**
 * Dominant-short skew carrying-fee index.
 */
acc_short_skew_index: i128;
  /**
 * Display-only harmonic entry price derived from long exposure.
 */
global_long_avg_price: i128;
  /**
 * Display-only harmonic entry price derived from short exposure.
 */
global_short_avg_price: i128;
  /**
 * Timestamp of the last keeper index update.
 */
last_index_update: u64;
  /**
 * Sum of long-position base exposure at `EXPOSURE_PRECISION`.
 */
long_base_exposure: i128;
  /**
 * Total notional size of all open long positions.
 */
long_open_interest: i128;
  /**
 * Sum of short-position base exposure at `EXPOSURE_PRECISION`.
 */
short_base_exposure: i128;
  /**
 * Total notional size of all open short positions.
 */
short_open_interest: i128;
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


/**
 * Data required during a WASM migration. Single definition for all contracts.
 */
export interface MigrationData {
  version: u32;
}


/**
 * Pending WASM upgrade — set by `propose_upgrade`, consumed by `upgrade`
 * (cleared atomically on a successful install), or cleared by `cancel_upgrade`.
 * Single shape across every protocol contract; all four contracts store it at
 * the shared `pending_upgrade` Symbol key in their own instance storage (see
 * `interfaces::upgrade::pending_upgrade_key`). `upgrade` refuses to install
 * unless `pending.wasm_hash` matches the supplied hash and `now >= eta`.
 */
export interface PendingUpgrade {
  eta: u64;
  wasm_hash: Buffer;
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


/**
 * Execution-bounty and open-fee parameters charged to traders.
 * `open_fee_bps` and `liquidation_bounty_bps` are in basis points;
 * `tp_sl_execution_fee` is a flat USDC amount at PRECISION scale.
 */
export interface FeeConfig {
  liquidation_bounty_bps: u32;
  open_fee_bps: u32;
  tp_sl_execution_fee: i128;
}


/**
 * Defines how protocol revenue is split between parties.
 * All values are in basis points (bps). Must sum to 10_000.
 */
export interface FeeSplits {
  dev_bps: u32;
  lp_bps: u32;
  staker_bps: u32;
}


/**
 * Global protocol risk and timing parameters.
 */
export interface ProtocolLimits {
  adl_pnl_bps: u32;
  adl_utilization_bps: u32;
  cooldown_duration: u64;
  liquidation_threshold_bps: u32;
  max_utilization_ratio: i128;
  min_collateral: i128;
  min_position_lifetime: u64;
}


/**
 * Time-based carrying-fee parameters (all rates in annualized basis points).
 */
export interface CarryingFeeConfig {
  base_borrow_rate_bps: i128;
  /**
 * Maximum skew surcharge at 100% concentration and 100% utilization.
 */
max_skew_rate_bps: i128;
  optimal_utilization_bps: i128;
  slope1_bps: i128;
  slope2_bps: i128;
}

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
   * Construct and simulate a cancel_upgrade transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  cancel_upgrade: ({caller}: {caller: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

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
      new ContractSpec([ "AAAABAAAAAAAAAAAAAAAEU9yYWNsZVJvdXRlckVycm9yAAAAAAAAEAAAAAAAAAASQWxyZWFkeUluaXRpYWxpemVkAAAAAAABAAAAAAAAAA5Ob3RJbml0aWFsaXplZAAAAAAAAgAAAAAAAAAMVW5hdXRob3JpemVkAAAAAwAAAIZFdmVyeSBvcmFjbGUgc291cmNlIHJldHVybmVkIGRhdGEgb2xkZXIgdGhhbiBgc3RhbGVuZXNzX3RocmVzaG9sZGAsCm9yIHJldHVybmVkIGludmFsaWQgKHplcm8vbmVnYXRpdmUpIHByaWNlcywgb3IgYSBmdXR1cmUgdGltZXN0YW1wLgAAAAAAClN0YWxlUHJpY2UAAAAAAAQAAAA5U3ByZWFkIGJldHdlZW4gc291cmNlIHByaWNlcyBleGNlZWRzIGBtYXhfZGV2aWF0aW9uX2Jwc2AuAAAAAAAAFVByaWNlRGV2aWF0aW9uVG9vSGlnaAAAAAAAAAUAAABBTm8gU0VQLTQwIG9yYWNsZSBzb3VyY2VzIGFyZSBjb25maWd1cmVkIGZvciB0aGUgcmVxdWVzdGVkIHN5bWJvbC4AAAAAAAAOTm9QcmljZVNvdXJjZXMAAAAAAAYAAAAvQ3Jvc3MtY29udHJhY3QgY2FsbCB0byBhbiBvcmFjbGUgc291cmNlIGZhaWxlZC4AAAAAEFByaWNlRmV0Y2hGYWlsZWQAAAAHAAAAT09yYWNsZSBjb25maWd1cmF0aW9uIGZpZWxkIGlzIGludmFsaWQgKGUuZy4sIHplcm8gdGhyZXNob2xkLCBvdXQtb2YtcmFuZ2UgYnBzKS4AAAAADUludmFsaWRDb25maWcAAAAAAAAIAAAAPUZld2VyIHRoYW4gYG1pbl9yZXF1aXJlZF9zb3VyY2VzYCB2YWxpZCBwcmljZXMgd2VyZSByZXR1cm5lZC4AAAAAAAATSW5zdWZmaWNpZW50U291cmNlcwAAAAAJAAAASGBzZXRfb3JhY2xlX3NvdXJjZXNgIGNhbGxlZCB3aXRoIG1vcmUgdGhhbiBgTUFYX09SQUNMRV9TT1VSQ0VTYCBlbnRyaWVzLgAAAA5Ub29NYW55U291cmNlcwAAAAAACgAAADVEZXZpYXRpb24gbWF0aCB3b3VsZCBvdmVyZmxvdyBvbiB0aGUgc3VwcGxpZWQgcHJpY2VzLgAAAAAAABFEZXZpYXRpb25PdmVyZmxvdwAAAAAAAAsAAABDYHVwZ3JhZGVgIHJlamVjdGVkIOKAlCBubyBgcHJvcG9zZV91cGdyYWRlYCB3YXMgbWFkZSBiZWZvcmUgY29tbWl0LgAAAAAQTm9QZW5kaW5nVXBncmFkZQAAAAwAAAA0YHVwZ3JhZGVgIHJlamVjdGVkIOKAlCB0aW1lbG9jayBoYXMgbm90IGVsYXBzZWQgeWV0LgAAABlVcGdyYWRlVGltZWxvY2tOb3RFbGFwc2VkAAAAAAAADQAAAF5gdXBncmFkZWAgcmVqZWN0ZWQg4oCUIGBuZXdfd2FzbV9oYXNoYCBkb2VzIG5vdCBtYXRjaCB0aGUgcHJvcG9zZWQKYFBlbmRpbmdVcGdyYWRlLndhc21faGFzaGAuAAAAAAATVXBncmFkZUhhc2hNaXNtYXRjaAAAAAAOAAAAe0Egc291cmNlJ3MgYGRlY2ltYWxzKClgIGRpZmZlcnMgZnJvbSBgc2hhcmVkOjpjb25zdGFudHM6OlBSSUNFX0RFQ0lNQUxTYCwKb3IgdGhlIHNvdXJjZSBjb3VsZCBub3QgYmUgcXVlcmllZCBmb3IgaXRzIHNjYWxlLgAAAAAVSW52YWxpZFNvdXJjZURlY2ltYWxzAAAAAAAADwAAAEJFdmVuLWNvdW50IG1lZGlhbiBhdmVyYWdpbmcgd291bGQgb3ZlcmZsb3cgb24gdGhlIHN1cHBsaWVkIHByaWNlcy4AAAAAAA5NZWRpYW5PdmVyZmxvdwAAAAAAEA==",
        "AAAABQAAAAAAAAAAAAAAClByaWNlRmV0Y2gAAAAAAAEAAAAFcHJpY2UAAAAAAAADAAAAAAAAAAZzeW1ib2wAAAAAABEAAAABAAAAAAAAAAVwcmljZQAAAAAAAAsAAAAAAAAAAAAAAAl0aW1lc3RhbXAAAAAAAAAGAAAAAAAAAAE=",
        "AAAABQAAAIdFbWl0dGVkIGJ5IGBzZXRfb3JhY2xlX2NvbmZpZ2Agd2hlbmV2ZXIgdGhlIGdsb2JhbCBzYWZldHkgdGhyZXNob2xkcwpjaGFuZ2UuIE1pcnJvcnMgZXZlcnkgZmllbGQgb2YgdGhlIG9uLWNoYWluIGBPcmFjbGVDb25maWdgIHN0cnVjdC4AAAAAAAAAABJPcmFjbGVDb25maWdVcGRhdGUAAAAAAAEAAAAGb3JjY2ZnAAAAAAAEAAAAAAAAAAlzdGFsZW5lc3MAAAAAAAAGAAAAAAAAAAAAAAAJZGV2aWF0aW9uAAAAAAAACwAAAAAAAAAAAAAADmNhY2hlX2R1cmF0aW9uAAAAAAAGAAAAAAAAAAAAAAAUbWluX3JlcXVpcmVkX3NvdXJjZXMAAAAEAAAAAAAAAAE=",
        "AAAABQAAAGRFbWl0dGVkIGJ5IGBzZXRfb3JhY2xlX3NvdXJjZXNgIHNvIG9mZi1jaGFpbiBtb25pdG9yaW5nIGNhbiBkZXRlY3QgZXZlcnkKcm90YXRpb24gb2YgdGhlIHNvdXJjZSBzZXQuAAAAAAAAABNPcmFjbGVTb3VyY2VzVXBkYXRlAAAAAAEAAAAGb3Jjc3JjAAAAAAACAAAAAAAAAAZzeW1ib2wAAAAAABEAAAABAAAAAAAAAAdzb3VyY2VzAAAAA+oAAAATAAAAAAAAAAE=",
        "AAAAAgAAAAAAAAAAAAAAClN0b3JhZ2VLZXkAAAAAAAgAAAAAAAAAFEluaXRpYWxpemF0aW9uIGZsYWcuAAAAC0luaXRpYWxpemVkAAAAAAAAAAAdTGlua2VkIENvbmZpZ01hbmFnZXIgYWRkcmVzcy4AAAAAAAANQ29uZmlnTWFuYWdlcgAAAAAAAAAAAAAcR2xvYmFsIG9yYWNsZSBjb25maWd1cmF0aW9uLgAAAAxPcmFjbGVDb25maWcAAAAAAAAAlk1vbm90b25pYyB2ZXJzaW9uIGJ1bXBlZCBvbiBldmVyeSBjb25maWcgdXBkYXRlLiBDYWNoZSBrZXlzIGluY2x1ZGUKdGhpcyB2YWx1ZSBzbyBjb25maWcgY2hhbmdlcyBpbnZhbGlkYXRlIG9sZCBtZWRpYW5zIHdpdGhvdXQgc2Nhbm5pbmcKZXZlcnkgc3ltYm9sLgAAAAAADUNvbmZpZ1ZlcnNpb24AAAAAAAABAAAAO1Blci1zeW1ib2wgZmxhdCBzb3VyY2UgbGlzdCAobm8gcHJpbWFyeS9zZWNvbmRhcnkgdGllcmluZykuAAAAAAdTb3VyY2VzAAAAAAEAAAARAAAAAQAAAIBMZWdhY3kgcGVyLXN5bWJvbCBjYWNoZWQgYWdncmVnYXRlZCBwcmljZSBrZXkuIEtlcHQgc28gdXBncmFkZWQKZGVwbG95bWVudHMgZG8gbm90IHRyeSB0byBkZWNvZGUgb2xkIGNhY2hlIGVudHJpZXMgYXMgVjIgdmFsdWVzLgAAAAtDYWNoZWRQcmljZQAAAAABAAAAEQAAAAEAAABBUGVyLXN5bWJvbCBjYWNoZWQgYWdncmVnYXRlZCBwcmljZSBmb3IgdGhlIGFjdGl2ZSBjb25maWcgdmVyc2lvbi4AAAAAAAANQ2FjaGVkUHJpY2VWMgAAAAAAAAIAAAARAAAABgAAAAAAAABIQ3VycmVudCBjb250cmFjdCB2ZXJzaW9uIOKAlCB3cml0dGVuIGJ5IGBfbWlncmF0ZWAgYWZ0ZXIgYSBXQVNNIHVwZ3JhZGUuAAAAB1ZlcnNpb24A",
        "AAAAAQAAANNDYWNoZWQgYWdncmVnYXRlZCBtZWRpYW4gcHJpY2UgZm9yIGEgc3ltYm9sLiBgZmV0Y2hlZF9hdGAgYm91bmRzIHJvdXRlcgpjYWNoZSBkdXJhdGlvbjsgYG9sZGVzdF9zb3VyY2VfdXBkYXRlYCBlbnN1cmVzIHRoZSBjYWNoZWQgbWVkaWFuIGlzIG5vdApzZXJ2ZWQgYWZ0ZXIgYW55IHNvdXJjZSByZXNwb25zZSB1c2VkIHRvIGNvbXB1dGUgaXQgaGFzIGdvbmUgc3RhbGUuAAAAAAAAAAALQ2FjaGVkUHJpY2UAAAAAAwAAAAAAAAAKZmV0Y2hlZF9hdAAAAAAABgAAAAAAAAAUb2xkZXN0X3NvdXJjZV91cGRhdGUAAAAGAAAAAAAAAAVwcmljZQAAAAAAAAs=",
        "AAAAAAAAAAAAAAAHbWlncmF0ZQAAAAACAAAAAAAAAA5taWdyYXRpb25fZGF0YQAAAAAH0AAAAA1NaWdyYXRpb25EYXRhAAAAAAAAAAAAAAhvcGVyYXRvcgAAABMAAAAA",
        "AAAAAAAAAAAAAAAHdXBncmFkZQAAAAACAAAAAAAAAA1uZXdfd2FzbV9oYXNoAAAAAAAD7gAAACAAAAAAAAAACG9wZXJhdG9yAAAAEwAAAAA=",
        "AAAAAAAAAAAAAAAJZ2V0X3ByaWNlAAAAAAAAAQAAAAAAAAAGc3ltYm9sAAAAAAARAAAAAQAAAAs=",
        "AAAAAAAAATZBdG9taWMtd2l0aC1kZXBsb3kgaW5pdGlhbGl6YXRpb24gKFNvcm9iYW4gY29uc3RydWN0b3IpLiBCaW5kcyB0aGUKbGlua2VkIENvbmZpZ01hbmFnZXIgb25jZSwgaW5zaWRlIHRoZSBkZXBsb3kgdHJhbnNhY3Rpb24sIHNvIG5vIHRoaXJkCnBhcnR5IGNhbiBmcm9udC1ydW4gaW5pdCBhbmQgcG9pbnQgdGhlIHJvdXRlciBhdCBhIG1hbGljaW91cyByb2xlCmF1dGhvcml0eS4gVGhlIHJvdXRlciBzdG9yZXMgbm8gYWRtaW4gb2YgaXRzIG93biDigJQgZXZlcnkgcm9sZSBjaGVjawpjcm9zcy1jYWxscyB0aGUgbGlua2VkIENvbmZpZ01hbmFnZXIuAAAAAAANX19jb25zdHJ1Y3RvcgAAAAAAAAEAAAAAAAAAFmNvbmZpZ19tYW5hZ2VyX2FkZHJlc3MAAAAAABMAAAAA",
        "AAAAAAAAAAAAAAAOY2FuY2VsX3VwZ3JhZGUAAAAAAAEAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAA=",
        "AAAAAAAAAAAAAAAPcHJvcG9zZV91cGdyYWRlAAAAAAIAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAAAAAAJd2FzbV9oYXNoAAAAAAAD7gAAACAAAAAA",
        "AAAAAAAAAAAAAAARYnVtcF9vcmFjbGVfc3RhdGUAAAAAAAAAAAAAAA==",
        "AAAAAAAAAAAAAAARZ2V0X29yYWNsZV9jb25maWcAAAAAAAAAAAAAAQAAB9AAAAAMT3JhY2xlQ29uZmln",
        "AAAAAAAAAAAAAAARc2V0X29yYWNsZV9jb25maWcAAAAAAAACAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAAAAAABmNvbmZpZwAAAAAH0AAAAAxPcmFjbGVDb25maWcAAAAA",
        "AAAAAAAAAAAAAAASc2V0X29yYWNsZV9zb3VyY2VzAAAAAAADAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAAAAAABnN5bWJvbAAAAAAAEQAAAAAAAAAHc291cmNlcwAAAAPqAAAAEwAAAAA=",
        "AAAAAQAAADVSZXByZXNlbnRzIGEgc2luZ2xlIHRyYWRlcidzIG9wZW4gbGV2ZXJhZ2VkIHBvc2l0aW9uLgAAAAAAAAAAAAAIUG9zaXRpb24AAAALAAAARkFkZGl0aXZlIGJhc2UgcXVhbnRpdHkgYXQgYEVYUE9TVVJFX1BSRUNJU0lPTmA7IGF1dGhvcml0YXRpdmUgZm9yIFBuTC4AAAAAAA1iYXNlX2V4cG9zdXJlAAAAAAAACwAAADlCYXNlbGluZSBmb3IgYHNpemUgKiBhY2NfYm9ycm93X2luZGV4IC8gSU5ERVhfUFJFQ0lTSU9OYC4AAAAAAAAPYm9ycm93X2ZlZV9kZWJ0AAAAAAsAAAAoVVNEQyBjb2xsYXRlcmFsIGRlcG9zaXRlZCBieSB0aGUgdHJhZGVyLgAAAApjb2xsYXRlcmFsAAAAAAALAAAAQkRpc3BsYXktb25seSBoYXJtb25pYyBlbnRyeSBwcmljZSBkZXJpdmVkIGZyb20gc2l6ZS9iYXNlIGV4cG9zdXJlLgAAAAAAC2VudHJ5X3ByaWNlAAAAAAsAAACMRmxhdCBVU0RDIGZlZSBlc2Nyb3dlZCB3aGVuIFRQIG9yIFNMIGlzIHNldC4gUGFpZCB0byBleGVjdXRvciBvbiB0cmlnZ2VyLCByZWZ1bmRlZCBvbiB1c2VyIGNsb3NlIC8gQURMLCBmb3JmZWl0ZWQgdG8gcmV2ZW51ZSBvbiBsaXF1aWRhdGlvbi4AAAAUZXhlY3V0aW9uX2ZlZV9lc2Nyb3cAAAALAAAALFRydWUgZm9yIGEgbG9uZyBwb3NpdGlvbiwgZmFsc2UgZm9yIGEgc2hvcnQuAAAAB2lzX2xvbmcAAAAAAQAAAE9CbG9jayB0aW1lc3RhbXAgd2hlbiB0aGUgcG9zaXRpb24gd2FzIGxhc3QgaW5jcmVhc2VkIChhbnRpLWZyb250LXJ1bm5pbmcgbG9jaykuAAAAABNsYXN0X2luY3JlYXNlZF90aW1lAAAAAAYAAAAmTm90aW9uYWwgc2l6ZSBvZiB0aGUgcG9zaXRpb24gaW4gVVNEQy4AAAAAAARzaXplAAAACwAAADhCYXNlbGluZSBmb3IgYHNpemUgKiBzaWRlX3NrZXdfaW5kZXggLyBJTkRFWF9QUkVDSVNJT05gLgAAAA1za2V3X2ZlZV9kZWJ0AAAAAAAACwAAAC1TdG9wLWxvc3MgcHJpY2UgKHNjYWxlZCBieSAxZTcpLiAwID0gbm90IHNldC4AAAAAAAAJc3RvcF9sb3NzAAAAAAAACwAAAC9UYWtlLXByb2ZpdCBwcmljZSAoc2NhbGVkIGJ5IDFlNykuIDAgPSBub3Qgc2V0LgAAAAALdGFrZV9wcm9maXQAAAAACw==",
        "AAAAAQAAADhHbG9iYWwgbWFya2V0IHN0YXRlIGZvciBhIHNpbmdsZSB0cmFkZWFibGUgYXNzZXQgc3ltYm9sLgAAAAAAAAAKTWFya2V0SW5mbwAAAAAACgAAADxDdW11bGF0aXZlIGJvcnJvdyBmZWUgaW5kZXggKGdyb3dzIG1vbm90b25pY2FsbHkgd2l0aCB0aW1lKS4AAAAQYWNjX2JvcnJvd19pbmRleAAAAAsAAAAmRG9taW5hbnQtbG9uZyBza2V3IGNhcnJ5aW5nLWZlZSBpbmRleC4AAAAAABNhY2NfbG9uZ19za2V3X2luZGV4AAAAAAsAAAAnRG9taW5hbnQtc2hvcnQgc2tldyBjYXJyeWluZy1mZWUgaW5kZXguAAAAABRhY2Nfc2hvcnRfc2tld19pbmRleAAAAAsAAAA9RGlzcGxheS1vbmx5IGhhcm1vbmljIGVudHJ5IHByaWNlIGRlcml2ZWQgZnJvbSBsb25nIGV4cG9zdXJlLgAAAAAAABVnbG9iYWxfbG9uZ19hdmdfcHJpY2UAAAAAAAALAAAAPkRpc3BsYXktb25seSBoYXJtb25pYyBlbnRyeSBwcmljZSBkZXJpdmVkIGZyb20gc2hvcnQgZXhwb3N1cmUuAAAAAAAWZ2xvYmFsX3Nob3J0X2F2Z19wcmljZQAAAAAACwAAACpUaW1lc3RhbXAgb2YgdGhlIGxhc3Qga2VlcGVyIGluZGV4IHVwZGF0ZS4AAAAAABFsYXN0X2luZGV4X3VwZGF0ZQAAAAAAAAYAAAA7U3VtIG9mIGxvbmctcG9zaXRpb24gYmFzZSBleHBvc3VyZSBhdCBgRVhQT1NVUkVfUFJFQ0lTSU9OYC4AAAAAEmxvbmdfYmFzZV9leHBvc3VyZQAAAAAACwAAAC9Ub3RhbCBub3Rpb25hbCBzaXplIG9mIGFsbCBvcGVuIGxvbmcgcG9zaXRpb25zLgAAAAASbG9uZ19vcGVuX2ludGVyZXN0AAAAAAALAAAAPFN1bSBvZiBzaG9ydC1wb3NpdGlvbiBiYXNlIGV4cG9zdXJlIGF0IGBFWFBPU1VSRV9QUkVDSVNJT05gLgAAABNzaG9ydF9iYXNlX2V4cG9zdXJlAAAAAAsAAAAwVG90YWwgbm90aW9uYWwgc2l6ZSBvZiBhbGwgb3BlbiBzaG9ydCBwb3NpdGlvbnMuAAAAE3Nob3J0X29wZW5faW50ZXJlc3QAAAAACw==",
        "AAAAAQAAAC5HbG9iYWwgc2FmZXR5IHRocmVzaG9sZHMgZm9yIHByaWNlIHZhbGlkYXRpb24uAAAAAAAAAAAADE9yYWNsZUNvbmZpZwAAAAQAAADzSG93IGxvbmcgYSBjYWNoZWQgYWdncmVnYXRlZCBwcmljZSByZW1haW5zIHZhbGlkIGFmdGVyIHRoZSByb3V0ZXIKZmV0Y2ggKGluIHNlY29uZHMpLiBBIGNhY2hlIGhpdCBhbHNvIHJlcXVpcmVzIGV2ZXJ5IHNvdXJjZSB0aW1lc3RhbXAKdXNlZCBmb3IgdGhlIGNhY2hlZCBtZWRpYW4gdG8gcmVtYWluIHdpdGhpbiBgc3RhbGVuZXNzX3RocmVzaG9sZGAuCk11c3QgYmUgPiAwIGFuZCA8PSBgc3RhbGVuZXNzX3RocmVzaG9sZGAuAAAAAA5jYWNoZV9kdXJhdGlvbgAAAAAABgAAAIpNYXhpbXVtIGFsbG93ZWQgc3ByZWFkIGJldHdlZW4gb3JhY2xlIHNvdXJjZXMgaW4gYmFzaXMgcG9pbnRzCihlLmcuLCAxMDAgPSAxJSkuIEJvdW5kZWQgYXQgYHNoYXJlZDo6Y29uc3RhbnRzOjpNQVhfREVWSUFUSU9OX0JQU19DRUlMSU5HYC4AAAAAABFtYXhfZGV2aWF0aW9uX2JwcwAAAAAAAAsAAADjTWluaW11bSBudW1iZXIgb2Ygc291cmNlIHJlc3BvbnNlcyB0aGF0IG11c3QgYWdyZWUgd2l0aGluCmBtYXhfZGV2aWF0aW9uX2Jwc2AgZm9yIE9yYWNsZVJvdXRlciB0byByZXR1cm4gYSBwcmljZS4gRmxvb3JlZCBhdApgc2hhcmVkOjpjb25zdGFudHM6Ok1JTl9SRVFVSVJFRF9TT1VSQ0VTX0ZMT09SYCwgY2VpbGluZ2VkIGF0CmBzaGFyZWQ6OmNvbnN0YW50czo6TUFYX09SQUNMRV9TT1VSQ0VTYC4AAAAAFG1pbl9yZXF1aXJlZF9zb3VyY2VzAAAABAAAAFlNYXhpbXVtIGFnZSBvZiBhbiBleHRlcm5hbCBTRVAtNDAgcHJpY2UgZmVlZCBiZWZvcmUgaXQgaXMgcmVqZWN0ZWQKYXMgc3RhbGUgKGluIHNlY29uZHMpLgAAAAAAABNzdGFsZW5lc3NfdGhyZXNob2xkAAAAAAY=",
        "AAAAAQAAAEtEYXRhIHJlcXVpcmVkIGR1cmluZyBhIFdBU00gbWlncmF0aW9uLiBTaW5nbGUgZGVmaW5pdGlvbiBmb3IgYWxsIGNvbnRyYWN0cy4AAAAAAAAAAA1NaWdyYXRpb25EYXRhAAAAAAAAAQAAAAAAAAAHdmVyc2lvbgAAAAAE",
        "AAAAAQAAAb5QZW5kaW5nIFdBU00gdXBncmFkZSDigJQgc2V0IGJ5IGBwcm9wb3NlX3VwZ3JhZGVgLCBjb25zdW1lZCBieSBgdXBncmFkZWAKKGNsZWFyZWQgYXRvbWljYWxseSBvbiBhIHN1Y2Nlc3NmdWwgaW5zdGFsbCksIG9yIGNsZWFyZWQgYnkgYGNhbmNlbF91cGdyYWRlYC4KU2luZ2xlIHNoYXBlIGFjcm9zcyBldmVyeSBwcm90b2NvbCBjb250cmFjdDsgYWxsIGZvdXIgY29udHJhY3RzIHN0b3JlIGl0IGF0CnRoZSBzaGFyZWQgYHBlbmRpbmdfdXBncmFkZWAgU3ltYm9sIGtleSBpbiB0aGVpciBvd24gaW5zdGFuY2Ugc3RvcmFnZSAoc2VlCmBpbnRlcmZhY2VzOjp1cGdyYWRlOjpwZW5kaW5nX3VwZ3JhZGVfa2V5YCkuIGB1cGdyYWRlYCByZWZ1c2VzIHRvIGluc3RhbGwKdW5sZXNzIGBwZW5kaW5nLndhc21faGFzaGAgbWF0Y2hlcyB0aGUgc3VwcGxpZWQgaGFzaCBhbmQgYG5vdyA+PSBldGFgLgAAAAAAAAAAAA5QZW5kaW5nVXBncmFkZQAAAAAAAgAAAAAAAAADZXRhAAAAAAYAAAAAAAAACXdhc21faGFzaAAAAAAAA+4AAAAg",
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
        "AAAAAgAAACJTdG9yYWdlIGtleSBmb3IgdGhlIHBhdXNhYmxlIHN0YXRlAAAAAAAAAAAAElBhdXNhYmxlU3RvcmFnZUtleQAAAAAAAQAAAAAAAAAySW5kaWNhdGVzIHdoZXRoZXIgdGhlIGNvbnRyYWN0IGlzIGluIHBhdXNlZCBzdGF0ZS4AAAAAAAZQYXVzZWQAAA==",
        "AAAAAQAAAL1FeGVjdXRpb24tYm91bnR5IGFuZCBvcGVuLWZlZSBwYXJhbWV0ZXJzIGNoYXJnZWQgdG8gdHJhZGVycy4KYG9wZW5fZmVlX2Jwc2AgYW5kIGBsaXF1aWRhdGlvbl9ib3VudHlfYnBzYCBhcmUgaW4gYmFzaXMgcG9pbnRzOwpgdHBfc2xfZXhlY3V0aW9uX2ZlZWAgaXMgYSBmbGF0IFVTREMgYW1vdW50IGF0IFBSRUNJU0lPTiBzY2FsZS4AAAAAAAAAAAAACUZlZUNvbmZpZwAAAAAAAAMAAAAAAAAAFmxpcXVpZGF0aW9uX2JvdW50eV9icHMAAAAAAAQAAAAAAAAADG9wZW5fZmVlX2JwcwAAAAQAAAAAAAAAE3RwX3NsX2V4ZWN1dGlvbl9mZWUAAAAACw==",
        "AAAAAQAAAHBEZWZpbmVzIGhvdyBwcm90b2NvbCByZXZlbnVlIGlzIHNwbGl0IGJldHdlZW4gcGFydGllcy4KQWxsIHZhbHVlcyBhcmUgaW4gYmFzaXMgcG9pbnRzIChicHMpLiBNdXN0IHN1bSB0byAxMF8wMDAuAAAAAAAAAAlGZWVTcGxpdHMAAAAAAAADAAAAAAAAAAdkZXZfYnBzAAAAAAQAAAAAAAAABmxwX2JwcwAAAAAABAAAAAAAAAAKc3Rha2VyX2JwcwAAAAAABA==",
        "AAAAAQAAACtHbG9iYWwgcHJvdG9jb2wgcmlzayBhbmQgdGltaW5nIHBhcmFtZXRlcnMuAAAAAAAAAAAOUHJvdG9jb2xMaW1pdHMAAAAAAAcAAAAAAAAAC2FkbF9wbmxfYnBzAAAAAAQAAAAAAAAAE2FkbF91dGlsaXphdGlvbl9icHMAAAAABAAAAAAAAAARY29vbGRvd25fZHVyYXRpb24AAAAAAAAGAAAAAAAAABlsaXF1aWRhdGlvbl90aHJlc2hvbGRfYnBzAAAAAAAABAAAAAAAAAAVbWF4X3V0aWxpemF0aW9uX3JhdGlvAAAAAAAACwAAAAAAAAAObWluX2NvbGxhdGVyYWwAAAAAAAsAAAAAAAAAFW1pbl9wb3NpdGlvbl9saWZldGltZQAAAAAAAAY=",
        "AAAAAQAAAEpUaW1lLWJhc2VkIGNhcnJ5aW5nLWZlZSBwYXJhbWV0ZXJzIChhbGwgcmF0ZXMgaW4gYW5udWFsaXplZCBiYXNpcyBwb2ludHMpLgAAAAAAAAAAABFDYXJyeWluZ0ZlZUNvbmZpZwAAAAAAAAUAAAAAAAAAFGJhc2VfYm9ycm93X3JhdGVfYnBzAAAACwAAAEJNYXhpbXVtIHNrZXcgc3VyY2hhcmdlIGF0IDEwMCUgY29uY2VudHJhdGlvbiBhbmQgMTAwJSB1dGlsaXphdGlvbi4AAAAAABFtYXhfc2tld19yYXRlX2JwcwAAAAAAAAsAAAAAAAAAF29wdGltYWxfdXRpbGl6YXRpb25fYnBzAAAAAAsAAAAAAAAACnNsb3BlMV9icHMAAAAAAAsAAAAAAAAACnNsb3BlMl9icHMAAAAAAAs=" ]),
      options
    )
  }
  public readonly fromJSON = {
    migrate: this.txFromJSON<null>,
        upgrade: this.txFromJSON<null>,
        get_price: this.txFromJSON<i128>,
        cancel_upgrade: this.txFromJSON<null>,
        propose_upgrade: this.txFromJSON<null>,
        bump_oracle_state: this.txFromJSON<null>,
        get_oracle_config: this.txFromJSON<OracleConfig>,
        set_oracle_config: this.txFromJSON<null>,
        set_oracle_sources: this.txFromJSON<null>
  }
}