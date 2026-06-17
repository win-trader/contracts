// Mirrors contracts/position-manager/src/math.rs and contracts/oracle-router PRECISION.
// Any change here must move in lockstep with the on-chain constants.
export const PRECISION = 10_000_000n; // 1e7 — USDC and price scaling
export const INDEX_PRECISION = 100_000_000_000_000n; // 1e14 — borrow/funding index scaling
export const BPS = 10_000n;
export const SECONDS_PER_YEAR = 31_536_000n; // 365 days
export const MAX_LEVERAGE_CAP = 200n;
