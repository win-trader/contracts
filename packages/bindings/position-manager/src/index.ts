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
  1: {message:"AlreadyInitialized"},
  2: {message:"NotInitialized"},
  3: {message:"Paused"},
  /**
   * New trade would push vault utilization past MaxUtilizationRatio (85%).
   */
  4: {message:"UtilizationCapBreached"},
  /**
   * decrease_position called before MinPositionLifetime has elapsed.
   */
  5: {message:"PositionNotOldEnough"},
  6: {message:"PositionNotFound"},
  7: {message:"Unauthorized"},
  8: {message:"ZeroAmount"},
  /**
   * liquidate_position called but the position is still healthy.
   */
  9: {message:"HealthFactorOk"},
  /**
   * deleverage_position called but ADL trigger conditions are not met.
   */
  10: {message:"AdlNotTriggered"},
  /**
   * Position leverage exceeds the per-market max leverage.
   */
  11: {message:"ExcessiveLeverage"},
  /**
   * No max leverage configured for this market symbol.
   */
  12: {message:"MarketNotConfigured"},
  /**
   * execute_order called but neither TP nor SL trigger condition is met.
   */
  13: {message:"OrderNotTriggered"},
  /**
   * Invalid take-profit or stop-loss price for the position direction.
   */
  14: {message:"InvalidTpSl"},
  /**
   * increase_position called with `is_long` opposite to the existing position's direction.
   */
  15: {message:"DirectionMismatch"},
  /**
   * Collateral below the protocol's min_collateral limit.
   */
  16: {message:"BelowMinCollateral"},
  /**
   * ADL target position is not profitable (PnL <= 0).
   */
  17: {message:"AdlTargetNotProfitable"},
  /**
   * Max leverage exceeds the absolute safety cap (200x).
   */
  18: {message:"LeverageCapExceeded"},
  /**
   * Mark price at execution time exceeded the trader's `acceptable_price`.
   */
  19: {message:"SlippageExceeded"},
  /**
   * `set_max_leverage` called with a value below `MIN_LEVERAGE`. Use
   * `disable_market` to take a market offline instead.
   */
  20: {message:"LeverageBelowFloor"},
  /**
   * Trading is disabled for this market — `enable_market` re-opens it.
   */
  21: {message:"MarketDisabled"},
  /**
   * `decrease_position` called with `size_delta > pos.size`. Use
   * `pos.size` (or simply close fully) instead of over-closing.
   */
  22: {message:"SizeDeltaExceedsPosition"},
  /**
   * `upgrade` rejected — no `propose_upgrade` was made before commit.
   */
  23: {message:"NoPendingUpgrade"},
  /**
   * `upgrade` rejected — timelock has not elapsed yet.
   */
  24: {message:"UpgradeTimelockNotElapsed"},
  /**
   * `upgrade` rejected — `new_wasm_hash` does not match the proposed
   * `PendingUpgrade.wasm_hash`.
   */
  25: {message:"UpgradeHashMismatch"}
}













export type StorageKey = {tag: "Initialized", values: void} | {tag: "VaultAddress", values: void} | {tag: "ConfigManager", values: void} | {tag: "OracleRouter", values: void} | {tag: "IsPaused", values: void} | {tag: "Version", values: void} | {tag: "RealizedPnl", values: void} | {tag: "TotalUnrealizedPnl", values: void} | {tag: "MarketUnrealizedPnl", values: readonly [string]} | {tag: "FundingPool", values: readonly [string]} | {tag: "LastUnpauseTime", values: void} | {tag: "LastPauseTime", values: void} | {tag: "MaxLeverage", values: readonly [string]} | {tag: "MarketDisabled", values: readonly [string]} | {tag: "Position", values: readonly [PositionKey]} | {tag: "Market", values: readonly [string]} | {tag: "MarketRegistry", values: void};


/**
 * Composite key for looking up a position by trader address and asset symbol.
 */
export interface PositionKey {
  symbol: string;
  trader: string;
}


/**
 * Represents a single trader's open leveraged position.
 */
export interface Position {
  /**
 * USDC collateral deposited by the trader.
 */
collateral: i128;
  /**
 * Global borrow accumulator index at position open (for lazy fee calc).
 */
entry_borrow_index: i128;
  /**
 * Global funding accumulator index at position open (for lazy fee calc).
 */
entry_funding_index: i128;
  /**
 * Oracle price at the time the position was opened (scaled by 1e7).
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
 * Cumulative funding rate index (signed; positive = longs pay shorts).
 */
acc_funding_index: i128;
  /**
 * Volume-weighted average entry price of all active long positions.
 */
global_long_avg_price: i128;
  /**
 * Volume-weighted average entry price of all active short positions.
 */
global_short_avg_price: i128;
  /**
 * Timestamp of the last keeper index update.
 */
last_index_update: u64;
  /**
 * Total notional size of all open long positions.
 */
long_open_interest: i128;
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
 * How long a cached aggregated price remains valid (in seconds). A
 * `get_price` call within this window of the last fetch returns the
 * cached value without re-querying sources. Must be > 0 and
 * <= `staleness_threshold` (otherwise the cache could outlive a fresh
 * source price and serve stale data).
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
  funding_cut_bps: u32;
  liquidation_threshold_bps: u32;
  max_utilization_ratio: i128;
  min_collateral: i128;
  min_position_lifetime: u64;
}


