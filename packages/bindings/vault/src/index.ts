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




export const VaultError = {
  1: {message:"Unauthorized"},
  2: {message:"AlreadyInitialized"},
  3: {message:"NotInitialized"},
  4: {message:"InvalidAmount"},
  5: {message:"InvalidConfig"},
  6: {message:"Paused"},
  7: {message:"InsufficientCash"},
  8: {message:"InvalidCaller"},
  9: {message:"ArithmeticError"},
  /**
   * `upgrade` called with no pending proposal.
   */
  10: {message:"UpgradeNoPending"},
  /**
   * `upgrade` called before the proposal's timelock eta.
   */
  11: {message:"UpgradeTimelockNotElapsed"},
  /**
   * `upgrade` called with a hash that differs from the proposal.
   */
  12: {message:"UpgradeHashMismatch"}
}





export type Key = {tag: "Initialized", values: void} | {tag: "ConfigManager", values: void} | {tag: "PositionManager", values: void} | {tag: "RequestRouter", values: void} | {tag: "LpConfig", values: void} | {tag: "Paused", values: void} | {tag: "Version", values: void};


export interface OwnerTokensKey {
  index: u32;
  owner: string;
}

/**
 * Storage keys for the data associated with the enumerable extension of
 * `NonFungibleToken`
 */
export type NFTEnumerableStorageKey = {tag: "TotalSupply", values: void} | {tag: "OwnerTokens", values: readonly [OwnerTokensKey]} | {tag: "OwnerTokensIndex", values: readonly [u32]} | {tag: "GlobalTokens", values: readonly [u32]} | {tag: "GlobalTokensIndex", values: readonly [u32]};


/**
 * Storage keys for the data associated with the consecutive extension of
 * `NonFungibleToken`
 */
export type NFTConsecutiveStorageKey = {tag: "Approval", values: readonly [u32]} | {tag: "Owner", values: readonly [u32]} | {tag: "OwnershipBucket", values: readonly [u32]} | {tag: "BurnedToken", values: readonly [u32]};






/**
 * Storage container for royalty information
 */
export interface RoyaltyInfo {
  basis_points: u32;
  receiver: string;
}

/**
 * Storage keys for royalty data
 */
export type NFTRoyaltiesStorageKey = {tag: "DefaultRoyalty", values: void} | {tag: "TokenRoyalty", values: readonly [u32]};





export const NonFungibleTokenError = {
  /**
   * Indicates a non-existent `token_id`.
   */
  200: {message:"NonExistentToken"},
  /**
   * Indicates an error related to the ownership over a particular token.
   * Used in transfers.
   */
  201: {message:"IncorrectOwner"},
  /**
   * Indicates a failure with the `operator`s approval. Used in transfers.
   */
  202: {message:"InsufficientApproval"},
  /**
   * Indicates a failure with the `approver` of a token to be approved. Used
   * in approvals.
   */
  203: {message:"InvalidApprover"},
  /**
   * Indicates an invalid value for `live_until_ledger` when setting
   * approvals.
   */
  204: {message:"InvalidLiveUntilLedger"},
  /**
   * Indicates overflow when adding two values
   */
  205: {message:"MathOverflow"},
  /**
   * Indicates all possible `token_id`s are already in use.
   */
  206: {message:"TokenIDsAreDepleted"},
  /**
   * Indicates an invalid amount to batch mint in `consecutive` extension.
   */
  207: {message:"InvalidAmount"},
  /**
   * Indicates the token does not exist in owner's list.
   */
  208: {message:"TokenNotFoundInOwnerList"},
  /**
   * Indicates the token does not exist in global list.
   */
  209: {message:"TokenNotFoundInGlobalList"},
  /**
   * Indicates access to unset metadata.
   */
  210: {message:"UnsetMetadata"},
  /**
   * Indicates the length of the base URI exceeds the maximum allowed.
   */
  211: {message:"BaseUriMaxLenExceeded"},
  /**
   * Indicates the royalty amount is higher than 10_000 (100%) basis points.
   */
  212: {message:"InvalidRoyaltyAmount"},
  /**
   * Indicates the length of the name exceeds the maximum allowed.
   */
  213: {message:"NameMaxLenExceeded"},
  /**
   * Indicates the length of the symbol exceeds the maximum allowed.
   */
  214: {message:"SymbolMaxLenExceeded"}
}

export type NFTSequentialStorageKey = {tag: "TokenIdCounter", values: void};


/**
 * Storage container for token metadata
 */
export interface Metadata {
  base_uri: string;
  name: string;
  symbol: string;
}


/**
 * Storage container for the token for which an approval is granted
 * and the ledger number at which this approval expires.
 */
export interface ApprovalData {
  approved: string;
  live_until_ledger: u32;
}

/**
 * Storage keys for the data associated with `NonFungibleToken`
 */
export type NFTStorageKey = {tag: "Owner", values: readonly [u32]} | {tag: "Balance", values: readonly [string]} | {tag: "Approval", values: readonly [u32]} | {tag: "ApprovalForAll", values: readonly [string, string]} | {tag: "Metadata", values: void};



/**
 * Hook types for modular compliance system.
 * 
 * Each hook type represents a specific event or validation point
 * where compliance modules can be executed.
 */
export type ComplianceHook = {tag: "Transferred", values: void} | {tag: "Created", values: void} | {tag: "Destroyed", values: void} | {tag: "CanTransfer", values: void} | {tag: "CanCreate", values: void};

export const ComplianceError = {
  /**
   * Indicates a module is already registered for this hook.
   */
  360: {message:"ModuleAlreadyRegistered"},
  /**
   * Indicates a module is not registered for this hook.
   */
  361: {message:"ModuleNotRegistered"},
  /**
   * Indicates a module bound is exceeded.
   */
  362: {message:"ModuleBoundExceeded"},
  /**
   * Indicates a token is not bound to this compliance contract.
   */
  363: {message:"TokenNotBound"}
}

/**
 * Storage keys for the modular compliance contract.
 */
export type ComplianceDataKey = {tag: "HookModules", values: readonly [ComplianceHook]};

/**
 * Error codes for document management operations.
 */
export const DocumentError = {
  /**
   * The specified document was not found.
   */
  380: {message:"DocumentNotFound"},
  /**
   * Maximum number of documents has been reached.
   */
  381: {message:"MaxDocumentsReached"},
  /**
   * The URI exceeds the maximum allowed length.
   */
  382: {message:"UriTooLong"}
}




/**
 * Represents a document with its metadata.
 */
export interface Document {
  /**
 * The hash of the document contents.
 */
document_hash: Buffer;
  /**
 * Timestamp when the document was last modified.
 */
timestamp: u64;
  /**
 * The URI where the document can be accessed.
 */
uri: string;
}

/**
 * Storage keys for document management.
 */
export type DocumentStorageKey = {tag: "Index", values: readonly [Buffer]} | {tag: "Bucket", values: readonly [u32]} | {tag: "Count", values: void};




export const ClaimIssuerError = {
  /**
   * Signature data length does not match the expected scheme.
   */
  350: {message:"SigDataMismatch"},
  /**
   * The provided key is empty.
   */
  351: {message:"KeyIsEmpty"},
  /**
   * The key is already allowed for the specified topic.
   */
  352: {message:"KeyAlreadyAllowed"},
  /**
   * The specified key was not found in the allowed keys.
   */
  353: {message:"KeyNotFound"},
  /**
   * The claim issuer is not allowed to sign claims about the specified
   * claim topic.
   */
  354: {message:"NotAllowed"},
  /**
   * Maximum limit exceeded (keys per topic or registries per key).
   */
  355: {message:"LimitExceeded"},
  /**
   * No signing keys found for the specified claim topic.
   */
  356: {message:"NoKeysForTopic"},
  /**
   * Invalid claim data encoding.
   */
  357: {message:"InvalidClaimDataExpiration"},
  /**
   * Recovery of the Secp256k1 public key failed.
   */
  358: {message:"Secp256k1RecoveryFailed"},
  /**
   * Indicates overflow when adding two values.
   */
  359: {message:"MathOverflow"}
}



export interface SigningKey {
  public_key: Buffer;
  scheme: u32;
}


/**
 * Signature data for Ed25519 scheme.
 */
export interface Ed25519SignatureData {
  public_key: Buffer;
  signature: Buffer;
}

/**
 * Storage keys for claim issuer key management.
 */
export type ClaimIssuerStorageKey = {tag: "Topics", values: readonly [u32]} | {tag: "Pairs", values: readonly [SigningKey]} | {tag: "RevokedClaim", values: readonly [Buffer]} | {tag: "ClaimNonce", values: readonly [string, u32]};


/**
 * Signature data for Secp256k1 scheme.
 */
export interface Secp256k1SignatureData {
  public_key: Buffer;
  recovery_id: u32;
  signature: Buffer;
}


/**
 * Signature data for Secp256r1 scheme.
 */
export interface Secp256r1SignatureData {
  public_key: Buffer;
  signature: Buffer;
}


export const ClaimsError = {
  /**
   * Claim  ID does not exist.
   */
  340: {message:"ClaimNotFound"},
  /**
   * Claim Issuer cannot validate the claim (revocation, signature mismatch,
   * unauthorized signing key, etc.)
   */
  341: {message:"ClaimNotValid"}
}




/**
 * Represents a claim stored on-chain.
 */
export interface Claim {
  /**
 * The claim data
 */
data: Buffer;
  /**
 * The address of the claim issuer
 */
issuer: string;
  /**
 * The signature scheme used
 */
scheme: u32;
  /**
 * The cryptographic signature
 */
signature: Buffer;
  /**
 * The claim topic (numeric identifier)
 */
topic: u32;
  /**
 * Optional URI for additional information
 */
uri: string;
}

/**
 * Storage keys for the data associated with Identity Claims.
 */
export type ClaimsStorageKey = {tag: "Claim", values: readonly [Buffer]} | {tag: "ClaimsByTopic", values: readonly [u32]};

/**
 * Storage keys for the data associated with `RWA` token
 */
export type IdentityVerifierStorageKey = {tag: "ClaimTopicsAndIssuers", values: void} | {tag: "IdentityRegistryStorage", values: void};



export const RWAError = {
  /**
   * Indicates an error related to insufficient balance for the operation.
   */
  300: {message:"InsufficientBalance"},
  /**
   * Indicates an error when an input must be >= 0.
   */
  301: {message:"LessThanZero"},
  /**
   * Indicates the address is frozen and cannot perform operations.
   */
  302: {message:"AddressFrozen"},
  /**
   * Indicates insufficient free tokens (due to partial freezing).
   */
  303: {message:"InsufficientFreeTokens"},
  /**
   * Indicates an identity cannot be verified.
   */
  304: {message:"IdentityVerificationFailed"},
  /**
   * Indicates the transfer does not comply with the compliance rules.
   */
  305: {message:"TransferNotCompliant"},
  /**
   * Indicates the mint operation does not comply with the compliance rules.
   */
  306: {message:"MintNotCompliant"},
  /**
   * Indicates the compliance contract is not set.
   */
  307: {message:"ComplianceNotSet"},
  /**
   * Indicates the onchain ID is not set.
   */
  308: {message:"OnchainIdNotSet"},
  /**
   * Indicates the version is not set.
   */
  309: {message:"VersionNotSet"},
  /**
   * Indicates the claim topics and issuers contract is not set.
   */
  310: {message:"ClaimTopicsAndIssuersNotSet"},
  /**
   * Indicates the identity registry storage contract is not set.
   */
  311: {message:"IdentityRegistryStorageNotSet"},
  /**
   * Indicates the identity verifier contract is not set.
   */
  312: {message:"IdentityVerifierNotSet"},
  /**
   * Indicates the old account and new account have different identities.
   */
  313: {message:"IdentityMismatch"}
}






export const ClaimTopicsAndIssuersError = {
  /**
   * Indicates a non-existent claim topic.
   */
  370: {message:"ClaimTopicDoesNotExist"},
  /**
   * Indicates a non-existent trusted issuer.
   */
  371: {message:"IssuerDoesNotExist"},
  /**
   * Indicates a claim topic already exists.
   */
  372: {message:"ClaimTopicAlreadyExists"},
  /**
   * Indicates a trusted issuer already exists.
   */
  373: {message:"IssuerAlreadyExists"},
  /**
   * Indicates max claim topics limit is reached.
   */
  374: {message:"MaxClaimTopicsLimitReached"},
  /**
   * Indicates max trusted issuers limit is reached.
   */
  375: {message:"MaxIssuersLimitReached"},
  /**
   * Indicates claim topics set provided for the issuer cannot be empty.
   */
  376: {message:"ClaimTopicsSetCannotBeEmpty"}
}

/**
 * Storage keys for the data associated with the claim topics and issuers
 * extension
 */
export type ClaimTopicsAndIssuersStorageKey = {tag: "ClaimTopics", values: void} | {tag: "TrustedIssuers", values: void} | {tag: "IssuerClaimTopics", values: readonly [string]} | {tag: "ClaimTopicIssuers", values: readonly [u32]};

/**
 * Error codes for the Identity Registry Storage system.
 */
export const IRSError = {
  /**
   * An identity already exists for the given account.
   */
  320: {message:"IdentityOverwrite"},
  /**
   * No identity found for the given account.
   */
  321: {message:"IdentityNotFound"},
  /**
   * Country data not found at the specified index.
   */
  322: {message:"CountryDataNotFound"},
  /**
   * Identity can't be with empty country data list.
   */
  323: {message:"EmptyCountryList"},
  /**
   * The maximum number of country entries has been reached.
   */
  324: {message:"MaxCountryEntriesReached"},
  /**
   * Account has been recovered and cannot be used.
   */
  325: {message:"AccountRecovered"},
  /**
   * Metadata has too many entries (exceeds MAX_METADATA_ENTRIES).
   */
  326: {message:"MetadataTooManyEntries"},
  /**
   * Metadata string value is too long (exceeds MAX_METADATA_STRING_LEN).
   */
  327: {message:"MetadataStringTooLong"}
}









/**
 * A country data containing the country relationship and optional metadata
 */
export interface CountryData {
  /**
 * Type of country relationship
 */
country: CountryRelation;
  /**
 * Optional metadata (e.g., visa type, validity period)
 */
metadata: Option<Map<string, string>>;
}

/**
 * Represents the type of identity holder
 */
export type IdentityType = {tag: "Individual", values: void} | {tag: "Organization", values: void};

/**
 * Storage keys for the data associated with Identity Storage Registry.
 */
export type IRSStorageKey = {tag: "Identity", values: readonly [string]} | {tag: "IdentityProfile", values: readonly [string]} | {tag: "RecoveredTo", values: readonly [string]};

/**
 * Unified country relationship that can be either individual or organizational
 */
export type CountryRelation = {tag: "Individual", values: readonly [IndividualCountryRelation]} | {tag: "Organization", values: readonly [OrganizationCountryRelation]};


/**
 * Complete identity profile containing identity type and country data
 */
export interface IdentityProfile {
  countries: Array<CountryData>;
  identity_type: IdentityType;
}

/**
 * Represents different types of country relationships for individuals
 * ISO 3166-1 numeric country code
 */
export type IndividualCountryRelation = {tag: "Residence", values: readonly [u32]} | {tag: "Citizenship", values: readonly [u32]} | {tag: "SourceOfFunds", values: readonly [u32]} | {tag: "TaxResidency", values: readonly [u32]} | {tag: "Custom", values: readonly [string, u32]};

/**
 * Represents different types of country relationships for organizations
 */
export type OrganizationCountryRelation = {tag: "Incorporation", values: readonly [u32]} | {tag: "OperatingJurisdiction", values: readonly [u32]} | {tag: "TaxJurisdiction", values: readonly [u32]} | {tag: "SourceOfFunds", values: readonly [u32]} | {tag: "Custom", values: readonly [string, u32]};











/**
 * Error codes for the Token Binder system.
 */
export const TokenBinderError = {
  /**
   * The specified token was not found in the bound tokens list.
   */
  330: {message:"TokenNotFound"},
  /**
   * Attempted to bind a token that is already bound.
   */
  331: {message:"TokenAlreadyBound"},
  /**
   * Total token capacity (MAX_TOKENS) has been reached.
   */
  332: {message:"MaxTokensReached"},
  /**
   * Batch bind size exceeded.
   */
  333: {message:"BindBatchTooLarge"},
  /**
   * The batch contains duplicates.
   */
  334: {message:"BindBatchDuplicates"}
}

/**
 * Storage keys for the token binder system.
 * 
 * - Tokens are stored in buckets of 100 addresses each
 * - Each bucket is a `Vec<Address>` stored under its bucket index
 * - Total count is tracked separately
 * - When a token is unbound, the last token is moved to fill the gap
 * (swap-remove pattern)
 */
export type TokenBinderStorageKey = {tag: "TokenBucket", values: readonly [u32]} | {tag: "TotalCount", values: void};

/**
 * Storage keys for the data associated with `RWA` token
 */
