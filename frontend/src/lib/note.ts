/*
 * Builds the withdrawal intent the judge submits.
 *
 * HONEST framing (plan §"Frontera honesta"): the operator custodies a demo note
 * for the judge. The UI provisions a fresh client-side demo note (random secret
 * + blinding ⇒ a fresh nullifier, so the sequencer's lock-by-nullifier never
 * rejects a repeat submit) and copies the judge's wallet address into the
 * `recipient` field. With the FixtureProver backend the sequencer settles the
 * FIXED N=8 demo batch and ignores the arbitrary recipient; with the RemoteProver
 * it honours it (that path is validated on GPU, s3/03/s3/05). Either way the
 * frontend sends the real recipient — it fabricates no proof artifacts here.
 *
 * The Merkle root + path shape come from the committed N=8 pool input set
 * (`golden/n8_inputs.json`) so the body is well-formed (non-empty path, 32B LE
 * values) and passes the sequencer's validation.
 */

import { StrKey } from "@stellar/stellar-base";
import type { SubmitBody } from "./api";

/** Public N=8 pool root (32B LE) from golden/n8_inputs.json. */
const POOL_ROOT_LE =
  "0x054be46b37e2cad7ab310dae784815e6fbbb0ad2785cafcd516dfa31c3daf314";

/** A valid depth-3 authentication path (note0 siblings) from golden/n8_inputs.json. */
const DEMO_PATH_LE = [
  "0x5b3830efa8bebc1e1c59f879a9f344c7997b6786e123aecadc5cf9dd36e44e30",
  "0x61e406bf06346cce2cfdb2a3f5afdaf76cb29e9ce9fc4a4cea6fe8c9142a042a",
  "0x8b5744adcf48f0b18bac59001148d22815d0ad0c167d87f8ec2d137b6c981703",
];

/** The fixed in-claro demo amount (stroops), mirrored from the demo batch. */
export const DEMO_AMOUNT = 1000;

function randomHex32(): string {
  const bytes = new Uint8Array(32);
  crypto.getRandomValues(bytes);
  let out = "0x";
  for (const b of bytes) out += b.toString(16).padStart(2, "0");
  return out;
}

function toHex32(bytes: Uint8Array): string {
  let out = "0x";
  for (const b of bytes) out += b.toString(16).padStart(2, "0");
  return out;
}

/** True if `address` is a valid Stellar G… ed25519 public key. */
export function isValidAddress(address: string): boolean {
  try {
    return StrKey.isValidEd25519PublicKey(address);
  } catch {
    return false;
  }
}

/**
 * Build the submit body for `recipientAddress` (a Stellar G… address). Throws a
 * clear error if the address is malformed. Each call mints a fresh demo note.
 */
export function buildDemoIntent(recipientAddress: string): SubmitBody {
  if (!isValidAddress(recipientAddress)) {
    throw new Error(
      "That doesn't look like a Stellar address. Connect a wallet with a G… testnet account.",
    );
  }
  const recipientRaw = StrKey.decodeEd25519PublicKey(recipientAddress);
  return {
    secret: randomHex32(),
    blinding: randomHex32(),
    amount: DEMO_AMOUNT,
    recipient: toHex32(recipientRaw),
    path: DEMO_PATH_LE,
    index: 0,
    merkle_root: POOL_ROOT_LE,
  };
}