/**
 * Borrow rate kink curve and funding rate parameters (all in basis points).
 */
export interface BorrowRateConfig {
  base_borrow_rate_bps: i128;
  base_funding_rate_bps: i128;
  optimal_utilization_bps: i128;
  slope1_bps: i128;
  slope2_bps: i128;
}

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
  set_tp_sl: ({trader, symbol, take_profit, stop_loss}: {trader: string, symbol: string, take_profit: i128, stop_loss: i128}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a set_vault transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Wire the linked Vault. ADMIN-only and one-shot: reverts with
   * `AlreadyInitialized` once the Vault is set, so the trusted Vault link
   * is immutable after deploy. Called once by the deployer immediately
   * after the Vault is deployed.
   */
  set_vault: ({caller, vault_address}: {caller: string, vault_address: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a get_market transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_market: ({symbol}: {symbol: string}, options?: MethodOptions) => Promise<AssembledTransaction<MarketInfo>>

  /**
   * Construct and simulate a get_position transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_position: ({trader, symbol}: {trader: string, symbol: string}, options?: MethodOptions) => Promise<AssembledTransaction<Position>>

  /**
   * Construct and simulate a realized_pnl transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  realized_pnl: (options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a bump_position transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  bump_position: ({user_address, symbol}: {user_address: string, symbol: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a enable_market transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  enable_market: ({caller, symbol}: {caller: string, symbol: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a execute_order transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  execute_order: ({caller, trader, symbol}: {caller: string, trader: string, symbol: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a cancel_upgrade transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  cancel_upgrade: ({caller}: {caller: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a disable_market transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  disable_market: ({caller, symbol}: {caller: string, symbol: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a update_indices transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  update_indices: ({caller, symbol}: {caller: string, symbol: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a propose_upgrade transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  propose_upgrade: ({caller, wasm_hash}: {caller: string, wasm_hash: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a get_max_leverage transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_max_leverage: ({symbol}: {symbol: string}, options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a set_max_leverage transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  set_max_leverage: ({caller, symbol, max_leverage}: {caller: string, symbol: string, max_leverage: i128}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a decrease_position transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  decrease_position: ({trader, symbol, size_delta, acceptable_price}: {trader: string, symbol: string, size_delta: i128, acceptable_price: i128}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a increase_position transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  increase_position: ({trader, symbol, size, collateral, is_long, take_profit, stop_loss, acceptable_price}: {trader: string, symbol: string, size: i128, collateral: i128, is_long: boolean, take_profit: i128, stop_loss: i128, acceptable_price: i128}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a is_market_disabled transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  is_market_disabled: ({symbol}: {symbol: string}, options?: MethodOptions) => Promise<AssembledTransaction<boolean>>

  /**
   * Construct and simulate a liquidate_position transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  liquidate_position: ({caller, trader, symbol}: {caller: string, trader: string, symbol: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a deleverage_position transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  deleverage_position: ({caller, trader, symbol}: {caller: string, trader: string, symbol: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a sync_unrealized_pnl transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  sync_unrealized_pnl: (options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a total_unrealized_pnl transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  total_unrealized_pnl: (options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a market_unrealized_pnl transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  market_unrealized_pnl: ({symbol}: {symbol: string}, options?: MethodOptions) => Promise<AssembledTransaction<i128>>

}
export class Client extends ContractClient {
  static async deploy<T = Client>(
        /** Constructor/Initialization Args for the contract's `__constructor` method */
        {config_manager, oracle_router}: {config_manager: string, oracle_router: string},
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
    return ContractClient.deploy({config_manager, oracle_router}, options)
  }
  constructor(public readonly options: ContractClientOptions) {
    super(
      new ContractSpec([ "AAAABAAAAAAAAAAAAAAAFFBvc2l0aW9uTWFuYWdlckVycm9yAAAAGQAAAAAAAAASQWxyZWFkeUluaXRpYWxpemVkAAAAAAABAAAAAAAAAA5Ob3RJbml0aWFsaXplZAAAAAAAAgAAAAAAAAAGUGF1c2VkAAAAAAADAAAARk5ldyB0cmFkZSB3b3VsZCBwdXNoIHZhdWx0IHV0aWxpemF0aW9uIHBhc3QgTWF4VXRpbGl6YXRpb25SYXRpbyAoODUlKS4AAAAAABZVdGlsaXphdGlvbkNhcEJyZWFjaGVkAAAAAAAEAAAAQGRlY3JlYXNlX3Bvc2l0aW9uIGNhbGxlZCBiZWZvcmUgTWluUG9zaXRpb25MaWZldGltZSBoYXMgZWxhcHNlZC4AAAAUUG9zaXRpb25Ob3RPbGRFbm91Z2gAAAAFAAAAAAAAABBQb3NpdGlvbk5vdEZvdW5kAAAABgAAAAAAAAAMVW5hdXRob3JpemVkAAAABwAAAAAAAAAKWmVyb0Ftb3VudAAAAAAACAAAADxsaXF1aWRhdGVfcG9zaXRpb24gY2FsbGVkIGJ1dCB0aGUgcG9zaXRpb24gaXMgc3RpbGwgaGVhbHRoeS4AAAAOSGVhbHRoRmFjdG9yT2sAAAAAAAkAAABCZGVsZXZlcmFnZV9wb3NpdGlvbiBjYWxsZWQgYnV0IEFETCB0cmlnZ2VyIGNvbmRpdGlvbnMgYXJlIG5vdCBtZXQuAAAAAAAPQWRsTm90VHJpZ2dlcmVkAAAAAAoAAAA2UG9zaXRpb24gbGV2ZXJhZ2UgZXhjZWVkcyB0aGUgcGVyLW1hcmtldCBtYXggbGV2ZXJhZ2UuAAAAAAARRXhjZXNzaXZlTGV2ZXJhZ2UAAAAAAAALAAAAMk5vIG1heCBsZXZlcmFnZSBjb25maWd1cmVkIGZvciB0aGlzIG1hcmtldCBzeW1ib2wuAAAAAAATTWFya2V0Tm90Q29uZmlndXJlZAAAAAAMAAAARGV4ZWN1dGVfb3JkZXIgY2FsbGVkIGJ1dCBuZWl0aGVyIFRQIG5vciBTTCB0cmlnZ2VyIGNvbmRpdGlvbiBpcyBtZXQuAAAAEU9yZGVyTm90VHJpZ2dlcmVkAAAAAAAADQAAAEJJbnZhbGlkIHRha2UtcHJvZml0IG9yIHN0b3AtbG9zcyBwcmljZSBmb3IgdGhlIHBvc2l0aW9uIGRpcmVjdGlvbi4AAAAAAAtJbnZhbGlkVHBTbAAAAAAOAAAAVmluY3JlYXNlX3Bvc2l0aW9uIGNhbGxlZCB3aXRoIGBpc19sb25nYCBvcHBvc2l0ZSB0byB0aGUgZXhpc3RpbmcgcG9zaXRpb24ncyBkaXJlY3Rpb24uAAAAAAARRGlyZWN0aW9uTWlzbWF0Y2gAAAAAAAAPAAAANUNvbGxhdGVyYWwgYmVsb3cgdGhlIHByb3RvY29sJ3MgbWluX2NvbGxhdGVyYWwgbGltaXQuAAAAAAAAEkJlbG93TWluQ29sbGF0ZXJhbAAAAAAAEAAAADFBREwgdGFyZ2V0IHBvc2l0aW9uIGlzIG5vdCBwcm9maXRhYmxlIChQbkwgPD0gMCkuAAAAAAAAFkFkbFRhcmdldE5vdFByb2ZpdGFibGUAAAAAABEAAAA0TWF4IGxldmVyYWdlIGV4Y2VlZHMgdGhlIGFic29sdXRlIHNhZmV0eSBjYXAgKDIwMHgpLgAAABNMZXZlcmFnZUNhcEV4Y2VlZGVkAAAAABIAAABGTWFyayBwcmljZSBhdCBleGVjdXRpb24gdGltZSBleGNlZWRlZCB0aGUgdHJhZGVyJ3MgYGFjY2VwdGFibGVfcHJpY2VgLgAAAAAAEFNsaXBwYWdlRXhjZWVkZWQAAAATAAAAc2BzZXRfbWF4X2xldmVyYWdlYCBjYWxsZWQgd2l0aCBhIHZhbHVlIGJlbG93IGBNSU5fTEVWRVJBR0VgLiBVc2UKYGRpc2FibGVfbWFya2V0YCB0byB0YWtlIGEgbWFya2V0IG9mZmxpbmUgaW5zdGVhZC4AAAAAEkxldmVyYWdlQmVsb3dGbG9vcgAAAAAAFAAAAERUcmFkaW5nIGlzIGRpc2FibGVkIGZvciB0aGlzIG1hcmtldCDigJQgYGVuYWJsZV9tYXJrZXRgIHJlLW9wZW5zIGl0LgAAAA5NYXJrZXREaXNhYmxlZAAAAAAAFQAAAHhgZGVjcmVhc2VfcG9zaXRpb25gIGNhbGxlZCB3aXRoIGBzaXplX2RlbHRhID4gcG9zLnNpemVgLiBVc2UKYHBvcy5zaXplYCAob3Igc2ltcGx5IGNsb3NlIGZ1bGx5KSBpbnN0ZWFkIG9mIG92ZXItY2xvc2luZy4AAAAYU2l6ZURlbHRhRXhjZWVkc1Bvc2l0aW9uAAAAFgAAAENgdXBncmFkZWAgcmVqZWN0ZWQg4oCUIG5vIGBwcm9wb3NlX3VwZ3JhZGVgIHdhcyBtYWRlIGJlZm9yZSBjb21taXQuAAAAABBOb1BlbmRpbmdVcGdyYWRlAAAAFwAAADRgdXBncmFkZWAgcmVqZWN0ZWQg4oCUIHRpbWVsb2NrIGhhcyBub3QgZWxhcHNlZCB5ZXQuAAAAGVVwZ3JhZGVUaW1lbG9ja05vdEVsYXBzZWQAAAAAAAAYAAAAXmB1cGdyYWRlYCByZWplY3RlZCDigJQgYG5ld193YXNtX2hhc2hgIGRvZXMgbm90IG1hdGNoIHRoZSBwcm9wb3NlZApgUGVuZGluZ1VwZ3JhZGUud2FzbV9oYXNoYC4AAAAAABNVcGdyYWRlSGFzaE1pc21hdGNoAAAAABk=",
        "AAAABQAAAAAAAAAAAAAAA0FkbAAAAAABAAAAA2FkbAAAAAAFAAAAAAAAAAZ0cmFkZXIAAAAAABMAAAABAAAAAAAAAAZzeW1ib2wAAAAAABEAAAAAAAAAAAAAAARzaXplAAAACwAAAAAAAAAAAAAAA3BubAAAAAALAAAAAAAAAAAAAAAKbWFya19wcmljZQAAAAAACwAAAAAAAAAB",
        "AAAABQAAAAAAAAAAAAAABVBhdXNlAAAAAAAAAQAAAAVwYXVzZQAAAAAAAAIAAAAAAAAACWlzX3BhdXNlZAAAAAAAAAEAAAAAAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAAAAAAAQ==",
        "AAAABQAAAAAAAAAAAAAAB1NldFRwU2wAAAAAAQAAAAV0cF9zbAAAAAAAAAQAAAAAAAAABnRyYWRlcgAAAAAAEwAAAAEAAAAAAAAABnN5bWJvbAAAAAAAEQAAAAAAAAAAAAAAC3Rha2VfcHJvZml0AAAAAAsAAAAAAAAAAAAAAAlzdG9wX2xvc3MAAAAAAAALAAAAAAAAAAE=",
        "AAAABQAAAAAAAAAAAAAACUxpcXVpZGF0ZQAAAAAAAAEAAAADbGlxAAAAAAkAAAAAAAAABnRyYWRlcgAAAAAAEwAAAAEAAAAAAAAABnN5bWJvbAAAAAAAEQAAAAAAAAAAAAAABHNpemUAAAALAAAAAAAAAAAAAAAKY29sbGF0ZXJhbAAAAAAACwAAAAAAAAAAAAAAA3BubAAAAAALAAAAAAAAAAAAAAAKYm9ycm93X2ZlZQAAAAAACwAAAAAAAAAAAAAAC2Z1bmRpbmdfZmVlAAAAAAsAAAAAAAAAAAAAAAptYXJrX3ByaWNlAAAAAAALAAAAAAAAAAAAAAAIZXhlY3V0b3IAAAATAAAAAAAAAAE=",
        "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",
        "AAAAAQAAAEtDb21wb3NpdGUga2V5IGZvciBsb29raW5nIHVwIGEgcG9zaXRpb24gYnkgdHJhZGVyIGFkZHJlc3MgYW5kIGFzc2V0IHN5bWJvbC4AAAAAAAAAAAtQb3NpdGlvbktleQAAAAACAAAAAAAAAAZzeW1ib2wAAAAAABEAAAAAAAAABnRyYWRlcgAAAAAAEw==",
        "AAAAAAAAAAAAAAAFcGF1c2UAAAAAAAABAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAA",
        "AAAAAAAAAAAAAAAHbWlncmF0ZQAAAAACAAAAAAAAAA5taWdyYXRpb25fZGF0YQAAAAAH0AAAAA1NaWdyYXRpb25EYXRhAAAAAAAAAAAAAAhvcGVyYXRvcgAAABMAAAAA",
        "AAAAAAAAAAAAAAAHdW5wYXVzZQAAAAABAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAA",
        "AAAAAAAAAAAAAAAHdXBncmFkZQAAAAACAAAAAAAAAA1uZXdfd2FzbV9oYXNoAAAAAAAD7gAAACAAAAAAAAAACG9wZXJhdG9yAAAAEwAAAAA=",
        "AAAAAAAAAAAAAAAJc2V0X3RwX3NsAAAAAAAABAAAAAAAAAAGdHJhZGVyAAAAAAATAAAAAAAAAAZzeW1ib2wAAAAAABEAAAAAAAAAC3Rha2VfcHJvZml0AAAAAAsAAAAAAAAACXN0b3BfbG9zcwAAAAAAAAsAAAAA",
        "AAAAAAAAAOJXaXJlIHRoZSBsaW5rZWQgVmF1bHQuIEFETUlOLW9ubHkgYW5kIG9uZS1zaG90OiByZXZlcnRzIHdpdGgKYEFscmVhZHlJbml0aWFsaXplZGAgb25jZSB0aGUgVmF1bHQgaXMgc2V0LCBzbyB0aGUgdHJ1c3RlZCBWYXVsdCBsaW5rCmlzIGltbXV0YWJsZSBhZnRlciBkZXBsb3kuIENhbGxlZCBvbmNlIGJ5IHRoZSBkZXBsb3llciBpbW1lZGlhdGVseQphZnRlciB0aGUgVmF1bHQgaXMgZGVwbG95ZWQuAAAAAAAJc2V0X3ZhdWx0AAAAAAAAAgAAAAAAAAAGY2FsbGVyAAAAAAATAAAAAAAAAA12YXVsdF9hZGRyZXNzAAAAAAAAEwAAAAA=",
        "AAAAAAAAAAAAAAAKZ2V0X21hcmtldAAAAAAAAQAAAAAAAAAGc3ltYm9sAAAAAAARAAAAAQAAB9AAAAAKTWFya2V0SW5mbwAA",
        "AAAAAAAAAAAAAAAMZ2V0X3Bvc2l0aW9uAAAAAgAAAAAAAAAGdHJhZGVyAAAAAAATAAAAAAAAAAZzeW1ib2wAAAAAABEAAAABAAAH0AAAAAhQb3NpdGlvbg==",
        "AAAAAAAAAAAAAAAMcmVhbGl6ZWRfcG5sAAAAAAAAAAEAAAAL",
        "AAAAAAAAAXNBdG9taWMtd2l0aC1kZXBsb3kgaW5pdGlhbGl6YXRpb24gKFNvcm9iYW4gY29uc3RydWN0b3IpLiBCaW5kcyB0aGUKbGlua2VkIENvbmZpZ01hbmFnZXIgYW5kIE9yYWNsZVJvdXRlciBvbmNlLCBpbnNpZGUgdGhlIGRlcGxveQp0cmFuc2FjdGlvbiwgc28gaW5pdCBjYW5ub3QgYmUgZnJvbnQtcnVuLiBUaGUgVmF1bHQgbGluayBpcyB3aXJlZAphZnRlcndhcmRzIHZpYSBgc2V0X3ZhdWx0YCDigJQgVmF1bHQgYmluZHMgaXRzIHRydXN0ZWQgUG9zaXRpb25NYW5hZ2VyIGluCml0cyBvd24gY29uc3RydWN0b3IsIHNvIFBNIGlzIGRlcGxveWVkIGZpcnN0IGFuZCBjYW5ub3Qga25vdyB0aGUgVmF1bHQKYWRkcmVzcyB1bnRpbCB0aGUgVmF1bHQgZXhpc3RzLgAAAAANX19jb25zdHJ1Y3RvcgAAAAAAAAIAAAAAAAAADmNvbmZpZ19tYW5hZ2VyAAAAAAATAAAAAAAAAA1vcmFjbGVfcm91dGVyAAAAAAAAEwAAAAA=",
        "AAAAAAAAAAAAAAANYnVtcF9wb3NpdGlvbgAAAAAAAAIAAAAAAAAADHVzZXJfYWRkcmVzcwAAABMAAAAAAAAABnN5bWJvbAAAAAAAEQAAAAA=",
        "AAAAAAAAAAAAAAANZW5hYmxlX21hcmtldAAAAAAAAAIAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAAAAAAGc3ltYm9sAAAAAAARAAAAAA==",
        "AAAAAAAAAAAAAAANZXhlY3V0ZV9vcmRlcgAAAAAAAAMAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAAAAAAGdHJhZGVyAAAAAAATAAAAAAAAAAZzeW1ib2wAAAAAABEAAAAA",
        "AAAAAAAAAAAAAAAOY2FuY2VsX3VwZ3JhZGUAAAAAAAEAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAA=",
        "AAAAAAAAAAAAAAAOZGlzYWJsZV9tYXJrZXQAAAAAAAIAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAAAAAAGc3ltYm9sAAAAAAARAAAAAA==",
        "AAAAAAAAAAAAAAAOdXBkYXRlX2luZGljZXMAAAAAAAIAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAAAAAAGc3ltYm9sAAAAAAARAAAAAA==",
        "AAAAAAAAAAAAAAAPcHJvcG9zZV91cGdyYWRlAAAAAAIAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAAAAAAJd2FzbV9oYXNoAAAAAAAD7gAAACAAAAAA",
        "AAAAAAAAAAAAAAAQZ2V0X21heF9sZXZlcmFnZQAAAAEAAAAAAAAABnN5bWJvbAAAAAAAEQAAAAEAAAAL",
        "AAAAAAAAAAAAAAAQc2V0X21heF9sZXZlcmFnZQAAAAMAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAAAAAAGc3ltYm9sAAAAAAARAAAAAAAAAAxtYXhfbGV2ZXJhZ2UAAAALAAAAAA==",
        "AAAAAAAAAAAAAAARZGVjcmVhc2VfcG9zaXRpb24AAAAAAAAEAAAAAAAAAAZ0cmFkZXIAAAAAABMAAAAAAAAABnN5bWJvbAAAAAAAEQAAAAAAAAAKc2l6ZV9kZWx0YQAAAAAACwAAAAAAAAAQYWNjZXB0YWJsZV9wcmljZQAAAAsAAAAA",
        "AAAAAAAAAAAAAAARaW5jcmVhc2VfcG9zaXRpb24AAAAAAAAIAAAAAAAAAAZ0cmFkZXIAAAAAABMAAAAAAAAABnN5bWJvbAAAAAAAEQAAAAAAAAAEc2l6ZQAAAAsAAAAAAAAACmNvbGxhdGVyYWwAAAAAAAsAAAAAAAAAB2lzX2xvbmcAAAAAAQAAAAAAAAALdGFrZV9wcm9maXQAAAAACwAAAAAAAAAJc3RvcF9sb3NzAAAAAAAACwAAAAAAAAAQYWNjZXB0YWJsZV9wcmljZQAAAAsAAAAA",
        "AAAAAAAAAAAAAAASaXNfbWFya2V0X2Rpc2FibGVkAAAAAAABAAAAAAAAAAZzeW1ib2wAAAAAABEAAAABAAAAAQ==",
        "AAAAAAAAAAAAAAASbGlxdWlkYXRlX3Bvc2l0aW9uAAAAAAADAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAAAAAABnRyYWRlcgAAAAAAEwAAAAAAAAAGc3ltYm9sAAAAAAARAAAAAA==",
        "AAAAAAAAAAAAAAATZGVsZXZlcmFnZV9wb3NpdGlvbgAAAAADAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAAAAAABnRyYWRlcgAAAAAAEwAAAAAAAAAGc3ltYm9sAAAAAAARAAAAAA==",
        "AAAAAAAAAAAAAAATc3luY191bnJlYWxpemVkX3BubAAAAAAAAAAAAA==",
        "AAAAAAAAAAAAAAAUdG90YWxfdW5yZWFsaXplZF9wbmwAAAAAAAAAAQAAAAs=",
        "AAAAAAAAAAAAAAAVbWFya2V0X3VucmVhbGl6ZWRfcG5sAAAAAAAAAQAAAAAAAAAGc3ltYm9sAAAAAAARAAAAAQAAAAs=",
        "AAAAAQAAADVSZXByZXNlbnRzIGEgc2luZ2xlIHRyYWRlcidzIG9wZW4gbGV2ZXJhZ2VkIHBvc2l0aW9uLgAAAAAAAAAAAAAIUG9zaXRpb24AAAAKAAAAKFVTREMgY29sbGF0ZXJhbCBkZXBvc2l0ZWQgYnkgdGhlIHRyYWRlci4AAAAKY29sbGF0ZXJhbAAAAAAACwAAAEVHbG9iYWwgYm9ycm93IGFjY3VtdWxhdG9yIGluZGV4IGF0IHBvc2l0aW9uIG9wZW4gKGZvciBsYXp5IGZlZSBjYWxjKS4AAAAAAAASZW50cnlfYm9ycm93X2luZGV4AAAAAAALAAAARkdsb2JhbCBmdW5kaW5nIGFjY3VtdWxhdG9yIGluZGV4IGF0IHBvc2l0aW9uIG9wZW4gKGZvciBsYXp5IGZlZSBjYWxjKS4AAAAAABNlbnRyeV9mdW5kaW5nX2luZGV4AAAAAAsAAABBT3JhY2xlIHByaWNlIGF0IHRoZSB0aW1lIHRoZSBwb3NpdGlvbiB3YXMgb3BlbmVkIChzY2FsZWQgYnkgMWU3KS4AAAAAAAALZW50cnlfcHJpY2UAAAAACwAAAIxGbGF0IFVTREMgZmVlIGVzY3Jvd2VkIHdoZW4gVFAgb3IgU0wgaXMgc2V0LiBQYWlkIHRvIGV4ZWN1dG9yIG9uIHRyaWdnZXIsIHJlZnVuZGVkIG9uIHVzZXIgY2xvc2UgLyBBREwsIGZvcmZlaXRlZCB0byByZXZlbnVlIG9uIGxpcXVpZGF0aW9uLgAAABRleGVjdXRpb25fZmVlX2VzY3JvdwAAAAsAAAAsVHJ1ZSBmb3IgYSBsb25nIHBvc2l0aW9uLCBmYWxzZSBmb3IgYSBzaG9ydC4AAAAHaXNfbG9uZwAAAAABAAAAT0Jsb2NrIHRpbWVzdGFtcCB3aGVuIHRoZSBwb3NpdGlvbiB3YXMgbGFzdCBpbmNyZWFzZWQgKGFudGktZnJvbnQtcnVubmluZyBsb2NrKS4AAAAAE2xhc3RfaW5jcmVhc2VkX3RpbWUAAAAABgAAACZOb3Rpb25hbCBzaXplIG9mIHRoZSBwb3NpdGlvbiBpbiBVU0RDLgAAAAAABHNpemUAAAALAAAALVN0b3AtbG9zcyBwcmljZSAoc2NhbGVkIGJ5IDFlNykuIDAgPSBub3Qgc2V0LgAAAAAAAAlzdG9wX2xvc3MAAAAAAAALAAAAL1Rha2UtcHJvZml0IHByaWNlIChzY2FsZWQgYnkgMWU3KS4gMCA9IG5vdCBzZXQuAAAAAAt0YWtlX3Byb2ZpdAAAAAAL",
        "AAAAAQAAADhHbG9iYWwgbWFya2V0IHN0YXRlIGZvciBhIHNpbmdsZSB0cmFkZWFibGUgYXNzZXQgc3ltYm9sLgAAAAAAAAAKTWFya2V0SW5mbwAAAAAABwAAADxDdW11bGF0aXZlIGJvcnJvdyBmZWUgaW5kZXggKGdyb3dzIG1vbm90b25pY2FsbHkgd2l0aCB0aW1lKS4AAAAQYWNjX2JvcnJvd19pbmRleAAAAAsAAABEQ3VtdWxhdGl2ZSBmdW5kaW5nIHJhdGUgaW5kZXggKHNpZ25lZDsgcG9zaXRpdmUgPSBsb25ncyBwYXkgc2hvcnRzKS4AAAARYWNjX2Z1bmRpbmdfaW5kZXgAAAAAAAALAAAAQVZvbHVtZS13ZWlnaHRlZCBhdmVyYWdlIGVudHJ5IHByaWNlIG9mIGFsbCBhY3RpdmUgbG9uZyBwb3NpdGlvbnMuAAAAAAAAFWdsb2JhbF9sb25nX2F2Z19wcmljZQAAAAAAAAsAAABCVm9sdW1lLXdlaWdodGVkIGF2ZXJhZ2UgZW50cnkgcHJpY2Ugb2YgYWxsIGFjdGl2ZSBzaG9ydCBwb3NpdGlvbnMuAAAAAAAWZ2xvYmFsX3Nob3J0X2F2Z19wcmljZQAAAAAACwAAACpUaW1lc3RhbXAgb2YgdGhlIGxhc3Qga2VlcGVyIGluZGV4IHVwZGF0ZS4AAAAAABFsYXN0X2luZGV4X3VwZGF0ZQAAAAAAAAYAAAAvVG90YWwgbm90aW9uYWwgc2l6ZSBvZiBhbGwgb3BlbiBsb25nIHBvc2l0aW9ucy4AAAAAEmxvbmdfb3Blbl9pbnRlcmVzdAAAAAAACwAAADBUb3RhbCBub3Rpb25hbCBzaXplIG9mIGFsbCBvcGVuIHNob3J0IHBvc2l0aW9ucy4AAAATc2hvcnRfb3Blbl9pbnRlcmVzdAAAAAAL",
        "AAAAAQAAAC5HbG9iYWwgc2FmZXR5IHRocmVzaG9sZHMgZm9yIHByaWNlIHZhbGlkYXRpb24uAAAAAAAAAAAADE9yYWNsZUNvbmZpZwAAAAQAAAEkSG93IGxvbmcgYSBjYWNoZWQgYWdncmVnYXRlZCBwcmljZSByZW1haW5zIHZhbGlkIChpbiBzZWNvbmRzKS4gQQpgZ2V0X3ByaWNlYCBjYWxsIHdpdGhpbiB0aGlzIHdpbmRvdyBvZiB0aGUgbGFzdCBmZXRjaCByZXR1cm5zIHRoZQpjYWNoZWQgdmFsdWUgd2l0aG91dCByZS1xdWVyeWluZyBzb3VyY2VzLiBNdXN0IGJlID4gMCBhbmQKPD0gYHN0YWxlbmVzc190aHJlc2hvbGRgIChvdGhlcndpc2UgdGhlIGNhY2hlIGNvdWxkIG91dGxpdmUgYSBmcmVzaApzb3VyY2UgcHJpY2UgYW5kIHNlcnZlIHN0YWxlIGRhdGEpLgAAAA5jYWNoZV9kdXJhdGlvbgAAAAAABgAAAIpNYXhpbXVtIGFsbG93ZWQgc3ByZWFkIGJldHdlZW4gb3JhY2xlIHNvdXJjZXMgaW4gYmFzaXMgcG9pbnRzCihlLmcuLCAxMDAgPSAxJSkuIEJvdW5kZWQgYXQgYHNoYXJlZDo6Y29uc3RhbnRzOjpNQVhfREVWSUFUSU9OX0JQU19DRUlMSU5HYC4AAAAAABFtYXhfZGV2aWF0aW9uX2JwcwAAAAAAAAsAAADjTWluaW11bSBudW1iZXIgb2Ygc291cmNlIHJlc3BvbnNlcyB0aGF0IG11c3QgYWdyZWUgd2l0aGluCmBtYXhfZGV2aWF0aW9uX2Jwc2AgZm9yIE9yYWNsZVJvdXRlciB0byByZXR1cm4gYSBwcmljZS4gRmxvb3JlZCBhdApgc2hhcmVkOjpjb25zdGFudHM6Ok1JTl9SRVFVSVJFRF9TT1VSQ0VTX0ZMT09SYCwgY2VpbGluZ2VkIGF0CmBzaGFyZWQ6OmNvbnN0YW50czo6TUFYX09SQUNMRV9TT1VSQ0VTYC4AAAAAFG1pbl9yZXF1aXJlZF9zb3VyY2VzAAAABAAAAFlNYXhpbXVtIGFnZSBvZiBhbiBleHRlcm5hbCBTRVAtNDAgcHJpY2UgZmVlZCBiZWZvcmUgaXQgaXMgcmVqZWN0ZWQKYXMgc3RhbGUgKGluIHNlY29uZHMpLgAAAAAAABNzdGFsZW5lc3NfdGhyZXNob2xkAAAAAAY=",
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
        "AAAAAQAAACtHbG9iYWwgcHJvdG9jb2wgcmlzayBhbmQgdGltaW5nIHBhcmFtZXRlcnMuAAAAAAAAAAAOUHJvdG9jb2xMaW1pdHMAAAAAAAgAAAAAAAAAC2FkbF9wbmxfYnBzAAAAAAQAAAAAAAAAE2FkbF91dGlsaXphdGlvbl9icHMAAAAABAAAAAAAAAARY29vbGRvd25fZHVyYXRpb24AAAAAAAAGAAAAAAAAAA9mdW5kaW5nX2N1dF9icHMAAAAABAAAAAAAAAAZbGlxdWlkYXRpb25fdGhyZXNob2xkX2JwcwAAAAAAAAQAAAAAAAAAFW1heF91dGlsaXphdGlvbl9yYXRpbwAAAAAAAAsAAAAAAAAADm1pbl9jb2xsYXRlcmFsAAAAAAALAAAAAAAAABVtaW5fcG9zaXRpb25fbGlmZXRpbWUAAAAAAAAG",
        "AAAAAQAAAElCb3Jyb3cgcmF0ZSBraW5rIGN1cnZlIGFuZCBmdW5kaW5nIHJhdGUgcGFyYW1ldGVycyAoYWxsIGluIGJhc2lzIHBvaW50cykuAAAAAAAAAAAAABBCb3Jyb3dSYXRlQ29uZmlnAAAABQAAAAAAAAAUYmFzZV9ib3Jyb3dfcmF0ZV9icHMAAAALAAAAAAAAABViYXNlX2Z1bmRpbmdfcmF0ZV9icHMAAAAAAAALAAAAAAAAABdvcHRpbWFsX3V0aWxpemF0aW9uX2JwcwAAAAALAAAAAAAAAApzbG9wZTFfYnBzAAAAAAALAAAAAAAAAApzbG9wZTJfYnBzAAAAAAAL" ]),
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
        realized_pnl: this.txFromJSON<i128>,
        bump_position: this.txFromJSON<null>,
        enable_market: this.txFromJSON<null>,
        execute_order: this.txFromJSON<null>,
        cancel_upgrade: this.txFromJSON<null>,
        disable_market: this.txFromJSON<null>,
        update_indices: this.txFromJSON<null>,
        propose_upgrade: this.txFromJSON<null>,
        get_max_leverage: this.txFromJSON<i128>,
        set_max_leverage: this.txFromJSON<null>,
        decrease_position: this.txFromJSON<null>,
        increase_position: this.txFromJSON<null>,
        is_market_disabled: this.txFromJSON<boolean>,
        liquidate_position: this.txFromJSON<null>,
        deleverage_position: this.txFromJSON<null>,
        sync_unrealized_pnl: this.txFromJSON<null>,
        total_unrealized_pnl: this.txFromJSON<i128>,
        market_unrealized_pnl: this.txFromJSON<i128>
  }
}