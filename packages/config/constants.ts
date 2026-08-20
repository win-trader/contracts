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

// Mirror of shared::constants::BPS.
export const BPS_DENOMINATOR = 10_000;
// Off-chain annualization constant. On-chain fee rates use SECONDS_PER_DAY.
export const SECONDS_PER_YEAR = 31_536_000;
// Mirror of shared::constants::SECONDS_PER_LEDGER.
export const SECONDS_PER_LEDGER = 5;
// Mirror of shared::constants::PRECISION (1e7 on-chain price scale).
export const PRECISION = 10_000_000n;

// ---------------------------------------------------------------------------
// Oracle publishers
// ---------------------------------------------------------------------------

/** Polling interval for CEX REST APIs. */
export const ORACLE_POLL_INTERVAL_MS = 1_000;
/** Reject CEX prints whose embedded source-timestamp is older than this.
 *  NOTE: KuCoin's level1 `time` is when the top of book last *changed*, not
 *  when the data was served. A thin pair (XLM-USDT) legitimately holds the
 *  same best bid/ask for tens of seconds, so a tight bound here rejects a
 *  perfectly good price and starves that market's feed entirely. Sized for
 *  the thinnest pair we quote, not the most liquid one. */
export const ORACLE_KUCOIN_STALENESS_MS = 60_000;
/** Per-tick sanity cap: reject prints whose delta vs the last *observed*
 *  price exceeds this. Bounded so a single rogue print cannot poison the
 *  on-chain median. Deliberately measured against the last observation
 *  rather than the last successful push — basing it on the push makes the
 *  gate an absorbing state, since a failed submit then freezes the baseline
 *  forever (see ORACLE_CONFIRMING_SAMPLES). */
export const ORACLE_MAX_DELTA_BPS_PER_TICK = 200;
/** Minimum interval between two consecutive on-chain pushes for one symbol. */
export const ORACLE_MIN_INTERVAL_BETWEEN_PUSHES_MS = 500;
/** Hard upper bound on on-chain pushes per minute per symbol. */
export const ORACLE_MAX_PUSHES_PER_MINUTE = 60;
/** HTTP fetch timeout for upstream CEX calls. */
export const ORACLE_FETCH_TIMEOUT_MS = 2_000;
/** Consecutive agreeing samples required before an out-of-band move is
 *  accepted as genuine and the outlier baseline re-bases onto it. One rogue
 *  print is discarded when the next tick disagrees with it; a real move is
 *  adopted after this many samples. This is what keeps the delta gate from
 *  latching permanently. */
export const ORACLE_CONFIRMING_SAMPLES = 2;
/** Publish unconditionally when the last on-chain push is older than this,
 *  bypassing the delta gate. A stale price halts the market outright at the
 *  router's staleness/quorum gates, whereas a moved price is still a price —
 *  so the publisher fails toward freshness. Must stay well under the
 *  router's `staleness_threshold`. */
export const ORACLE_FORCE_PUBLISH_AFTER_MS = 20_000;
/** Bounded retries for a failed on-chain submit (TRY_AGAIN_LATER and other
 *  transient RPC/congestion failures). `set_price` is a blind overwrite, so
 *  a duplicate landing is harmless and retrying is safe. */
export const ORACLE_SUBMIT_RETRIES = 2;
/** Base backoff between submit retries; doubles per attempt. */
export const ORACLE_SUBMIT_RETRY_BACKOFF_MS = 250;
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
