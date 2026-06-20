//! Note commitment and nullifier derivation.
//!
//! Byte-exact port of the PoC's note crypto
//! (`stellar-private-payments/app/crates/core/prover/src/crypto.rs` and
//! `.../notes.rs`, mirrored by `circuits/src/test/utils/{keypair,transaction}.rs`).
//! Every hash is Poseidon2 over BN254 (see [`crate::poseidon2`]); every domain
//! separator is an explicit, named `const` (consensus-critical — no magic
//! numbers).
//!
//! ## Derivations
//! ```text
//! pubkey     = Poseidon2_t3([priv_key, 0, DOMAIN_PUBKEY])[0]            (t=3)
//! commitment = Poseidon2_t4([amount, pubkey, blinding, DOMAIN_COMMIT])[0]      (t=4)
//! signature  = Poseidon2_t4([priv_key, commitment, path_idx, DOMAIN_SIG])[0]   (t=4)
//! nullifier  = Poseidon2_t4([commitment, path_idx, signature, DOMAIN_NULL])[0] (t=4)
//! ```
//! `path_idx` is the leaf index as a field element (its u64 value).
//!
//! Serialization across the wire is 32-byte LITTLE-ENDIAN
//! (`Fr::into_bigint().to_bytes_le()`), matching the PoC's `scalar_to_bytes`.

use ark_ff::{BigInteger, PrimeField};

use crate::poseidon2::{self, Fr};

// ── Domain separators (must match the PoC exactly) ────────────────────────────
// Sources (stellar-private-payments):
//   pubkey     domain 3 — prover/src/crypto.rs:168 ; circuits .../keypair.rs derive_public_key
//   commitment domain 1 — prover/src/crypto.rs:106 ; circuits .../transaction.rs commitment
//   signature  domain 4 — prover/src/crypto.rs:120 ; circuits .../keypair.rs sign
//   nullifier  domain 2 — prover/src/crypto.rs:137 ; circuits .../transaction.rs nullifier

/// Domain separator for public-key derivation (`Poseidon2_t3`, lane 1 padded 0).
pub const DOMAIN_PUBKEY: u64 = 3;
/// Domain separator for leaf commitments (`Poseidon2_t4`).
pub const DOMAIN_COMMITMENT: u64 = 1;
/// Domain separator for the spend signature (`Poseidon2_t4`).
pub const DOMAIN_SIGNATURE: u64 = 4;
/// Domain separator for the nullifier (`Poseidon2_t4`).
pub const DOMAIN_NULLIFIER: u64 = 2;

/// A spendable note (the private witness for a single withdrawal).
pub struct Note {
    /// Amount in claro (field element). Amounts are NOT hidden in this design.
    pub amount: Fr,
    /// Spending private key.
    pub priv_key: Fr,
    /// Commitment blinding factor.
    pub blinding: Fr,
    /// Index of this note's commitment leaf in the pool Merkle tree.
    pub leaf_index: u64,
}

/// The leaf index encoded as a field element (`path_indices`), matching the
/// PoC, which packs the u64 leaf index into the low 8 LE bytes of a field
/// element (`prover/src/notes.rs:67-68`). For any in-range `u64` this equals
/// `Fr::from(leaf_index)`.
#[inline]
pub fn path_indices(leaf_index: u64) -> Fr {
    Fr::from(leaf_index)
}

/// `pubkey = Poseidon2_t3([priv_key, 0, DOMAIN_PUBKEY])[0]`.
///
/// The `0` lane is padding because Poseidon2 has no t = 1 instance over BN254
/// (PoC `keypair.rs::derive_public_key`).
pub fn public_key(priv_key: Fr) -> Fr {
    poseidon2::hash(&[priv_key, Fr::from(0u64), Fr::from(DOMAIN_PUBKEY)])
}

/// `commitment = Poseidon2_t4([amount, pubkey, blinding, DOMAIN_COMMITMENT])[0]`,
/// where `pubkey = public_key(note.priv_key)`.
pub fn commitment(note: &Note) -> Fr {
    let pubkey = public_key(note.priv_key);
    poseidon2::hash(&[
        note.amount,
        pubkey,
        note.blinding,
        Fr::from(DOMAIN_COMMITMENT),
    ])
}

/// `signature = Poseidon2_t4([priv_key, commitment, path_indices, DOMAIN_SIGNATURE])[0]`.
///
/// `commitment` is passed in (rather than recomputed) so callers that already
/// hold it avoid a redundant hash; it must equal [`commitment`] for this note.
pub fn signature(note: &Note, commitment: Fr) -> Fr {
    poseidon2::hash(&[
        note.priv_key,
        commitment,
        path_indices(note.leaf_index),
        Fr::from(DOMAIN_SIGNATURE),
    ])
}

/// `nullifier = Poseidon2_t4([commitment, path_indices, signature, DOMAIN_NULLIFIER])[0]`.
///
/// Two-step derivation: first the signature over `(secret, commitment, path)`,
/// then the nullifier over `(commitment, path, signature)` — matching the PoC
/// (`prover/src/notes.rs`, `transaction.rs`). `secret` is the note's spending
/// private key.
pub fn nullifier(note: &Note, secret: Fr) -> Fr {
    let commitment = commitment(note);
    // Derive the signature with the provided spending secret (defense-in-depth:
    // an honest caller passes secret == note.priv_key).
    let sig = poseidon2::hash(&[
        secret,
        commitment,
        path_indices(note.leaf_index),
        Fr::from(DOMAIN_SIGNATURE),
    ]);
    poseidon2::hash(&[
        commitment,
        path_indices(note.leaf_index),
        sig,
        Fr::from(DOMAIN_NULLIFIER),
    ])
}

/// Decode a field element from 32-byte little-endian bytes
/// (`from_le_bytes_mod_order`, matching the PoC `bytes_to_scalar`).
#[inline]
pub fn fr_from_le_bytes(bytes: &[u8; 32]) -> Fr {
    Fr::from_le_bytes_mod_order(bytes)
}

/// Encode a field element to 32-byte little-endian bytes
/// (`into_bigint().to_bytes_le()` padded to 32, matching the PoC
/// `scalar_to_bytes`).
#[inline]
pub fn fr_to_le_bytes(f: &Fr) -> [u8; 32] {
    let v = f.into_bigint().to_bytes_le();
    debug_assert!(v.len() <= 32);
    let mut out = [0u8; 32];
    out[..v.len()].copy_from_slice(&v);
    out
}
