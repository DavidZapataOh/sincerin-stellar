//! Core value types for the sequencer: request ids, the withdrawal intent a user
//! submits, the per-request status (the API state machine), and the proving
//! artifacts.
//!
//! All field-element-shaped bytes are 32-byte **little-endian** — the SAME
//! convention as `zk_core::witness` (the prover's wire format). The sequencer
//! never invents a second serialization; it builds `zk_core::witness::GuestInput`
//! values directly (lock 3 — byte-compatible with the real prover).

use serde::{Deserialize, Serialize};

/// Opaque, monotonically-issued handle a user polls with [`crate::Sequencer::get_status`].
///
/// `Copy` (it is a `u64`) so it is cheap to pass by value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RequestId(pub u64);

impl core::fmt::Display for RequestId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "req-{}", self.0)
    }
}

/// The per-request **API state machine** (CONTEXT.md AC4.3, plan §1.2).
///
/// Lifecycle: `Pending → Batched → Proving → Settled | Failed`. On a collision a
/// note's request returns to `Pending` (see [`crate::Sequencer::handle_collision`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Status {
    /// Validated + note reserved (locked by nullifier); waiting to be batched.
    Pending,
    /// Assigned to a batch that has not started proving yet.
    Batched,
    /// The batch is in the (multi-hour) prove. The user keeps polling.
    Proving,
    /// Settled on-chain. Carries the settle transaction hash (hex).
    Settled {
        /// The settle tx hash (hex), resolvable on the explorer.
        tx_hash: String,
    },
    /// Permanently failed for this request (carries a human-readable reason).
    Failed {
        /// Why this request failed (e.g. invalid witness, settle reverted).
        reason: String,
    },
}

impl Status {
    /// True once the request reached a terminal state (`Settled`/`Failed`).
    pub fn is_terminal(&self) -> bool {
        matches!(self, Status::Settled { .. } | Status::Failed { .. })
    }
}

/// What a user submits to `submit_withdrawal`. Mirrors one `NoteWitness` plus the
/// public `merkle_root` the note is proven against.
///
/// **Trust boundary (AC4.4):** the operator receives these secrets (Diseño B) and
/// therefore sees the note↔recipient mapping. Unlinkability is on-chain/public,
/// NOT against the operator. See `sequencer/README.md`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WithdrawalIntent {
    /// Spending secret (`priv_key`), 32B LE.
    pub secret: [u8; 32],
    /// Commitment blinding factor, 32B LE.
    pub blinding: [u8; 32],
    /// In-claro amount; also the payout amount.
    pub amount: u128,
    /// Payout recipient (opaque 32-byte address; copied verbatim to the journal).
    pub recipient: [u8; 32],
    /// Merkle authentication path (one sibling per level, leaf level first), 32B LE.
    pub path: Vec<[u8; 32]>,
    /// Leaf index of this note's commitment in the pool tree.
    pub index: u64,
    /// Public Merkle root this note is proven against, 32B LE.
    pub merkle_root: [u8; 32],
}

/// A proven batch — exactly the three artifacts `settle_batch` consumes. Produced
/// by a [`crate::prover::Prover`] and never hand-built outside the test fixture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProvedBatch {
    /// Groth16/BN254 seal bytes.
    pub seal: Vec<u8>,
    /// 32-byte guest image id the receipt was proven under.
    pub image_id: [u8; 32],
    /// Raw committed journal bytes `{ root, nullifiers, payouts }`.
    pub journal: Vec<u8>,
}
