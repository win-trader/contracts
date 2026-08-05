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




export const RequestRouterError = {
  1: {message:"InvalidAmount"},
  2: {message:"InvalidRequest"},
  3: {message:"TooEarly"},
  4: {message:"QueueBlocked"},
  5: {message:"LpActionBlocked"},
  6: {message:"NoOracleRound"},
  7: {message:"Unauthorized"},
  /**
   * `upgrade` called with no pending proposal.
   */
  8: {message:"UpgradeNoPending"},
  /**
   * `upgrade` called before the proposal's timelock eta.
   */
  9: {message:"UpgradeTimelockNotElapsed"},
  /**
   * `upgrade` called with a hash that differs from the proposal.
   */
  10: {message:"UpgradeHashMismatch"}
}




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
  min_collateral: i128;
  min_position_lifetime: u64;
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
  migrate: ({data, operator}: {data: MigrationData, operator: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a upgrade transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  upgrade: ({new_wasm_hash, operator}: {new_wasm_hash: Buffer, operator: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a get_request transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_request: ({request_id}: {request_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<LpRequest>>

  /**
   * Construct and simulate a resolve_next transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  resolve_next: ({executor}: {executor: string}, options?: MethodOptions) => Promise<AssembledTransaction<SettlementResult>>

  /**
   * Construct and simulate a cancel_upgrade transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  cancel_upgrade: ({caller}: {caller: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a propose_upgrade transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  propose_upgrade: ({caller, wasm_hash}: {caller: string, wasm_hash: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a request_deposit transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  request_deposit: ({owner, assets}: {owner: string, assets: i128}, options?: MethodOptions) => Promise<AssembledTransaction<u64>>

  /**
   * Construct and simulate a request_withdrawal transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  request_withdrawal: ({owner, shares}: {owner: string, shares: i128}, options?: MethodOptions) => Promise<AssembledTransaction<u64>>

  /**
   * Construct and simulate a next_request_to_resolve transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  next_request_to_resolve: (options?: MethodOptions) => Promise<AssembledTransaction<u64>>

}
export class Client extends ContractClient {
  static async deploy<T = Client>(
        /** Constructor/Initialization Args for the contract's `__constructor` method */
        {asset_address, vault_address, oracle_router, config_manager_address}: {asset_address: string, vault_address: string, oracle_router: string, config_manager_address: string},
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
    return ContractClient.deploy({asset_address, vault_address, oracle_router, config_manager_address}, options)
  }
  constructor(public readonly options: ContractClientOptions) {
    super(
      new ContractSpec([ "AAAABAAAAAAAAAAAAAAAElJlcXVlc3RSb3V0ZXJFcnJvcgAAAAAACgAAAAAAAAANSW52YWxpZEFtb3VudAAAAAAAAAEAAAAAAAAADkludmFsaWRSZXF1ZXN0AAAAAAACAAAAAAAAAAhUb29FYXJseQAAAAMAAAAAAAAADFF1ZXVlQmxvY2tlZAAAAAQAAAAAAAAAD0xwQWN0aW9uQmxvY2tlZAAAAAAFAAAAAAAAAA1Ob09yYWNsZVJvdW5kAAAAAAAABgAAAAAAAAAMVW5hdXRob3JpemVkAAAABwAAACpgdXBncmFkZWAgY2FsbGVkIHdpdGggbm8gcGVuZGluZyBwcm9wb3NhbC4AAAAAABBVcGdyYWRlTm9QZW5kaW5nAAAACAAAADRgdXBncmFkZWAgY2FsbGVkIGJlZm9yZSB0aGUgcHJvcG9zYWwncyB0aW1lbG9jayBldGEuAAAAGVVwZ3JhZGVUaW1lbG9ja05vdEVsYXBzZWQAAAAAAAAJAAAAPGB1cGdyYWRlYCBjYWxsZWQgd2l0aCBhIGhhc2ggdGhhdCBkaWZmZXJzIGZyb20gdGhlIHByb3Bvc2FsLgAAABNVcGdyYWRlSGFzaE1pc21hdGNoAAAAAAo=",
        "AAAABQAAAAAAAAAAAAAAEExwUmVxdWVzdENyZWF0ZWQAAAABAAAABWxwcmVxAAAAAAAABQAAAAAAAAAKcmVxdWVzdF9pZAAAAAAABgAAAAAAAAAAAAAABW93bmVyAAAAAAAAEwAAAAAAAAAAAAAABGtpbmQAAAfQAAAADUxwUmVxdWVzdEtpbmQAAAAAAAAAAAAAREVzY3Jvd2VkIGNvbGxhdGVyYWwgZm9yIGEgZGVwb3NpdDsgZXNjcm93ZWQgc2hhcmVzIGZvciBhIHdpdGhkcmF3YWwuAAAABmFtb3VudAAAAAAACwAAAAAAAAAAAAAADWV4ZWN1dGVfYWZ0ZXIAAAAAAAAGAAAAAAAAAAE=",
        "AAAABQAAAIRUZXJtaW5hbCBvdXRjb21lIG9mIHRoZSBGSUZPIGhlYWQ6IGBTZXR0bGVkYCB3aXRoIHRoZSBtaW50ZWQgc2hhcmVzIC8KcGFpZCBhc3NldHMsIG9yIGBGYWlsZWRgIC8gYEV4cGlyZWRgIHdpdGggdGhlIGVzY3JvdyByZXR1cm5lZC4AAAAAAAAAEUxwUmVxdWVzdFJlc29sdmVkAAAAAAAAAQAAAAVscHJlcwAAAAAAAAUAAAAAAAAACnJlcXVlc3RfaWQAAAAAAAYAAAAAAAAAAAAAAAVvd25lcgAAAAAAABMAAAAAAAAAAAAAAARraW5kAAAH0AAAAA1McFJlcXVlc3RLaW5kAAAAAAAAAAAAAAAAAAAGc3RhdHVzAAAAAAfQAAAAD0xwUmVxdWVzdFN0YXR1cwAAAAAAAAAAQlNoYXJlcyBtaW50ZWQgKGRlcG9zaXQpIG9yIGFzc2V0cyBwYWlkICh3aXRoZHJhd2FsKTsgMCBvbiBmYWlsdXJlLgAAAAAADnNldHRsZWRfYW1vdW50AAAAAAALAAAAAAAAAAE=",
        "AAAAAAAAAAAAAAAHbWlncmF0ZQAAAAACAAAAAAAAAARkYXRhAAAH0AAAAA1NaWdyYXRpb25EYXRhAAAAAAAAAAAAAAhvcGVyYXRvcgAAABMAAAAA",
        "AAAAAAAAAAAAAAAHdXBncmFkZQAAAAACAAAAAAAAAA1uZXdfd2FzbV9oYXNoAAAAAAAD7gAAACAAAAAAAAAACG9wZXJhdG9yAAAAEwAAAAA=",
        "AAAAAAAAAAAAAAALZ2V0X3JlcXVlc3QAAAAAAQAAAAAAAAAKcmVxdWVzdF9pZAAAAAAABgAAAAEAAAfQAAAACUxwUmVxdWVzdAAAAA==",
        "AAAAAAAAAAAAAAAMcmVzb2x2ZV9uZXh0AAAAAQAAAAAAAAAIZXhlY3V0b3IAAAATAAAAAQAAB9AAAAAQU2V0dGxlbWVudFJlc3VsdA==",
        "AAAAAAAAAAAAAAANX19jb25zdHJ1Y3RvcgAAAAAAAAQAAAAAAAAADWFzc2V0X2FkZHJlc3MAAAAAAAATAAAAAAAAAA12YXVsdF9hZGRyZXNzAAAAAAAAEwAAAAAAAAANb3JhY2xlX3JvdXRlcgAAAAAAABMAAAAAAAAAFmNvbmZpZ19tYW5hZ2VyX2FkZHJlc3MAAAAAABMAAAAA",
        "AAAAAAAAAAAAAAAOY2FuY2VsX3VwZ3JhZGUAAAAAAAEAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAA=",
        "AAAAAAAAAAAAAAAPcHJvcG9zZV91cGdyYWRlAAAAAAIAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAAAAAAJd2FzbV9oYXNoAAAAAAAD7gAAACAAAAAA",
        "AAAAAAAAAAAAAAAPcmVxdWVzdF9kZXBvc2l0AAAAAAIAAAAAAAAABW93bmVyAAAAAAAAEwAAAAAAAAAGYXNzZXRzAAAAAAALAAAAAQAAAAY=",
        "AAAAAAAAAAAAAAAScmVxdWVzdF93aXRoZHJhd2FsAAAAAAACAAAAAAAAAAVvd25lcgAAAAAAABMAAAAAAAAABnNoYXJlcwAAAAAACwAAAAEAAAAG",
        "AAAAAAAAAAAAAAAXbmV4dF9yZXF1ZXN0X3RvX3Jlc29sdmUAAAAAAAAAAAEAAAAG",
        "AAAAAQAAAYVBdXRob3JpdGF0aXZlIHBlci1tYXJrZXQgc3RhdGU6IHRoZSBzaWRlIGFnZ3JlZ2F0ZXMsIHRoZSBmdW5kaW5nIGluZGljZXMsCnRoZSBmdW5kaW5nIEVNQSwgYW5kIHRoZSBtYXJrZXQgY29uZmlndXJhdGlvbi4KClNvcm9iYW4gbGltaXRzIFVEVCBmaWVsZCBuYW1lcyB0byAzMCBjaGFyYWN0ZXJzLCBzbyB3aGVyZSBhIGRvYyBnbG9zc2FyeQp0ZXJtIGlzIGxvbmdlciB0aGUgZmllbGQgZHJvcHMgdGhlIHJlZHVuZGFudCBxdWFsaWZpZXIgYW5kIGl0cyBkb2MKY29tbWVudCBjYXJyaWVzIHRoZSBmdWxsIHRlcm0gKGUuZy4gYHJlY2VpdmVyX2JhY2tlZF9pbmRleF9sb25nYCBpcyB0aGUKZG9jJ3MgYHJlY2VpdmVyX2JhY2tlZF9wYXllcl9pbmRleGAgZm9yIHRoZSBsb25nIHNpZGUpLgAAAAAAAAAAAAAGTWFya2V0AAAAAAARAAAAAAAAAAZjb25maWcAAAAAB9AAAAAMTWFya2V0Q29uZmlnAAAAQ2BJTkRFWF9QUkVDSVNJT05gLXNjYWxlZCBicHMvZGF5IHBheWVyIHJhdGUgYXMgb2YgdGhlIGxhc3QgcmVmcmVzaC4AAAAAEmN1cnJlbnRfcGF5ZXJfcmF0ZQAAAAAACwAAAAAAAAASY3VycmVudF9wYXllcl9zaWRlAAAAAAfQAAAACVBheWVyU2lkZQAAAAAAAAAAAAAXbGFzdF9mdW5kaW5nX2NoZWNrcG9pbnQAAAAABgAAAAAAAAAEbG9uZwAAB9AAAAAKTWFya2V0U2lkZQAAAAAAXUN1bXVsYXRpdmUgcGF5ZXIgZmVlIHBlciB1bml0IG9mIGRvbWluYW50LXNpZGUgc2l6ZSB0aGF0IGlzIExQCnJldmVudWUgb24gY29sbGVjdGlvbiAowqc4LjIpLgAAAAAAABRscF9iYWNrZWRfaW5kZXhfbG9uZwAAAAsAAAAAAAAAFWxwX2JhY2tlZF9pbmRleF9zaG9ydAAAAAAAAAsAAAAAAAAAEmxwX3BheWVyX3JlbWFpbmRlcgAAAAAACwAAADtTdWItc3Ryb29wIGNhcnJ5IG9mIHRoZSByZWNlaXZlci1saWFiaWxpdHkgYWNjcnVhbCAowqc4LjMpLgAAAAARcGVuZGluZ19yZW1haW5kZXIAAAAAAAALAAAAhUN1bXVsYXRpdmUgcGF5ZXIgZmVlIHBlciB1bml0IG9mIGRvbWluYW50LXNpZGUgc2l6ZSB3aG9zZSBjb2xsZWN0aW9uCnJlc3RvcmVzIGNhc2ggYmFja2luZyBhbiBhbHJlYWR5LWFjY3J1ZWQgcmVjZWl2ZXIgY2xhaW0gKMKnOC4yKS4AAAAAAAAacmVjZWl2ZXJfYmFja2VkX2luZGV4X2xvbmcAAAAAAAsAAAAAAAAAG3JlY2VpdmVyX2JhY2tlZF9pbmRleF9zaG9ydAAAAAALAAAAPkN1bXVsYXRpdmUgZnVuZGluZyBjcmVkaXQgcGVyIHVuaXQgb2YgbGlnaHQtc2lkZSBzaXplICjCpzguMikuAAAAAAATcmVjZWl2ZXJfaW5kZXhfbG9uZwAAAAALAAAAAAAAABhyZWNlaXZlcl9pbmRleF9yZW1haW5kZXIAAAALAAAAAAAAABRyZWNlaXZlcl9pbmRleF9zaG9ydAAAAAsAAAAAAAAAGHJlY2VpdmVyX3BheWVyX3JlbWFpbmRlcgAAAAsAAAAAAAAABXNob3J0AAAAAAAH0AAAAApNYXJrZXRTaWRlAAAAAAC+wqc4LjEgc2lnbmVkIEVNQSBvZiB0aGUgaW5zdGFudGFuZW91cyBza2V3OiBhIGZyYWN0aW9uIG9mIG9uZSBhdApgSU5ERVhfUFJFQ0lTSU9OYCBzY2FsZSwgcG9zaXRpdmUgd2hlbiBoaXN0b3J5IHNheXMgbG9uZ3MgZG9taW5hdGUuCkRlY2F5cyB0b3dhcmQgdGhlIGN1cnJlbnQgc2tldyB3aXRoIHRoZSBnbG9iYWwgaGFsZi1saWZlLgAAAAAACHNrZXdfZW1hAAAACw==",
        "AAAAAQAAAAAAAAAAAAAACExwQ29uZmlnAAAAAwAAAAAAAAAQbHBfcmVxdWVzdF9kZWxheQAAAAYAAAAAAAAAHG1heF93aXRoZHJhd191dGlsaXphdGlvbl9icHMAAAAEAAAAAAAAABptaW5fZGVwb3NpdF9uYXZfZmFjdG9yX2JwcwAAAAAABA==",
        "AAAAAQAAADVSZXByZXNlbnRzIGEgc2luZ2xlIHRyYWRlcidzIG9wZW4gbGV2ZXJhZ2VkIHBvc2l0aW9uLgAAAAAAAAAAAAAIUG9zaXRpb24AAAAQAAAAIUFzc2V0IHVuaXRzIGF0IGBQUklDRV9QUkVDSVNJT05gLgAAAAAAAA1iYXNlX2V4cG9zdXJlAAAAAAAACwAAAAAAAAALYm9ycm93X2RlYnQAAAAACwAAAClDYXNoIG93bmVkIGJ5IGFuIG9wdGlvbmFsLW9yZGVyIGV4ZWN1dG9yLgAAAAAAABBleGVjdXRpb25fYnVkZ2V0AAAACwAAAAAAAAAYZnVuZGluZ19wYWlkX3RvX2xwc19kZWJ0AAAACwAAAAAAAAAeZnVuZGluZ19wYWlkX3RvX3JlY2VpdmVyc19kZWJ0AAAAAAALAAAAAAAAABVmdW5kaW5nX3JlY2VpdmVkX2RlYnQAAAAAAAALAAAAAAAAAAJpZAAAAAAABgAAAAAAAAAHaXNfbG9uZwAAAAABAAAAAAAAABNsYXN0X2luY3JlYXNlZF90aW1lAAAAAAYAAAAAAAAABm1hcmtldAAAAAAAEQAAAAAAAAAFb3duZXIAAAAAAAATAAAALkZpeGVkIGdyb3NzIGNhcGFjaXR5IGFzc2lnbmVkIHdoZW4gcmlzayBvcGVucy4AAAAAAApyaXNrX3VuaXRzAAAAAAALAAAAIlVTRCBub3Rpb25hbCBhdCBgUFJJQ0VfUFJFQ0lTSU9OYC4AAAAAAARzaXplAAAACwAAADtUcmlnZ2VyIHByaWNlIGZvciB0aGUgb3B0aW9uYWwgc3RvcC1sb3NzIG9yZGVyOyBgMGAgPSBub25lLgAAAAAJc3RvcF9sb3NzAAAAAAAACwAAAMpUcmFkZXItb3duZWQgY29sbGF0ZXJhbCByZWNvcmRlZCBpbiBjb250cmFjdCBzdGF0ZSAodGhlIGRvYydzCiJzdG9yZWQgY29sbGF0ZXJhbCIpLiBFZmZlY3RpdmUgY29sbGF0ZXJhbCDigJQgc3RvcmVkIGNvbGxhdGVyYWwgYWZ0ZXIKcGVuZGluZyBmZWVzIGFuZCBmdW5kaW5nIGNyZWRpdHMg4oCUIGlzIGFsd2F5cyBkZXJpdmVkLCBuZXZlciBzdG9yZWQuAAAAAAARc3RvcmVkX2NvbGxhdGVyYWwAAAAAAAALAAAAPVRyaWdnZXIgcHJpY2UgZm9yIHRoZSBvcHRpb25hbCB0YWtlLXByb2ZpdCBvcmRlcjsgYDBgID0gbm9uZS4AAAAAAAALdGFrZV9wcm9maXQAAAAACw==",
        "AAAAAQAAAAAAAAAAAAAACUxwUmVxdWVzdAAAAAAAAAcAAAAAAAAABmFtb3VudAAAAAAACwAAAAAAAAANZXhlY3V0ZV9hZnRlcgAAAAAAAAYAAAAAAAAAAmlkAAAAAAAGAAAAAAAAAARraW5kAAAH0AAAAA1McFJlcXVlc3RLaW5kAAAAAAAAAAAAAAVvd25lcgAAAAAAABMAAAAAAAAADHJlcXVlc3RfdGltZQAAAAYAAAAAAAAABnN0YXR1cwAAAAAH0AAAAA9McFJlcXVlc3RTdGF0dXMA",
        "AAAAAgAAANFXaGljaCBzaWRlIGN1cnJlbnRseSBwYXlzIGZ1bmRpbmcgKMKnOC4xOiB0aGUgc2lkZSB0aGUgYmxlbmRlZCBpbnRlZ3JhbApza2V3IHBvaW50cyBhdCDigJQgdW5kZXIgdGhlIEVNQSB0aGlzIGNhbiBiZSB0aGUgKmxpZ2h0ZXIqIHNpZGUgZm9yIGEKd2hpbGUgYWZ0ZXIgdGhlIGJvb2sgZmxpcHMpLiBgTm9uZWAgd2hlbiB0aGUgYmxlbmQgaXMgZXhhY3RseSB6ZXJvLgAAAAAAAAAAAAAJUGF5ZXJTaWRlAAAAAAAAAwAAAAAAAAAAAAAABE5vbmUAAAAAAAAAAAAAAARMb25nAAAAAAAAAAAAAAAFU2hvcnQAAAA=",
        "AAAAAgAAAAAAAAAAAAAACVJpc2tTdGF0ZQAAAAAAAAQAAAAAAAAAAAAAAAZOb3JtYWwAAAAAAAAAAAAAAAAAB1dhcm5pbmcAAAAAAAAAAAAAAAADQWRsAAAAAAAAAAAAAAAAB0hhcmRDYXAA",
        "AAAAAQAAAAAAAAAAAAAACk1hcmtldFNpZGUAAAAAAAUAAAAAAAAADWJhc2VfZXhwb3N1cmUAAAAAAAALAAAAAAAAAApyaXNrX3N0YXRlAAAAAAfQAAAACVJpc2tTdGF0ZQAAAAAAAAAAAAAKcmlza191bml0cwAAAAAACwAAAAAAAAASc2l6ZV9vcGVuX2ludGVyZXN0AAAAAAALAAAAAAAAABdzdG9yZWRfY29sbGF0ZXJhbF90b3RhbAAAAAAL",
        "AAAAAQAAAAAAAAAAAAAAClJvdW5kUHJpY2UAAAAAAAIAAAAAAAAABXByaWNlAAAAAAAACwAAAAAAAAAGc3ltYm9sAAAAAAAR",
        "AAAAAQAAAAAAAAAAAAAAC09yYWNsZVJvdW5kAAAAAAUAAAAAAAAAAmlkAAAAAAAGAAAAAAAAAAtwcmV2aW91c19pZAAAAAAGAAAAAAAAABJwcmV2aW91c190aW1lc3RhbXAAAAAAAAYAAAAAAAAABnByaWNlcwAAAAAD6gAAB9AAAAAKUm91bmRQcmljZQAAAAAAAAAAAAl0aW1lc3RhbXAAAAAAAAAG",
        "AAAAAQAAAAAAAAAAAAAADEdsb2JhbENvbmZpZwAAAAwAAAAAAAAAGGJhc2VfYm9ycm93X3JhdGVfYnBzX2RheQAAAAsAAABfwqc4LjEgaGFsZi1saWZlIG9mIHRoZSBmdW5kaW5nIHNrZXcgRU1BLCBzZWNvbmRzIChnbG9iYWw6IG9uZSBtZW1vcnkKaG9yaXpvbiBmb3IgZXZlcnkgbWFya2V0KS4AAAAAGWZ1bmRpbmdfaGFsZl9saWZlX3NlY29uZHMAAAAAAAAGAAAAAAAAABloYXJkX2NhcF9mYWN0b3JfbGltaXRfYnBzAAAAAAAABAAAAAAAAAAUbHBfcmV2ZW51ZV9zaGFyZV9icHMAAAAEAAAAAAAAABJtYXhfYWN0aXZlX21hcmtldHMAAAAAAAQAAAAAAAAADm1heF9hZGxfcmV3YXJkAAAAAAALAAAAAAAAABptYXhfaW5zb2x2ZW50X3RvdWNoX3Jld2FyZAAAAAAACwAAAAAAAAAbbWF4X3ZhcmlhYmxlX2JvcnJvd19icHNfZGF5AAAAAAsAAAAAAAAADm1pbl9jb2xsYXRlcmFsAAAAAAALAAAAAAAAABVtaW5fcG9zaXRpb25fbGlmZXRpbWUAAAAAAAAGAAAAAAAAABdyaXNrX2NhcGFjaXR5X2xpbWl0X2JwcwAAAAAEAAAAAAAAAB1yaXNrX2tlZXBlcl9yZXZlbnVlX3NoYXJlX2JwcwAAAAAAAAQ=",
        "AAAAAQAAAAAAAAAAAAAADE1hcmtldENvbmZpZwAAABEAAAAAAAAAEmFkbF9wbmxfZmFjdG9yX2JwcwAAAAAABAAAAAAAAAAOYWRsX3Jld2FyZF9icHMAAAAAAAQAAAA0wqcxMS4xIGNsb3NpbmctZmVlIHRpZXIgd2hlbiB0aGUgY2xvc2Ugd29yc2VucyBza2V3LgAAABJjbG9zZV9mZWVfaGlnaF9icHMAAAAAAAQAAABCwqcxMS4xIGNsb3NpbmctZmVlIHRpZXIgd2hlbiB0aGUgY2xvc2UgaW1wcm92ZXMgb3IgcHJlc2VydmVzIHNrZXcuAAAAAAARY2xvc2VfZmVlX2xvd19icHMAAAAAAAAEAAAAAAAAABdoYXJkX2NhcF9wbmxfZmFjdG9yX2JwcwAAAAAEAAAAgk1hcmdpbiByZXF1aXJlZCB0byBvcGVuIG9yIGFkZCByaXNrICjCpzEyLjMpLiBEaXZpZGVzIG1heCBsZXZlcmFnZToKYSBwb3NpdGlvbiBtYXkgbm90IGJlIGNyZWF0ZWQgY2xvc2VyIHRvIGxpcXVpZGF0aW9uIHRoYW4gdGhpcy4AAAAAABJpbml0aWFsX21hcmdpbl9icHMAAAAAAAQAAACHwqc4LjEgd2VpZ2h0IG9mIHRoZSBpbnN0YW50YW5lb3VzIHNrZXcgaW4gdGhlIGZ1bmRpbmcgYmxlbmQsIGluIGJwczsKdGhlIHJlc3QgaXMgdGhlIGhhbGYtbGlmZSBFTUEuIGBCUFNgIHJlcHJvZHVjZXMgcHVyZSBpbnN0YW50IHNrZXcuAAAAABJpbnN0YW50X3dlaWdodF9icHMAAAAAAAQAAAAAAAAAFmxpcXVpZGF0aW9uX3Jld2FyZF9icHMAAAAAAAQAAAB8TWFyZ2luIGJlbG93IHdoaWNoIHRoZSBwb3NpdGlvbiBpcyBsaXF1aWRhdGFibGUgKMKnMTIuMykuIE11c3Qgbm90CmV4Y2VlZCBgaW5pdGlhbF9tYXJnaW5fYnBzYDsgdGhlIGdhcCBpcyB0aGUgZW50cnkgYnVmZmVyLgAAABZtYWludGVuYW5jZV9tYXJnaW5fYnBzAAAAAAAEAAAAAAAAABZtYXJrZXRfcmlza19mYWN0b3JfYnBzAAAAAAAEAAAAAAAAABhtYXhfZnVuZGluZ19yYXRlX2Jwc19kYXkAAAALAAAAAAAAABZtYXhfbG9uZ19iYXNlX2V4cG9zdXJlAAAAAAALAAAAAAAAABttYXhfbG9uZ19zaXplX29wZW5faW50ZXJlc3QAAAAACwAAAAAAAAAXbWF4X3Nob3J0X2Jhc2VfZXhwb3N1cmUAAAAACwAAAAAAAAAcbWF4X3Nob3J0X3NpemVfb3Blbl9pbnRlcmVzdAAAAAsAAAAAAAAAF3JlY292ZXJ5X3BubF9mYWN0b3JfYnBzAAAAAAQAAAAAAAAAFndhcm5pbmdfcG5sX2ZhY3Rvcl9icHMAAAAAAAQ=",
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
        get_request: this.txFromJSON<LpRequest>,
        resolve_next: this.txFromJSON<SettlementResult>,
        cancel_upgrade: this.txFromJSON<null>,
        propose_upgrade: this.txFromJSON<null>,
        request_deposit: this.txFromJSON<u64>,
        request_withdrawal: this.txFromJSON<u64>,
        next_request_to_resolve: this.txFromJSON<u64>
  }
}