export type RWAStorageKey = {tag: "AddressFrozen", values: readonly [string]} | {tag: "FrozenTokens", values: readonly [string]} | {tag: "Compliance", values: void} | {tag: "OnchainId", values: void} | {tag: "Version", values: void} | {tag: "IdentityVerifier", values: void};



export const VaultTokenError = {
  /**
   * Indicates access to uninitialized vault asset address.
   */
  400: {message:"VaultAssetAddressNotSet"},
  /**
   * Indicates that vault asset address is already set.
   */
  401: {message:"VaultAssetAddressAlreadySet"},
  /**
   * Indicates that vault virtual decimals offset is already set.
   */
  402: {message:"VaultVirtualDecimalsOffsetAlreadySet"},
  /**
   * Indicates the amount is not a valid vault assets value.
   */
  403: {message:"VaultInvalidAssetsAmount"},
  /**
   * Indicates the amount is not a valid vault shares value.
   */
  404: {message:"VaultInvalidSharesAmount"},
  /**
   * Attempted to deposit more assets than the max amount for address.
   */
  405: {message:"VaultExceededMaxDeposit"},
  /**
   * Attempted to mint more shares than the max amount for address.
   */
  406: {message:"VaultExceededMaxMint"},
  /**
   * Attempted to withdraw more assets than the max amount for address.
   */
  407: {message:"VaultExceededMaxWithdraw"},
  /**
   * Attempted to redeem more shares than the max amount for address.
   */
  408: {message:"VaultExceededMaxRedeem"},
  /**
   * Maximum number of decimals offset exceeded
   */
  409: {message:"VaultMaxDecimalsOffsetExceeded"},
  /**
   * Indicates overflow due to mathematical operations
   */
  410: {message:"MathOverflow"}
}

/**
 * Storage keys for the data associated with the vault extension
 */
export type VaultStorageKey = {tag: "AssetAddress", values: void} | {tag: "VirtualDecimalsOffset", values: void};




/**
 * Storage keys for the data associated with the allowlist extension
 */
export type AllowListStorageKey = {tag: "Allowed", values: readonly [string]};



/**
 * Storage keys for the data associated with the blocklist extension
 */
export type BlockListStorageKey = {tag: "Blocked", values: readonly [string]};




export const FungibleTokenError = {
  /**
   * Indicates an error related to the current balance of account from which
   * tokens are expected to be transferred.
   */
  100: {message:"InsufficientBalance"},
  /**
   * Indicates a failure with the allowance mechanism when a given spender
   * doesn't have enough allowance.
   */
  101: {message:"InsufficientAllowance"},
  /**
   * Indicates an invalid value for `live_until_ledger` when setting an
   * allowance.
   */
  102: {message:"InvalidLiveUntilLedger"},
  /**
   * Indicates an error when an input that must be >= 0
   */
  103: {message:"LessThanZero"},
  /**
   * Indicates overflow when adding two values
   */
  104: {message:"MathOverflow"},
  /**
   * Indicates access to uninitialized metadata
   */
  105: {message:"UnsetMetadata"},
  /**
   * Indicates that the operation would have caused `total_supply` to exceed
   * the `cap`.
   */
  106: {message:"ExceededCap"},
  /**
   * Indicates the supplied `cap` is not a valid cap value.
   */
  107: {message:"InvalidCap"},
  /**
   * Indicates the Cap was not set.
   */
  108: {message:"CapNotSet"},
  /**
   * Indicates the SAC address was not set.
   */
  109: {message:"SACNotSet"},
  /**
   * Indicates a SAC address different than expected.
   */
  110: {message:"SACAddressMismatch"},
  /**
   * Indicates a missing function parameter in the SAC contract context.
   */
  111: {message:"SACMissingFnParam"},
  /**
   * Indicates an invalid function parameter in the SAC contract context.
   */
  112: {message:"SACInvalidFnParam"},
  /**
   * The user is not allowed to perform this operation
   */
  113: {message:"UserNotAllowed"},
  /**
   * The user is blocked and cannot perform this operation
   */
  114: {message:"UserBlocked"}
}

/**
 * Storage key for accessing the SAC address
 */
export type SACAdminGenericDataKey = {tag: "Sac", values: void};

/**
 * Storage key for accessing the SAC address
 */
export type SACAdminWrapperDataKey = {tag: "Sac", values: void};


/**
 * Storage container for token metadata
 */
export interface Metadata {
  decimals: u32;
  name: string;
  symbol: string;
}

/**
 * Storage keys for the data associated with `FungibleToken`
 */
export type StorageKey = {tag: "TotalSupply", values: void} | {tag: "Balance", values: readonly [string]} | {tag: "Allowance", values: readonly [AllowanceKey]};


/**
 * Storage key that maps to [`AllowanceData`]
 */
export interface AllowanceKey {
  owner: string;
  spender: string;
}


/**
 * Storage container for the amount of tokens for which an allowance is granted
 * and the ledger number at which this allowance expires.
 */
export interface AllowanceData {
  amount: i128;
  live_until_ledger: u32;
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
   * Construct and simulate a name transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the name for this token.
   * 
   * # Arguments
   * 
   * * `e` - Access to Soroban environment.
   */
  name: (options?: MethodOptions) => Promise<AssembledTransaction<string>>

  /**
   * Construct and simulate a pause transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  pause: ({caller}: {caller: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a symbol transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the symbol for this token.
   * 
   * # Arguments
   * 
   * * `e` - Access to Soroban environment.
   */
  symbol: (options?: MethodOptions) => Promise<AssembledTransaction<string>>

