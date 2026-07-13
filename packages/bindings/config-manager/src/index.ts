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




export const ConfigManagerError = {
  1: {message:"AlreadyInitialized"},
  2: {message:"NotInitialized"},
  3: {message:"Unauthorized"},
  /**
   * `set_upgrade_timelock` called with seconds below `MIN_UPGRADE_TIMELOCK`.
   */
  6: {message:"UpgradeTimelockTooShort"},
  /**
   * `propose_admin(caller, new_admin)` rejected because `caller == new_admin`.
   */
  7: {message:"InvalidAdminProposal"},
  /**
   * `accept_admin` rejected — caller is not the currently pending admin.
   */
  8: {message:"NotPendingAdmin"},
  /**
   * `accept_admin` rejected — there is no pending admin proposal.
   */
  9: {message:"NoPendingAdmin"},
  /**
   * `upgrade` rejected — no `propose_upgrade` was made before commit.
   * The two-step upgrade flow requires a prior proposal.
   */
  10: {message:"NoPendingUpgrade"},
  /**
   * `upgrade` rejected — timelock has not elapsed yet.
   */
  11: {message:"UpgradeTimelockNotElapsed"},
  /**
   * `upgrade` rejected — `new_wasm_hash` does not match the proposed
   * `PendingUpgrade.wasm_hash`.
   */
  12: {message:"UpgradeHashMismatch"},
  /**
   * `accept_admin` rejected — the proposal is older than
   * `ADMIN_PROPOSAL_TTL_SECS`.
   */
  13: {message:"AdminProposalExpired"},
  /**
   * `set_upgrade_timelock` called with seconds above
   * `MAX_UPGRADE_TIMELOCK_SECS`.
   */
  14: {message:"UpgradeTimelockTooLong"},
  /**
   * FeeSplits components (lp/dev/staker) do not sum to exactly BPS.
   */
  22: {message:"InvalidFeeSplitSum"},
  /**
   * `min_collateral` is not strictly positive.
   */
  30: {message:"InvalidMinCollateral"},
  /**
   * `max_utilization_ratio` is out of (0, BPS] range.
   */
  31: {message:"InvalidMaxUtilization"},
  /**
   * Reserved to keep subsequent error discriminants stable.
   */
  32: {message:"ReservedProtocolLimit32"},
  /**
   * `adl_pnl_bps` is below `MIN_ADL_PNL_BPS` or above BPS.
   */
  33: {message:"InvalidAdlPnl"},
  /**
   * `adl_utilization_bps` is out of (0, BPS] range.
   */
  34: {message:"InvalidAdlUtilization"},
  /**
   * `liquidation_threshold_bps` is below `MIN_LIQUIDATION_THRESHOLD_BPS`
   * or exceeds 10% of collateral.
   */
  35: {message:"InvalidLiquidationThreshold"},
  /**
   * `cooldown_duration` exceeds `MAX_COOLDOWN_DURATION`.
   */
  36: {message:"InvalidCooldownDuration"},
  /**
   * `min_position_lifetime` exceeds `MAX_MIN_POSITION_LIFETIME_SECS`.
   */
  37: {message:"InvalidMinPositionLifetime"},
  /**
   * A CarryingFeeConfig rate is negative.
   */
  40: {message:"InvalidBorrowRateNegative"},
  /**
   * `optimal_utilization_bps` is out of (0, BPS] range.
   */
  41: {message:"InvalidOptimalUtilization"},
  /**
   * `slope2_bps < slope1_bps` — kink curve must be non-decreasing.
   */
  42: {message:"InvalidSlopeOrdering"},
  /**
   * `slope2_bps` exceeds `MAX_SLOPE2_BPS`.
   */
  43: {message:"InvalidSlopeTooSteep"},
  /**
   * `open_fee_bps` exceeds `MAX_OPEN_FEE_BPS`.
   */
  44: {message:"InvalidOpenFee"},
  /**
   * `liquidation_bounty_bps` exceeds `MAX_LIQUIDATION_BOUNTY_BPS`.
   */
  45: {message:"InvalidLiquidationBounty"},
  /**
   * `tp_sl_execution_fee` is negative or exceeds `MAX_TP_SL_EXECUTION_FEE`.
   */
  46: {message:"InvalidTpSlExecutionFee"},
  /**
   * `base_borrow_rate_bps` exceeds `MAX_BASE_BORROW_RATE_BPS`.
   */
  47: {message:"InvalidBaseBorrowRate"},
  /**
   * `max_skew_rate_bps` exceeds `MAX_SKEW_RATE_BPS`.
   */
  48: {message:"InvalidMaxSkewRate"}
}









export type StorageKey = {tag: "Initialized", values: void} | {tag: "FeeSplits", values: void} | {tag: "FeeConfig", values: void} | {tag: "ProtocolLimits", values: void} | {tag: "CarryingFeeConfig", values: void} | {tag: "UpgradeTimelock", values: void} | {tag: "PendingAdmin", values: void} | {tag: "Version", values: void};


/**
 * Pending admin transfer awaiting `accept_admin`. The proposal timestamp
 * bounds its lifetime to `ADMIN_PROPOSAL_TTL_SECS`.
 */
export interface PendingAdminProposal {
  admin: string;
  proposed_at: u64;
}

