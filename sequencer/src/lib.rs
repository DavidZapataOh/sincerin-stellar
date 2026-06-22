//! `sequencer` — single-operator MVP (CONTEXT.md D4 / AC4.3, AC4.4).
//!
//! Reserves notes (lock-by-nullifier so two batches never include the same
//! note), assembles batches at N (or timeout), proves ASYNC behind a [`Prover`]
//! (the user never blocks on the multi-hour prove), pays + sends the on-chain
//! settle, and on a collision drops the spent note, re-queues it, and rebuilds
//! the N−1 `GuestInput`.
//!
//! ## Trust boundary (AC4.4)
//! The operator receives note secrets (Diseño B) → sees note↔recipient. The
//! unlinkability this rollup provides is ON-CHAIN/public, NOT against the
//! operator. See `sequencer/README.md`.
//!
//! ## What is synchronous vs async
//! The state machine, the nullifier lock, batch assembly, and the collision
//! rebuild are PURE synchronous logic (unit-tested exhaustively). Only `prove`
//! is async (behind [`Prover`]); the orchestration driver ([`run_batch`]) wires
//! them together off the submit path.

#![deny(unsafe_code)]
#![deny(missing_docs)]

use std::collections::BTreeMap;

pub mod batch;
pub mod prover;
pub mod types;

use types::{RequestId, Status, WithdrawalIntent};
use zk_core::witness::GuestInput;

/// Error returned by `submit_withdrawal` when an intent cannot be accepted.
#[derive(Debug, PartialEq, Eq)]
pub enum SubmitError {
    /// The note's nullifier is already reserved by a live (non-terminal) request
    /// — the lock (lock-by-nullifier) rejects the duplicate so it cannot enter a
    /// second batch. Carries the existing request id.
    AlreadyReserved(RequestId),
    /// The intent is malformed (empty path, root length, etc.).
    Invalid(String),
}

impl core::fmt::Display for SubmitError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            SubmitError::AlreadyReserved(id) => {
                write!(f, "note already reserved by {id} (lock)")
            }
            SubmitError::Invalid(s) => write!(f, "invalid intent: {s}"),
        }
    }
}

/// One live request: the user's intent, its nullifier (the lock key), and its
/// current [`Status`].
#[derive(Clone, Debug)]
struct Request {
    intent: WithdrawalIntent,
    nullifier: [u8; 32],
    status: Status,
}

/// An assembled batch: the requests it covers, in order, sharing one root.
#[derive(Clone, Debug)]
pub struct Batch {
    /// The requests in this batch (order == journal order).
    pub request_ids: Vec<RequestId>,
    /// The intents, parallel to `request_ids`.
    pub intents: Vec<WithdrawalIntent>,
    /// The shared pool root (32B LE) all members prove membership against.
    pub merkle_root: [u8; 32],
}

impl Batch {
    /// Build the `GuestInput` for this batch (LOCK 3: byte-compatible — the exact
    /// `zk_core::witness::GuestInput` the prover/executor consume).
    pub fn guest_input(&self) -> GuestInput {
        batch::guest_input_of(&self.intents)
    }
}

/// Outcome of [`Sequencer::handle_collision`]: the rebuilt N−k batch (`None` if
/// every note collided) plus the dropped requests (now back to `Pending`).
#[derive(Debug)]
pub struct CollisionOutcome {
    /// The rebuilt batch over the surviving notes, or `None` if none survived.
    pub rebuilt: Option<Batch>,
    /// The requests dropped (spent on-chain) and re-queued to `Pending`.
    pub dropped: Vec<RequestId>,
}

/// The single-operator sequencer. Holds the mempool of live requests and the
/// nullifier lock. `prove` is delegated to a [`prover::Prover`] (the only seam to
/// proving hardware).
pub struct Sequencer {
    /// Live requests by id. Terminal requests stay here so `get_status` keeps
    /// answering (the user can always poll their id).
    requests: BTreeMap<RequestId, Request>,
    /// Lock-by-nullifier: nullifier → the request currently holding it. A note
    /// can be reserved by AT MOST ONE live request (lock sufficiency, plan §5f).
    reserved: BTreeMap<[u8; 32], RequestId>,
    /// Monotonic id source.
    next_id: u64,
    /// Batch size that triggers assembly.
    batch_size: usize,
}

impl Sequencer {
    /// Create a sequencer that assembles a batch once `batch_size` notes are
    /// `Pending` (and shar a root). `batch_size` must be ≥ 1.
    pub fn new(batch_size: usize) -> Self {
        assert!(batch_size >= 1, "batch_size must be >= 1");
        Self {
            requests: BTreeMap::new(),
            reserved: BTreeMap::new(),
            next_id: 0,
            batch_size,
        }
    }

