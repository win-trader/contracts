/**
 * Single source of truth for off-chain numeric constants.
 *
 * Every TypeScript service (api / indexer / keeper / oracle publishers)
 * imports from here. No service hardcodes its own copy of these values.
 *
 * Mirror of the on-chain `shared::constants` module for values that have an
 * on-chain counterpart (BPS_DENOMINATOR, SECONDS_PER_YEAR).
 */

// ---------------------------------------------------------------------------
// Math / time (mirror of on-chain shared::constants)
// ---------------------------------------------------------------------------

// Mirror of shared::constants::BPS — see docs/adr/0004
export const BPS_DENOMINATOR = 10_000;
// Mirror of shared::constants::SECONDS_PER_YEAR — see docs/adr/0004
export const SECONDS_PER_YEAR = 31_536_000;
// Mirror of shared::constants::SECONDS_PER_LEDGER — see docs/adr/0004
export const SECONDS_PER_LEDGER = 5;
// Mirror of shared::constants::PRECISION (1e7 on-chain price scale) — see docs/adr/0004
export const PRECISION = 10_000_000n;

// ---------------------------------------------------------------------------
// Oracle publishers
// ---------------------------------------------------------------------------

/** Polling interval for CEX REST APIs. */
export const ORACLE_POLL_INTERVAL_MS = 1_000;
/** Reject CEX prints whose embedded source-timestamp is older than this. */
export const ORACLE_KUCOIN_STALENESS_MS = 5_000;
/** Per-tick sanity cap: reject prints whose delta vs the last published price
 *  exceeds this fraction of the prior price. Bounded so a single rogue print
 *  cannot poison the on-chain median. */
export const ORACLE_MAX_DELTA_BPS_PER_TICK = 200;
/** Minimum interval between two consecutive on-chain pushes for one symbol. */
export const ORACLE_MIN_INTERVAL_BETWEEN_PUSHES_MS = 500;
/** Hard upper bound on on-chain pushes per minute per symbol. */
export const ORACLE_MAX_PUSHES_PER_MINUTE = 60;
/** HTTP fetch timeout for upstream CEX calls. */
export const ORACLE_FETCH_TIMEOUT_MS = 2_000;
/** Number of confirming samples a publisher must observe before pushing
 *  (defends against a single-tick anomaly). */
export const ORACLE_CONFIRMING_SAMPLES = 2;
/** User-Agent string sent to upstream CEX APIs. */
export const ORACLE_USER_AGENT = "stellars-oracle/0.1";

// ---------------------------------------------------------------------------
// Keeper
// ---------------------------------------------------------------------------

/** Max fee in stroops the keeper will pay per Soroban submission. */
export const KEEPER_MAX_FEE_STROOPS = 100_000_000; // 10 XLM ceiling
/** Per-submission timeout. */
export const KEEPER_TX_TIMEOUT_SECONDS = 30;
/** Daily fee budget for the keeper (sum of accepted submission fees). */
export const KEEPER_DAILY_FEE_BUDGET_STROOPS = 10_000_000_000; // 1_000 XLM
/** Dedup TTL for liquidation / ADL submissions. */
export const KEEPER_LIQUIDATION_DEDUP_TTL_MS = 60_000;
/** Dedup TTL for index updates. */
export const KEEPER_INDEX_UPDATE_DEDUP_TTL_MS = 5_000;
/** Inclusion-poll cadence — how often we re-check getTransaction(hash). */
export const KEEPER_INCLUSION_POLL_INTERVAL_MS = 2_000;
/** Maximum total wait time for ledger inclusion before treating a tx as failed. */
export const KEEPER_INCLUSION_MAX_WAIT_MS = 30_000;

// ---------------------------------------------------------------------------
// API + SSE
// ---------------------------------------------------------------------------

/** Heartbeat ping interval for every open SSE connection. */
export const SSE_HEARTBEAT_INTERVAL_MS = 15_000;
/** Max events held in the per-subscriber broadcast queue before drop-oldest. */
export const SSE_BUFFER_MAX_LEN = 1_000;

// ---------------------------------------------------------------------------
// Indexer
// ---------------------------------------------------------------------------

/** Default RPC polling interval — events catch-up cadence. */
export const INDEXER_POLL_INTERVAL_MS = 500;
/** Backoff start when RPC returns 429/5xx. */
export const INDEXER_RPC_RETRY_BASE_MS = 1_000;
/** Maximum backoff (capped). */
export const INDEXER_RPC_RETRY_MAX_MS = 30_000;
