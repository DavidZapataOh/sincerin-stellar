//! Soroban rollup contract — Confidential Payments Rollup on Stellar.
//!
//! Entry point: `settle_batch(seal, image_id, journal)`
//!
//! **AC4.1 (CONTEXT.md D4):** In one atomic tx:
//!   1. Verify the RISC Zero Groth16/BN254 receipt.
//!   2. Assert `merkle_root ∈ pool.is_known_root`.
//!   3. For each nullifier: assert !is_spent; mark_spent.
//!   4. For each payout: transfer.
//!
//! **AC4.2:** Any duplicate/spent nullifier reverts the entire tx (todo-o-nada).
//!
//! Sprint s1/01 scope: skeleton `#[contract]` that compiles.
//! Logic implemented in s2/02 after N=2 end-to-end proves in testnet (s1/05).
#![no_std]
#![deny(unsafe_code)]
// soroban-sdk macros generate code without doc comments; allow globally here.
#![allow(missing_docs)]

use soroban_sdk::{contract, contractimpl, Env};

/// Rollup contract.
#[contract]
pub struct RollupContract;

#[contractimpl]
impl RollupContract {
    /// Settle a batch of N private withdrawals.
    ///
    /// Arguments (to be typed properly in s2/02):
    /// - `seal`       — Groth16/BN254 seal bytes from the RISC Zero receipt.
    /// - `image_id`   — 32-byte image ID of the trusted guest binary.
    /// - `journal`    — public journal: { merkle_root, nullifiers, payouts }.
    ///
    /// Atomically verifies the receipt on-chain then processes all payouts.
    pub fn settle_batch(env: Env) {
        // TODO (s2/02): decode args, call risc0-router verify, assert root,
        //               iterate nullifiers (assert !spent; mark), iterate payouts
        //               (transfer via SAC).
        let _ = env;
        soroban_sdk::panic_with_error!(&env, &crate::error::Error::NotImplemented);
    }
}

/// Contract errors.
pub mod error {
    use soroban_sdk::contracterror;

    /// Rollup contract error codes.
    #[contracterror]
    #[derive(Copy, Clone, Debug, Eq, PartialEq)]
    #[repr(u32)]
    pub enum Error {
        /// Function not yet implemented (scaffold placeholder).
        NotImplemented = 1,
        /// The provided Merkle root is not known to the pool.
        UnknownRoot = 2,
        /// A nullifier has already been spent (replay attack or batch dup).
        NullifierSpent = 3,
        /// The RISC Zero receipt failed on-chain verification.
        InvalidReceipt = 4,
    }
}