    /// Validate + **reserve** (lock by nullifier) + enqueue an intent, returning a
    /// `request_id` IMMEDIATELY. Never blocks on proving (plan §1.1).
    ///
    /// Rejects (does NOT enqueue) if the note's nullifier is already reserved by a
    /// live request — the lock guarantees a note cannot enter two batches.
    pub fn submit_withdrawal(
        &mut self,
        intent: WithdrawalIntent,
    ) -> Result<RequestId, SubmitError> {
        // ── validate ──────────────────────────────────────────────────────────
        if intent.path.is_empty() {
            return Err(SubmitError::Invalid("empty merkle path".into()));
        }

        // ── derive nullifier (the lock key) ─────────────────────────────────────
        let nullifier = batch::nullifier_of(&intent);

        // ── lock: reject a note already held by a LIVE request ──────────────────
        if let Some(existing) = self.reserved.get(&nullifier) {
            return Err(SubmitError::AlreadyReserved(*existing));
        }

        // ── enqueue ─────────────────────────────────────────────────────────────
        let id = RequestId(self.next_id);
        self.next_id += 1;
        self.reserved.insert(nullifier, id);
        self.requests.insert(
            id,
            Request {
                intent,
                nullifier,
                status: Status::Pending,
            },
        );
        Ok(id)
    }

    /// Poll the status of a request. `None` if the id was never issued.
    pub fn get_status(&self, id: RequestId) -> Option<Status> {
        self.requests.get(&id).map(|r| r.status.clone())
    }

    /// Number of requests currently `Pending` (eligible for the next batch).
    pub fn pending_count(&self) -> usize {
        self.requests
            .values()
            .filter(|r| r.status == Status::Pending)
            .count()
    }

    /// True iff a batch can be assembled now: ≥ `batch_size` pending requests
    /// share a single root.
    pub fn ready_to_batch(&self) -> bool {
        self.assemble_candidate().is_some()
    }

    /// Find a set of `batch_size` `Pending` requests that share one root, in id
    /// order. Returns the ids (not yet transitioned).
    fn assemble_candidate(&self) -> Option<Vec<RequestId>> {
        // Group pending request ids by root (id-ordered within each group).
        let mut by_root: BTreeMap<[u8; 32], Vec<RequestId>> = BTreeMap::new();
        for (id, r) in &self.requests {
            if r.status == Status::Pending {
                by_root.entry(r.intent.merkle_root).or_default().push(*id);
            }
        }
        by_root
            .into_values()
            .find(|ids| ids.len() >= self.batch_size)
            .map(|mut ids| {
                ids.truncate(self.batch_size);
                ids
            })
    }

    /// Assemble a batch (trigger at N): transition `batch_size` same-root pending
    /// requests to `Batched` and return the [`Batch`]. `None` if not ready.
    ///
    /// Timeout-triggered assembly is the same operation with a lower threshold;
    /// the driver lowers `batch_size` effectively by calling [`Self::force_batch`].
    pub fn try_assemble_batch(&mut self) -> Option<Batch> {
        let ids = self.assemble_candidate()?;
        Some(self.assemble_ids(ids))
    }

    /// Force-assemble a batch from up to `max` same-root pending requests even
    /// below `batch_size` (the timeout path). `None` if there are no pending
    /// requests at all.
    pub fn force_batch(&mut self, max: usize) -> Option<Batch> {
        let mut by_root: BTreeMap<[u8; 32], Vec<RequestId>> = BTreeMap::new();
        for (id, r) in &self.requests {
            if r.status == Status::Pending {
                by_root.entry(r.intent.merkle_root).or_default().push(*id);
            }
        }
        // Largest same-root group first (most efficient batch).
        let mut groups: Vec<Vec<RequestId>> = by_root.into_values().collect();
        groups.sort_by_key(|g| std::cmp::Reverse(g.len()));
        let mut ids = groups.into_iter().next()?;
        ids.truncate(max.max(1));
        Some(self.assemble_ids(ids))
    }

    /// Transition the given pending ids → `Batched` and materialize the [`Batch`].
    fn assemble_ids(&mut self, ids: Vec<RequestId>) -> Batch {
        let mut intents = Vec::with_capacity(ids.len());
        let mut root = [0u8; 32];
        for (k, id) in ids.iter().enumerate() {
            let r = self.requests.get_mut(id).expect("candidate id exists");
            debug_assert_eq!(r.status, Status::Pending, "candidate must be pending");
            if k == 0 {
                root = r.intent.merkle_root;
            } else {
                debug_assert_eq!(r.intent.merkle_root, root, "batch must share one root");
            }
            r.status = Status::Batched;
            intents.push(r.intent.clone());
        }
        Batch {
            request_ids: ids,
            intents,
            merkle_root: root,
        }
    }

