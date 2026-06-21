//! Soroban rollup contract — Confidential Payments Rollup on Stellar.
//!
//! Entry point: `settle_batch(seal, image_id, journal_bytes)`.
//!
//! **AC4.1 (CONTEXT.md D4) — one atomic tx, in this exact order:**
//!   0. **Bind to OUR guest:** `assert image_id == ROLLUP_GUEST_ID` (FIRST).
//!   1. Verify the RISC Zero Groth16/BN254 receipt (cross-contract `verifier`).
//!   2. Decode the journal → `{ root, nullifiers, payouts }` (s2/01 decoder).
//!   3. Assert `root ∈ pool.is_known_root` (cross-contract `pool`).
//!   4. For each nullifier IN ORDER: assert `!is_spent`; `mark_spent`.
//!   5. For each payout: `token.transfer(self, recipient, amount)`.
//!
//! **AC4.2 (todo-o-nada):** Any failed assert panics, and Soroban reverts the
//! ENTIRE transaction — no partial nullifier marks, no partial transfers. The
//! sequential `mark_spent` also catches an intra-batch duplicate nullifier: the
//! second occurrence fails its own `!is_spent` on the second pass.
//!
//! ## Why the ordering is security-load-bearing
//! `image_id` is an **attacker-controlled parameter**. A valid Groth16 proof of
//! a *different* guest program (one that emits a well-formed journal with
//! attacker-chosen nullifiers/payouts) would pass the verifier. Binding
//! `image_id` to OUR fixed `ROLLUP_GUEST_ID` *before* anything else is what stops
//! that program from draining funds. Verifying the receipt before touching state,
//! and marking ALL nullifiers before ANY transfer, keeps the all-or-nothing
//! guarantee exact.
#![no_std]
#![deny(unsafe_code)]
// soroban-sdk macros generate code without doc comments; allow globally here.
#![allow(missing_docs)]

use soroban_sdk::{
    contract, contractclient, contractimpl, contracttype, panic_with_error, token::TokenClient,
    Address, Bytes, BytesN, Env, U256,
};

use crate::error::Error;

/// Canonical journal decoder (mirror of `zk-core::journal`, raw `Bytes`).
pub mod journal;

/// The 32-byte RISC Zero image id of OUR rollup guest binary
/// (`methods::ROLLUP_GUEST_ID`, frozen at `out/receipt/image_id.hex`).
///
/// **This is the trust anchor of the whole contract.** `settle_batch` asserts the
/// caller-supplied `image_id` equals this constant before doing anything else, so
/// only a proof of *this exact program* can move funds. It is a compile-time
/// `const` (not config) on purpose: the trusted guest is fixed for a deployment,
/// and a const is the most auditable form — there is no setter, no storage slot,
/// nothing an admin key could repoint.
pub const ROLLUP_GUEST_ID: [u8; 32] = [
    0x3f, 0x03, 0x6c, 0x52, 0x8c, 0x18, 0xed, 0x4c, 0xd6, 0x6a, 0x89, 0x88, 0x39, 0xd3, 0x43, 0xfd,
    0x4a, 0x51, 0xbb, 0x87, 0x35, 0xac, 0xec, 0x05, 0x77, 0xc6, 0xc6, 0x38, 0x9c, 0x5d, 0x4d, 0x22,
];

/// Minimal client for the deployed RISC Zero verifier (`CBQF…`, version 3.0.0).
///
/// Mirrors `risc0_interface::RiscZeroVerifierInterface::verify` exactly:
/// `journal` is the **sha256 digest** of the journal bytes, NOT the raw journal.
/// We declare the client locally (instead of depending on the verifier crate) so
/// the rollup wasm stays lean — it needs only this one method.
#[contractclient(name = "VerifierClient")]
pub trait VerifierInterface {
    /// Verify a Groth16/BN254 receipt. Errors (e.g. `InvalidProof`) propagate as
    /// a host error, which the panicking client turns into a revert.
    fn verify(env: Env, seal: Bytes, image_id: BytesN<32>, journal: BytesN<32>);
}

/// Minimal client for the deployed privacy-pool (`stellar-private-payments`).
///
/// `is_known_root` takes the root as a **`U256`** (the pool stores Merkle nodes
/// as `U256` field elements, big-endian). See [`root_le_to_u256`] for the
/// endianness bridge from the journal's little-endian root bytes.
#[contractclient(name = "PoolClient")]
pub trait PoolInterface {
    /// Returns `true` iff `root` is in the pool's recent root history.
    fn is_known_root(env: Env, root: U256) -> bool;
}

