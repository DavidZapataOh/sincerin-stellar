//! `zk-core` — shared no_std primitives for the Confidential Payments Rollup.
//!
//! This crate is compiled in three contexts:
//!   1. RISC Zero guest (no_std, target: riscv32im-risc0-zkvm-elf)
//!   2. Soroban contract (no_std, target: wasm32v1-none)
//!   3. Host / test binaries (std enabled via the "std" feature)
//!
//! **AC1.2 / AC1.3 (BLOQUEANTE):** The Poseidon2 parameters and Merkle tree
//! logic MUST produce byte-identical outputs to the PoC
//! (`stellar-private-payments`). The cross-check test (`tests/crosscheck_poc.rs`,
//! s1/02) guards this: a single differing byte fails the build.
//!
//! ## Crypto reproduced (all confirmed against PoC source — see `docs/params.md`)
//! - Poseidon2-BN254 (HorizenLabs `zkhash`), instances t = 2, 3, 4, all
//!   d = 5, RF = 8, RP = 56. Field = `ark_bn254::Fr` (identical modulus to the
//!   PoC's `FpBN256`).
//! - `pubkey     = Poseidon2_t3([sk, 0, 3])[0]`
//! - `commitment = Poseidon2_t4([amount, pubkey, blinding, 1])[0]`
//! - `signature  = Poseidon2_t4([priv_key, commitment, path_indices, 4])[0]`
//! - `nullifier  = Poseidon2_t4([commitment, path_indices, signature, 2])[0]`
//! - Merkle node = `Poseidon2_t2_perm([left, right])[0] + left` (feed-forward).
//! - Serialization = 32-byte LITTLE-ENDIAN (`Fr::into_bigint().to_bytes_le()`).
#![no_std]
#![deny(unsafe_code)]
#![deny(missing_docs)]

pub mod merkle;
pub mod note;
pub mod poseidon2;

mod poseidon2_constants;

/// Journal codec: `{ merkle_root, [nullifier_i], [(recipient_i, amount_i)] }`.
///
/// **AC3.1:** the journal contains exactly these fields.
pub mod journal {
    // TODO (s1/03): implement after note/nullifier types are stable.
}