export const RoleTransferError = {
  2200: {message:"NoPendingTransfer"},
  2201: {message:"InvalidLiveUntilLedger"},
  2202: {message:"InvalidPendingAccount"}
}





export const AccessControlError = {
  2000: {message:"Unauthorized"},
  2001: {message:"AdminNotSet"},
  2002: {message:"IndexOutOfBounds"},
  2003: {message:"AdminRoleNotFound"},
  2004: {message:"RoleCountIsNotZero"},
  2005: {message:"RoleNotFound"},
  2006: {message:"AdminAlreadySet"},
  2007: {message:"RoleNotHeld"},
  2008: {message:"RoleIsEmpty"},
  2009: {message:"TransferInProgress"},
  2010: {message:"MaxRolesExceeded"}
}




/**
 * Storage key for enumeration of accounts per role.
 */
export interface RoleAccountKey {
  index: u32;
  role: string;
}

/**
 * Storage keys for the data associated with the access control
 */
export type AccessControlStorageKey = {tag: "ExistingRoles", values: void} | {tag: "RoleAccounts", values: readonly [RoleAccountKey]} | {tag: "HasRole", values: readonly [string, string]} | {tag: "RoleAccountsCount", values: readonly [string]} | {tag: "RoleAdmin", values: readonly [string]} | {tag: "Admin", values: void} | {tag: "PendingAdmin", values: void};

export const OwnableError = {
  2100: {message:"OwnerNotSet"},
  2101: {message:"TransferInProgress"},
  2102: {message:"OwnerAlreadySet"}
}




/**
 * Storage keys for `Ownable` utility.
 */
