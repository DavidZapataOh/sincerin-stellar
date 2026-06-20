//! Host binary — Confidential Payments Rollup.
//!
//! Responsibilities (sprint s1/01: skeleton only):
//!   1. Load golden inputs from `golden/`.
//!   2. Execute the guest inside the RISC Zero zkVM (dev mode or real proving).
//!   3. Wrap the STARK receipt to Groth16/BN254 (via Docker + risc0-ethereum-contracts).
//!   4. Serialize the receipt for submission to the Soroban `settle_batch` contract.
//!
//! **AC5.1:** `make prove` → receipt verifies against the deployed Soroban verifier.
//! **AC5.2:** risc0-zkvm is pinned to =3.0.5 (=3.0.0 is yanked; see Cargo.toml).
//!
//! Sprint s1/01 scope: prints the guest image ID and exits.  Actual proving
//! logic added in s1/04.

use risc0_zkvm::{default_prover, ExecutorEnv};
use methods::{ROLLUP_GUEST_ELF, ROLLUP_GUEST_ID};

fn main() {
    // TODO (s1/04): load golden inputs, build ExecutorEnv, prove, wrap Groth16.

    println!("rollup-guest image ID: {:?}", ROLLUP_GUEST_ID);

    // s1/01 smoke: execute in dev mode (RISC0_DEV_MODE=1) — no real proving.
    // This validates that the guest ELF is compilable and the executor runs.
    let env = ExecutorEnv::builder()
        .build()
        .expect("failed to build ExecutorEnv");

    let prover = default_prover();
    let receipt = prover
        .prove(env, ROLLUP_GUEST_ELF)
        .expect("proving failed");

    // In dev mode the receipt is a fake; in production it is Groth16.
    println!("receipt journal: {:?}", receipt.receipt.journal.bytes);
    println!("host: done (s1/01 skeleton)");
}