/// Persistent storage keys.
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Deployed RISC Zero verifier contract address.
    Verifier,
    /// Deployed privacy-pool contract address (owns `is_known_root`).
    Pool,
    /// Token contract the rollup pays out from (SAC / token interface).
    Token,
    /// Per-nullifier spent marker. Presence == spent. Global across all batches.
    Spent(BytesN<32>),
}

/// Rollup contract.
#[contract]
pub struct RollupContract;

#[contractimpl]
impl RollupContract {
    /// Initialize the rollup with the contracts it talks to.
    ///
    /// - `verifier` — the deployed RISC Zero verifier (or router) address.
    /// - `pool`     — the deployed privacy-pool that answers `is_known_root`.
    /// - `token`    — the token contract funds are paid out from. The rollup
    ///   contract itself must hold/control the balance it transfers.
    ///
    /// The trusted guest id is NOT configurable — it is the compile-time
    /// [`ROLLUP_GUEST_ID`] constant (see its docs for why).
    pub fn __constructor(env: Env, verifier: Address, pool: Address, token: Address) {
        let storage = env.storage().instance();
        storage.set(&DataKey::Verifier, &verifier);
        storage.set(&DataKey::Pool, &pool);
        storage.set(&DataKey::Token, &token);
    }

    /// Settle a batch of N private withdrawals atomically (AC4.1 / AC4.2).
    ///
    /// - `seal`         — Groth16/BN254 seal from the RISC Zero receipt.
    /// - `image_id`     — claimed 32-byte guest image id (**attacker-controlled**;
    ///   asserted against [`ROLLUP_GUEST_ID`] FIRST).
    /// - `journal_bytes`— the raw committed journal `{ root, nullifiers, payouts }`.
    ///
    /// Panics (reverting the whole tx) on: wrong image id, failed receipt
    /// verification, malformed journal, unknown root, or any already-spent /
    /// duplicate nullifier. On success every nullifier is marked spent and every
    /// payout transferred.
    pub fn settle_batch(env: Env, seal: Bytes, image_id: BytesN<32>, journal_bytes: Bytes) {
        // ── 0. Bind to OUR guest (CRITICAL, must be first) ──────────────────────
        // image_id is attacker-controlled; without this a valid proof of ANY
        // program with a well-formed journal would drain funds.
        let expected = BytesN::from_array(&env, &ROLLUP_GUEST_ID);
        if image_id != expected {
            panic_with_error!(&env, Error::WrongImageId);
        }

        // ── 1. Verify the receipt on-chain ──────────────────────────────────────
        // The verifier's `journal` argument is the SHA-256 digest of the bytes.
        let digest: BytesN<32> = env.crypto().sha256(&journal_bytes).into();
        let verifier_addr: Address = env
            .storage()
            .instance()
            .get(&DataKey::Verifier)
            .unwrap_or_else(|| panic_with_error!(&env, Error::NotInitialized));
        // Non-`try_` client method: any verifier error becomes a host panic → revert.
        VerifierClient::new(&env, &verifier_addr).verify(&seal, &image_id, &digest);

        // ── 2. Decode the journal (s2/01) ───────────────────────────────────────
        let decoded = match journal::decode(&env, &journal_bytes) {
            Ok(d) => d,
            Err(_) => panic_with_error!(&env, Error::MalformedJournal),
        };

        // ── 3. Assert the root is known to the pool ─────────────────────────────
        // Journal root is little-endian Fr bytes; the pool wants a big-endian
        // U256. Bridge the endianness here (see `root_le_to_u256`).
        let root_u256 = root_le_to_u256(&env, &decoded.root);
        let pool_addr: Address = env
            .storage()
            .instance()
            .get(&DataKey::Pool)
            .unwrap_or_else(|| panic_with_error!(&env, Error::NotInitialized));
        if !PoolClient::new(&env, &pool_addr).is_known_root(&root_u256) {
            panic_with_error!(&env, Error::UnknownRoot);
        }

        // ── 4. Double-spend: mark ALL nullifiers (in order) before any transfer ──
        // Sequential mark catches inter-batch replay (already-spent) AND
        // intra-batch dups (a repeat fails `!is_spent` on its second pass).
        let storage = env.storage().persistent();
        for nf in decoded.nullifiers.iter() {
            let key = DataKey::Spent(nf.clone());
            if storage.has(&key) {
                panic_with_error!(&env, Error::NullifierSpent);
            }
            storage.set(&key, &());
        }

        // ── 5. Payouts: transfer to each recipient ──────────────────────────────
        let token_addr: Address = env
            .storage()
            .instance()
            .get(&DataKey::Token)
            .unwrap_or_else(|| panic_with_error!(&env, Error::NotInitialized));
        let token = TokenClient::new(&env, &token_addr);
        let this = env.current_contract_address();
        for (recipient, amount) in decoded.payouts.iter() {
            let to = recipient_to_address(&env, &recipient);
            token.transfer(&this, &to, &amount);
        }
    }
}