export type OwnableStorageKey = {tag: "Owner", values: void} | {tag: "PendingOwner", values: void};


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
   * Construct and simulate a has_role transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  has_role: ({role, account}: {role: string, account: string}, options?: MethodOptions) => Promise<AssembledTransaction<boolean>>

  /**
   * Construct and simulate a grant_role transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  grant_role: ({caller, role, account}: {caller: string, role: string, account: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a revoke_role transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  revoke_role: ({caller, role, account}: {caller: string, role: string, account: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a accept_admin transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  accept_admin: ({new_admin}: {new_admin: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a propose_admin transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  propose_admin: ({caller, new_admin}: {caller: string, new_admin: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a cancel_upgrade transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  cancel_upgrade: ({caller}: {caller: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a get_fee_config transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_fee_config: (options?: MethodOptions) => Promise<AssembledTransaction<FeeConfig>>

  /**
   * Construct and simulate a get_fee_splits transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_fee_splits: (options?: MethodOptions) => Promise<AssembledTransaction<FeeSplits>>

  /**
   * Construct and simulate a set_fee_config transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  set_fee_config: ({caller, config}: {caller: string, config: FeeConfig}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a propose_upgrade transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  propose_upgrade: ({caller, wasm_hash}: {caller: string, wasm_hash: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a bump_config_state transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  bump_config_state: (options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a get_pending_admin transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_pending_admin: (options?: MethodOptions) => Promise<AssembledTransaction<Option<string>>>

  /**
   * Construct and simulate a update_fee_splits transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  update_fee_splits: ({caller, fee_splits}: {caller: string, fee_splits: FeeSplits}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a get_protocol_limits transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_protocol_limits: (options?: MethodOptions) => Promise<AssembledTransaction<ProtocolLimits>>

  /**
   * Construct and simulate a get_upgrade_timelock transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_upgrade_timelock: (options?: MethodOptions) => Promise<AssembledTransaction<u64>>

  /**
   * Construct and simulate a set_upgrade_timelock transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  set_upgrade_timelock: ({caller, seconds}: {caller: string, seconds: u64}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a cancel_admin_proposal transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  cancel_admin_proposal: ({caller}: {caller: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a update_protocol_limits transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  update_protocol_limits: ({caller, limits}: {caller: string, limits: ProtocolLimits}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a get_carrying_fee_config transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_carrying_fee_config: (options?: MethodOptions) => Promise<AssembledTransaction<CarryingFeeConfig>>

  /**
   * Construct and simulate a update_carrying_fee_config transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  update_carrying_fee_config: ({caller, config}: {caller: string, config: CarryingFeeConfig}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

}
export class Client extends ContractClient {
  static async deploy<T = Client>(
        /** Constructor/Initialization Args for the contract's `__constructor` method */
        {admin_address}: {admin_address: string},
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
    return ContractClient.deploy({admin_address}, options)
  }
  constructor(public readonly options: ContractClientOptions) {
    super(
      new ContractSpec([ "AAAABAAAAAAAAAAAAAAAEkNvbmZpZ01hbmFnZXJFcnJvcgAAAAAAHgAAAAAAAAASQWxyZWFkeUluaXRpYWxpemVkAAAAAAABAAAAAAAAAA5Ob3RJbml0aWFsaXplZAAAAAAAAgAAAAAAAAAMVW5hdXRob3JpemVkAAAAAwAAAEhgc2V0X3VwZ3JhZGVfdGltZWxvY2tgIGNhbGxlZCB3aXRoIHNlY29uZHMgYmVsb3cgYE1JTl9VUEdSQURFX1RJTUVMT0NLYC4AAAAXVXBncmFkZVRpbWVsb2NrVG9vU2hvcnQAAAAABgAAAEpgcHJvcG9zZV9hZG1pbihjYWxsZXIsIG5ld19hZG1pbilgIHJlamVjdGVkIGJlY2F1c2UgYGNhbGxlciA9PSBuZXdfYWRtaW5gLgAAAAAAFEludmFsaWRBZG1pblByb3Bvc2FsAAAABwAAAEZgYWNjZXB0X2FkbWluYCByZWplY3RlZCDigJQgY2FsbGVyIGlzIG5vdCB0aGUgY3VycmVudGx5IHBlbmRpbmcgYWRtaW4uAAAAAAAPTm90UGVuZGluZ0FkbWluAAAAAAgAAAA/YGFjY2VwdF9hZG1pbmAgcmVqZWN0ZWQg4oCUIHRoZXJlIGlzIG5vIHBlbmRpbmcgYWRtaW4gcHJvcG9zYWwuAAAAAA5Ob1BlbmRpbmdBZG1pbgAAAAAACQAAAHhgdXBncmFkZWAgcmVqZWN0ZWQg4oCUIG5vIGBwcm9wb3NlX3VwZ3JhZGVgIHdhcyBtYWRlIGJlZm9yZSBjb21taXQuClRoZSB0d28tc3RlcCB1cGdyYWRlIGZsb3cgcmVxdWlyZXMgYSBwcmlvciBwcm9wb3NhbC4AAAAQTm9QZW5kaW5nVXBncmFkZQAAAAoAAAA0YHVwZ3JhZGVgIHJlamVjdGVkIOKAlCB0aW1lbG9jayBoYXMgbm90IGVsYXBzZWQgeWV0LgAAABlVcGdyYWRlVGltZWxvY2tOb3RFbGFwc2VkAAAAAAAACwAAAF5gdXBncmFkZWAgcmVqZWN0ZWQg4oCUIGBuZXdfd2FzbV9oYXNoYCBkb2VzIG5vdCBtYXRjaCB0aGUgcHJvcG9zZWQKYFBlbmRpbmdVcGdyYWRlLndhc21faGFzaGAuAAAAAAATVXBncmFkZUhhc2hNaXNtYXRjaAAAAAAMAAAAUWBhY2NlcHRfYWRtaW5gIHJlamVjdGVkIOKAlCB0aGUgcHJvcG9zYWwgaXMgb2xkZXIgdGhhbgpgQURNSU5fUFJPUE9TQUxfVFRMX1NFQ1NgLgAAAAAAABRBZG1pblByb3Bvc2FsRXhwaXJlZAAAAA0AAABNYHNldF91cGdyYWRlX3RpbWVsb2NrYCBjYWxsZWQgd2l0aCBzZWNvbmRzIGFib3ZlCmBNQVhfVVBHUkFERV9USU1FTE9DS19TRUNTYC4AAAAAAAAWVXBncmFkZVRpbWVsb2NrVG9vTG9uZwAAAAAADgAAAD9GZWVTcGxpdHMgY29tcG9uZW50cyAobHAvZGV2L3N0YWtlcikgZG8gbm90IHN1bSB0byBleGFjdGx5IEJQUy4AAAAAEkludmFsaWRGZWVTcGxpdFN1bQAAAAAAFgAAACpgbWluX2NvbGxhdGVyYWxgIGlzIG5vdCBzdHJpY3RseSBwb3NpdGl2ZS4AAAAAABRJbnZhbGlkTWluQ29sbGF0ZXJhbAAAAB4AAAAxYG1heF91dGlsaXphdGlvbl9yYXRpb2AgaXMgb3V0IG9mICgwLCBCUFNdIHJhbmdlLgAAAAAAABVJbnZhbGlkTWF4VXRpbGl6YXRpb24AAAAAAAAfAAAAN1Jlc2VydmVkIHRvIGtlZXAgc3Vic2VxdWVudCBlcnJvciBkaXNjcmltaW5hbnRzIHN0YWJsZS4AAAAAF1Jlc2VydmVkUHJvdG9jb2xMaW1pdDMyAAAAACAAAAA2YGFkbF9wbmxfYnBzYCBpcyBiZWxvdyBgTUlOX0FETF9QTkxfQlBTYCBvciBhYm92ZSBCUFMuAAAAAAANSW52YWxpZEFkbFBubAAAAAAAACEAAAAvYGFkbF91dGlsaXphdGlvbl9icHNgIGlzIG91dCBvZiAoMCwgQlBTXSByYW5nZS4AAAAAFUludmFsaWRBZGxVdGlsaXphdGlvbgAAAAAAACIAAABiYGxpcXVpZGF0aW9uX3RocmVzaG9sZF9icHNgIGlzIGJlbG93IGBNSU5fTElRVUlEQVRJT05fVEhSRVNIT0xEX0JQU2AKb3IgZXhjZWVkcyAxMCUgb2YgY29sbGF0ZXJhbC4AAAAAABtJbnZhbGlkTGlxdWlkYXRpb25UaHJlc2hvbGQAAAAAIwAAADRgY29vbGRvd25fZHVyYXRpb25gIGV4Y2VlZHMgYE1BWF9DT09MRE9XTl9EVVJBVElPTmAuAAAAF0ludmFsaWRDb29sZG93bkR1cmF0aW9uAAAAACQAAABBYG1pbl9wb3NpdGlvbl9saWZldGltZWAgZXhjZWVkcyBgTUFYX01JTl9QT1NJVElPTl9MSUZFVElNRV9TRUNTYC4AAAAAAAAaSW52YWxpZE1pblBvc2l0aW9uTGlmZXRpbWUAAAAAACUAAAAlQSBDYXJyeWluZ0ZlZUNvbmZpZyByYXRlIGlzIG5lZ2F0aXZlLgAAAAAAABlJbnZhbGlkQm9ycm93UmF0ZU5lZ2F0aXZlAAAAAAAAKAAAADNgb3B0aW1hbF91dGlsaXphdGlvbl9icHNgIGlzIG91dCBvZiAoMCwgQlBTXSByYW5nZS4AAAAAGUludmFsaWRPcHRpbWFsVXRpbGl6YXRpb24AAAAAAAApAAAAQGBzbG9wZTJfYnBzIDwgc2xvcGUxX2Jwc2Ag4oCUIGtpbmsgY3VydmUgbXVzdCBiZSBub24tZGVjcmVhc2luZy4AAAAUSW52YWxpZFNsb3BlT3JkZXJpbmcAAAAqAAAAJmBzbG9wZTJfYnBzYCBleGNlZWRzIGBNQVhfU0xPUEUyX0JQU2AuAAAAAAAUSW52YWxpZFNsb3BlVG9vU3RlZXAAAAArAAAAKmBvcGVuX2ZlZV9icHNgIGV4Y2VlZHMgYE1BWF9PUEVOX0ZFRV9CUFNgLgAAAAAADkludmFsaWRPcGVuRmVlAAAAAAAsAAAAPmBsaXF1aWRhdGlvbl9ib3VudHlfYnBzYCBleGNlZWRzIGBNQVhfTElRVUlEQVRJT05fQk9VTlRZX0JQU2AuAAAAAAAYSW52YWxpZExpcXVpZGF0aW9uQm91bnR5AAAALQAAAEdgdHBfc2xfZXhlY3V0aW9uX2ZlZWAgaXMgbmVnYXRpdmUgb3IgZXhjZWVkcyBgTUFYX1RQX1NMX0VYRUNVVElPTl9GRUVgLgAAAAAXSW52YWxpZFRwU2xFeGVjdXRpb25GZWUAAAAALgAAADpgYmFzZV9ib3Jyb3dfcmF0ZV9icHNgIGV4Y2VlZHMgYE1BWF9CQVNFX0JPUlJPV19SQVRFX0JQU2AuAAAAAAAVSW52YWxpZEJhc2VCb3Jyb3dSYXRlAAAAAAAALwAAADBgbWF4X3NrZXdfcmF0ZV9icHNgIGV4Y2VlZHMgYE1BWF9TS0VXX1JBVEVfQlBTYC4AAAASSW52YWxpZE1heFNrZXdSYXRlAAAAAAAw",
        "AAAABQAAAAAAAAAAAAAAClJvbGVDaGFuZ2UAAAAAAAEAAAAEcm9sZQAAAAMAAAAAAAAABHJvbGUAAAARAAAAAAAAAAAAAAAHYWNjb3VudAAAAAATAAAAAAAAAAAAAAAIaXNfZ3JhbnQAAAABAAAAAAAAAAE=",
        "AAAABQAAAAAAAAAAAAAADExpbWl0c1VwZGF0ZQAAAAEAAAAGbGltaXRzAAAAAAAHAAAAAAAAAA5taW5fY29sbGF0ZXJhbAAAAAAACwAAAAAAAAAAAAAAEWNvb2xkb3duX2R1cmF0aW9uAAAAAAAABgAAAAAAAAAAAAAAFW1pbl9wb3NpdGlvbl9saWZldGltZQAAAAAAAAYAAAAAAAAAAAAAABVtYXhfdXRpbGl6YXRpb25fcmF0aW8AAAAAAAALAAAAAAAAAAAAAAALYWRsX3BubF9icHMAAAAABAAAAAAAAAAAAAAAE2FkbF91dGlsaXphdGlvbl9icHMAAAAABAAAAAAAAAAAAAAAGWxpcXVpZGF0aW9uX3RocmVzaG9sZF9icHMAAAAAAAAEAAAAAAAAAAE=",
        "AAAABQAAAAAAAAAAAAAADUFkbWluUHJvcG9zZWQAAAAAAAABAAAACWFkbWlucHJvcAAAAAAAAAIAAAAAAAAACHByb3Bvc2VyAAAAEwAAAAAAAAAAAAAACW5ld19hZG1pbgAAAAAAABMAAAAAAAAAAQ==",
        "AAAABQAAAAAAAAAAAAAAD0ZlZUNvbmZpZ1VwZGF0ZQAAAAABAAAABmZlZWNuZgAAAAAAAwAAAAAAAAAMb3Blbl9mZWVfYnBzAAAABAAAAAAAAAAAAAAAFmxpcXVpZGF0aW9uX2JvdW50eV9icHMAAAAAAAQAAAAAAAAAAAAAABN0cF9zbF9leGVjdXRpb25fZmVlAAAAAAsAAAAAAAAAAQ==",
        "AAAABQAAAAAAAAAAAAAAD0ZlZVNwbGl0c1VwZGF0ZQAAAAABAAAABmZlZWNmZwAAAAAAAwAAAAAAAAAGbHBfYnBzAAAAAAAEAAAAAAAAAAAAAAAHZGV2X2JwcwAAAAAEAAAAAAAAAAAAAAAKc3Rha2VyX2JwcwAAAAAABAAAAAAAAAAB",
        "AAAABQAAAAAAAAAAAAAAEUNhcnJ5aW5nRmVlVXBkYXRlAAAAAAAAAQAAAAVyYXRlcwAAAAAAAAUAAAAAAAAAFGJhc2VfYm9ycm93X3JhdGVfYnBzAAAACwAAAAAAAAAAAAAACnNsb3BlMV9icHMAAAAAAAsAAAAAAAAAAAAAAApzbG9wZTJfYnBzAAAAAAALAAAAAAAAAAAAAAAXb3B0aW1hbF91dGlsaXphdGlvbl9icHMAAAAACwAAAAAAAAAAAAAAEW1heF9za2V3X3JhdGVfYnBzAAAAAAAACwAAAAAAAAAB",
        "AAAABQAAAAAAAAAAAAAAFVVwZ3JhZGVUaW1lbG9ja1VwZGF0ZQAAAAAAAAEAAAAFdXBndGwAAAAAAAABAAAAAAAAABB0aW1lbG9ja19zZWNvbmRzAAAABgAAAAAAAAAB",
        "AAAABQAAAAAAAAAAAAAAFkFkbWluUHJvcG9zYWxDYW5jZWxsZWQAAAAAAAEAAAAIYWRtaW5jeGwAAAABAAAAAAAAAAljYW5jZWxsZXIAAAAAAAATAAAAAAAAAAE=",
        "AAAAAgAAAAAAAAAAAAAAClN0b3JhZ2VLZXkAAAAAAAgAAAAAAAAAQkluaXRpYWxpemF0aW9uIGZsYWcg4oCUIHNldCB0byBgdHJ1ZWAgYWZ0ZXIgYGluaXRpYWxpemVgIHN1Y2NlZWRzLgAAAAAAC0luaXRpYWxpemVkAAAAAAAAAAAYRmVlIHNwbGl0IGNvbmZpZ3VyYXRpb24uAAAACUZlZVNwbGl0cwAAAAAAAAAAAAApRXhlY3V0aW9uLWJvdW50eSBhbmQgb3Blbi1mZWUgcGFyYW1ldGVycy4AAAAAAAAJRmVlQ29uZmlnAAAAAAAAAAAAAExQcm90b2NvbCByaXNrIGFuZCB0aW1pbmcgbGltaXRzIChzaW5nbGUgc3RydWN0IHJlcGxhY2VzIGZvdXIgc2VwYXJhdGUga2V5cykuAAAADlByb3RvY29sTGltaXRzAAAAAAAAAAAAP0JvcnJvdy1yYXRlIGN1cnZlIGFuZCBkb21pbmFudC1zaWRlIHNrZXcgY2FycnlpbmctcmF0ZSBjZWlsaW5nLgAAAAARQ2FycnlpbmdGZWVDb25maWcAAAAAAAAAAAAAZkNvbmZpZ3VyYWJsZSB1cGdyYWRlIHRpbWVsb2NrIGluIHNlY29uZHMuIEZsb29yIGVuZm9yY2VkIGF0CmBzaGFyZWQ6OmNvbnN0YW50czo6TUlOX1VQR1JBREVfVElNRUxPQ0tgLgAAAAAAD1VwZ3JhZGVUaW1lbG9jawAAAAAAAAAAQVBlbmRpbmcgYWRtaW4gYXdhaXRpbmcgYGFjY2VwdF9hZG1pbmAg4oCUIHNldCBieSBgcHJvcG9zZV9hZG1pbmAuAAAAAAAADFBlbmRpbmdBZG1pbgAAAAAAAAAwQ3VycmVudCBjb250cmFjdCB2ZXJzaW9uICh3cml0dGVuIGJ5IG1pZ3JhdGlvbikuAAAAB1ZlcnNpb24A",
        "AAAAAQAAAHhQZW5kaW5nIGFkbWluIHRyYW5zZmVyIGF3YWl0aW5nIGBhY2NlcHRfYWRtaW5gLiBUaGUgcHJvcG9zYWwgdGltZXN0YW1wCmJvdW5kcyBpdHMgbGlmZXRpbWUgdG8gYEFETUlOX1BST1BPU0FMX1RUTF9TRUNTYC4AAAAAAAAAFFBlbmRpbmdBZG1pblByb3Bvc2FsAAAAAgAAAAAAAAAFYWRtaW4AAAAAAAATAAAAAAAAAAtwcm9wb3NlZF9hdAAAAAAG",
        "AAAAAAAAAAAAAAAHbWlncmF0ZQAAAAACAAAAAAAAAA5taWdyYXRpb25fZGF0YQAAAAAH0AAAAA1NaWdyYXRpb25EYXRhAAAAAAAAAAAAAAhvcGVyYXRvcgAAABMAAAAA",
        "AAAAAAAAAAAAAAAHdXBncmFkZQAAAAACAAAAAAAAAA1uZXdfd2FzbV9oYXNoAAAAAAAD7gAAACAAAAAAAAAACG9wZXJhdG9yAAAAEwAAAAA=",
        "AAAAAAAAAAAAAAAIaGFzX3JvbGUAAAACAAAAAAAAAARyb2xlAAAAEQAAAAAAAAAHYWNjb3VudAAAAAATAAAAAQAAAAE=",
        "AAAAAAAAAAAAAAAKZ3JhbnRfcm9sZQAAAAAAAwAAAAAAAAAGY2FsbGVyAAAAAAATAAAAAAAAAARyb2xlAAAAEQAAAAAAAAAHYWNjb3VudAAAAAATAAAAAA==",
        "AAAAAAAAAAAAAAALcmV2b2tlX3JvbGUAAAAAAwAAAAAAAAAGY2FsbGVyAAAAAAATAAAAAAAAAARyb2xlAAAAEQAAAAAAAAAHYWNjb3VudAAAAAATAAAAAA==",
        "AAAAAAAAAAAAAAAMYWNjZXB0X2FkbWluAAAAAQAAAAAAAAAJbmV3X2FkbWluAAAAAAAAEwAAAAA=",
        "AAAAAAAAARFBdG9taWMtd2l0aC1kZXBsb3kgaW5pdGlhbGl6YXRpb24gKFNvcm9iYW4gY29uc3RydWN0b3IpLiBSdW5zIGV4YWN0bHkKb25jZSwgaW5zaWRlIHRoZSBkZXBsb3kgdHJhbnNhY3Rpb24sIHNvIHRoZXJlIGlzIG5vIHdpbmRvdyBpbiB3aGljaCBhCnRoaXJkIHBhcnR5IGNvdWxkIGZyb250LXJ1biBpbml0IGFuZCBjbGFpbSB0aGUgYWRtaW4gcm9sZS4gR3JhbnRzCkRFRkFVTFRfQURNSU5fUk9MRSB0byBgYWRtaW5fYWRkcmVzc2AgYW5kIHNlZWRzIHZhbGlkYXRlZCBkZWZhdWx0cy4AAAAAAAANX19jb25zdHJ1Y3RvcgAAAAAAAAEAAAAAAAAADWFkbWluX2FkZHJlc3MAAAAAAAATAAAAAA==",
        "AAAAAAAAAAAAAAANcHJvcG9zZV9hZG1pbgAAAAAAAAIAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAAAAAAJbmV3X2FkbWluAAAAAAAAEwAAAAA=",
        "AAAAAAAAAAAAAAAOY2FuY2VsX3VwZ3JhZGUAAAAAAAEAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAA=",
        "AAAAAAAAAAAAAAAOZ2V0X2ZlZV9jb25maWcAAAAAAAAAAAABAAAH0AAAAAlGZWVDb25maWcAAAA=",
        "AAAAAAAAAAAAAAAOZ2V0X2ZlZV9zcGxpdHMAAAAAAAAAAAABAAAH0AAAAAlGZWVTcGxpdHMAAAA=",
        "AAAAAAAAAAAAAAAOc2V0X2ZlZV9jb25maWcAAAAAAAIAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAAAAAAGY29uZmlnAAAAAAfQAAAACUZlZUNvbmZpZwAAAAAAAAA=",
        "AAAAAAAAAAAAAAAPcHJvcG9zZV91cGdyYWRlAAAAAAIAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAAAAAAJd2FzbV9oYXNoAAAAAAAD7gAAACAAAAAA",
        "AAAAAAAAAAAAAAARYnVtcF9jb25maWdfc3RhdGUAAAAAAAAAAAAAAA==",
        "AAAAAAAAAAAAAAARZ2V0X3BlbmRpbmdfYWRtaW4AAAAAAAAAAAAAAQAAA+gAAAAT",
        "AAAAAAAAAAAAAAARdXBkYXRlX2ZlZV9zcGxpdHMAAAAAAAACAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAAAAAACmZlZV9zcGxpdHMAAAAAB9AAAAAJRmVlU3BsaXRzAAAAAAAAAA==",
        "AAAAAAAAAAAAAAATZ2V0X3Byb3RvY29sX2xpbWl0cwAAAAAAAAAAAQAAB9AAAAAOUHJvdG9jb2xMaW1pdHMAAA==",
        "AAAAAAAAAAAAAAAUZ2V0X3VwZ3JhZGVfdGltZWxvY2sAAAAAAAAAAQAAAAY=",
        "AAAAAAAAAAAAAAAUc2V0X3VwZ3JhZGVfdGltZWxvY2sAAAACAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAAAAAAB3NlY29uZHMAAAAABgAAAAA=",
        "AAAAAAAAAAAAAAAVY2FuY2VsX2FkbWluX3Byb3Bvc2FsAAAAAAAAAQAAAAAAAAAGY2FsbGVyAAAAAAATAAAAAA==",
        "AAAAAAAAAAAAAAAWdXBkYXRlX3Byb3RvY29sX2xpbWl0cwAAAAAAAgAAAAAAAAAGY2FsbGVyAAAAAAATAAAAAAAAAAZsaW1pdHMAAAAAB9AAAAAOUHJvdG9jb2xMaW1pdHMAAAAAAAA=",
        "AAAAAAAAAAAAAAAXZ2V0X2NhcnJ5aW5nX2ZlZV9jb25maWcAAAAAAAAAAAEAAAfQAAAAEUNhcnJ5aW5nRmVlQ29uZmlnAAAA",
        "AAAAAAAAAAAAAAAadXBkYXRlX2NhcnJ5aW5nX2ZlZV9jb25maWcAAAAAAAIAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAAAAAAGY29uZmlnAAAAAAfQAAAAEUNhcnJ5aW5nRmVlQ29uZmlnAAAAAAAAAA==",
        "AAAABAAAAAAAAAAAAAAAEVJvbGVUcmFuc2ZlckVycm9yAAAAAAAAAwAAAAAAAAARTm9QZW5kaW5nVHJhbnNmZXIAAAAAAAiYAAAAAAAAABZJbnZhbGlkTGl2ZVVudGlsTGVkZ2VyAAAAAAiZAAAAAAAAABVJbnZhbGlkUGVuZGluZ0FjY291bnQAAAAAAAia",
        "AAAABQAAACVFdmVudCBlbWl0dGVkIHdoZW4gYSByb2xlIGlzIGdyYW50ZWQuAAAAAAAAAAAAAAtSb2xlR3JhbnRlZAAAAAABAAAADHJvbGVfZ3JhbnRlZAAAAAMAAAAAAAAABHJvbGUAAAARAAAAAQAAAAAAAAAHYWNjb3VudAAAAAATAAAAAQAAAAAAAAAGY2FsbGVyAAAAAAATAAAAAAAAAAI=",
        "AAAABQAAACVFdmVudCBlbWl0dGVkIHdoZW4gYSByb2xlIGlzIHJldm9rZWQuAAAAAAAAAAAAAAtSb2xlUmV2b2tlZAAAAAABAAAADHJvbGVfcmV2b2tlZAAAAAMAAAAAAAAABHJvbGUAAAARAAAAAQAAAAAAAAAHYWNjb3VudAAAAAATAAAAAQAAAAAAAAAGY2FsbGVyAAAAAAATAAAAAAAAAAI=",
        "AAAABQAAAC9FdmVudCBlbWl0dGVkIHdoZW4gdGhlIGFkbWluIHJvbGUgaXMgcmVub3VuY2VkLgAAAAAAAAAADkFkbWluUmVub3VuY2VkAAAAAAABAAAAD2FkbWluX3Jlbm91bmNlZAAAAAABAAAAAAAAAAVhZG1pbgAAAAAAABMAAAABAAAAAg==",
        "AAAABQAAACtFdmVudCBlbWl0dGVkIHdoZW4gYSByb2xlIGFkbWluIGlzIGNoYW5nZWQuAAAAAAAAAAAQUm9sZUFkbWluQ2hhbmdlZAAAAAEAAAAScm9sZV9hZG1pbl9jaGFuZ2VkAAAAAAADAAAAAAAAAARyb2xlAAAAEQAAAAEAAAAAAAAAE3ByZXZpb3VzX2FkbWluX3JvbGUAAAAAEQAAAAAAAAAAAAAADm5ld19hZG1pbl9yb2xlAAAAAAARAAAAAAAAAAI=",
        "AAAABAAAAAAAAAAAAAAAEkFjY2Vzc0NvbnRyb2xFcnJvcgAAAAAACwAAAAAAAAAMVW5hdXRob3JpemVkAAAH0AAAAAAAAAALQWRtaW5Ob3RTZXQAAAAH0QAAAAAAAAAQSW5kZXhPdXRPZkJvdW5kcwAAB9IAAAAAAAAAEUFkbWluUm9sZU5vdEZvdW5kAAAAAAAH0wAAAAAAAAASUm9sZUNvdW50SXNOb3RaZXJvAAAAAAfUAAAAAAAAAAxSb2xlTm90Rm91bmQAAAfVAAAAAAAAAA9BZG1pbkFscmVhZHlTZXQAAAAH1gAAAAAAAAALUm9sZU5vdEhlbGQAAAAH1wAAAAAAAAALUm9sZUlzRW1wdHkAAAAH2AAAAAAAAAASVHJhbnNmZXJJblByb2dyZXNzAAAAAAfZAAAAAAAAABBNYXhSb2xlc0V4Y2VlZGVkAAAH2g==",
        "AAAABQAAADJFdmVudCBlbWl0dGVkIHdoZW4gYW4gYWRtaW4gdHJhbnNmZXIgaXMgY29tcGxldGVkLgAAAAAAAAAAABZBZG1pblRyYW5zZmVyQ29tcGxldGVkAAAAAAABAAAAGGFkbWluX3RyYW5zZmVyX2NvbXBsZXRlZAAAAAIAAAAAAAAACW5ld19hZG1pbgAAAAAAABMAAAABAAAAAAAAAA5wcmV2aW91c19hZG1pbgAAAAAAEwAAAAAAAAAC",
        "AAAABQAAADJFdmVudCBlbWl0dGVkIHdoZW4gYW4gYWRtaW4gdHJhbnNmZXIgaXMgaW5pdGlhdGVkLgAAAAAAAAAAABZBZG1pblRyYW5zZmVySW5pdGlhdGVkAAAAAAABAAAAGGFkbWluX3RyYW5zZmVyX2luaXRpYXRlZAAAAAMAAAAAAAAADWN1cnJlbnRfYWRtaW4AAAAAAAATAAAAAQAAAAAAAAAJbmV3X2FkbWluAAAAAAAAEwAAAAAAAAAAAAAAEWxpdmVfdW50aWxfbGVkZ2VyAAAAAAAABAAAAAAAAAAC",
        "AAAAAQAAADFTdG9yYWdlIGtleSBmb3IgZW51bWVyYXRpb24gb2YgYWNjb3VudHMgcGVyIHJvbGUuAAAAAAAAAAAAAA5Sb2xlQWNjb3VudEtleQAAAAAAAgAAAAAAAAAFaW5kZXgAAAAAAAAEAAAAAAAAAARyb2xlAAAAEQ==",
        "AAAAAgAAADxTdG9yYWdlIGtleXMgZm9yIHRoZSBkYXRhIGFzc29jaWF0ZWQgd2l0aCB0aGUgYWNjZXNzIGNvbnRyb2wAAAAAAAAAF0FjY2Vzc0NvbnRyb2xTdG9yYWdlS2V5AAAAAAcAAAAAAAAAAAAAAA1FeGlzdGluZ1JvbGVzAAAAAAAAAQAAAAAAAAAMUm9sZUFjY291bnRzAAAAAQAAB9AAAAAOUm9sZUFjY291bnRLZXkAAAAAAAEAAAAAAAAAB0hhc1JvbGUAAAAAAgAAABMAAAARAAAAAQAAAAAAAAARUm9sZUFjY291bnRzQ291bnQAAAAAAAABAAAAEQAAAAEAAAAAAAAACVJvbGVBZG1pbgAAAAAAAAEAAAARAAAAAAAAAAAAAAAFQWRtaW4AAAAAAAAAAAAAAAAAAAxQZW5kaW5nQWRtaW4=",
        "AAAABAAAAAAAAAAAAAAADE93bmFibGVFcnJvcgAAAAMAAAAAAAAAC093bmVyTm90U2V0AAAACDQAAAAAAAAAElRyYW5zZmVySW5Qcm9ncmVzcwAAAAAINQAAAAAAAAAPT3duZXJBbHJlYWR5U2V0AAAACDY=",
        "AAAABQAAADZFdmVudCBlbWl0dGVkIHdoZW4gYW4gb3duZXJzaGlwIHRyYW5zZmVyIGlzIGluaXRpYXRlZC4AAAAAAAAAAAART3duZXJzaGlwVHJhbnNmZXIAAAAAAAABAAAAEm93bmVyc2hpcF90cmFuc2ZlcgAAAAAAAwAAAAAAAAAJb2xkX293bmVyAAAAAAAAEwAAAAAAAAAAAAAACW5ld19vd25lcgAAAAAAABMAAAAAAAAAAAAAABFsaXZlX3VudGlsX2xlZGdlcgAAAAAAAAQAAAAAAAAAAg==",
        "AAAABQAAACpFdmVudCBlbWl0dGVkIHdoZW4gb3duZXJzaGlwIGlzIHJlbm91bmNlZC4AAAAAAAAAAAAST3duZXJzaGlwUmVub3VuY2VkAAAAAAABAAAAE293bmVyc2hpcF9yZW5vdW5jZWQAAAAAAQAAAAAAAAAJb2xkX293bmVyAAAAAAAAEwAAAAAAAAAC",
        "AAAABQAAADZFdmVudCBlbWl0dGVkIHdoZW4gYW4gb3duZXJzaGlwIHRyYW5zZmVyIGlzIGNvbXBsZXRlZC4AAAAAAAAAAAAaT3duZXJzaGlwVHJhbnNmZXJDb21wbGV0ZWQAAAAAAAEAAAAcb3duZXJzaGlwX3RyYW5zZmVyX2NvbXBsZXRlZAAAAAEAAAAAAAAACW5ld19vd25lcgAAAAAAABMAAAAAAAAAAg==",
        "AAAAAgAAACNTdG9yYWdlIGtleXMgZm9yIGBPd25hYmxlYCB1dGlsaXR5LgAAAAAAAAAAEU93bmFibGVTdG9yYWdlS2V5AAAAAAAAAgAAAAAAAAAAAAAABU93bmVyAAAAAAAAAAAAAAAAAAAMUGVuZGluZ093bmVy",
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
        has_role: this.txFromJSON<boolean>,
        grant_role: this.txFromJSON<null>,
        revoke_role: this.txFromJSON<null>,
        accept_admin: this.txFromJSON<null>,
        propose_admin: this.txFromJSON<null>,
        cancel_upgrade: this.txFromJSON<null>,
        get_fee_config: this.txFromJSON<FeeConfig>,
        get_fee_splits: this.txFromJSON<FeeSplits>,
        set_fee_config: this.txFromJSON<null>,
        propose_upgrade: this.txFromJSON<null>,
        bump_config_state: this.txFromJSON<null>,
        get_pending_admin: this.txFromJSON<Option<string>>,
        update_fee_splits: this.txFromJSON<null>,
        get_protocol_limits: this.txFromJSON<ProtocolLimits>,
        get_upgrade_timelock: this.txFromJSON<u64>,
        set_upgrade_timelock: this.txFromJSON<null>,
        cancel_admin_proposal: this.txFromJSON<null>,
        update_protocol_limits: this.txFromJSON<null>,
        get_carrying_fee_config: this.txFromJSON<CarryingFeeConfig>,
        update_carrying_fee_config: this.txFromJSON<null>
  }
}