// Browser-safe entry. No Keypair imports. Use `@win-trader/protocol-clients/node`
// for keypair-based signers.

import { rpc } from "@stellar/stellar-sdk";
import type { SignTransaction, SignAuthEntry } from "@stellar/stellar-sdk/contract";

export interface NetworkEnv {
  rpcUrl: string;
  networkPassphrase: string;
}

/**
 * Everything a binding Client needs to attribute and sign a transaction.
 * `publicKey` alone is enough for read-only simulation; full signing requires
 * `signTransaction` / `signAuthEntry` (browser wallets supply these too — see
 * Freighter's signature shape). Signature shapes track the SDK's so wallet
 * adapters and basicNodeSigner are both assignable to Signer.
 */
export interface Signer {
  publicKey: string;
  signTransaction?: SignTransaction;
  signAuthEntry?: SignAuthEntry;
}

type ClientCtor<C> = new (opts: {
  contractId: string;
  networkPassphrase: string;
  rpcUrl: string;
  allowHttp?: boolean;
  publicKey?: string;
  signTransaction?: SignTransaction;
  signAuthEntry?: SignAuthEntry;
}) => C;

/**
 * Construct a binding Client with the cross-cutting options dict baked in.
 * `signer` is optional — omit for spec-only access; pass a publicKey-only
 * Signer for read-only simulation; pass a full Signer for sign-and-send.
 */
export function client<C>(
  ClientClass: ClientCtor<C>,
  env: NetworkEnv,
  contractId: string,
  signer?: Signer,
): C {
  return new ClientClass({
    contractId,
    networkPassphrase: env.networkPassphrase,
    rpcUrl: env.rpcUrl,
    allowHttp: env.rpcUrl.startsWith("http://"),
    publicKey: signer?.publicKey,
    signTransaction: signer?.signTransaction,
    signAuthEntry: signer?.signAuthEntry,
  });
}

/** Construct an RPC server with allowHttp derived from the URL scheme. */
export function rpcServer(env: NetworkEnv): rpc.Server {
  return new rpc.Server(env.rpcUrl, { allowHttp: env.rpcUrl.startsWith("http://") });
}

/** Read-only signer — wraps a bare publicKey for simulation source attribution. */
export function readOnlySigner(publicKey: string): Signer {
  return { publicKey };
}