/// Convert a journal Merkle root (**little-endian** `Fr` bytes) into the pool's
/// **big-endian** `U256`.
///
/// ### Why reverse the bytes
/// The journal serializes field elements little-endian
/// (`zk-core::note::fr_to_le_bytes`, matching the PoC `scalar_to_bytes`). The
/// privacy-pool, however, stores and compares Merkle roots as a `U256` built from
/// the field element's **big-endian** bytes: the PoC's `scalar_to_u256` does
/// `s.into_bigint().to_bytes_be()` → `U256::from_be_bytes`, and the pool's own
/// `u256_to_bytes` round-trips via `to_be_bytes`
/// (`pool.rs:724`, `merkle_with_history.rs:209`). `U256` is a numeric value, so it
/// must equal the field element's magnitude — feeding the LE bytes unreversed
/// would compare a byte-swapped (wrong) number and reject every valid root.
///
/// Therefore: take the 32 LE bytes, reverse them to big-endian, and
/// `U256::from_be_bytes`. (This endianness is only *fully* exercised against the
/// real pool on testnet in s2/03; the unit tests here use a mock pool.)
fn root_le_to_u256(env: &Env, root_le: &BytesN<32>) -> U256 {
    let mut be = root_le.to_array();
    be.reverse(); // little-endian → big-endian
    U256::from_be_bytes(env, &Bytes::from_array(env, &be))
}

/// Convert a journal payout recipient (opaque 32 bytes, taken **verbatim**) into a
/// Soroban [`Address`].
///
/// ### Scheme: account ed25519 public key (G…), bytes verbatim
/// The 32 recipient bytes are interpreted as the **ed25519 public key of a Stellar
/// account address** (a `G…` strkey payload) and passed through byte-for-byte (NO
/// endianness reversal — unlike the root, the recipient is not a reduced field
/// element; `zk-core::journal::encode` copies `p.recipient` verbatim, and the
/// decoder reads it verbatim, so this end-to-end map is the identity on the bytes).
/// Withdrawals in a privacy pool pay user accounts, whose 32-byte master key fits
/// exactly one journal slot, so "account id" is the correct, deterministic
/// discriminant.
///
/// ### Why this is safe / exact
/// We use [`AddressPayload::AccountIdPublicKeyEd25519`] (soroban-sdk
/// `hazmat-address`), the canonical, wasm-portable inverse of address→raw-key. It
/// is deterministic (fixed XDR header ‖ the 32 bytes) and total over all 32-byte
/// inputs, so the binding *journal recipient bytes ⇒ paid address* is exact and
/// auditable. We deliberately do NOT accept a contract id (`C…`) discriminant:
/// 32 opaque bytes cannot distinguish account from contract, so we fix ONE scheme
/// rather than guess. If the named account does not exist or lacks a trustline,
/// `token.transfer` fails and the whole `settle_batch` reverts (AC4.2) — funds are
/// never sent to an unroutable address.
fn recipient_to_address(env: &Env, recipient: &BytesN<32>) -> Address {
    use soroban_sdk::address_payload::AddressPayload;
    AddressPayload::AccountIdPublicKeyEd25519(recipient.clone()).to_address(env)
}

/// Contract errors.
pub mod error {
    use soroban_sdk::contracterror;

    /// Rollup contract error codes.
    #[contracterror]
    #[derive(Copy, Clone, Debug, Eq, PartialEq)]
    #[repr(u32)]
    pub enum Error {
        /// The contract has not been initialized (`__constructor` not run).
        NotInitialized = 1,
        /// The provided Merkle root is not known to the pool.
        UnknownRoot = 2,
        /// A nullifier has already been spent (inter-batch replay or intra-batch dup).
        NullifierSpent = 3,
        /// The RISC Zero receipt failed on-chain verification.
        InvalidReceipt = 4,
        /// `image_id` does not match OUR trusted guest (`ROLLUP_GUEST_ID`).
        WrongImageId = 5,
        /// The journal bytes could not be decoded (wrong length / bad amount).
        MalformedJournal = 6,
    }
}

#[cfg(test)]
mod test;
