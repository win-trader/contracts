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


/**
 * Authoritative per-market state: the side aggregates, the funding indices,
 * the current funding flows, and the market configuration.
 *
 * Soroban limits UDT field names to 30 characters, so where a doc glossary
 * term is longer the field drops the redundant qualifier and its doc
 * comment carries the full term (e.g. `receiver_backed_index_long` is the
 * doc's `receiver_backed_payer_index` for the long side).
 */
export interface Market {
  config: MarketConfig;
  /**
 * `INDEX_PRECISION`-scaled bps/day payer rate for the current interval.
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
  /**
 * LP-allocated remainder of the payer flow, cash/second at
 * `INDEX_PRECISION` (§8.1).
 */
lp_flow_per_second: i128;
  lp_payer_remainder: i128;
  /**
 * Cumulative payer fee per unit of dominant-side size whose collection
 * restores cash backing an already-accrued receiver claim (§8.2).
 */
receiver_backed_index_long: i128;
  receiver_backed_index_short: i128;
  /**
 * Receiver-allocated share of the payer flow, cash/second at
 * `INDEX_PRECISION` (§8.1).
 */
receiver_flow_per_second: i128;
  receiver_flow_remainder: i128;
  /**
 * Cumulative funding credit per unit of light-side size (§8.2).
 */
receiver_index_long: i128;
  receiver_index_remainder: i128;
  receiver_index_short: i128;
  receiver_payer_remainder: i128;
  short: MarketSide;
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
 * Which side currently pays funding (§8.1: the side with more base
 * exposure). `None` when the market is balanced or empty.
 */
export type PayerSide = {tag: "None", values: void} | {tag: "Long", values: void} | {tag: "Short", values: void};

export type RiskState = {tag: "Normal", values: void} | {tag: "Warning", values: void} | {tag: "Adl", values: void} | {tag: "HardCap", values: void};


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
        "AAAABQAAAINFbWl0dGVkIGJ5IGBwdWJsaXNoX3JvdW5kYCDigJQgdGhlIHB1c2ggc2lnbmFsIGZvciBhbnl0aGluZyB3YWl0aW5nIG9uIGEKY2Fub25pY2FsIHJvdW5kICh0aGUgRklGTyBMUCByZXF1ZXN0IHF1ZXVlIGluIHBhcnRpY3VsYXIpLgAAAAAAAAAADlJvdW5kUHVibGlzaGVkAAAAAAABAAAACHJvdW5kcHViAAAAAwAAAAAAAAACaWQAAAAAAAYAAAAAAAAAAAAAAAl0aW1lc3RhbXAAAAAAAAAGAAAAAAAAAAAAAAALcHJldmlvdXNfaWQAAAAABgAAAAAAAAAB",
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
        "AAAAAQAAAY9BdXRob3JpdGF0aXZlIHBlci1tYXJrZXQgc3RhdGU6IHRoZSBzaWRlIGFnZ3JlZ2F0ZXMsIHRoZSBmdW5kaW5nIGluZGljZXMsCnRoZSBjdXJyZW50IGZ1bmRpbmcgZmxvd3MsIGFuZCB0aGUgbWFya2V0IGNvbmZpZ3VyYXRpb24uCgpTb3JvYmFuIGxpbWl0cyBVRFQgZmllbGQgbmFtZXMgdG8gMzAgY2hhcmFjdGVycywgc28gd2hlcmUgYSBkb2MgZ2xvc3NhcnkKdGVybSBpcyBsb25nZXIgdGhlIGZpZWxkIGRyb3BzIHRoZSByZWR1bmRhbnQgcXVhbGlmaWVyIGFuZCBpdHMgZG9jCmNvbW1lbnQgY2FycmllcyB0aGUgZnVsbCB0ZXJtIChlLmcuIGByZWNlaXZlcl9iYWNrZWRfaW5kZXhfbG9uZ2AgaXMgdGhlCmRvYydzIGByZWNlaXZlcl9iYWNrZWRfcGF5ZXJfaW5kZXhgIGZvciB0aGUgbG9uZyBzaWRlKS4AAAAAAAAAAAZNYXJrZXQAAAAAABIAAAAAAAAABmNvbmZpZwAAAAAH0AAAAAxNYXJrZXRDb25maWcAAABFYElOREVYX1BSRUNJU0lPTmAtc2NhbGVkIGJwcy9kYXkgcGF5ZXIgcmF0ZSBmb3IgdGhlIGN1cnJlbnQgaW50ZXJ2YWwuAAAAAAAAEmN1cnJlbnRfcGF5ZXJfcmF0ZQAAAAAACwAAAAAAAAASY3VycmVudF9wYXllcl9zaWRlAAAAAAfQAAAACVBheWVyU2lkZQAAAAAAAAAAAAAXbGFzdF9mdW5kaW5nX2NoZWNrcG9pbnQAAAAABgAAAAAAAAAEbG9uZwAAB9AAAAAKTWFya2V0U2lkZQAAAAAAXUN1bXVsYXRpdmUgcGF5ZXIgZmVlIHBlciB1bml0IG9mIGRvbWluYW50LXNpZGUgc2l6ZSB0aGF0IGlzIExQCnJldmVudWUgb24gY29sbGVjdGlvbiAowqc4LjIpLgAAAAAAABRscF9iYWNrZWRfaW5kZXhfbG9uZwAAAAsAAAAAAAAAFWxwX2JhY2tlZF9pbmRleF9zaG9ydAAAAAAAAAsAAABTTFAtYWxsb2NhdGVkIHJlbWFpbmRlciBvZiB0aGUgcGF5ZXIgZmxvdywgY2FzaC9zZWNvbmQgYXQKYElOREVYX1BSRUNJU0lPTmAgKMKnOC4xKS4AAAAAEmxwX2Zsb3dfcGVyX3NlY29uZAAAAAAACwAAAAAAAAASbHBfcGF5ZXJfcmVtYWluZGVyAAAAAAALAAAAhUN1bXVsYXRpdmUgcGF5ZXIgZmVlIHBlciB1bml0IG9mIGRvbWluYW50LXNpZGUgc2l6ZSB3aG9zZSBjb2xsZWN0aW9uCnJlc3RvcmVzIGNhc2ggYmFja2luZyBhbiBhbHJlYWR5LWFjY3J1ZWQgcmVjZWl2ZXIgY2xhaW0gKMKnOC4yKS4AAAAAAAAacmVjZWl2ZXJfYmFja2VkX2luZGV4X2xvbmcAAAAAAAsAAAAAAAAAG3JlY2VpdmVyX2JhY2tlZF9pbmRleF9zaG9ydAAAAAALAAAAVVJlY2VpdmVyLWFsbG9jYXRlZCBzaGFyZSBvZiB0aGUgcGF5ZXIgZmxvdywgY2FzaC9zZWNvbmQgYXQKYElOREVYX1BSRUNJU0lPTmAgKMKnOC4xKS4AAAAAAAAYcmVjZWl2ZXJfZmxvd19wZXJfc2Vjb25kAAAACwAAAAAAAAAXcmVjZWl2ZXJfZmxvd19yZW1haW5kZXIAAAAACwAAAD5DdW11bGF0aXZlIGZ1bmRpbmcgY3JlZGl0IHBlciB1bml0IG9mIGxpZ2h0LXNpZGUgc2l6ZSAowqc4LjIpLgAAAAAAE3JlY2VpdmVyX2luZGV4X2xvbmcAAAAACwAAAAAAAAAYcmVjZWl2ZXJfaW5kZXhfcmVtYWluZGVyAAAACwAAAAAAAAAUcmVjZWl2ZXJfaW5kZXhfc2hvcnQAAAALAAAAAAAAABhyZWNlaXZlcl9wYXllcl9yZW1haW5kZXIAAAALAAAAAAAAAAVzaG9ydAAAAAAAB9AAAAAKTWFya2V0U2lkZQAA",
        "AAAAAQAAAAAAAAAAAAAACExwQ29uZmlnAAAAAwAAAAAAAAAQbHBfcmVxdWVzdF9kZWxheQAAAAYAAAAAAAAAHG1heF93aXRoZHJhd191dGlsaXphdGlvbl9icHMAAAAEAAAAAAAAABptaW5fZGVwb3NpdF9uYXZfZmFjdG9yX2JwcwAAAAAABA==",
        "AAAAAQAAADVSZXByZXNlbnRzIGEgc2luZ2xlIHRyYWRlcidzIG9wZW4gbGV2ZXJhZ2VkIHBvc2l0aW9uLgAAAAAAAAAAAAAIUG9zaXRpb24AAAAQAAAAIUFzc2V0IHVuaXRzIGF0IGBQUklDRV9QUkVDSVNJT05gLgAAAAAAAA1iYXNlX2V4cG9zdXJlAAAAAAAACwAAAAAAAAALYm9ycm93X2RlYnQAAAAACwAAAClDYXNoIG93bmVkIGJ5IGFuIG9wdGlvbmFsLW9yZGVyIGV4ZWN1dG9yLgAAAAAAABBleGVjdXRpb25fYnVkZ2V0AAAACwAAAAAAAAAYZnVuZGluZ19wYWlkX3RvX2xwc19kZWJ0AAAACwAAAAAAAAAeZnVuZGluZ19wYWlkX3RvX3JlY2VpdmVyc19kZWJ0AAAAAAALAAAAAAAAABVmdW5kaW5nX3JlY2VpdmVkX2RlYnQAAAAAAAALAAAAAAAAAAJpZAAAAAAABgAAAAAAAAAHaXNfbG9uZwAAAAABAAAAAAAAABNsYXN0X2luY3JlYXNlZF90aW1lAAAAAAYAAAAAAAAABm1hcmtldAAAAAAAEQAAAAAAAAAFb3duZXIAAAAAAAATAAAALkZpeGVkIGdyb3NzIGNhcGFjaXR5IGFzc2lnbmVkIHdoZW4gcmlzayBvcGVucy4AAAAAAApyaXNrX3VuaXRzAAAAAAALAAAAIlVTRCBub3Rpb25hbCBhdCBgUFJJQ0VfUFJFQ0lTSU9OYC4AAAAAAARzaXplAAAACwAAADtUcmlnZ2VyIHByaWNlIGZvciB0aGUgb3B0aW9uYWwgc3RvcC1sb3NzIG9yZGVyOyBgMGAgPSBub25lLgAAAAAJc3RvcF9sb3NzAAAAAAAACwAAAMpUcmFkZXItb3duZWQgY29sbGF0ZXJhbCByZWNvcmRlZCBpbiBjb250cmFjdCBzdGF0ZSAodGhlIGRvYydzCiJzdG9yZWQgY29sbGF0ZXJhbCIpLiBFZmZlY3RpdmUgY29sbGF0ZXJhbCDigJQgc3RvcmVkIGNvbGxhdGVyYWwgYWZ0ZXIKcGVuZGluZyBmZWVzIGFuZCBmdW5kaW5nIGNyZWRpdHMg4oCUIGlzIGFsd2F5cyBkZXJpdmVkLCBuZXZlciBzdG9yZWQuAAAAAAARc3RvcmVkX2NvbGxhdGVyYWwAAAAAAAALAAAAPVRyaWdnZXIgcHJpY2UgZm9yIHRoZSBvcHRpb25hbCB0YWtlLXByb2ZpdCBvcmRlcjsgYDBgID0gbm9uZS4AAAAAAAALdGFrZV9wcm9maXQAAAAACw==",
        "AAAAAQAAAAAAAAAAAAAACUxwUmVxdWVzdAAAAAAAAAcAAAAAAAAABmFtb3VudAAAAAAACwAAAAAAAAANZXhlY3V0ZV9hZnRlcgAAAAAAAAYAAAAAAAAAAmlkAAAAAAAGAAAAAAAAAARraW5kAAAH0AAAAA1McFJlcXVlc3RLaW5kAAAAAAAAAAAAAAVvd25lcgAAAAAAABMAAAAAAAAADHJlcXVlc3RfdGltZQAAAAYAAAAAAAAABnN0YXR1cwAAAAAH0AAAAA9McFJlcXVlc3RTdGF0dXMA",
        "AAAAAgAAAHlXaGljaCBzaWRlIGN1cnJlbnRseSBwYXlzIGZ1bmRpbmcgKMKnOC4xOiB0aGUgc2lkZSB3aXRoIG1vcmUgYmFzZQpleHBvc3VyZSkuIGBOb25lYCB3aGVuIHRoZSBtYXJrZXQgaXMgYmFsYW5jZWQgb3IgZW1wdHkuAAAAAAAAAAAAAAlQYXllclNpZGUAAAAAAAADAAAAAAAAAAAAAAAETm9uZQAAAAAAAAAAAAAABExvbmcAAAAAAAAAAAAAAAVTaG9ydAAAAA==",
        "AAAAAgAAAAAAAAAAAAAACVJpc2tTdGF0ZQAAAAAAAAQAAAAAAAAAAAAAAAZOb3JtYWwAAAAAAAAAAAAAAAAAB1dhcm5pbmcAAAAAAAAAAAAAAAADQWRsAAAAAAAAAAAAAAAAB0hhcmRDYXAA",
        "AAAAAQAAAAAAAAAAAAAACk1hcmtldFNpZGUAAAAAAAUAAAAAAAAADWJhc2VfZXhwb3N1cmUAAAAAAAALAAAAAAAAAApyaXNrX3N0YXRlAAAAAAfQAAAACVJpc2tTdGF0ZQAAAAAAAAAAAAAKcmlza191bml0cwAAAAAACwAAAAAAAAASc2l6ZV9vcGVuX2ludGVyZXN0AAAAAAALAAAAAAAAABdzdG9yZWRfY29sbGF0ZXJhbF90b3RhbAAAAAAL",
        "AAAAAQAAAAAAAAAAAAAAClJvdW5kUHJpY2UAAAAAAAIAAAAAAAAABXByaWNlAAAAAAAACwAAAAAAAAAGc3ltYm9sAAAAAAAR",
        "AAAAAQAAAAAAAAAAAAAAC09yYWNsZVJvdW5kAAAAAAUAAAAAAAAAAmlkAAAAAAAGAAAAAAAAAAtwcmV2aW91c19pZAAAAAAGAAAAAAAAABJwcmV2aW91c190aW1lc3RhbXAAAAAAAAYAAAAAAAAABnByaWNlcwAAAAAD6gAAB9AAAAAKUm91bmRQcmljZQAAAAAAAAAAAAl0aW1lc3RhbXAAAAAAAAAG",
        "AAAAAQAAAAAAAAAAAAAADEdsb2JhbENvbmZpZwAAAAsAAAAAAAAAGGJhc2VfYm9ycm93X3JhdGVfYnBzX2RheQAAAAsAAAAAAAAAGWhhcmRfY2FwX2ZhY3Rvcl9saW1pdF9icHMAAAAAAAAEAAAAAAAAABRscF9yZXZlbnVlX3NoYXJlX2JwcwAAAAQAAAAAAAAAEm1heF9hY3RpdmVfbWFya2V0cwAAAAAABAAAAAAAAAAObWF4X2FkbF9yZXdhcmQAAAAAAAsAAAAAAAAAGm1heF9pbnNvbHZlbnRfdG91Y2hfcmV3YXJkAAAAAAALAAAAAAAAABttYXhfdmFyaWFibGVfYm9ycm93X2Jwc19kYXkAAAAACwAAAAAAAAAObWluX2NvbGxhdGVyYWwAAAAAAAsAAAAAAAAAFW1pbl9wb3NpdGlvbl9saWZldGltZQAAAAAAAAYAAAAAAAAAF3Jpc2tfY2FwYWNpdHlfbGltaXRfYnBzAAAAAAQAAAAAAAAAHXJpc2tfa2VlcGVyX3JldmVudWVfc2hhcmVfYnBzAAAAAAAABA==",
        "AAAAAQAAAAAAAAAAAAAADE1hcmtldENvbmZpZwAAAA8AAAAAAAAAEmFkbF9wbmxfZmFjdG9yX2JwcwAAAAAABAAAAAAAAAAOYWRsX3Jld2FyZF9icHMAAAAAAAQAAAAAAAAAF2hhcmRfY2FwX3BubF9mYWN0b3JfYnBzAAAAAAQAAAAAAAAAFmxpcXVpZGF0aW9uX3Jld2FyZF9icHMAAAAAAAQAAAAAAAAAFm1haW50ZW5hbmNlX21hcmdpbl9icHMAAAAAAAQAAAAAAAAAFm1hcmtldF9yaXNrX2ZhY3Rvcl9icHMAAAAAAAQAAAAAAAAAGG1heF9mdW5kaW5nX3JhdGVfYnBzX2RheQAAAAsAAAAAAAAAFm1heF9sb25nX2Jhc2VfZXhwb3N1cmUAAAAAAAsAAAAAAAAAG21heF9sb25nX3NpemVfb3Blbl9pbnRlcmVzdAAAAAALAAAAAAAAABdtYXhfc2hvcnRfYmFzZV9leHBvc3VyZQAAAAALAAAAAAAAABxtYXhfc2hvcnRfc2l6ZV9vcGVuX2ludGVyZXN0AAAACwAAAAAAAAARb3Blbl9mZWVfaGlnaF9icHMAAAAAAAAEAAAAAAAAABBvcGVuX2ZlZV9sb3dfYnBzAAAABAAAAAAAAAAXcmVjb3ZlcnlfcG5sX2ZhY3Rvcl9icHMAAAAABAAAAAAAAAAWd2FybmluZ19wbmxfZmFjdG9yX2JwcwAAAAAABA==",
        "AAAAAQAAAC5HbG9iYWwgc2FmZXR5IHRocmVzaG9sZHMgZm9yIHByaWNlIHZhbGlkYXRpb24uAAAAAAAAAAAADE9yYWNsZUNvbmZpZwAAAAQAAADzSG93IGxvbmcgYSBjYWNoZWQgYWdncmVnYXRlZCBwcmljZSByZW1haW5zIHZhbGlkIGFmdGVyIHRoZSByb3V0ZXIKZmV0Y2ggKGluIHNlY29uZHMpLiBBIGNhY2hlIGhpdCBhbHNvIHJlcXVpcmVzIGV2ZXJ5IHNvdXJjZSB0aW1lc3RhbXAKdXNlZCBmb3IgdGhlIGNhY2hlZCBtZWRpYW4gdG8gcmVtYWluIHdpdGhpbiBgc3RhbGVuZXNzX3RocmVzaG9sZGAuCk11c3QgYmUgPiAwIGFuZCA8PSBgc3RhbGVuZXNzX3RocmVzaG9sZGAuAAAAAA5jYWNoZV9kdXJhdGlvbgAAAAAABgAAAIlNYXhpbXVtIGFsbG93ZWQgc3ByZWFkIGJldHdlZW4gb3JhY2xlIHNvdXJjZXMgaW4gYmFzaXMgcG9pbnRzCihlLmcuLCAxMDAgPSAxJSkuIEJvdW5kZWQgYXQgYGNyYXRlOjpjb25zdGFudHM6Ok1BWF9ERVZJQVRJT05fQlBTX0NFSUxJTkdgLgAAAAAAABFtYXhfZGV2aWF0aW9uX2JwcwAAAAAAAAsAAADhTWluaW11bSBudW1iZXIgb2Ygc291cmNlIHJlc3BvbnNlcyB0aGF0IG11c3QgYWdyZWUgd2l0aGluCmBtYXhfZGV2aWF0aW9uX2Jwc2AgZm9yIE9yYWNsZVJvdXRlciB0byByZXR1cm4gYSBwcmljZS4gRmxvb3JlZCBhdApgY3JhdGU6OmNvbnN0YW50czo6TUlOX1JFUVVJUkVEX1NPVVJDRVNfRkxPT1JgLCBjZWlsaW5nZWQgYXQKYGNyYXRlOjpjb25zdGFudHM6Ok1BWF9PUkFDTEVfU09VUkNFU2AuAAAAAAAAFG1pbl9yZXF1aXJlZF9zb3VyY2VzAAAABAAAAFlNYXhpbXVtIGFnZSBvZiBhbiBleHRlcm5hbCBTRVAtNDAgcHJpY2UgZmVlZCBiZWZvcmUgaXQgaXMgcmVqZWN0ZWQKYXMgc3RhbGUgKGluIHNlY29uZHMpLgAAAAAAABNzdGFsZW5lc3NfdGhyZXNob2xkAAAAAAY=",
        "AAAAAgAAAAAAAAAAAAAADUxwUmVxdWVzdEtpbmQAAAAAAAACAAAAAAAAAAAAAAAHRGVwb3NpdAAAAAAAAAAAAAAAAApXaXRoZHJhd2FsAAA=",
        "AAAAAQAAAEtEYXRhIHJlcXVpcmVkIGR1cmluZyBhIFdBU00gbWlncmF0aW9uLiBTaW5nbGUgZGVmaW5pdGlvbiBmb3IgYWxsIGNvbnRyYWN0cy4AAAAAAAAAAA1NaWdyYXRpb25EYXRhAAAAAAAAAQAAAAAAAAAHdmVyc2lvbgAAAAAE",
        "AAAAAQAAAbBQZW5kaW5nIFdBU00gdXBncmFkZSDigJQgc2V0IGJ5IGBwcm9wb3NlX3VwZ3JhZGVgLCBjb25zdW1lZCBieSBgdXBncmFkZWAKKGNsZWFyZWQgYXRvbWljYWxseSBvbiBhIHN1Y2Nlc3NmdWwgaW5zdGFsbCksIG9yIGNsZWFyZWQgYnkgYGNhbmNlbF91cGdyYWRlYC4KU2luZ2xlIHNoYXBlIGFjcm9zcyBldmVyeSBwcm90b2NvbCBjb250cmFjdC4gQ29udHJhY3RzIHN0b3JlIGl0IGF0CnRoZSBzaGFyZWQgYHBlbmRpbmdfdXBncmFkZWAgU3ltYm9sIGtleSBpbiB0aGVpciBvd24gaW5zdGFuY2Ugc3RvcmFnZSAoc2VlCmBjcmF0ZTo6dXBncmFkZTo6cGVuZGluZ191cGdyYWRlX2tleWApLiBgdXBncmFkZWAgcmVmdXNlcyB0byBpbnN0YWxsCnVubGVzcyBgcGVuZGluZy53YXNtX2hhc2hgIG1hdGNoZXMgdGhlIHN1cHBsaWVkIGhhc2ggYW5kIGBub3cgPj0gZXRhYC4AAAAAAAAADlBlbmRpbmdVcGdyYWRlAAAAAAACAAAAAAAAAANldGEAAAAABgAAAAAAAAAJd2FzbV9oYXNoAAAAAAAD7gAAACA=",
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
