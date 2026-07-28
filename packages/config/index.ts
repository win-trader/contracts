import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export type Network = "local" | "testnet" | "mainnet";

export interface ContractInfo {
  /** Deployed contract address (C...). Empty string if not yet deployed. */
  address: string;
  /** Ledger sequence at which this contract was deployed. 0 if not yet deployed. */
  startLedger: number;
}

export interface NetworkContracts {
  vault: ContractInfo;
  requestRouter: ContractInfo;
  positionManager: ContractInfo;
  configManager: ContractInfo;
  oracleRouter: ContractInfo;
  oracle: ContractInfo;
  /** Secondary admin-published oracle used before live CEX sources are wired. */
  oracle2?: ContractInfo;
  /** Per-source oracle instances populated by scripts/deploy-cex-oracles.sh. */
  binanceOracle: ContractInfo;
  kucoinOracle: ContractInfo;
  /** Test-only mock USDC token. Empty address on mainnet. */
  mockToken: ContractInfo;
}

export interface NetworkConfig {
  rpcUrl: string;
  networkPassphrase: string;
  /** Markets supported on this network. Single source of truth for oracle
   *  publishers, indexer poller, frontend symbol picker, and deploy scripts. */
  tickers: readonly string[];
  contracts: NetworkContracts;
}

export type Addresses = Record<Network, NetworkConfig>;

// __dirname resolves to:
//   - packages/config/dist  when consumed as a built package (the normal case)
//   - packages/config       when imported directly from source (e.g. via bun)
const here = dirname(fileURLToPath(import.meta.url));

// The current network. Deployed services set NETWORK; defaults to local for dev.
const NETWORK: Network = (process.env.NETWORK as Network) || "local";

function isSingleNetwork(x: unknown): x is NetworkConfig {
  return (
    typeof x === "object" && x !== null && "contracts" in x && "rpcUrl" in x
  );
}

// A per-network file injected via ADDRESSES_JSON is the chosen network — kept
// here so getNetworkConfig returns it regardless of the requested key.
let injected: NetworkConfig | null = null;

// Resolution order for the address/network registry:
//   1. ADDRESSES_JSON env path — deployment-provided. Either a single-network
//      file (deployments/<network>.json) or the combined record.
//   2. the in-package addresses.json — local-dev / fallback (combined record),
//      a generated artifact so `make deploy` updates are picked up at runtime.
function loadAddresses(): Addresses {
  const candidates = [
    process.env.ADDRESSES_JSON,
    resolve(here, "..", "addresses.json"),
    resolve(here, "addresses.json"),
  ].filter((p): p is string => Boolean(p));
  for (const path of candidates) {
    let data: unknown;
    try {
      data = JSON.parse(readFileSync(path, "utf8"));
    } catch {
      continue;
    }
    if (isSingleNetwork(data)) {
      injected = data;
      return { [NETWORK]: data } as Addresses;
    }
    return data as Addresses;
  }
  throw new Error(
    `@win-trader/config: address registry not found. Looked in:\n  ${candidates.join("\n  ")}`,
  );
}

export const config: Addresses = loadAddresses();

export function getNetworkConfig(network: Network): NetworkConfig {
  // When ADDRESSES_JSON injected a single-network file, that file IS the
  // selected network — return it regardless of the requested key.
  return injected ?? config[network];
}

/** The network env a binding client needs (`@win-trader/protocol-clients`). */
export interface NetworkEnv {
  rpcUrl: string;
  networkPassphrase: string;
}

export interface ResolvedNetwork {
  network: Network;
  config: NetworkConfig;
  env: NetworkEnv;
}

export interface ResolveNetworkOptions {
  /** Override the network instead of reading `process.env.NETWORK`. */
  network?: Network;
  /** Contract keys whose address must be non-empty; throws if any is missing. */
  require?: (keyof NetworkContracts)[];
}

/**
 * One-stop network bootstrap for backend services: resolve the network (from
 * `process.env.NETWORK`, defaulting to `local`), load its config, validate that
 * the required contract addresses are populated, and hand back a ready
 * `NetworkEnv`. Replaces the resolve→validate→build dance each service used to
 * hand-roll with its own (inconsistent) `NETWORK` default.
 */
export function resolveNetwork(opts: ResolveNetworkOptions = {}): ResolvedNetwork {
  const network = opts.network ?? ((process.env.NETWORK as Network) || "local");
  const networkConfig = getNetworkConfig(network);

  for (const key of opts.require ?? []) {
    if (!networkConfig.contracts[key]?.address) {
      throw new Error(
        `@win-trader/config: contract "${String(key)}" has no address for network "${network}". ` +
          `Run a deploy to populate addresses.json (or ADDRESSES_JSON).`,
      );
    }
  }

  return {
    network,
    config: networkConfig,
    env: { rpcUrl: networkConfig.rpcUrl, networkPassphrase: networkConfig.networkPassphrase },
  };
}

export * from "./constants.js";
