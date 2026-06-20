//! RISC Zero guest: Confidential Payments Rollup validity check.
//!
//! Sprint s1/01: hello-world skeleton — commits "hello from guest" to the
//! journal.  The actual rollup logic (AC1.1, AC1.2, AC3.1–AC3.2) is
//! implemented in s1/03 after the cross-check (s1/02) passes.
//!
//! Design (CONTEXT.md D1):
//!   - Re-executes membership + nullifier derivation + balance conservation
//!     NATIVELY in Rust inside the zkVM.
//!   - Does NOT verify any SNARK in-zkVM (zero pairings inside the guest).
//!   - Poseidon2-BN254 parameters MUST match the PoC exactly (AC1.2/AC1.3).

use risc0_zkvm::guest::env;

fn main() {
    // TODO (s1/03): read private inputs (notes + Merkle paths) from env::read().
    // TODO (s1/03): read public input (merkle_root) from env::read().
    // TODO (s1/03): validate membership, nullifier derivation, balance sum.
    // TODO (s1/03): write journal { merkle_root, nullifiers, payouts } via env::commit().

    // s1/01 hello-world: commit a sentinel so the receipt can be verified.
    let msg: &str = "hello from rollup-guest (s1/01 skeleton)";
    env::commit(&msg);
}
