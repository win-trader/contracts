// Mirrors contracts/shared/src/constants.rs. Any change here must move in
// lockstep with the on-chain constants.
export const PRECISION = 10_000_000n; // 1e7 — USDC and price scaling
export const INDEX_PRECISION = 100_000_000_000_000n; // 1e14 — rate and index scaling
export const BPS = 10_000n;
export const SECONDS_PER_DAY = 86_400n;
