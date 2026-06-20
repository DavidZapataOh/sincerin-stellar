//! Guest I/O wire types — the **private witness** the host hands the RISC Zero
//! guest, plus the public `merkle_root`.
//!
//! These live in `zk-core` (not the guest crate) so the host (which builds the
//! `ExecutorEnv`) and the guest (which reads it) share one byte-identical
//! definition. They are feature-gated behind `witness` (which pulls `serde`)
//! so the Soroban contract — which also compiles `zk-core` but never needs the
//! guest I/O — does not drag in serde.
//!
//! ## Serialization rationale (consensus-critical)
//! `ark_bn254::Fr` does **not** implement `serde::Serialize` in our config
//! (ark-ff exposes serde only as a dev-dependency), and enabling it would risk
//! a different byte form than the PoC. So every field element crosses the wire
//! as a **32-byte little-endian** array (`note::fr_to_le_bytes` /
//! `fr_from_le_bytes`) — the same convention as the PoC's `scalar_to_bytes`.
//! `u128`/`u64`/`[u8;32]`/`Vec<…>` all serialize natively, so the RISC Zero
//! serde framing (`env::write` / `env::read`) round-trips them losslessly.
//!
//! ## ASP hook (stretch — NOT implemented here)
//! `NoteWitness` is laid out so a future `asp_path: Vec<[u8;32]>` +
//! non-membership witness can be appended **without** changing the existing
//! fields, and the guest's per-note loop can slot the non-membership check in
//! after membership (CONTEXT.md D7) — no refactor of this type required.

extern crate alloc;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

/// One withdrawal's private witness.
///
/// Field elements are 32-byte **little-endian** (`note::fr_*_le_bytes`).
/// `amount` is the in-claro value (NOT hidden); it is bound into the note's
/// commitment as `Fr::from(amount)`, so it cannot be forged without breaking
/// Merkle membership.
///
/// NOTE: `blinding` is REQUIRED even though an earlier draft of the field list
/// omitted it — the commitment is `Poseidon2(amount, pubkey, blinding, 1)`, so
/// the guest cannot recompute the commitment (and therefore membership) without
/// it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteWitness {
    /// Spending secret (`priv_key`), 32B LE. Derives `pubkey` and the nullifier.
    pub secret: [u8; 32],
    /// Commitment blinding factor, 32B LE.
    pub blinding: [u8; 32],
    /// In-claro amount; also the payout amount. Bound into the commitment.
    pub amount: u128,
    /// Payout recipient (opaque 32-byte address; copied verbatim to the journal).
    pub recipient: [u8; 32],
    /// Merkle authentication path: one sibling per level, leaf level first.
    /// Each sibling is 32B LE.
    pub path: Vec<[u8; 32]>,
    /// Leaf index of this note's commitment in the pool tree (also `path_indices`).
    pub index: u64,
}

/// The full private input to the guest: `N` note witnesses + the public root.
///
/// `merkle_root` is 32B **LE**; the guest asserts each note's recomputed root
/// equals it (membership) and echoes it into the committed journal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuestInput {
    /// The `N` withdrawals being aggregated.
    pub notes: Vec<NoteWitness>,
    /// Public Merkle root the whole batch is proven against, 32B LE.
    pub merkle_root: [u8; 32],
}
