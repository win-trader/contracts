// Node-only entry. Imports Keypair, which pulls in crypto and is unsafe to
// bundle for the browser. Browser callers use the wallet to provide a Signer.

import { Keypair } from "@stellar/stellar-sdk";
import { basicNodeSigner } from "@stellar/stellar-sdk/contract";
import type { Signer } from "./index.js";

/** Build a Signer from a secret key, ready to be passed to `client(...)`. */
export function keypairSigner(secret: string, networkPassphrase: string): Signer {
  const kp = Keypair.fromSecret(secret);
  const { signTransaction, signAuthEntry } = basicNodeSigner(kp, networkPassphrase);
  return {
    publicKey: kp.publicKey(),
    signTransaction,
    signAuthEntry,
  };
}