  /**
   * Construct and simulate a approve transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Sets the amount of tokens a `spender` is allowed to spend on behalf of
   * an `owner`. Overrides any existing allowance set between `spender` and
   * `owner`.
   * 
   * # Arguments
   * 
   * * `e` - Access to Soroban environment.
   * * `owner` - The address holding the tokens.
   * * `spender` - The address authorized to spend the tokens.
   * * `amount` - The amount of tokens made available to `spender`.
   * * `live_until_ledger` - The ledger number at which the allowance
   * expires.
   * 
   * # Errors
   * 
   * * [`FungibleTokenError::InvalidLiveUntilLedger`] - Occurs when
   * attempting to set `live_until_ledger` that is less than the current
   * ledger number and greater than `0`.
   * * [`FungibleTokenError::LessThanZero`] - Occurs when `amount < 0`.
   * 
   * # Events
   * 
   * * topics - `["approve", from: Address, spender: Address]`
   * * data - `[amount: i128, live_until_ledger: u32]`
   */
  approve: ({owner, spender, amount, live_until_ledger}: {owner: string, spender: string, amount: i128, live_until_ledger: u32}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a balance transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the amount of tokens held by `account`.
   * 
   * # Arguments
   * 
   * * `e` - Access to the Soroban environment.
   * * `account` - The address for which the balance is being queried.
   */
  balance: ({account}: {account: string}, options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a migrate transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  migrate: ({data, operator}: {data: MigrationData, operator: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a unpause transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  unpause: ({caller}: {caller: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a upgrade transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  upgrade: ({new_wasm_hash, operator}: {new_wasm_hash: Buffer, operator: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a decimals transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  decimals: (options?: MethodOptions) => Promise<AssembledTransaction<u32>>

  /**
   * Construct and simulate a transfer transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  transfer: ({from, to, amount}: {from: string, to: string, amount: i128}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a allowance transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the amount of tokens a `spender` is allowed to spend on behalf
   * of an `owner`.
   * 
   * # Arguments
   * 
   * * `e` - Access to Soroban environment.
   * * `owner` - The address holding the tokens.
   * * `spender` - The address authorized to spend the tokens.
   */
  allowance: ({owner, spender}: {owner: string, spender: string}, options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a query_asset transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  query_asset: (options?: MethodOptions) => Promise<AssembledTransaction<string>>

  /**
   * Construct and simulate a total_supply transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the total amount of tokens in circulation.
   * 
   * # Arguments
   * 
   * * `e` - Access to the Soroban environment.
   */
  total_supply: (options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a get_lp_config transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_lp_config: (options?: MethodOptions) => Promise<AssembledTransaction<LpConfig>>

  /**
   * Construct and simulate a physical_cash transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  physical_cash: (options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a set_lp_config transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  set_lp_config: ({caller, config}: {caller: string, config: LpConfig}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a transfer_from transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  transfer_from: ({spender, from, to, amount}: {spender: string, from: string, to: string, amount: i128}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a cancel_upgrade transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  cancel_upgrade: ({caller}: {caller: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a settle_deposit transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  settle_deposit: ({caller, owner, assets, round}: {caller: string, owner: string, assets: i128, round: OracleRound}, options?: MethodOptions) => Promise<AssembledTransaction<SettlementResult>>

  /**
   * Construct and simulate a transfer_claim transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  transfer_claim: ({caller, recipient, amount, claims_after}: {caller: string, recipient: string, amount: i128, claims_after: i128}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a propose_upgrade transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  propose_upgrade: ({caller, wasm_hash}: {caller: string, wasm_hash: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a bump_vault_state transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  bump_vault_state: (options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a settle_withdrawal transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  settle_withdrawal: ({caller, owner, shares, round}: {caller: string, owner: string, shares: i128, round: OracleRound}, options?: MethodOptions) => Promise<AssembledTransaction<SettlementResult>>

  /**
   * Construct and simulate a receive_collateral transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  receive_collateral: ({caller, from, amount}: {caller: string, from: string, amount: i128}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a set_request_router transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  set_request_router: ({caller, request_router}: {caller: string, request_router: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a total_share_supply transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  total_share_supply: (options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a accounting_snapshot transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  accounting_snapshot: ({round}: {round: OracleRound}, options?: MethodOptions) => Promise<AssembledTransaction<AccountingSnapshot>>

  /**
   * Construct and simulate a can_create_lp_request transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  can_create_lp_request: (options?: MethodOptions) => Promise<AssembledTransaction<boolean>>

  /**
   * Construct and simulate a transfer_safety_claim transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  transfer_safety_claim: ({caller, recipient, amount}: {caller: string, recipient: string, amount: i128}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

}
export class Client extends ContractClient {
  static async deploy<T = Client>(
        /** Constructor/Initialization Args for the contract's `__constructor` method */
        {asset_address, config_manager, position_manager, lp_config}: {asset_address: string, config_manager: string, position_manager: string, lp_config: LpConfig},
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
    return ContractClient.deploy({asset_address, config_manager, position_manager, lp_config}, options)
  }
  constructor(public readonly options: ContractClientOptions) {
    super(
      new ContractSpec([ "AAAABAAAAAAAAAAAAAAAClZhdWx0RXJyb3IAAAAAAAwAAAAAAAAADFVuYXV0aG9yaXplZAAAAAEAAAAAAAAAEkFscmVhZHlJbml0aWFsaXplZAAAAAAAAgAAAAAAAAAOTm90SW5pdGlhbGl6ZWQAAAAAAAMAAAAAAAAADUludmFsaWRBbW91bnQAAAAAAAAEAAAAAAAAAA1JbnZhbGlkQ29uZmlnAAAAAAAABQAAAAAAAAAGUGF1c2VkAAAAAAAGAAAAAAAAABBJbnN1ZmZpY2llbnRDYXNoAAAABwAAAAAAAAANSW52YWxpZENhbGxlcgAAAAAAAAgAAAAAAAAAD0FyaXRobWV0aWNFcnJvcgAAAAAJAAAAKmB1cGdyYWRlYCBjYWxsZWQgd2l0aCBubyBwZW5kaW5nIHByb3Bvc2FsLgAAAAAAEFVwZ3JhZGVOb1BlbmRpbmcAAAAKAAAANGB1cGdyYWRlYCBjYWxsZWQgYmVmb3JlIHRoZSBwcm9wb3NhbCdzIHRpbWVsb2NrIGV0YS4AAAAZVXBncmFkZVRpbWVsb2NrTm90RWxhcHNlZAAAAAAAAAsAAAA8YHVwZ3JhZGVgIGNhbGxlZCB3aXRoIGEgaGFzaCB0aGF0IGRpZmZlcnMgZnJvbSB0aGUgcHJvcG9zYWwuAAAAE1VwZ3JhZGVIYXNoTWlzbWF0Y2gAAAAADA==",
        "AAAABQAAAAAAAAAAAAAADFBhdXNlQ2hhbmdlZAAAAAEAAAAFcGF1c2UAAAAAAAABAAAAAAAAAAZwYXVzZWQAAAAAAAEAAAAAAAAAAQ==",
        "AAAABQAAANlBIHF1ZXVlZCBkZXBvc2l0IHNldHRsZWQ6IGBhc3NldHNgIGVudGVyZWQgdGhlIHZhdWx0LCBgc2hhcmVzYCBtaW50ZWQgdG8KYG93bmVyYCAowqcxMy41KS4gYHNoYXJlX3N1cHBseWAgYW5kIGB2YXVsdF9uYXZgIGFyZSB0aGUgcG9zdC1zZXR0bGVtZW50CnRvdGFscywgc28gdGhlIGluZGV4ZXIgY2FuIHRyYWNrIHNoYXJlIHByaWNpbmcgd2l0aG91dCBhIHNlcGFyYXRlIHJlYWQuAAAAAAAAAAAAAA5EZXBvc2l0U2V0dGxlZAAAAAAAAQAAAAVscGRlcAAAAAAAAAUAAAAAAAAABW93bmVyAAAAAAAAEwAAAAAAAAAAAAAABmFzc2V0cwAAAAAACwAAAAAAAAAAAAAABnNoYXJlcwAAAAAACwAAAAAAAAAAAAAADHNoYXJlX3N1cHBseQAAAAsAAAAAAAAAAAAAAAl2YXVsdF9uYXYAAAAAAAALAAAAAAAAAAI=",
        "AAAABQAAAAAAAAAAAAAAD0xwQ29uZmlnVXBkYXRlZAAAAAABAAAABWNmZ2xwAAAAAAAAAQAAAAAAAAAGY29uZmlnAAAAAAfQAAAACExwQ29uZmlnAAAAAAAAAAI=",
        "AAAABQAAAI9BIHF1ZXVlZCB3aXRoZHJhd2FsIHNldHRsZWQ6IGBzaGFyZXNgIGJ1cm5lZCwgYGFzc2V0c2AgcGFpZCB0byBgb3duZXJgCijCpzEzLjYpLiBgc2hhcmVfc3VwcGx5YCBhbmQgYHZhdWx0X25hdmAgYXJlIHRoZSBwb3N0LXNldHRsZW1lbnQgdG90YWxzLgAAAAAAAAAAEVdpdGhkcmF3YWxTZXR0bGVkAAAAAAAAAQAAAARscHdkAAAABQAAAAAAAAAFb3duZXIAAAAAAAATAAAAAAAAAAAAAAAGc2hhcmVzAAAAAAALAAAAAAAAAAAAAAAGYXNzZXRzAAAAAAALAAAAAAAAAAAAAAAMc2hhcmVfc3VwcGx5AAAACwAAAAAAAAAAAAAACXZhdWx0X25hdgAAAAAAAAsAAAAAAAAAAg==",
        "AAAAAgAAAAAAAAAAAAAAA0tleQAAAAAHAAAAAAAAAAAAAAALSW5pdGlhbGl6ZWQAAAAAAAAAAAAAAAANQ29uZmlnTWFuYWdlcgAAAAAAAAAAAAAAAAAAD1Bvc2l0aW9uTWFuYWdlcgAAAAAAAAAAAAAAAA1SZXF1ZXN0Um91dGVyAAAAAAAAAAAAAAAAAAAITHBDb25maWcAAAAAAAAAAAAAAAZQYXVzZWQAAAAAAAAAAAAAAAAAB1ZlcnNpb24A",
        "AAAAAAAAAFVSZXR1cm5zIHRoZSBuYW1lIGZvciB0aGlzIHRva2VuLgoKIyBBcmd1bWVudHMKCiogYGVgIC0gQWNjZXNzIHRvIFNvcm9iYW4gZW52aXJvbm1lbnQuAAAAAAAABG5hbWUAAAAAAAAAAQAAABA=",
        "AAAAAAAAAAAAAAAFcGF1c2UAAAAAAAABAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAA",
        "AAAAAAAAAFdSZXR1cm5zIHRoZSBzeW1ib2wgZm9yIHRoaXMgdG9rZW4uCgojIEFyZ3VtZW50cwoKKiBgZWAgLSBBY2Nlc3MgdG8gU29yb2JhbiBlbnZpcm9ubWVudC4AAAAABnN5bWJvbAAAAAAAAAAAAAEAAAAQ",
        "AAAAAAAAAyZTZXRzIHRoZSBhbW91bnQgb2YgdG9rZW5zIGEgYHNwZW5kZXJgIGlzIGFsbG93ZWQgdG8gc3BlbmQgb24gYmVoYWxmIG9mCmFuIGBvd25lcmAuIE92ZXJyaWRlcyBhbnkgZXhpc3RpbmcgYWxsb3dhbmNlIHNldCBiZXR3ZWVuIGBzcGVuZGVyYCBhbmQKYG93bmVyYC4KCiMgQXJndW1lbnRzCgoqIGBlYCAtIEFjY2VzcyB0byBTb3JvYmFuIGVudmlyb25tZW50LgoqIGBvd25lcmAgLSBUaGUgYWRkcmVzcyBob2xkaW5nIHRoZSB0b2tlbnMuCiogYHNwZW5kZXJgIC0gVGhlIGFkZHJlc3MgYXV0aG9yaXplZCB0byBzcGVuZCB0aGUgdG9rZW5zLgoqIGBhbW91bnRgIC0gVGhlIGFtb3VudCBvZiB0b2tlbnMgbWFkZSBhdmFpbGFibGUgdG8gYHNwZW5kZXJgLgoqIGBsaXZlX3VudGlsX2xlZGdlcmAgLSBUaGUgbGVkZ2VyIG51bWJlciBhdCB3aGljaCB0aGUgYWxsb3dhbmNlCmV4cGlyZXMuCgojIEVycm9ycwoKKiBbYEZ1bmdpYmxlVG9rZW5FcnJvcjo6SW52YWxpZExpdmVVbnRpbExlZGdlcmBdIC0gT2NjdXJzIHdoZW4KYXR0ZW1wdGluZyB0byBzZXQgYGxpdmVfdW50aWxfbGVkZ2VyYCB0aGF0IGlzIGxlc3MgdGhhbiB0aGUgY3VycmVudApsZWRnZXIgbnVtYmVyIGFuZCBncmVhdGVyIHRoYW4gYDBgLgoqIFtgRnVuZ2libGVUb2tlbkVycm9yOjpMZXNzVGhhblplcm9gXSAtIE9jY3VycyB3aGVuIGBhbW91bnQgPCAwYC4KCiMgRXZlbnRzCgoqIHRvcGljcyAtIGBbImFwcHJvdmUiLCBmcm9tOiBBZGRyZXNzLCBzcGVuZGVyOiBBZGRyZXNzXWAKKiBkYXRhIC0gYFthbW91bnQ6IGkxMjgsIGxpdmVfdW50aWxfbGVkZ2VyOiB1MzJdYAAAAAAAB2FwcHJvdmUAAAAABAAAAAAAAAAFb3duZXIAAAAAAAATAAAAAAAAAAdzcGVuZGVyAAAAABMAAAAAAAAABmFtb3VudAAAAAAACwAAAAAAAAARbGl2ZV91bnRpbF9sZWRnZXIAAAAAAAAEAAAAAA==",
        "AAAAAAAAAKpSZXR1cm5zIHRoZSBhbW91bnQgb2YgdG9rZW5zIGhlbGQgYnkgYGFjY291bnRgLgoKIyBBcmd1bWVudHMKCiogYGVgIC0gQWNjZXNzIHRvIHRoZSBTb3JvYmFuIGVudmlyb25tZW50LgoqIGBhY2NvdW50YCAtIFRoZSBhZGRyZXNzIGZvciB3aGljaCB0aGUgYmFsYW5jZSBpcyBiZWluZyBxdWVyaWVkLgAAAAAAB2JhbGFuY2UAAAAAAQAAAAAAAAAHYWNjb3VudAAAAAATAAAAAQAAAAs=",
        "AAAAAAAAAAAAAAAHbWlncmF0ZQAAAAACAAAAAAAAAARkYXRhAAAH0AAAAA1NaWdyYXRpb25EYXRhAAAAAAAAAAAAAAhvcGVyYXRvcgAAABMAAAAA",
        "AAAAAAAAAAAAAAAHdW5wYXVzZQAAAAABAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAA",
        "AAAAAAAAAAAAAAAHdXBncmFkZQAAAAACAAAAAAAAAA1uZXdfd2FzbV9oYXNoAAAAAAAD7gAAACAAAAAAAAAACG9wZXJhdG9yAAAAEwAAAAA=",
        "AAAAAAAAAAAAAAAIZGVjaW1hbHMAAAAAAAAAAQAAAAQ=",
        "AAAAAAAAAAAAAAAIdHJhbnNmZXIAAAADAAAAAAAAAARmcm9tAAAAEwAAAAAAAAACdG8AAAAAABQAAAAAAAAABmFtb3VudAAAAAAACwAAAAA=",
        "AAAAAAAAAPBSZXR1cm5zIHRoZSBhbW91bnQgb2YgdG9rZW5zIGEgYHNwZW5kZXJgIGlzIGFsbG93ZWQgdG8gc3BlbmQgb24gYmVoYWxmCm9mIGFuIGBvd25lcmAuCgojIEFyZ3VtZW50cwoKKiBgZWAgLSBBY2Nlc3MgdG8gU29yb2JhbiBlbnZpcm9ubWVudC4KKiBgb3duZXJgIC0gVGhlIGFkZHJlc3MgaG9sZGluZyB0aGUgdG9rZW5zLgoqIGBzcGVuZGVyYCAtIFRoZSBhZGRyZXNzIGF1dGhvcml6ZWQgdG8gc3BlbmQgdGhlIHRva2Vucy4AAAAJYWxsb3dhbmNlAAAAAAAAAgAAAAAAAAAFb3duZXIAAAAAAAATAAAAAAAAAAdzcGVuZGVyAAAAABMAAAABAAAACw==",
        "AAAAAAAAAAAAAAALcXVlcnlfYXNzZXQAAAAAAAAAAAEAAAAT",
        "AAAAAAAAAGtSZXR1cm5zIHRoZSB0b3RhbCBhbW91bnQgb2YgdG9rZW5zIGluIGNpcmN1bGF0aW9uLgoKIyBBcmd1bWVudHMKCiogYGVgIC0gQWNjZXNzIHRvIHRoZSBTb3JvYmFuIGVudmlyb25tZW50LgAAAAAMdG90YWxfc3VwcGx5AAAAAAAAAAEAAAAL",
        "AAAAAAAAAAAAAAANX19jb25zdHJ1Y3RvcgAAAAAAAAQAAAAAAAAADWFzc2V0X2FkZHJlc3MAAAAAAAATAAAAAAAAAA5jb25maWdfbWFuYWdlcgAAAAAAEwAAAAAAAAAQcG9zaXRpb25fbWFuYWdlcgAAABMAAAAAAAAACWxwX2NvbmZpZwAAAAAAB9AAAAAITHBDb25maWcAAAAA",
        "AAAAAAAAAAAAAAANZ2V0X2xwX2NvbmZpZwAAAAAAAAAAAAABAAAH0AAAAAhMcENvbmZpZw==",
        "AAAAAAAAAAAAAAANcGh5c2ljYWxfY2FzaAAAAAAAAAAAAAABAAAACw==",
        "AAAAAAAAAAAAAAANc2V0X2xwX2NvbmZpZwAAAAAAAAIAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAAAAAAGY29uZmlnAAAAAAfQAAAACExwQ29uZmlnAAAAAA==",
        "AAAAAAAAAAAAAAANdHJhbnNmZXJfZnJvbQAAAAAAAAQAAAAAAAAAB3NwZW5kZXIAAAAAEwAAAAAAAAAEZnJvbQAAABMAAAAAAAAAAnRvAAAAAAATAAAAAAAAAAZhbW91bnQAAAAAAAsAAAAA",
        "AAAAAAAAAAAAAAAOY2FuY2VsX3VwZ3JhZGUAAAAAAAEAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAA=",
        "AAAAAAAAAAAAAAAOc2V0dGxlX2RlcG9zaXQAAAAAAAQAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAAAAAAFb3duZXIAAAAAAAATAAAAAAAAAAZhc3NldHMAAAAAAAsAAAAAAAAABXJvdW5kAAAAAAAH0AAAAAtPcmFjbGVSb3VuZAAAAAABAAAH0AAAABBTZXR0bGVtZW50UmVzdWx0",
        "AAAAAAAAAAAAAAAOdHJhbnNmZXJfY2xhaW0AAAAAAAQAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAAAAAAJcmVjaXBpZW50AAAAAAAAEwAAAAAAAAAGYW1vdW50AAAAAAALAAAAAAAAAAxjbGFpbXNfYWZ0ZXIAAAALAAAAAA==",
        "AAAAAAAAAAAAAAAPcHJvcG9zZV91cGdyYWRlAAAAAAIAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAAAAAAJd2FzbV9oYXNoAAAAAAAD7gAAACAAAAAA",
        "AAAAAAAAAAAAAAAQYnVtcF92YXVsdF9zdGF0ZQAAAAAAAAAA",
        "AAAAAAAAAAAAAAARc2V0dGxlX3dpdGhkcmF3YWwAAAAAAAAEAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAAAAAABW93bmVyAAAAAAAAEwAAAAAAAAAGc2hhcmVzAAAAAAALAAAAAAAAAAVyb3VuZAAAAAAAB9AAAAALT3JhY2xlUm91bmQAAAAAAQAAB9AAAAAQU2V0dGxlbWVudFJlc3VsdA==",
        "AAAAAAAAAAAAAAAScmVjZWl2ZV9jb2xsYXRlcmFsAAAAAAADAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAAAAAABGZyb20AAAATAAAAAAAAAAZhbW91bnQAAAAAAAsAAAAA",
        "AAAAAAAAAAAAAAASc2V0X3JlcXVlc3Rfcm91dGVyAAAAAAACAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAAAAAADnJlcXVlc3Rfcm91dGVyAAAAAAATAAAAAA==",
        "AAAAAAAAAAAAAAASdG90YWxfc2hhcmVfc3VwcGx5AAAAAAAAAAAAAQAAAAs=",
        "AAAAAAAAAAAAAAATYWNjb3VudGluZ19zbmFwc2hvdAAAAAABAAAAAAAAAAVyb3VuZAAAAAAAB9AAAAALT3JhY2xlUm91bmQAAAAAAQAAB9AAAAASQWNjb3VudGluZ1NuYXBzaG90AAA=",
        "AAAAAAAAAAAAAAAVY2FuX2NyZWF0ZV9scF9yZXF1ZXN0AAAAAAAAAAAAAAEAAAAB",
        "AAAAAAAAAAAAAAAVdHJhbnNmZXJfc2FmZXR5X2NsYWltAAAAAAAAAwAAAAAAAAAGY2FsbGVyAAAAAAATAAAAAAAAAAlyZWNpcGllbnQAAAAAAAATAAAAAAAAAAZhbW91bnQAAAAAAAsAAAAA",
        "AAAAAQAAAAAAAAAAAAAADk93bmVyVG9rZW5zS2V5AAAAAAACAAAAAAAAAAVpbmRleAAAAAAAAAQAAAAAAAAABW93bmVyAAAAAAAAEw==",
        "AAAAAgAAAFhTdG9yYWdlIGtleXMgZm9yIHRoZSBkYXRhIGFzc29jaWF0ZWQgd2l0aCB0aGUgZW51bWVyYWJsZSBleHRlbnNpb24gb2YKYE5vbkZ1bmdpYmxlVG9rZW5gAAAAAAAAABdORlRFbnVtZXJhYmxlU3RvcmFnZUtleQAAAAAFAAAAAAAAAAAAAAALVG90YWxTdXBwbHkAAAAAAQAAAAAAAAALT3duZXJUb2tlbnMAAAAAAQAAB9AAAAAOT3duZXJUb2tlbnNLZXkAAAAAAAEAAAAAAAAAEE93bmVyVG9rZW5zSW5kZXgAAAABAAAABAAAAAEAAAAAAAAADEdsb2JhbFRva2VucwAAAAEAAAAEAAAAAQAAAAAAAAARR2xvYmFsVG9rZW5zSW5kZXgAAAAAAAABAAAABA==",
        "AAAABQAAADFFdmVudCBlbWl0dGVkIHdoZW4gY29uc2VjdXRpdmUgdG9rZW5zIGFyZSBtaW50ZWQuAAAAAAAAAAAAAA9Db25zZWN1dGl2ZU1pbnQAAAAAAQAAABBjb25zZWN1dGl2ZV9taW50AAAAAwAAAAAAAAACdG8AAAAAABMAAAABAAAAAAAAAA1mcm9tX3Rva2VuX2lkAAAAAAAABAAAAAAAAAAAAAAAC3RvX3Rva2VuX2lkAAAAAAQAAAAAAAAAAg==",
        "AAAAAgAAAFlTdG9yYWdlIGtleXMgZm9yIHRoZSBkYXRhIGFzc29jaWF0ZWQgd2l0aCB0aGUgY29uc2VjdXRpdmUgZXh0ZW5zaW9uIG9mCmBOb25GdW5naWJsZVRva2VuYAAAAAAAAAAAAAAYTkZUQ29uc2VjdXRpdmVTdG9yYWdlS2V5AAAABAAAAAEAAAAAAAAACEFwcHJvdmFsAAAAAQAAAAQAAAABAAAAAAAAAAVPd25lcgAAAAAAAAEAAAAEAAAAAQAAAAAAAAAPT3duZXJzaGlwQnVja2V0AAAAAAEAAAAEAAAAAQAAAAAAAAALQnVybmVkVG9rZW4AAAAAAQAAAAQ=",
        "AAAABQAAACVFdmVudCBlbWl0dGVkIHdoZW4gYSB0b2tlbiBpcyBidXJuZWQuAAAAAAAAAAAAAARCdXJuAAAAAQAAAARidXJuAAAAAgAAAAAAAAAEZnJvbQAAABMAAAABAAAAAAAAAAh0b2tlbl9pZAAAAAQAAAAAAAAAAg==",
        "AAAABQAAAChFdmVudCBlbWl0dGVkIHdoZW4gdG9rZW4gcm95YWx0eSBpcyBzZXQuAAAAAAAAAA9TZXRUb2tlblJveWFsdHkAAAAAAQAAABFzZXRfdG9rZW5fcm95YWx0eQAAAAAAAAMAAAAAAAAACHJlY2VpdmVyAAAAEwAAAAEAAAAAAAAACHRva2VuX2lkAAAABAAAAAEAAAAAAAAADGJhc2lzX3BvaW50cwAAAAQAAAAAAAAAAg==",
        "AAAABQAAACpFdmVudCBlbWl0dGVkIHdoZW4gZGVmYXVsdCByb3lhbHR5IGlzIHNldC4AAAAAAAAAAAARU2V0RGVmYXVsdFJveWFsdHkAAAAAAAABAAAAE3NldF9kZWZhdWx0X3JveWFsdHkAAAAAAgAAAAAAAAAIcmVjZWl2ZXIAAAATAAAAAQAAAAAAAAAMYmFzaXNfcG9pbnRzAAAABAAAAAAAAAAC",
        "AAAABQAAACxFdmVudCBlbWl0dGVkIHdoZW4gdG9rZW4gcm95YWx0eSBpcyByZW1vdmVkLgAAAAAAAAASUmVtb3ZlVG9rZW5Sb3lhbHR5AAAAAAABAAAAFHJlbW92ZV90b2tlbl9yb3lhbHR5AAAAAQAAAAAAAAAIdG9rZW5faWQAAAAEAAAAAQAAAAI=",
        "AAAAAQAAAClTdG9yYWdlIGNvbnRhaW5lciBmb3Igcm95YWx0eSBpbmZvcm1hdGlvbgAAAAAAAAAAAAALUm95YWx0eUluZm8AAAAAAgAAAAAAAAAMYmFzaXNfcG9pbnRzAAAABAAAAAAAAAAIcmVjZWl2ZXIAAAAT",
        "AAAAAgAAAB1TdG9yYWdlIGtleXMgZm9yIHJveWFsdHkgZGF0YQAAAAAAAAAAAAAWTkZUUm95YWx0aWVzU3RvcmFnZUtleQAAAAAAAgAAAAAAAAAAAAAADkRlZmF1bHRSb3lhbHR5AAAAAAABAAAAAAAAAAxUb2tlblJveWFsdHkAAAABAAAABA==",
        "AAAABQAAACVFdmVudCBlbWl0dGVkIHdoZW4gYSB0b2tlbiBpcyBtaW50ZWQuAAAAAAAAAAAAAARNaW50AAAAAQAAAARtaW50AAAAAgAAAAAAAAACdG8AAAAAABMAAAABAAAAAAAAAAh0b2tlbl9pZAAAAAQAAAAAAAAAAg==",
        "AAAABQAAACpFdmVudCBlbWl0dGVkIHdoZW4gYW4gYXBwcm92YWwgaXMgZ3JhbnRlZC4AAAAAAAAAAAAHQXBwcm92ZQAAAAABAAAAB2FwcHJvdmUAAAAABAAAAAAAAAAIYXBwcm92ZXIAAAATAAAAAQAAAAAAAAAIdG9rZW5faWQAAAAEAAAAAQAAAAAAAAAIYXBwcm92ZWQAAAATAAAAAAAAAAAAAAARbGl2ZV91bnRpbF9sZWRnZXIAAAAAAAAEAAAAAAAAAAI=",
        "AAAABQAAACpFdmVudCBlbWl0dGVkIHdoZW4gYSB0b2tlbiBpcyB0cmFuc2ZlcnJlZC4AAAAAAAAAAAAIVHJhbnNmZXIAAAABAAAACHRyYW5zZmVyAAAAAwAAAAAAAAAEZnJvbQAAABMAAAABAAAAAAAAAAJ0bwAAAAAAEwAAAAEAAAAAAAAACHRva2VuX2lkAAAABAAAAAAAAAAC",
        "AAAABQAAADZFdmVudCBlbWl0dGVkIHdoZW4gYXBwcm92YWwgZm9yIGFsbCB0b2tlbnMgaXMgZ3JhbnRlZC4AAAAAAAAAAAANQXBwcm92ZUZvckFsbAAAAAAAAAEAAAAPYXBwcm92ZV9mb3JfYWxsAAAAAAMAAAAAAAAABW93bmVyAAAAAAAAEwAAAAEAAAAAAAAACG9wZXJhdG9yAAAAEwAAAAAAAAAAAAAAEWxpdmVfdW50aWxfbGVkZ2VyAAAAAAAABAAAAAAAAAAC",
        "AAAABAAAAAAAAAAAAAAAFU5vbkZ1bmdpYmxlVG9rZW5FcnJvcgAAAAAAAA8AAAAkSW5kaWNhdGVzIGEgbm9uLWV4aXN0ZW50IGB0b2tlbl9pZGAuAAAAEE5vbkV4aXN0ZW50VG9rZW4AAADIAAAAV0luZGljYXRlcyBhbiBlcnJvciByZWxhdGVkIHRvIHRoZSBvd25lcnNoaXAgb3ZlciBhIHBhcnRpY3VsYXIgdG9rZW4uClVzZWQgaW4gdHJhbnNmZXJzLgAAAAAOSW5jb3JyZWN0T3duZXIAAAAAAMkAAABFSW5kaWNhdGVzIGEgZmFpbHVyZSB3aXRoIHRoZSBgb3BlcmF0b3JgcyBhcHByb3ZhbC4gVXNlZCBpbiB0cmFuc2ZlcnMuAAAAAAAAFEluc3VmZmljaWVudEFwcHJvdmFsAAAAygAAAFVJbmRpY2F0ZXMgYSBmYWlsdXJlIHdpdGggdGhlIGBhcHByb3ZlcmAgb2YgYSB0b2tlbiB0byBiZSBhcHByb3ZlZC4gVXNlZAppbiBhcHByb3ZhbHMuAAAAAAAAD0ludmFsaWRBcHByb3ZlcgAAAADLAAAASkluZGljYXRlcyBhbiBpbnZhbGlkIHZhbHVlIGZvciBgbGl2ZV91bnRpbF9sZWRnZXJgIHdoZW4gc2V0dGluZwphcHByb3ZhbHMuAAAAAAAWSW52YWxpZExpdmVVbnRpbExlZGdlcgAAAAAAzAAAAClJbmRpY2F0ZXMgb3ZlcmZsb3cgd2hlbiBhZGRpbmcgdHdvIHZhbHVlcwAAAAAAAAxNYXRoT3ZlcmZsb3cAAADNAAAANkluZGljYXRlcyBhbGwgcG9zc2libGUgYHRva2VuX2lkYHMgYXJlIGFscmVhZHkgaW4gdXNlLgAAAAAAE1Rva2VuSURzQXJlRGVwbGV0ZWQAAAAAzgAAAEVJbmRpY2F0ZXMgYW4gaW52YWxpZCBhbW91bnQgdG8gYmF0Y2ggbWludCBpbiBgY29uc2VjdXRpdmVgIGV4dGVuc2lvbi4AAAAAAAANSW52YWxpZEFtb3VudAAAAAAAAM8AAAAzSW5kaWNhdGVzIHRoZSB0b2tlbiBkb2VzIG5vdCBleGlzdCBpbiBvd25lcidzIGxpc3QuAAAAABhUb2tlbk5vdEZvdW5kSW5Pd25lckxpc3QAAADQAAAAMkluZGljYXRlcyB0aGUgdG9rZW4gZG9lcyBub3QgZXhpc3QgaW4gZ2xvYmFsIGxpc3QuAAAAAAAZVG9rZW5Ob3RGb3VuZEluR2xvYmFsTGlzdAAAAAAAANEAAAAjSW5kaWNhdGVzIGFjY2VzcyB0byB1bnNldCBtZXRhZGF0YS4AAAAADVVuc2V0TWV0YWRhdGEAAAAAAADSAAAAQUluZGljYXRlcyB0aGUgbGVuZ3RoIG9mIHRoZSBiYXNlIFVSSSBleGNlZWRzIHRoZSBtYXhpbXVtIGFsbG93ZWQuAAAAAAAAFUJhc2VVcmlNYXhMZW5FeGNlZWRlZAAAAAAAANMAAABHSW5kaWNhdGVzIHRoZSByb3lhbHR5IGFtb3VudCBpcyBoaWdoZXIgdGhhbiAxMF8wMDAgKDEwMCUpIGJhc2lzIHBvaW50cy4AAAAAFEludmFsaWRSb3lhbHR5QW1vdW50AAAA1AAAAD1JbmRpY2F0ZXMgdGhlIGxlbmd0aCBvZiB0aGUgbmFtZSBleGNlZWRzIHRoZSBtYXhpbXVtIGFsbG93ZWQuAAAAAAAAEk5hbWVNYXhMZW5FeGNlZWRlZAAAAAAA1QAAAD9JbmRpY2F0ZXMgdGhlIGxlbmd0aCBvZiB0aGUgc3ltYm9sIGV4Y2VlZHMgdGhlIG1heGltdW0gYWxsb3dlZC4AAAAAFFN5bWJvbE1heExlbkV4Y2VlZGVkAAAA1g==",
        "AAAAAgAAAAAAAAAAAAAAF05GVFNlcXVlbnRpYWxTdG9yYWdlS2V5AAAAAAEAAAAAAAAAAAAAAA5Ub2tlbklkQ291bnRlcgAA",
        "AAAAAQAAACRTdG9yYWdlIGNvbnRhaW5lciBmb3IgdG9rZW4gbWV0YWRhdGEAAAAAAAAACE1ldGFkYXRhAAAAAwAAAAAAAAAIYmFzZV91cmkAAAAQAAAAAAAAAARuYW1lAAAAEAAAAAAAAAAGc3ltYm9sAAAAAAAQ",
        "AAAAAQAAAHZTdG9yYWdlIGNvbnRhaW5lciBmb3IgdGhlIHRva2VuIGZvciB3aGljaCBhbiBhcHByb3ZhbCBpcyBncmFudGVkCmFuZCB0aGUgbGVkZ2VyIG51bWJlciBhdCB3aGljaCB0aGlzIGFwcHJvdmFsIGV4cGlyZXMuAAAAAAAAAAAADEFwcHJvdmFsRGF0YQAAAAIAAAAAAAAACGFwcHJvdmVkAAAAEwAAAAAAAAARbGl2ZV91bnRpbF9sZWRnZXIAAAAAAAAE",
        "AAAAAgAAADxTdG9yYWdlIGtleXMgZm9yIHRoZSBkYXRhIGFzc29jaWF0ZWQgd2l0aCBgTm9uRnVuZ2libGVUb2tlbmAAAAAAAAAADU5GVFN0b3JhZ2VLZXkAAAAAAAAFAAAAAQAAAAAAAAAFT3duZXIAAAAAAAABAAAABAAAAAEAAAAAAAAAB0JhbGFuY2UAAAAAAQAAABMAAAABAAAAAAAAAAhBcHByb3ZhbAAAAAEAAAAEAAAAAQAAAAAAAAAOQXBwcm92YWxGb3JBbGwAAAAAAAIAAAATAAAAEwAAAAAAAAAAAAAACE1ldGFkYXRh",
        "AAAABQAAADNFdmVudCBlbWl0dGVkIHdoZW4gYSBtb2R1bGUgaXMgYWRkZWQgdG8gY29tcGxpYW5jZS4AAAAAAAAAAAtNb2R1bGVBZGRlZAAAAAABAAAADG1vZHVsZV9hZGRlZAAAAAIAAAAAAAAABGhvb2sAAAfQAAAADkNvbXBsaWFuY2VIb29rAAAAAAABAAAAAAAAAAZtb2R1bGUAAAAAABMAAAAAAAAAAg==",
        "AAAABQAAADdFdmVudCBlbWl0dGVkIHdoZW4gYSBtb2R1bGUgaXMgcmVtb3ZlZCBmcm9tIGNvbXBsaWFuY2UuAAAAAAAAAAANTW9kdWxlUmVtb3ZlZAAAAAAAAAEAAAAObW9kdWxlX3JlbW92ZWQAAAAAAAIAAAAAAAAABGhvb2sAAAfQAAAADkNvbXBsaWFuY2VIb29rAAAAAAABAAAAAAAAAAZtb2R1bGUAAAAAABMAAAAAAAAAAg==",
        "AAAAAgAAAJNIb29rIHR5cGVzIGZvciBtb2R1bGFyIGNvbXBsaWFuY2Ugc3lzdGVtLgoKRWFjaCBob29rIHR5cGUgcmVwcmVzZW50cyBhIHNwZWNpZmljIGV2ZW50IG9yIHZhbGlkYXRpb24gcG9pbnQKd2hlcmUgY29tcGxpYW5jZSBtb2R1bGVzIGNhbiBiZSBleGVjdXRlZC4AAAAAAAAAAA5Db21wbGlhbmNlSG9vawAAAAAABQAAAAAAAACeQ2FsbGVkIGFmdGVyIHRva2VucyBhcmUgc3VjY2Vzc2Z1bGx5IHRyYW5zZmVycmVkIGZyb20gb25lIHdhbGxldCB0bwphbm90aGVyLiBNb2R1bGVzIHJlZ2lzdGVyZWQgZm9yIHRoaXMgaG9vayBjYW4gdXBkYXRlIHRoZWlyIHN0YXRlCmJhc2VkIG9uIHRyYW5zZmVyIGV2ZW50cy4AAAAAAAtUcmFuc2ZlcnJlZAAAAAAAAAAAkUNhbGxlZCBhZnRlciB0b2tlbnMgYXJlIHN1Y2Nlc3NmdWxseSBjcmVhdGVkL21pbnRlZCB0byBhIHdhbGxldC4KTW9kdWxlcyByZWdpc3RlcmVkIGZvciB0aGlzIGhvb2sgY2FuIHVwZGF0ZSB0aGVpciBzdGF0ZSBiYXNlZCBvbiBtaW50aW5nCmV2ZW50cy4AAAAAAAAHQ3JlYXRlZAAAAAAAAAAAlUNhbGxlZCBhZnRlciB0b2tlbnMgYXJlIHN1Y2Nlc3NmdWxseSBkZXN0cm95ZWQvYnVybmVkIGZyb20gYSB3YWxsZXQuCk1vZHVsZXMgcmVnaXN0ZXJlZCBmb3IgdGhpcyBob29rIGNhbiB1cGRhdGUgdGhlaXIgc3RhdGUgYmFzZWQgb24gYnVybmluZwpldmVudHMuAAAAAAAACURlc3Ryb3llZAAAAAAAAAAAAADMQ2FsbGVkIGR1cmluZyB0cmFuc2ZlciB2YWxpZGF0aW9uIHRvIGNoZWNrIGlmIGEgdHJhbnNmZXIgc2hvdWxkIGJlCmFsbG93ZWQuIE1vZHVsZXMgcmVnaXN0ZXJlZCBmb3IgdGhpcyBob29rIGNhbiBpbXBsZW1lbnQgdHJhbnNmZXIKcmVzdHJpY3Rpb25zLiBUaGlzIGlzIGEgUkVBRC1vbmx5IG9wZXJhdGlvbiBhbmQgc2hvdWxkIG5vdCBtb2RpZnkKc3RhdGUuAAAAC0NhblRyYW5zZmVyAAAAAAAAAADOQ2FsbGVkIGR1cmluZyBtaW50IHZhbGlkYXRpb24gdG8gY2hlY2sgaWYgYSBtaW50IG9wZXJhdGlvbiBzaG91bGQgYmUKYWxsb3dlZC4gTW9kdWxlcyByZWdpc3RlcmVkIGZvciB0aGlzIGhvb2sgY2FuIGltcGxlbWVudCB0cmFuc2ZlcgpyZXN0cmljdGlvbnMuIFRoaXMgaXMgYSBSRUFELW9ubHkgb3BlcmF0aW9uIGFuZCBzaG91bGQgbm90IG1vZGlmeQpzdGF0ZS4AAAAAAAlDYW5DcmVhdGUAAAA=",
        "AAAABAAAAAAAAAAAAAAAD0NvbXBsaWFuY2VFcnJvcgAAAAAEAAAAN0luZGljYXRlcyBhIG1vZHVsZSBpcyBhbHJlYWR5IHJlZ2lzdGVyZWQgZm9yIHRoaXMgaG9vay4AAAAAF01vZHVsZUFscmVhZHlSZWdpc3RlcmVkAAAAAWgAAAAzSW5kaWNhdGVzIGEgbW9kdWxlIGlzIG5vdCByZWdpc3RlcmVkIGZvciB0aGlzIGhvb2suAAAAABNNb2R1bGVOb3RSZWdpc3RlcmVkAAAAAWkAAAAlSW5kaWNhdGVzIGEgbW9kdWxlIGJvdW5kIGlzIGV4Y2VlZGVkLgAAAAAAABNNb2R1bGVCb3VuZEV4Y2VlZGVkAAAAAWoAAAA7SW5kaWNhdGVzIGEgdG9rZW4gaXMgbm90IGJvdW5kIHRvIHRoaXMgY29tcGxpYW5jZSBjb250cmFjdC4AAAAADVRva2VuTm90Qm91bmQAAAAAAAFr",
        "AAAAAgAAADFTdG9yYWdlIGtleXMgZm9yIHRoZSBtb2R1bGFyIGNvbXBsaWFuY2UgY29udHJhY3QuAAAAAAAAAAAAABFDb21wbGlhbmNlRGF0YUtleQAAAAAAAAEAAAABAAAAPE1hcHMgQ29tcGxpYW5jZUhvb2sgLT4gYFZlYzxBZGRyZXNzPmAgZm9yIHJlZ2lzdGVyZWQgbW9kdWxlcwAAAAtIb29rTW9kdWxlcwAAAAABAAAH0AAAAA5Db21wbGlhbmNlSG9vawAA",
        "AAAABAAAAC9FcnJvciBjb2RlcyBmb3IgZG9jdW1lbnQgbWFuYWdlbWVudCBvcGVyYXRpb25zLgAAAAAAAAAADURvY3VtZW50RXJyb3IAAAAAAAADAAAAJVRoZSBzcGVjaWZpZWQgZG9jdW1lbnQgd2FzIG5vdCBmb3VuZC4AAAAAAAAQRG9jdW1lbnROb3RGb3VuZAAAAXwAAAAtTWF4aW11bSBudW1iZXIgb2YgZG9jdW1lbnRzIGhhcyBiZWVuIHJlYWNoZWQuAAAAAAAAE01heERvY3VtZW50c1JlYWNoZWQAAAABfQAAACtUaGUgVVJJIGV4Y2VlZHMgdGhlIG1heGltdW0gYWxsb3dlZCBsZW5ndGguAAAAAApVcmlUb29Mb25nAAAAAAF+",
        "AAAABQAAAClFdmVudCBlbWl0dGVkIHdoZW4gYSBkb2N1bWVudCBpcyByZW1vdmVkLgAAAAAAAAAAAAAPRG9jdW1lbnRSZW1vdmVkAAAAAAEAAAAQZG9jdW1lbnRfcmVtb3ZlZAAAAAEAAAAAAAAABG5hbWUAAAPuAAAAIAAAAAEAAAAC",
        "AAAABQAAAD1FdmVudCBlbWl0dGVkIHdoZW4gYSBkb2N1bWVudCBpcyB1cGRhdGVkIChhZGRlZCBvciBtb2RpZmllZCkuAAAAAAAAAAAAAA9Eb2N1bWVudFVwZGF0ZWQAAAAAAQAAABBkb2N1bWVudF91cGRhdGVkAAAABAAAAAAAAAAEbmFtZQAAA+4AAAAgAAAAAQAAAAAAAAADdXJpAAAAABAAAAAAAAAAAAAAAA1kb2N1bWVudF9oYXNoAAAAAAAD7gAAACAAAAAAAAAAAAAAAAl0aW1lc3RhbXAAAAAAAAAGAAAAAAAAAAI=",
        "AAAAAQAAAChSZXByZXNlbnRzIGEgZG9jdW1lbnQgd2l0aCBpdHMgbWV0YWRhdGEuAAAAAAAAAAhEb2N1bWVudAAAAAMAAAAiVGhlIGhhc2ggb2YgdGhlIGRvY3VtZW50IGNvbnRlbnRzLgAAAAAADWRvY3VtZW50X2hhc2gAAAAAAAPuAAAAIAAAAC5UaW1lc3RhbXAgd2hlbiB0aGUgZG9jdW1lbnQgd2FzIGxhc3QgbW9kaWZpZWQuAAAAAAAJdGltZXN0YW1wAAAAAAAABgAAACtUaGUgVVJJIHdoZXJlIHRoZSBkb2N1bWVudCBjYW4gYmUgYWNjZXNzZWQuAAAAAAN1cmkAAAAAEA==",
        "AAAAAgAAACVTdG9yYWdlIGtleXMgZm9yIGRvY3VtZW50IG1hbmFnZW1lbnQuAAAAAAAAAAAAABJEb2N1bWVudFN0b3JhZ2VLZXkAAAAAAAMAAAABAAAAJ01hcHMgZG9jdW1lbnQgbmFtZSB0byBpdHMgZ2xvYmFsIGluZGV4LgAAAAAFSW5kZXgAAAAAAAABAAAD7gAAACAAAAABAAAAOU1hcHMgYnVja2V0IGluZGV4IHRvIGEgdmVjdG9yIG9mIChuYW1lLCBkb2N1bWVudCkgdHVwbGVzLgAAAAAAAAZCdWNrZXQAAAAAAAEAAAAEAAAAAAAAABlUb3RhbCBjb3VudCBvZiBkb2N1bWVudHMuAAAAAAAABUNvdW50AAAA",
        "AAAABQAAAEFFdmVudCBlbWl0dGVkIHdoZW4gYSBrZXkgaXMgYWxsb3dlZCBmb3IgYSBzY2hlbWUgYW5kIGNsYWltIHRvcGljLgAAAAAAAAAAAAAKS2V5QWxsb3dlZAAAAAAAAQAAAAtrZXlfYWxsb3dlZAAAAAAEAAAAAAAAAApwdWJsaWNfa2V5AAAAAAAOAAAAAQAAAAAAAAAIcmVnaXN0cnkAAAATAAAAAAAAAAAAAAAGc2NoZW1lAAAAAAAEAAAAAAAAAAAAAAALY2xhaW1fdG9waWMAAAAABAAAAAAAAAAC",
        "AAAABQAAAEJFdmVudCBlbWl0dGVkIHdoZW4gYSBrZXkgaXMgcmVtb3ZlZCBmcm9tIGEgc2NoZW1lIGFuZCBjbGFpbSB0b3BpYy4AAAAAAAAAAAAKS2V5UmVtb3ZlZAAAAAAAAQAAAAtrZXlfcmVtb3ZlZAAAAAAEAAAAAAAAAApwdWJsaWNfa2V5AAAAAAAOAAAAAQAAAAAAAAAIcmVnaXN0cnkAAAATAAAAAAAAAAAAAAAGc2NoZW1lAAAAAAAEAAAAAAAAAAAAAAALY2xhaW1fdG9waWMAAAAABAAAAAAAAAAC",
        "AAAABQAAACZFdmVudCBlbWl0dGVkIHdoZW4gYSBjbGFpbSBpcyByZXZva2VkLgAAAAAAAAAAAAxDbGFpbVJldm9rZWQAAAABAAAADWNsYWltX3Jldm9rZWQAAAAAAAAEAAAAAAAAAAhpZGVudGl0eQAAABMAAAABAAAAAAAAAAtjbGFpbV90b3BpYwAAAAAEAAAAAQAAAAAAAAAHcmV2b2tlZAAAAAABAAAAAQAAAAAAAAAKY2xhaW1fZGF0YQAAAAAADgAAAAAAAAAC",
        "AAAABAAAAAAAAAAAAAAAEENsYWltSXNzdWVyRXJyb3IAAAAKAAAAOVNpZ25hdHVyZSBkYXRhIGxlbmd0aCBkb2VzIG5vdCBtYXRjaCB0aGUgZXhwZWN0ZWQgc2NoZW1lLgAAAAAAAA9TaWdEYXRhTWlzbWF0Y2gAAAABXgAAABpUaGUgcHJvdmlkZWQga2V5IGlzIGVtcHR5LgAAAAAACktleUlzRW1wdHkAAAAAAV8AAAAzVGhlIGtleSBpcyBhbHJlYWR5IGFsbG93ZWQgZm9yIHRoZSBzcGVjaWZpZWQgdG9waWMuAAAAABFLZXlBbHJlYWR5QWxsb3dlZAAAAAAAAWAAAAA0VGhlIHNwZWNpZmllZCBrZXkgd2FzIG5vdCBmb3VuZCBpbiB0aGUgYWxsb3dlZCBrZXlzLgAAAAtLZXlOb3RGb3VuZAAAAAFhAAAAT1RoZSBjbGFpbSBpc3N1ZXIgaXMgbm90IGFsbG93ZWQgdG8gc2lnbiBjbGFpbXMgYWJvdXQgdGhlIHNwZWNpZmllZApjbGFpbSB0b3BpYy4AAAAACk5vdEFsbG93ZWQAAAAAAWIAAAA+TWF4aW11bSBsaW1pdCBleGNlZWRlZCAoa2V5cyBwZXIgdG9waWMgb3IgcmVnaXN0cmllcyBwZXIga2V5KS4AAAAAAA1MaW1pdEV4Y2VlZGVkAAAAAAABYwAAADRObyBzaWduaW5nIGtleXMgZm91bmQgZm9yIHRoZSBzcGVjaWZpZWQgY2xhaW0gdG9waWMuAAAADk5vS2V5c0ZvclRvcGljAAAAAAFkAAAAHEludmFsaWQgY2xhaW0gZGF0YSBlbmNvZGluZy4AAAAaSW52YWxpZENsYWltRGF0YUV4cGlyYXRpb24AAAAAAWUAAAAsUmVjb3Zlcnkgb2YgdGhlIFNlY3AyNTZrMSBwdWJsaWMga2V5IGZhaWxlZC4AAAAXU2VjcDI1NmsxUmVjb3ZlcnlGYWlsZWQAAAABZgAAACpJbmRpY2F0ZXMgb3ZlcmZsb3cgd2hlbiBhZGRpbmcgdHdvIHZhbHVlcy4AAAAAAAxNYXRoT3ZlcmZsb3cAAAFn",
        "AAAABQAAAE5FdmVudCBlbWl0dGVkIHdoZW4gY2xhaW0gc2lnbmF0dXJlcyBhcmUgaW52YWxpZGF0ZWQgYnkgaW5jcmVtZW50aW5nIHRoZQpub25jZS4AAAAAAAAAAAAVU2lnbmF0dXJlc0ludmFsaWRhdGVkAAAAAAAAAQAAABZzaWduYXR1cmVzX2ludmFsaWRhdGVkAAAAAAADAAAAAAAAAAhpZGVudGl0eQAAABMAAAABAAAAAAAAAAtjbGFpbV90b3BpYwAAAAAEAAAAAQAAAAAAAAAFbm9uY2UAAAAAAAAEAAAAAAAAAAI=",
        "AAAAAQAAAAAAAAAAAAAAClNpZ25pbmdLZXkAAAAAAAIAAAAAAAAACnB1YmxpY19rZXkAAAAAAA4AAAAAAAAABnNjaGVtZQAAAAAABA==",
        "AAAAAQAAACJTaWduYXR1cmUgZGF0YSBmb3IgRWQyNTUxOSBzY2hlbWUuAAAAAAAAAAAAFEVkMjU1MTlTaWduYXR1cmVEYXRhAAAAAgAAAAAAAAAKcHVibGljX2tleQAAAAAD7gAAACAAAAAAAAAACXNpZ25hdHVyZQAAAAAAA+4AAABA",
        "AAAAAgAAAC1TdG9yYWdlIGtleXMgZm9yIGNsYWltIGlzc3VlciBrZXkgbWFuYWdlbWVudC4AAAAAAAAAAAAAFUNsYWltSXNzdWVyU3RvcmFnZUtleQAAAAAAAAQAAAABAAAAH01hcHMgVG9waWMgLT4gYFZlYzxTaWduaW5nS2V5PmAAAAAABlRvcGljcwAAAAAAAQAAAAQAAAABAAAAKU1hcHMgU2lnbmluZ0tleSAtPiBWZWM8KFRvcGljLCBSZWdpc3RyeSk+AAAAAAAABVBhaXJzAAAAAAAAAQAAB9AAAAAKU2lnbmluZ0tleQAAAAAAAQAAADBUcmFja3MgZXhwbGljaXRseSByZXZva2VkIGNsYWltcyBieSBjbGFpbSBkaWdlc3QAAAAMUmV2b2tlZENsYWltAAAAAQAAA+4AAAAgAAAAAQAAAD1UcmFja3MgY3VycmVudCBub25jZSBmb3IgYSBzcGVjaWZpYyBpZGVudGl0eSBhbmQgY2xhaW0gdG9waWNzAAAAAAAACkNsYWltTm9uY2UAAAAAAAIAAAATAAAABA==",
        "AAAAAQAAACRTaWduYXR1cmUgZGF0YSBmb3IgU2VjcDI1NmsxIHNjaGVtZS4AAAAAAAAAFlNlY3AyNTZrMVNpZ25hdHVyZURhdGEAAAAAAAMAAAAAAAAACnB1YmxpY19rZXkAAAAAA+4AAABBAAAAAAAAAAtyZWNvdmVyeV9pZAAAAAAEAAAAAAAAAAlzaWduYXR1cmUAAAAAAAPuAAAAQA==",
        "AAAAAQAAACRTaWduYXR1cmUgZGF0YSBmb3IgU2VjcDI1NnIxIHNjaGVtZS4AAAAAAAAAFlNlY3AyNTZyMVNpZ25hdHVyZURhdGEAAAAAAAIAAAAAAAAACnB1YmxpY19rZXkAAAAAA+4AAABBAAAAAAAAAAlzaWduYXR1cmUAAAAAAAPuAAAAQA==",
        "AAAABQAAACRFdmVudCBlbWl0dGVkIHdoZW4gYSBjbGFpbSBpcyBhZGRlZC4AAAAAAAAACkNsYWltQWRkZWQAAAAAAAEAAAALY2xhaW1fYWRkZWQAAAAAAQAAAAAAAAAFY2xhaW0AAAAAAAfQAAAABUNsYWltAAAAAAAAAQAAAAI=",
        "AAAABAAAAAAAAAAAAAAAC0NsYWltc0Vycm9yAAAAAAIAAAAZQ2xhaW0gIElEIGRvZXMgbm90IGV4aXN0LgAAAAAAAA1DbGFpbU5vdEZvdW5kAAAAAAABVAAAAGdDbGFpbSBJc3N1ZXIgY2Fubm90IHZhbGlkYXRlIHRoZSBjbGFpbSAocmV2b2NhdGlvbiwgc2lnbmF0dXJlIG1pc21hdGNoLAp1bmF1dGhvcml6ZWQgc2lnbmluZyBrZXksIGV0Yy4pAAAAAA1DbGFpbU5vdFZhbGlkAAAAAAABVQ==",
        "AAAABQAAACZFdmVudCBlbWl0dGVkIHdoZW4gYSBjbGFpbSBpcyBjaGFuZ2VkLgAAAAAAAAAAAAxDbGFpbUNoYW5nZWQAAAABAAAADWNsYWltX2NoYW5nZWQAAAAAAAABAAAAAAAAAAVjbGFpbQAAAAAAB9AAAAAFQ2xhaW0AAAAAAAABAAAAAg==",
        "AAAABQAAACZFdmVudCBlbWl0dGVkIHdoZW4gYSBjbGFpbSBpcyByZW1vdmVkLgAAAAAAAAAAAAxDbGFpbVJlbW92ZWQAAAABAAAADWNsYWltX3JlbW92ZWQAAAAAAAABAAAAAAAAAAVjbGFpbQAAAAAAB9AAAAAFQ2xhaW0AAAAAAAABAAAAAg==",
        "AAAAAQAAACNSZXByZXNlbnRzIGEgY2xhaW0gc3RvcmVkIG9uLWNoYWluLgAAAAAAAAAABUNsYWltAAAAAAAABgAAAA5UaGUgY2xhaW0gZGF0YQAAAAAABGRhdGEAAAAOAAAAH1RoZSBhZGRyZXNzIG9mIHRoZSBjbGFpbSBpc3N1ZXIAAAAABmlzc3VlcgAAAAAAEwAAABlUaGUgc2lnbmF0dXJlIHNjaGVtZSB1c2VkAAAAAAAABnNjaGVtZQAAAAAABAAAABtUaGUgY3J5cHRvZ3JhcGhpYyBzaWduYXR1cmUAAAAACXNpZ25hdHVyZQAAAAAAAA4AAAAkVGhlIGNsYWltIHRvcGljIChudW1lcmljIGlkZW50aWZpZXIpAAAABXRvcGljAAAAAAAABAAAACdPcHRpb25hbCBVUkkgZm9yIGFkZGl0aW9uYWwgaW5mb3JtYXRpb24AAAAAA3VyaQAAAAAQ",
        "AAAAAgAAADpTdG9yYWdlIGtleXMgZm9yIHRoZSBkYXRhIGFzc29jaWF0ZWQgd2l0aCBJZGVudGl0eSBDbGFpbXMuAAAAAAAAAAAAEENsYWltc1N0b3JhZ2VLZXkAAAACAAAAAQAAABtNYXBzIGNsYWltIElEIHRvIGNsYWltIGRhdGEAAAAABUNsYWltAAAAAAAAAQAAA+4AAAAgAAAAAQAAACFNYXBzIHRvcGljIHRvIHZlY3RvciBvZiBjbGFpbSBJRHMAAAAAAAANQ2xhaW1zQnlUb3BpYwAAAAAAAAEAAAAE",
        "AAAAAgAAADVTdG9yYWdlIGtleXMgZm9yIHRoZSBkYXRhIGFzc29jaWF0ZWQgd2l0aCBgUldBYCB0b2tlbgAAAAAAAAAAAAAaSWRlbnRpdHlWZXJpZmllclN0b3JhZ2VLZXkAAAAAAAIAAAAAAAAAKUNsYWltIFRvcGljcyBhbmQgSXNzdWVycyBjb250cmFjdCBhZGRyZXNzAAAAAAAAFUNsYWltVG9waWNzQW5kSXNzdWVycwAAAAAAAAAAAAAqSWRlbnRpdHkgUmVnaXN0cnkgU3RvcmFnZSBjb250cmFjdCBhZGRyZXNzAAAAAAAXSWRlbnRpdHlSZWdpc3RyeVN0b3JhZ2UA",
        "AAAABQAAACVFdmVudCBlbWl0dGVkIHdoZW4gdG9rZW5zIGFyZSBidXJuZWQuAAAAAAAAAAAAAARCdXJuAAAAAQAAAARidXJuAAAAAgAAAAAAAAAEZnJvbQAAABMAAAABAAAAAAAAAAZhbW91bnQAAAAAAAsAAAAAAAAAAg==",
        "AAAABQAAACVFdmVudCBlbWl0dGVkIHdoZW4gdG9rZW5zIGFyZSBtaW50ZWQuAAAAAAAAAAAAAARNaW50AAAAAQAAAARtaW50AAAAAgAAAAAAAAACdG8AAAAAABMAAAABAAAAAAAAAAZhbW91bnQAAAAAAAsAAAAAAAAAAg==",
        "AAAABAAAAAAAAAAAAAAACFJXQUVycm9yAAAADgAAAEVJbmRpY2F0ZXMgYW4gZXJyb3IgcmVsYXRlZCB0byBpbnN1ZmZpY2llbnQgYmFsYW5jZSBmb3IgdGhlIG9wZXJhdGlvbi4AAAAAAAATSW5zdWZmaWNpZW50QmFsYW5jZQAAAAEsAAAALkluZGljYXRlcyBhbiBlcnJvciB3aGVuIGFuIGlucHV0IG11c3QgYmUgPj0gMC4AAAAAAAxMZXNzVGhhblplcm8AAAEtAAAAPkluZGljYXRlcyB0aGUgYWRkcmVzcyBpcyBmcm96ZW4gYW5kIGNhbm5vdCBwZXJmb3JtIG9wZXJhdGlvbnMuAAAAAAANQWRkcmVzc0Zyb3plbgAAAAAAAS4AAAA9SW5kaWNhdGVzIGluc3VmZmljaWVudCBmcmVlIHRva2VucyAoZHVlIHRvIHBhcnRpYWwgZnJlZXppbmcpLgAAAAAAABZJbnN1ZmZpY2llbnRGcmVlVG9rZW5zAAAAAAEvAAAAKUluZGljYXRlcyBhbiBpZGVudGl0eSBjYW5ub3QgYmUgdmVyaWZpZWQuAAAAAAAAGklkZW50aXR5VmVyaWZpY2F0aW9uRmFpbGVkAAAAAAEwAAAAQUluZGljYXRlcyB0aGUgdHJhbnNmZXIgZG9lcyBub3QgY29tcGx5IHdpdGggdGhlIGNvbXBsaWFuY2UgcnVsZXMuAAAAAAAAFFRyYW5zZmVyTm90Q29tcGxpYW50AAABMQAAAEdJbmRpY2F0ZXMgdGhlIG1pbnQgb3BlcmF0aW9uIGRvZXMgbm90IGNvbXBseSB3aXRoIHRoZSBjb21wbGlhbmNlIHJ1bGVzLgAAAAAQTWludE5vdENvbXBsaWFudAAAATIAAAAtSW5kaWNhdGVzIHRoZSBjb21wbGlhbmNlIGNvbnRyYWN0IGlzIG5vdCBzZXQuAAAAAAAAEENvbXBsaWFuY2VOb3RTZXQAAAEzAAAAJEluZGljYXRlcyB0aGUgb25jaGFpbiBJRCBpcyBub3Qgc2V0LgAAAA9PbmNoYWluSWROb3RTZXQAAAABNAAAACFJbmRpY2F0ZXMgdGhlIHZlcnNpb24gaXMgbm90IHNldC4AAAAAAAANVmVyc2lvbk5vdFNldAAAAAAAATUAAAA7SW5kaWNhdGVzIHRoZSBjbGFpbSB0b3BpY3MgYW5kIGlzc3VlcnMgY29udHJhY3QgaXMgbm90IHNldC4AAAAAG0NsYWltVG9waWNzQW5kSXNzdWVyc05vdFNldAAAAAE2AAAAPEluZGljYXRlcyB0aGUgaWRlbnRpdHkgcmVnaXN0cnkgc3RvcmFnZSBjb250cmFjdCBpcyBub3Qgc2V0LgAAAB1JZGVudGl0eVJlZ2lzdHJ5U3RvcmFnZU5vdFNldAAAAAAAATcAAAA0SW5kaWNhdGVzIHRoZSBpZGVudGl0eSB2ZXJpZmllciBjb250cmFjdCBpcyBub3Qgc2V0LgAAABZJZGVudGl0eVZlcmlmaWVyTm90U2V0AAAAAAE4AAAAREluZGljYXRlcyB0aGUgb2xkIGFjY291bnQgYW5kIG5ldyBhY2NvdW50IGhhdmUgZGlmZmVyZW50IGlkZW50aXRpZXMuAAAAEElkZW50aXR5TWlzbWF0Y2gAAAE5",
        "AAAABQAAACpFdmVudCBlbWl0dGVkIHdoZW4gYSBjbGFpbSB0b3BpYyBpcyBhZGRlZC4AAAAAAAAAAAAPQ2xhaW1Ub3BpY0FkZGVkAAAAAAEAAAARY2xhaW1fdG9waWNfYWRkZWQAAAAAAAABAAAAAAAAAAtjbGFpbV90b3BpYwAAAAAEAAAAAQAAAAI=",
        "AAAABQAAACxFdmVudCBlbWl0dGVkIHdoZW4gYSBjbGFpbSB0b3BpYyBpcyByZW1vdmVkLgAAAAAAAAARQ2xhaW1Ub3BpY1JlbW92ZWQAAAAAAAABAAAAE2NsYWltX3RvcGljX3JlbW92ZWQAAAAAAQAAAAAAAAALY2xhaW1fdG9waWMAAAAABAAAAAEAAAAC",
        "AAAABQAAAC1FdmVudCBlbWl0dGVkIHdoZW4gYSB0cnVzdGVkIGlzc3VlciBpcyBhZGRlZC4AAAAAAAAAAAAAElRydXN0ZWRJc3N1ZXJBZGRlZAAAAAAAAQAAABR0cnVzdGVkX2lzc3Vlcl9hZGRlZAAAAAIAAAAAAAAADnRydXN0ZWRfaXNzdWVyAAAAAAATAAAAAQAAAAAAAAAMY2xhaW1fdG9waWNzAAAD6gAAAAQAAAAAAAAAAg==",
        "AAAABQAAAC1FdmVudCBlbWl0dGVkIHdoZW4gaXNzdWVyIHRvcGljcyBhcmUgdXBkYXRlZC4AAAAAAAAAAAAAE0lzc3VlclRvcGljc1VwZGF0ZWQAAAAAAQAAABVpc3N1ZXJfdG9waWNzX3VwZGF0ZWQAAAAAAAACAAAAAAAAAA50cnVzdGVkX2lzc3VlcgAAAAAAEwAAAAEAAAAAAAAADGNsYWltX3RvcGljcwAAA+oAAAAEAAAAAAAAAAI=",
        "AAAABQAAAC9FdmVudCBlbWl0dGVkIHdoZW4gYSB0cnVzdGVkIGlzc3VlciBpcyByZW1vdmVkLgAAAAAAAAAAFFRydXN0ZWRJc3N1ZXJSZW1vdmVkAAAAAQAAABZ0cnVzdGVkX2lzc3Vlcl9yZW1vdmVkAAAAAAABAAAAAAAAAA50cnVzdGVkX2lzc3VlcgAAAAAAEwAAAAEAAAAC",
        "AAAABAAAAAAAAAAAAAAAGkNsYWltVG9waWNzQW5kSXNzdWVyc0Vycm9yAAAAAAAHAAAAJUluZGljYXRlcyBhIG5vbi1leGlzdGVudCBjbGFpbSB0b3BpYy4AAAAAAAAWQ2xhaW1Ub3BpY0RvZXNOb3RFeGlzdAAAAAABcgAAAChJbmRpY2F0ZXMgYSBub24tZXhpc3RlbnQgdHJ1c3RlZCBpc3N1ZXIuAAAAEklzc3VlckRvZXNOb3RFeGlzdAAAAAABcwAAACdJbmRpY2F0ZXMgYSBjbGFpbSB0b3BpYyBhbHJlYWR5IGV4aXN0cy4AAAAAF0NsYWltVG9waWNBbHJlYWR5RXhpc3RzAAAAAXQAAAAqSW5kaWNhdGVzIGEgdHJ1c3RlZCBpc3N1ZXIgYWxyZWFkeSBleGlzdHMuAAAAAAATSXNzdWVyQWxyZWFkeUV4aXN0cwAAAAF1AAAALEluZGljYXRlcyBtYXggY2xhaW0gdG9waWNzIGxpbWl0IGlzIHJlYWNoZWQuAAAAGk1heENsYWltVG9waWNzTGltaXRSZWFjaGVkAAAAAAF2AAAAL0luZGljYXRlcyBtYXggdHJ1c3RlZCBpc3N1ZXJzIGxpbWl0IGlzIHJlYWNoZWQuAAAAABZNYXhJc3N1ZXJzTGltaXRSZWFjaGVkAAAAAAF3AAAAQ0luZGljYXRlcyBjbGFpbSB0b3BpY3Mgc2V0IHByb3ZpZGVkIGZvciB0aGUgaXNzdWVyIGNhbm5vdCBiZSBlbXB0eS4AAAAAG0NsYWltVG9waWNzU2V0Q2Fubm90QmVFbXB0eQAAAAF4",
        "AAAAAgAAAFBTdG9yYWdlIGtleXMgZm9yIHRoZSBkYXRhIGFzc29jaWF0ZWQgd2l0aCB0aGUgY2xhaW0gdG9waWNzIGFuZCBpc3N1ZXJzCmV4dGVuc2lvbgAAAAAAAAAfQ2xhaW1Ub3BpY3NBbmRJc3N1ZXJzU3RvcmFnZUtleQAAAAAEAAAAAAAAACBTdG9yZXMgdGhlIGNsYWltIHRvcGljcyByZWdpc3RyeQAAAAtDbGFpbVRvcGljcwAAAAAAAAAAI1N0b3JlcyB0aGUgdHJ1c3RlZCBpc3N1ZXJzIHJlZ2lzdHJ5AAAAAA5UcnVzdGVkSXNzdWVycwAAAAAAAQAAAD1TdG9yZXMgdGhlIGNsYWltIHRvcGljcyBhbGxvd2VkIGZvciBhIHNwZWNpZmljIHRydXN0ZWQgaXNzdWVyAAAAAAAAEUlzc3VlckNsYWltVG9waWNzAAAAAAAAAQAAABMAAAABAAAAPVN0b3JlcyB0aGUgdHJ1c3RlZCBpc3N1ZXJzIGFsbG93ZWQgZm9yIGEgc3BlY2lmaWMgY2xhaW0gdG9waWMAAAAAAAARQ2xhaW1Ub3BpY0lzc3VlcnMAAAAAAAABAAAABA==",
        "AAAABAAAADVFcnJvciBjb2RlcyBmb3IgdGhlIElkZW50aXR5IFJlZ2lzdHJ5IFN0b3JhZ2Ugc3lzdGVtLgAAAAAAAAAAAAAISVJTRXJyb3IAAAAIAAAAMUFuIGlkZW50aXR5IGFscmVhZHkgZXhpc3RzIGZvciB0aGUgZ2l2ZW4gYWNjb3VudC4AAAAAAAARSWRlbnRpdHlPdmVyd3JpdGUAAAAAAAFAAAAAKE5vIGlkZW50aXR5IGZvdW5kIGZvciB0aGUgZ2l2ZW4gYWNjb3VudC4AAAAQSWRlbnRpdHlOb3RGb3VuZAAAAUEAAAAuQ291bnRyeSBkYXRhIG5vdCBmb3VuZCBhdCB0aGUgc3BlY2lmaWVkIGluZGV4LgAAAAAAE0NvdW50cnlEYXRhTm90Rm91bmQAAAABQgAAAC9JZGVudGl0eSBjYW4ndCBiZSB3aXRoIGVtcHR5IGNvdW50cnkgZGF0YSBsaXN0LgAAAAAQRW1wdHlDb3VudHJ5TGlzdAAAAUMAAAA3VGhlIG1heGltdW0gbnVtYmVyIG9mIGNvdW50cnkgZW50cmllcyBoYXMgYmVlbiByZWFjaGVkLgAAAAAYTWF4Q291bnRyeUVudHJpZXNSZWFjaGVkAAABRAAAAC5BY2NvdW50IGhhcyBiZWVuIHJlY292ZXJlZCBhbmQgY2Fubm90IGJlIHVzZWQuAAAAAAAQQWNjb3VudFJlY292ZXJlZAAAAUUAAAA9TWV0YWRhdGEgaGFzIHRvbyBtYW55IGVudHJpZXMgKGV4Y2VlZHMgTUFYX01FVEFEQVRBX0VOVFJJRVMpLgAAAAAAABZNZXRhZGF0YVRvb01hbnlFbnRyaWVzAAAAAAFGAAAARE1ldGFkYXRhIHN0cmluZyB2YWx1ZSBpcyB0b28gbG9uZyAoZXhjZWVkcyBNQVhfTUVUQURBVEFfU1RSSU5HX0xFTikuAAAAFU1ldGFkYXRhU3RyaW5nVG9vTG9uZwAAAAAAAUc=",
        "AAAABQAAADhFdmVudCBlbWl0dGVkIHdoZW4gYW4gaWRlbnRpdHkgaXMgc3RvcmVkIGZvciBhbiBhY2NvdW50LgAAAAAAAAAOSWRlbnRpdHlTdG9yZWQAAAAAAAEAAAAPaWRlbnRpdHlfc3RvcmVkAAAAAAIAAAAAAAAAB2FjY291bnQAAAAAEwAAAAEAAAAAAAAACGlkZW50aXR5AAAAEwAAAAEAAAAC",
        "AAAABQAAACpFdmVudCBlbWl0dGVkIGZvciBjb3VudHJ5IGRhdGEgb3BlcmF0aW9ucy4AAAAAAAAAAAAQQ291bnRyeURhdGFBZGRlZAAAAAEAAAASY291bnRyeV9kYXRhX2FkZGVkAAAAAAACAAAAAAAAAAdhY2NvdW50AAAAABMAAAABAAAAAAAAAAxjb3VudHJ5X2RhdGEAAAfQAAAAC0NvdW50cnlEYXRhAAAAAAEAAAAC",
        "AAAABQAAADpFdmVudCBlbWl0dGVkIHdoZW4gYW4gaWRlbnRpdHkgaXMgbW9kaWZpZWQgZm9yIGFuIGFjY291bnQuAAAAAAAAAAAAEElkZW50aXR5TW9kaWZpZWQAAAABAAAAEWlkZW50aXR5X21vZGlmaWVkAAAAAAAAAgAAAAAAAAAMb2xkX2lkZW50aXR5AAAAEwAAAAEAAAAAAAAADG5ld19pZGVudGl0eQAAABMAAAABAAAAAg==",
        "AAAABQAAADpFdmVudCBlbWl0dGVkIHdoZW4gYW4gaWRlbnRpdHkgaXMgcmVtb3ZlZCBmcm9tIGFuIGFjY291bnQuAAAAAAAAAAAAEElkZW50aXR5VW5zdG9yZWQAAAABAAAAEWlkZW50aXR5X3Vuc3RvcmVkAAAAAAAAAgAAAAAAAAAHYWNjb3VudAAAAAATAAAAAQAAAAAAAAAIaWRlbnRpdHkAAAATAAAAAQAAAAI=",
        "AAAABQAAAD5FdmVudCBlbWl0dGVkIHdoZW4gYW4gaWRlbnRpdHkgaXMgcmVjb3ZlcmVkIGZvciBhIG5ldyBhY2NvdW50LgAAAAAAAAAAABFJZGVudGl0eVJlY292ZXJlZAAAAAAAAAEAAAASaWRlbnRpdHlfcmVjb3ZlcmVkAAAAAAACAAAAAAAAAAtvbGRfYWNjb3VudAAAAAATAAAAAQAAAAAAAAALbmV3X2FjY291bnQAAAAAEwAAAAEAAAAC",
        "AAAABQAAAAAAAAAAAAAAEkNvdW50cnlEYXRhUmVtb3ZlZAAAAAAAAQAAABRjb3VudHJ5X2RhdGFfcmVtb3ZlZAAAAAIAAAAAAAAAB2FjY291bnQAAAAAEwAAAAEAAAAAAAAADGNvdW50cnlfZGF0YQAAB9AAAAALQ291bnRyeURhdGEAAAAAAQAAAAI=",
        "AAAABQAAAAAAAAAAAAAAE0NvdW50cnlEYXRhTW9kaWZpZWQAAAAAAQAAABVjb3VudHJ5X2RhdGFfbW9kaWZpZWQAAAAAAAACAAAAAAAAAAdhY2NvdW50AAAAABMAAAABAAAAAAAAAAxjb3VudHJ5X2RhdGEAAAfQAAAAC0NvdW50cnlEYXRhAAAAAAEAAAAC",
        "AAAAAQAAAEhBIGNvdW50cnkgZGF0YSBjb250YWluaW5nIHRoZSBjb3VudHJ5IHJlbGF0aW9uc2hpcCBhbmQgb3B0aW9uYWwgbWV0YWRhdGEAAAAAAAAAC0NvdW50cnlEYXRhAAAAAAIAAAAcVHlwZSBvZiBjb3VudHJ5IHJlbGF0aW9uc2hpcAAAAAdjb3VudHJ5AAAAB9AAAAAPQ291bnRyeVJlbGF0aW9uAAAAADRPcHRpb25hbCBtZXRhZGF0YSAoZS5nLiwgdmlzYSB0eXBlLCB2YWxpZGl0eSBwZXJpb2QpAAAACG1ldGFkYXRhAAAD6AAAA+wAAAARAAAAEA==",
        "AAAAAgAAACZSZXByZXNlbnRzIHRoZSB0eXBlIG9mIGlkZW50aXR5IGhvbGRlcgAAAAAAAAAAAAxJZGVudGl0eVR5cGUAAAACAAAAAAAAAAAAAAAKSW5kaXZpZHVhbAAAAAAAAAAAAAAAAAAMT3JnYW5pemF0aW9u",
        "AAAAAgAAAERTdG9yYWdlIGtleXMgZm9yIHRoZSBkYXRhIGFzc29jaWF0ZWQgd2l0aCBJZGVudGl0eSBTdG9yYWdlIFJlZ2lzdHJ5LgAAAAAAAAANSVJTU3RvcmFnZUtleQAAAAAAAAMAAAABAAAAKE1hcHMgYWNjb3VudCBhZGRyZXNzIHRvIGlkZW50aXR5IGFkZHJlc3MAAAAISWRlbnRpdHkAAAABAAAAEwAAAAEAAAAwTWFwcyBhbiBhY2NvdW50IHRvIGl0cyBjb21wbGV0ZSBpZGVudGl0eSBwcm9maWxlAAAAD0lkZW50aXR5UHJvZmlsZQAAAAABAAAAEwAAAAEAAAAuTWFwcyBvbGQgYWNjb3VudCB0byBuZXcgYWNjb3VudCBhZnRlciByZWNvdmVyeQAAAAAAC1JlY292ZXJlZFRvAAAAAAEAAAAT",
        "AAAAAgAAAExVbmlmaWVkIGNvdW50cnkgcmVsYXRpb25zaGlwIHRoYXQgY2FuIGJlIGVpdGhlciBpbmRpdmlkdWFsIG9yIG9yZ2FuaXphdGlvbmFsAAAAAAAAAA9Db3VudHJ5UmVsYXRpb24AAAAAAgAAAAEAAAAAAAAACkluZGl2aWR1YWwAAAAAAAEAAAfQAAAAGUluZGl2aWR1YWxDb3VudHJ5UmVsYXRpb24AAAAAAAABAAAAAAAAAAxPcmdhbml6YXRpb24AAAABAAAH0AAAABtPcmdhbml6YXRpb25Db3VudHJ5UmVsYXRpb24A",
        "AAAAAQAAAENDb21wbGV0ZSBpZGVudGl0eSBwcm9maWxlIGNvbnRhaW5pbmcgaWRlbnRpdHkgdHlwZSBhbmQgY291bnRyeSBkYXRhAAAAAAAAAAAPSWRlbnRpdHlQcm9maWxlAAAAAAIAAAAAAAAACWNvdW50cmllcwAAAAAAA+oAAAfQAAAAC0NvdW50cnlEYXRhAAAAAAAAAAANaWRlbnRpdHlfdHlwZQAAAAAAB9AAAAAMSWRlbnRpdHlUeXBl",
        "AAAAAgAAAGNSZXByZXNlbnRzIGRpZmZlcmVudCB0eXBlcyBvZiBjb3VudHJ5IHJlbGF0aW9uc2hpcHMgZm9yIGluZGl2aWR1YWxzCklTTyAzMTY2LTEgbnVtZXJpYyBjb3VudHJ5IGNvZGUAAAAAAAAAABlJbmRpdmlkdWFsQ291bnRyeVJlbGF0aW9uAAAAAAAABQAAAAEAAAAUQ291bnRyeSBvZiByZXNpZGVuY2UAAAAJUmVzaWRlbmNlAAAAAAAAAQAAAAQAAAABAAAAFkNvdW50cnkgb2YgY2l0aXplbnNoaXAAAAAAAAtDaXRpemVuc2hpcAAAAAABAAAABAAAAAEAAAAdQ291bnRyeSB3aGVyZSBmdW5kcyBvcmlnaW5hdGUAAAAAAAANU291cmNlT2ZGdW5kcwAAAAAAAAEAAAAEAAAAAQAAAClUYXggcmVzaWRlbmN5IChjYW4gZGlmZmVyIGZyb20gcmVzaWRlbmNlKQAAAAAAAAxUYXhSZXNpZGVuY3kAAAABAAAABAAAAAEAAAApQ3VzdG9tIGNvdW50cnkgdHlwZSBmb3IgZnV0dXJlIGV4dGVuc2lvbnMAAAAAAAAGQ3VzdG9tAAAAAAACAAAAEQAAAAQ=",
        "AAAAAgAAAEVSZXByZXNlbnRzIGRpZmZlcmVudCB0eXBlcyBvZiBjb3VudHJ5IHJlbGF0aW9uc2hpcHMgZm9yIG9yZ2FuaXphdGlvbnMAAAAAAAAAAAAAG09yZ2FuaXphdGlvbkNvdW50cnlSZWxhdGlvbgAAAAAFAAAAAQAAACVDb3VudHJ5IG9mIGluY29ycG9yYXRpb24vcmVnaXN0cmF0aW9uAAAAAAAADUluY29ycG9yYXRpb24AAAAAAAABAAAABAAAAAEAAAAlQ291bnRyaWVzIHdoZXJlIG9yZ2FuaXphdGlvbiBvcGVyYXRlcwAAAAAAABVPcGVyYXRpbmdKdXJpc2RpY3Rpb24AAAAAAAABAAAABAAAAAEAAAAQVGF4IGp1cmlzZGljdGlvbgAAAA9UYXhKdXJpc2RpY3Rpb24AAAAAAQAAAAQAAAABAAAAHUNvdW50cnkgd2hlcmUgZnVuZHMgb3JpZ2luYXRlAAAAAAAADVNvdXJjZU9mRnVuZHMAAAAAAAABAAAABAAAAAEAAAApQ3VzdG9tIGNvdW50cnkgdHlwZSBmb3IgZnV0dXJlIGV4dGVuc2lvbnMAAAAAAAAGQ3VzdG9tAAAAAAACAAAAEQAAAAQ=",
        "AAAABQAAACVFdmVudCBlbWl0dGVkIHdoZW4gdG9rZW5zIGFyZSBmcm96ZW4uAAAAAAAAAAAAAAxUb2tlbnNGcm96ZW4AAAABAAAADXRva2Vuc19mcm96ZW4AAAAAAAACAAAAAAAAAAx1c2VyX2FkZHJlc3MAAAATAAAAAQAAAAAAAAAGYW1vdW50AAAAAAALAAAAAAAAAAI=",
        "AAAABQAAADRFdmVudCBlbWl0dGVkIHdoZW4gYW4gYWRkcmVzcyBpcyBmcm96ZW4gb3IgdW5mcm96ZW4uAAAAAAAAAA1BZGRyZXNzRnJvemVuAAAAAAAAAQAAAA5hZGRyZXNzX2Zyb3plbgAAAAAAAgAAAAAAAAAMdXNlcl9hZGRyZXNzAAAAEwAAAAEAAAAAAAAACWlzX2Zyb3plbgAAAAAAAAEAAAABAAAAAg==",
        "AAAABQAAAC5FdmVudCBlbWl0dGVkIHdoZW4gY29tcGxpYW5jZSBjb250cmFjdCBpcyBzZXQuAAAAAAAAAAAADUNvbXBsaWFuY2VTZXQAAAAAAAABAAAADmNvbXBsaWFuY2Vfc2V0AAAAAAABAAAAAAAAAApjb21wbGlhbmNlAAAAAAATAAAAAQAAAAI=",
        "AAAABQAAACdFdmVudCBlbWl0dGVkIHdoZW4gdG9rZW5zIGFyZSB1bmZyb3plbi4AAAAAAAAAAA5Ub2tlbnNVbmZyb3plbgAAAAAAAQAAAA90b2tlbnNfdW5mcm96ZW4AAAAAAgAAAAAAAAAMdXNlcl9hZGRyZXNzAAAAEwAAAAEAAAAAAAAABmFtb3VudAAAAAAACwAAAAAAAAAC",
        "AAAABQAAACxFdmVudCBlbWl0dGVkIHdoZW4gYSByZWNvdmVyeSBpcyBzdWNjZXNzZnVsLgAAAAAAAAAPUmVjb3ZlcnlTdWNjZXNzAAAAAAEAAAAQcmVjb3Zlcnlfc3VjY2VzcwAAAAIAAAAAAAAAC29sZF9hY2NvdW50AAAAABMAAAABAAAAAAAAAAtuZXdfYWNjb3VudAAAAAATAAAAAQAAAAI=",
        "AAAABQAAADVFdmVudCBlbWl0dGVkIHdoZW4gaWRlbnRpdHkgdmVyaWZpZXIgY29udHJhY3QgaXMgc2V0LgAAAAAAAAAAAAATSWRlbnRpdHlWZXJpZmllclNldAAAAAABAAAAFWlkZW50aXR5X3ZlcmlmaWVyX3NldAAAAAAAAAEAAAAAAAAAEWlkZW50aXR5X3ZlcmlmaWVyAAAAAAAAEwAAAAEAAAAC",
        "AAAABQAAAC9FdmVudCBlbWl0dGVkIHdoZW4gdG9rZW4gb25jaGFpbiBJRCBpcyB1cGRhdGVkLgAAAAAAAAAAFVRva2VuT25jaGFpbklkVXBkYXRlZAAAAAAAAAEAAAAYdG9rZW5fb25jaGFpbl9pZF91cGRhdGVkAAAAAQAAAAAAAAAKb25jaGFpbl9pZAAAAAAAEwAAAAEAAAAC",
        "AAAABQAAADxFdmVudCBlbWl0dGVkIHdoZW4gY2xhaW0gdG9waWNzIGFuZCBpc3N1ZXJzIGNvbnRyYWN0IGlzIHNldC4AAAAAAAAAGENsYWltVG9waWNzQW5kSXNzdWVyc1NldAAAAAEAAAAcY2xhaW1fdG9waWNzX2FuZF9pc3N1ZXJzX3NldAAAAAEAAAAAAAAAGGNsYWltX3RvcGljc19hbmRfaXNzdWVycwAAABMAAAABAAAAAg==",
        "AAAABQAAADRFdmVudCBlbWl0dGVkIHdoZW4gYSB0b2tlbiBpcyBib3VuZCB0byB0aGUgY29udHJhY3QuAAAAAAAAAApUb2tlbkJvdW5kAAAAAAABAAAAC3Rva2VuX2JvdW5kAAAAAAEAAAAAAAAABXRva2VuAAAAAAAAEwAAAAEAAAAC",
        "AAAABQAAADhFdmVudCBlbWl0dGVkIHdoZW4gYSB0b2tlbiBpcyB1bmJvdW5kIGZyb20gdGhlIGNvbnRyYWN0LgAAAAAAAAAMVG9rZW5VbmJvdW5kAAAAAQAAAA10b2tlbl91bmJvdW5kAAAAAAAAAQAAAAAAAAAFdG9rZW4AAAAAAAATAAAAAQAAAAI=",
        "AAAABAAAAChFcnJvciBjb2RlcyBmb3IgdGhlIFRva2VuIEJpbmRlciBzeXN0ZW0uAAAAAAAAABBUb2tlbkJpbmRlckVycm9yAAAABQAAADtUaGUgc3BlY2lmaWVkIHRva2VuIHdhcyBub3QgZm91bmQgaW4gdGhlIGJvdW5kIHRva2VucyBsaXN0LgAAAAANVG9rZW5Ob3RGb3VuZAAAAAAAAUoAAAAwQXR0ZW1wdGVkIHRvIGJpbmQgYSB0b2tlbiB0aGF0IGlzIGFscmVhZHkgYm91bmQuAAAAEVRva2VuQWxyZWFkeUJvdW5kAAAAAAABSwAAADNUb3RhbCB0b2tlbiBjYXBhY2l0eSAoTUFYX1RPS0VOUykgaGFzIGJlZW4gcmVhY2hlZC4AAAAAEE1heFRva2Vuc1JlYWNoZWQAAAFMAAAAGUJhdGNoIGJpbmQgc2l6ZSBleGNlZWRlZC4AAAAAAAARQmluZEJhdGNoVG9vTGFyZ2UAAAAAAAFNAAAAHlRoZSBiYXRjaCBjb250YWlucyBkdXBsaWNhdGVzLgAAAAAAE0JpbmRCYXRjaER1cGxpY2F0ZXMAAAABTg==",
        "AAAAAgAAARxTdG9yYWdlIGtleXMgZm9yIHRoZSB0b2tlbiBiaW5kZXIgc3lzdGVtLgoKLSBUb2tlbnMgYXJlIHN0b3JlZCBpbiBidWNrZXRzIG9mIDEwMCBhZGRyZXNzZXMgZWFjaAotIEVhY2ggYnVja2V0IGlzIGEgYFZlYzxBZGRyZXNzPmAgc3RvcmVkIHVuZGVyIGl0cyBidWNrZXQgaW5kZXgKLSBUb3RhbCBjb3VudCBpcyB0cmFja2VkIHNlcGFyYXRlbHkKLSBXaGVuIGEgdG9rZW4gaXMgdW5ib3VuZCwgdGhlIGxhc3QgdG9rZW4gaXMgbW92ZWQgdG8gZmlsbCB0aGUgZ2FwCihzd2FwLXJlbW92ZSBwYXR0ZXJuKQAAAAAAAAAVVG9rZW5CaW5kZXJTdG9yYWdlS2V5AAAAAAAAAgAAAAEAAABFTWFwcyBidWNrZXQgaW5kZXggdG8gYSB2ZWN0b3Igb2YgdG9rZW4gYWRkcmVzc2VzIChtYXggMTAwIHBlciBidWNrZXQpAAAAAAAAC1Rva2VuQnVja2V0AAAAAAEAAAAEAAAAAAAAABtUb3RhbCBjb3VudCBvZiBib3VuZCB0b2tlbnMAAAAAClRvdGFsQ291bnQAAA==",
        "AAAAAgAAADVTdG9yYWdlIGtleXMgZm9yIHRoZSBkYXRhIGFzc29jaWF0ZWQgd2l0aCBgUldBYCB0b2tlbgAAAAAAAAAAAAANUldBU3RvcmFnZUtleQAAAAAAAAYAAAABAAAAP0Zyb3plbiBzdGF0dXMgb2YgYW4gYWRkcmVzcyAodHJ1ZSA9IGZyb3plbiwgZmFsc2UgPSBub3QgZnJvemVuKQAAAAANQWRkcmVzc0Zyb3plbgAAAAAAAAEAAAATAAAAAQAAAC5BbW91bnQgb2YgdG9rZW5zIGZyb3plbiBmb3IgYSBzcGVjaWZpYyBhZGRyZXNzAAAAAAAMRnJvemVuVG9rZW5zAAAAAQAAABMAAAAAAAAAG0NvbXBsaWFuY2UgY29udHJhY3QgYWRkcmVzcwAAAAAKQ29tcGxpYW5jZQAAAAAAAAAAABpPbmNoYWluSUQgY29udHJhY3QgYWRkcmVzcwAAAAAACU9uY2hhaW5JZAAAAAAAAAAAAAAUVmVyc2lvbiBvZiB0aGUgdG9rZW4AAAAHVmVyc2lvbgAAAAAAAAAAIklkZW50aXR5IFZlcmlmaWVyIGNvbnRyYWN0IGFkZHJlc3MAAAAAABBJZGVudGl0eVZlcmlmaWVy",
        "AAAABQAAAEJFdmVudCBlbWl0dGVkIHdoZW4gdW5kZXJseWluZyBhc3NldHMgYXJlIGRlcG9zaXRlZCBpbnRvIHRoZSB2YXVsdC4AAAAAAAAAAAAHRGVwb3NpdAAAAAABAAAAB2RlcG9zaXQAAAAABQAAAAAAAAAIb3BlcmF0b3IAAAATAAAAAQAAAAAAAAAEZnJvbQAAABMAAAABAAAAAAAAAAhyZWNlaXZlcgAAABMAAAABAAAAAAAAAAZhc3NldHMAAAAAAAsAAAAAAAAAAAAAAAZzaGFyZXMAAAAAAAsAAAAAAAAAAg==",
        "AAAABQAAAENFdmVudCBlbWl0dGVkIHdoZW4gc2hhcmVzIGFyZSBleGNoYW5nZWQgYmFjayBmb3IgdW5kZXJseWluZyBhc3NldHMuAAAAAAAAAAAIV2l0aGRyYXcAAAABAAAACHdpdGhkcmF3AAAABQAAAAAAAAAIb3BlcmF0b3IAAAATAAAAAQAAAAAAAAAIcmVjZWl2ZXIAAAATAAAAAQAAAAAAAAAFb3duZXIAAAAAAAATAAAAAQAAAAAAAAAGYXNzZXRzAAAAAAALAAAAAAAAAAAAAAAGc2hhcmVzAAAAAAALAAAAAAAAAAI=",
        "AAAABAAAAAAAAAAAAAAAD1ZhdWx0VG9rZW5FcnJvcgAAAAALAAAANkluZGljYXRlcyBhY2Nlc3MgdG8gdW5pbml0aWFsaXplZCB2YXVsdCBhc3NldCBhZGRyZXNzLgAAAAAAF1ZhdWx0QXNzZXRBZGRyZXNzTm90U2V0AAAAAZAAAAAySW5kaWNhdGVzIHRoYXQgdmF1bHQgYXNzZXQgYWRkcmVzcyBpcyBhbHJlYWR5IHNldC4AAAAAABtWYXVsdEFzc2V0QWRkcmVzc0FscmVhZHlTZXQAAAABkQAAADxJbmRpY2F0ZXMgdGhhdCB2YXVsdCB2aXJ0dWFsIGRlY2ltYWxzIG9mZnNldCBpcyBhbHJlYWR5IHNldC4AAAAkVmF1bHRWaXJ0dWFsRGVjaW1hbHNPZmZzZXRBbHJlYWR5U2V0AAABkgAAADdJbmRpY2F0ZXMgdGhlIGFtb3VudCBpcyBub3QgYSB2YWxpZCB2YXVsdCBhc3NldHMgdmFsdWUuAAAAABhWYXVsdEludmFsaWRBc3NldHNBbW91bnQAAAGTAAAAN0luZGljYXRlcyB0aGUgYW1vdW50IGlzIG5vdCBhIHZhbGlkIHZhdWx0IHNoYXJlcyB2YWx1ZS4AAAAAGFZhdWx0SW52YWxpZFNoYXJlc0Ftb3VudAAAAZQAAABBQXR0ZW1wdGVkIHRvIGRlcG9zaXQgbW9yZSBhc3NldHMgdGhhbiB0aGUgbWF4IGFtb3VudCBmb3IgYWRkcmVzcy4AAAAAAAAXVmF1bHRFeGNlZWRlZE1heERlcG9zaXQAAAABlQAAAD5BdHRlbXB0ZWQgdG8gbWludCBtb3JlIHNoYXJlcyB0aGFuIHRoZSBtYXggYW1vdW50IGZvciBhZGRyZXNzLgAAAAAAFFZhdWx0RXhjZWVkZWRNYXhNaW50AAABlgAAAEJBdHRlbXB0ZWQgdG8gd2l0aGRyYXcgbW9yZSBhc3NldHMgdGhhbiB0aGUgbWF4IGFtb3VudCBmb3IgYWRkcmVzcy4AAAAAABhWYXVsdEV4Y2VlZGVkTWF4V2l0aGRyYXcAAAGXAAAAQEF0dGVtcHRlZCB0byByZWRlZW0gbW9yZSBzaGFyZXMgdGhhbiB0aGUgbWF4IGFtb3VudCBmb3IgYWRkcmVzcy4AAAAWVmF1bHRFeGNlZWRlZE1heFJlZGVlbQAAAAABmAAAACpNYXhpbXVtIG51bWJlciBvZiBkZWNpbWFscyBvZmZzZXQgZXhjZWVkZWQAAAAAAB5WYXVsdE1heERlY2ltYWxzT2Zmc2V0RXhjZWVkZWQAAAAAAZkAAAAxSW5kaWNhdGVzIG92ZXJmbG93IGR1ZSB0byBtYXRoZW1hdGljYWwgb3BlcmF0aW9ucwAAAAAAAAxNYXRoT3ZlcmZsb3cAAAGa",
        "AAAAAgAAAD1TdG9yYWdlIGtleXMgZm9yIHRoZSBkYXRhIGFzc29jaWF0ZWQgd2l0aCB0aGUgdmF1bHQgZXh0ZW5zaW9uAAAAAAAAAAAAAA9WYXVsdFN0b3JhZ2VLZXkAAAAAAgAAAAAAAAAyU3RvcmVzIHRoZSBhZGRyZXNzIG9mIHRoZSB2YXVsdCdzIHVuZGVybHlpbmcgYXNzZXQAAAAAAAxBc3NldEFkZHJlc3MAAAAAAAAAL1N0b3JlcyB0aGUgdmlydHVhbCBkZWNpbWFscyBvZmZzZXQgb2YgdGhlIHZhdWx0AAAAABVWaXJ0dWFsRGVjaW1hbHNPZmZzZXQAAAA=",
        "AAAABQAAACVFdmVudCBlbWl0dGVkIHdoZW4gdG9rZW5zIGFyZSBidXJuZWQuAAAAAAAAAAAAAARCdXJuAAAAAQAAAARidXJuAAAAAgAAAAAAAAAEZnJvbQAAABMAAAABAAAAAAAAAAZhbW91bnQAAAAAAAsAAAAAAAAAAg==",
        "AAAABQAAADhFdmVudCBlbWl0dGVkIHdoZW4gYSB1c2VyIGlzIGFsbG93ZWQgdG8gdHJhbnNmZXIgdG9rZW5zLgAAAAAAAAALVXNlckFsbG93ZWQAAAAAAQAAAAx1c2VyX2FsbG93ZWQAAAABAAAAAAAAAAR1c2VyAAAAEwAAAAEAAAAC",
        "AAAABQAAAEFFdmVudCBlbWl0dGVkIHdoZW4gYSB1c2VyIGlzIGRpc2FsbG93ZWQgZnJvbSB0cmFuc2ZlcnJpbmcgdG9rZW5zLgAAAAAAAAAAAAAOVXNlckRpc2FsbG93ZWQAAAAAAAEAAAAPdXNlcl9kaXNhbGxvd2VkAAAAAAEAAAAAAAAABHVzZXIAAAATAAAAAQAAAAI=",
        "AAAAAgAAAEFTdG9yYWdlIGtleXMgZm9yIHRoZSBkYXRhIGFzc29jaWF0ZWQgd2l0aCB0aGUgYWxsb3dsaXN0IGV4dGVuc2lvbgAAAAAAAAAAAAATQWxsb3dMaXN0U3RvcmFnZUtleQAAAAABAAAAAQAAACdTdG9yZXMgdGhlIGFsbG93ZWQgc3RhdHVzIG9mIGFuIGFjY291bnQAAAAAB0FsbG93ZWQAAAAAAQAAABM=",
        "AAAABQAAAD5FdmVudCBlbWl0dGVkIHdoZW4gYSB1c2VyIGlzIGJsb2NrZWQgZnJvbSB0cmFuc2ZlcnJpbmcgdG9rZW5zLgAAAAAAAAAAAAtVc2VyQmxvY2tlZAAAAAABAAAADHVzZXJfYmxvY2tlZAAAAAEAAAAAAAAABHVzZXIAAAATAAAAAQAAAAI=",
        "AAAABQAAAEZFdmVudCBlbWl0dGVkIHdoZW4gYSB1c2VyIGlzIHVuYmxvY2tlZCBhbmQgYWxsb3dlZCB0byB0cmFuc2ZlciB0b2tlbnMuAAAAAAAAAAAADVVzZXJVbmJsb2NrZWQAAAAAAAABAAAADnVzZXJfdW5ibG9ja2VkAAAAAAABAAAAAAAAAAR1c2VyAAAAEwAAAAEAAAAC",
        "AAAAAgAAAEFTdG9yYWdlIGtleXMgZm9yIHRoZSBkYXRhIGFzc29jaWF0ZWQgd2l0aCB0aGUgYmxvY2tsaXN0IGV4dGVuc2lvbgAAAAAAAAAAAAATQmxvY2tMaXN0U3RvcmFnZUtleQAAAAABAAAAAQAAACdTdG9yZXMgdGhlIGJsb2NrZWQgc3RhdHVzIG9mIGFuIGFjY291bnQAAAAAB0Jsb2NrZWQAAAAAAQAAABM=",
        "AAAABQAAACVFdmVudCBlbWl0dGVkIHdoZW4gdG9rZW5zIGFyZSBtaW50ZWQuAAAAAAAAAAAAAARNaW50AAAAAQAAAARtaW50AAAAAgAAAAAAAAACdG8AAAAAABMAAAABAAAAAAAAAAZhbW91bnQAAAAAAAsAAAAAAAAAAg==",
        "AAAABQAAACxFdmVudCBlbWl0dGVkIHdoZW4gYW4gYWxsb3dhbmNlIGlzIGFwcHJvdmVkLgAAAAAAAAAHQXBwcm92ZQAAAAABAAAAB2FwcHJvdmUAAAAABAAAAAAAAAAFb3duZXIAAAAAAAATAAAAAQAAAAAAAAAHc3BlbmRlcgAAAAATAAAAAQAAAAAAAAAGYW1vdW50AAAAAAALAAAAAAAAAAAAAAARbGl2ZV91bnRpbF9sZWRnZXIAAAAAAAAEAAAAAAAAAAI=",
        "AAAABQAAADxFdmVudCBlbWl0dGVkIHdoZW4gdG9rZW5zIGFyZSB0cmFuc2ZlcnJlZCBiZXR3ZWVuIGFkZHJlc3Nlcy4AAAAAAAAACFRyYW5zZmVyAAAAAQAAAAh0cmFuc2ZlcgAAAAQAAAAAAAAABGZyb20AAAATAAAAAQAAAAAAAAACdG8AAAAAABMAAAABAAAAAAAAAAt0b19tdXhlZF9pZAAAAAPoAAAABgAAAAAAAAAAAAAABmFtb3VudAAAAAAACwAAAAAAAAAC",
        "AAAABAAAAAAAAAAAAAAAEkZ1bmdpYmxlVG9rZW5FcnJvcgAAAAAADwAAAG5JbmRpY2F0ZXMgYW4gZXJyb3IgcmVsYXRlZCB0byB0aGUgY3VycmVudCBiYWxhbmNlIG9mIGFjY291bnQgZnJvbSB3aGljaAp0b2tlbnMgYXJlIGV4cGVjdGVkIHRvIGJlIHRyYW5zZmVycmVkLgAAAAAAE0luc3VmZmljaWVudEJhbGFuY2UAAAAAZAAAAGRJbmRpY2F0ZXMgYSBmYWlsdXJlIHdpdGggdGhlIGFsbG93YW5jZSBtZWNoYW5pc20gd2hlbiBhIGdpdmVuIHNwZW5kZXIKZG9lc24ndCBoYXZlIGVub3VnaCBhbGxvd2FuY2UuAAAAFUluc3VmZmljaWVudEFsbG93YW5jZQAAAAAAAGUAAABNSW5kaWNhdGVzIGFuIGludmFsaWQgdmFsdWUgZm9yIGBsaXZlX3VudGlsX2xlZGdlcmAgd2hlbiBzZXR0aW5nIGFuCmFsbG93YW5jZS4AAAAAAAAWSW52YWxpZExpdmVVbnRpbExlZGdlcgAAAAAAZgAAADJJbmRpY2F0ZXMgYW4gZXJyb3Igd2hlbiBhbiBpbnB1dCB0aGF0IG11c3QgYmUgPj0gMAAAAAAADExlc3NUaGFuWmVybwAAAGcAAAApSW5kaWNhdGVzIG92ZXJmbG93IHdoZW4gYWRkaW5nIHR3byB2YWx1ZXMAAAAAAAAMTWF0aE92ZXJmbG93AAAAaAAAACpJbmRpY2F0ZXMgYWNjZXNzIHRvIHVuaW5pdGlhbGl6ZWQgbWV0YWRhdGEAAAAAAA1VbnNldE1ldGFkYXRhAAAAAAAAaQAAAFJJbmRpY2F0ZXMgdGhhdCB0aGUgb3BlcmF0aW9uIHdvdWxkIGhhdmUgY2F1c2VkIGB0b3RhbF9zdXBwbHlgIHRvIGV4Y2VlZAp0aGUgYGNhcGAuAAAAAAALRXhjZWVkZWRDYXAAAAAAagAAADZJbmRpY2F0ZXMgdGhlIHN1cHBsaWVkIGBjYXBgIGlzIG5vdCBhIHZhbGlkIGNhcCB2YWx1ZS4AAAAAAApJbnZhbGlkQ2FwAAAAAABrAAAAHkluZGljYXRlcyB0aGUgQ2FwIHdhcyBub3Qgc2V0LgAAAAAACUNhcE5vdFNldAAAAAAAAGwAAAAmSW5kaWNhdGVzIHRoZSBTQUMgYWRkcmVzcyB3YXMgbm90IHNldC4AAAAAAAlTQUNOb3RTZXQAAAAAAABtAAAAMEluZGljYXRlcyBhIFNBQyBhZGRyZXNzIGRpZmZlcmVudCB0aGFuIGV4cGVjdGVkLgAAABJTQUNBZGRyZXNzTWlzbWF0Y2gAAAAAAG4AAABDSW5kaWNhdGVzIGEgbWlzc2luZyBmdW5jdGlvbiBwYXJhbWV0ZXIgaW4gdGhlIFNBQyBjb250cmFjdCBjb250ZXh0LgAAAAARU0FDTWlzc2luZ0ZuUGFyYW0AAAAAAABvAAAAREluZGljYXRlcyBhbiBpbnZhbGlkIGZ1bmN0aW9uIHBhcmFtZXRlciBpbiB0aGUgU0FDIGNvbnRyYWN0IGNvbnRleHQuAAAAEVNBQ0ludmFsaWRGblBhcmFtAAAAAAAAcAAAADFUaGUgdXNlciBpcyBub3QgYWxsb3dlZCB0byBwZXJmb3JtIHRoaXMgb3BlcmF0aW9uAAAAAAAADlVzZXJOb3RBbGxvd2VkAAAAAABxAAAANVRoZSB1c2VyIGlzIGJsb2NrZWQgYW5kIGNhbm5vdCBwZXJmb3JtIHRoaXMgb3BlcmF0aW9uAAAAAAAAC1VzZXJCbG9ja2VkAAAAAHI=",
        "AAAAAgAAAClTdG9yYWdlIGtleSBmb3IgYWNjZXNzaW5nIHRoZSBTQUMgYWRkcmVzcwAAAAAAAAAAAAAWU0FDQWRtaW5HZW5lcmljRGF0YUtleQAAAAAAAQAAAAAAAAAAAAAAA1NhYwA=",
        "AAAAAgAAAClTdG9yYWdlIGtleSBmb3IgYWNjZXNzaW5nIHRoZSBTQUMgYWRkcmVzcwAAAAAAAAAAAAAWU0FDQWRtaW5XcmFwcGVyRGF0YUtleQAAAAAAAQAAAAAAAAAAAAAAA1NhYwA=",
        "AAAAAQAAACRTdG9yYWdlIGNvbnRhaW5lciBmb3IgdG9rZW4gbWV0YWRhdGEAAAAAAAAACE1ldGFkYXRhAAAAAwAAAAAAAAAIZGVjaW1hbHMAAAAEAAAAAAAAAARuYW1lAAAAEAAAAAAAAAAGc3ltYm9sAAAAAAAQ",
        "AAAAAgAAADlTdG9yYWdlIGtleXMgZm9yIHRoZSBkYXRhIGFzc29jaWF0ZWQgd2l0aCBgRnVuZ2libGVUb2tlbmAAAAAAAAAAAAAAClN0b3JhZ2VLZXkAAAAAAAMAAAAAAAAAAAAAAAtUb3RhbFN1cHBseQAAAAABAAAAAAAAAAdCYWxhbmNlAAAAAAEAAAATAAAAAQAAAAAAAAAJQWxsb3dhbmNlAAAAAAAAAQAAB9AAAAAMQWxsb3dhbmNlS2V5",
        "AAAAAQAAACpTdG9yYWdlIGtleSB0aGF0IG1hcHMgdG8gW2BBbGxvd2FuY2VEYXRhYF0AAAAAAAAAAAAMQWxsb3dhbmNlS2V5AAAAAgAAAAAAAAAFb3duZXIAAAAAAAATAAAAAAAAAAdzcGVuZGVyAAAAABM=",
        "AAAAAQAAAINTdG9yYWdlIGNvbnRhaW5lciBmb3IgdGhlIGFtb3VudCBvZiB0b2tlbnMgZm9yIHdoaWNoIGFuIGFsbG93YW5jZSBpcyBncmFudGVkCmFuZCB0aGUgbGVkZ2VyIG51bWJlciBhdCB3aGljaCB0aGlzIGFsbG93YW5jZSBleHBpcmVzLgAAAAAAAAAADUFsbG93YW5jZURhdGEAAAAAAAACAAAAAAAAAAZhbW91bnQAAAAAAAsAAAAAAAAAEWxpdmVfdW50aWxfbGVkZ2VyAAAAAAAABA==",
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
    name: this.txFromJSON<string>,
        pause: this.txFromJSON<null>,
        symbol: this.txFromJSON<string>,
        approve: this.txFromJSON<null>,
        balance: this.txFromJSON<i128>,
        migrate: this.txFromJSON<null>,
        unpause: this.txFromJSON<null>,
        upgrade: this.txFromJSON<null>,
        decimals: this.txFromJSON<u32>,
        transfer: this.txFromJSON<null>,
        allowance: this.txFromJSON<i128>,
        query_asset: this.txFromJSON<string>,
        total_supply: this.txFromJSON<i128>,
        get_lp_config: this.txFromJSON<LpConfig>,
        physical_cash: this.txFromJSON<i128>,
        set_lp_config: this.txFromJSON<null>,
        transfer_from: this.txFromJSON<null>,
        cancel_upgrade: this.txFromJSON<null>,
        settle_deposit: this.txFromJSON<SettlementResult>,
        transfer_claim: this.txFromJSON<null>,
        propose_upgrade: this.txFromJSON<null>,
        bump_vault_state: this.txFromJSON<null>,
        settle_withdrawal: this.txFromJSON<SettlementResult>,
        receive_collateral: this.txFromJSON<null>,
        set_request_router: this.txFromJSON<null>,
        total_share_supply: this.txFromJSON<i128>,
        accounting_snapshot: this.txFromJSON<AccountingSnapshot>,
        can_create_lp_request: this.txFromJSON<boolean>,
        transfer_safety_claim: this.txFromJSON<null>
  }
}