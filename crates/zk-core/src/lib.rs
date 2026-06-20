//! `zk-core` — shared no_std primitives for the Confidential Payments Rollup.
//!
//! This crate is compiled in three contexts:
//!   1. RISC Zero guest (no_std, target: riscv32im-risc0-zkvm-elf)
//!   2. Soroban contract (no_std, target: wasm32v1-none)
//!   3. Host / test binaries (std enabled via the "std" feature)
//!
//! **AC1.2 / AC1.3 (BLOQUEANTE):** The Poseidon2 parameters and Merkle tree
//! logic MUST produce byte-identical outputs to the PoC
//! (`stellar-private-payments`). The cross-check test (s1/02) guards this.
//! Do NOT add hashing primitives here until the cross-check passes.
//!
//! Sprint s1/01 scope: skeleton only.  Types and implementations are stubs
//! that will be filled in by s1/02 (cross-check) and s1/03 (guest logic).
#![no_std]
#![deny(unsafe_code)]
#![deny(missing_docs)]

// ── Module stubs (filled in by subsequent tasks) ──────────────────────────

/// Poseidon2 over the BN254 scalar field.
///
/// Parameters must match `circuits/poseidon2/*` in `stellar-private-payments`
/// exactly.  Confirmed by the cross-check test in s1/02.
pub mod poseidon2 {
    // TODO (s1/02): port Poseidon2-BN254 from PoC; cross-check FIRST.
}

/// Incremental Merkle tree (BN254 field, Poseidon2 hashing).
pub mod merkle {
    // TODO (s1/02): port MerkleTree from PoC after cross-check passes.
}

/// Note commitment and nullifier derivation.
///
/// **AC1.2:** commitment(note) and nullifier(note, key) must be byte-identical
/// to the PoC for the same input.
pub mod note {
    // TODO (s1/02): derive after Poseidon2 is confirmed byte-identical.
}

/// Journal codec: `{ merkle_root, [nullifier_i], [(recipient_i, amount_i)] }`.
///
/// **AC3.1:** the journal contains exactly these fields.
pub mod journal {
    // TODO (s1/03): implement after note/nullifier types are stable.
}