    /// Mark a batch as proving (state machine: `Batched → Proving`).
    pub fn mark_proving(&mut self, batch: &Batch) {
        for id in &batch.request_ids {
            if let Some(r) = self.requests.get_mut(id) {
                r.status = Status::Proving;
            }
        }
    }

    /// Mark a batch settled (`Proving → Settled{tx_hash}`) and RELEASE the locks
    /// (the notes are now spent on-chain; their nullifiers move to the on-chain
    /// `is_spent` set, so the in-memory reservation is no longer needed).
    pub fn mark_settled(&mut self, batch: &Batch, tx_hash: &str) {
        for id in &batch.request_ids {
            if let Some(r) = self.requests.get_mut(id) {
                r.status = Status::Settled {
                    tx_hash: tx_hash.to_string(),
                };
                self.reserved.remove(&r.nullifier);
            }
        }
    }

    /// Mark a batch failed (`* → Failed{reason}`) and release the locks so the
    /// notes can be re-submitted.
    pub fn mark_failed(&mut self, batch: &Batch, reason: &str) {
        for id in &batch.request_ids {
            if let Some(r) = self.requests.get_mut(id) {
                r.status = Status::Failed {
                    reason: reason.to_string(),
                };
                self.reserved.remove(&r.nullifier);
            }
        }
    }

    // ═════════════════════════════════════════════════════════════════════════
    // COLLISION (plan §5) — security-critical, code touching the fund flow.
    // ═════════════════════════════════════════════════════════════════════════

    /// Given an assembled `batch` and the set of nullifiers already spent ON-CHAIN
    /// (`is_spent_le`, 32B LE each), DROP the spent notes (→ `Pending`, re-queued)
    /// and REBUILD the surviving batch.
    ///
    /// Invariants (plan §5, SEC-reviewed):
    /// - **root unchanged:** the rebuilt batch keeps the SAME `merkle_root` (it is
    ///   the pool root, not derived from the batch). Surviving paths stay valid.
    /// - **journal changes:** the rebuilt `GuestInput` has exactly the surviving
    ///   notes (the dropped nullifier(s) do NOT appear) → a new prove is needed.
    /// - **no loss / no dup:** each dropped request returns to `Pending` exactly
    ///   once, keeps its lock (its note is NOT spent — a different note collided),
    ///   and can be re-batched later. Survivors stay `Batched`.
    ///
    /// `is_spent_le` is the REAL on-chain answer (queried by the driver), so this
    /// is the true anti-replay guard, not a guess.
    pub fn handle_collision(
        &mut self,
        batch: &Batch,
        is_spent_le: &[[u8; 32]],
    ) -> CollisionOutcome {
        use std::collections::BTreeSet;
        let spent: BTreeSet<[u8; 32]> = is_spent_le.iter().copied().collect();

        let mut survivors_ids = Vec::new();
        let mut survivors_intents = Vec::new();
        let mut dropped = Vec::new();

        for (id, intent) in batch.request_ids.iter().zip(batch.intents.iter()) {
            let nf = batch::nullifier_of(intent);
            if spent.contains(&nf) {
                // This note's nullifier is already on-chain `is_spent` (a competing
                // batch settled it first). It MUST leave this batch — including it
                // again would make `settle_batch` revert the whole tx (AC4.2). The
                // plan (§5e) re-queues it to Pending without loss/dup; the policy
                // layer (out of MVP) then decides retry vs reject. We keep its lock
                // held (still reserved by this request) until that decision.
                if let Some(r) = self.requests.get_mut(id) {
                    r.status = Status::Pending;
                }
                dropped.push(*id);
            } else {
                survivors_ids.push(*id);
                survivors_intents.push(intent.clone());
            }
        }

        let rebuilt = if survivors_ids.is_empty() {
            None
        } else {
            Some(Batch {
                request_ids: survivors_ids,
                intents: survivors_intents,
                merkle_root: batch.merkle_root, // UNCHANGED — pool root.
            })
        };

        CollisionOutcome { rebuilt, dropped }
    }

    /// Number of live (non-terminal) reservations held (lock occupancy).
    pub fn reserved_count(&self) -> usize {
        self.reserved.len()
    }
}

#[cfg(test)]
mod tests;
