//! RISC Zero guest — Confidential Payments Rollup validity check (N withdrawals).
//!
//! Realizes CONTEXT.md **D1/AC1.1** (re-execute validity NATIVELY in Rust; NO
//! SNARK verified in-zkVM — zero pairings) and **D3/AC3.1, AC3.2** (journal
//! layout + membership + distinct nullifiers + balance conservation).
//!
//! ## What the guest proves (per batch)
//! Private input: `GuestInput { notes: [NoteWitness; N], merkle_root }`.
//! Public output (journal): `{ merkle_root, [nullifier_i], [(recipient_i, amount_i)] }`.
//!
//! For each note, NATIVELY in Rust (via `zk-core`, byte-identical to the PoC):
//!   1. derive `pubkey = Poseidon2_t3([secret, 0, 3])` and
//!      `commitment   = Poseidon2_t4([amount, pubkey, blinding, 1])`;
//!   2. **membership** — recompute the Merkle root from `(commitment, path,
//!      index)` and `assert_eq!` the public `merkle_root`. Because the commitment
//!      binds `amount` and `blinding`, a forged amount (or blinding) changes the
//!      commitment and FAILS this check → value cannot be forged;
//!   3. derive `nullifier = Poseidon2_t4([commitment, path_idx, signature, 2])`;
//!   4. **balance conservation** — accumulate `amount` into a running `u128` sum
//!      with CHECKED arithmetic; `assert` no overflow (defends the aggregate).
//! Then:
//!   5. **distinctness (AC3.2)** — `assert` the N nullifiers are pairwise
//!      distinct (defense-in-depth; the contract also catches dups, AC4.2);
//!   6. `env::commit_slice` the canonical journal = `journal::encode(root,
//!      nullifiers, payouts)` (raw bytes, so the contract reads it verbatim).
//!
//! ## ASP hook (stretch — NOT implemented, CONTEXT.md D7)
//! The per-note loop is structured so a non-membership check against an ASP
//! deny-set root can be inserted right after membership (step 2/3) using an
//! `asp_path` added to `NoteWitness` — no restructuring of this loop required.

use risc0_zkvm::guest::env;

use zk_core::journal::{self, Payout};
use zk_core::note::{self, Note};
use zk_core::poseidon2::Fr;
use zk_core::witness::GuestInput;

/// Pool-wide Merkle tree depth (number of levels = number of siblings on every
/// authentication path). The pool is a fixed-depth binary tree of `2^TREE_DEPTH`
/// leaves; every membership path therefore has EXACTLY `TREE_DEPTH` siblings and
/// every valid leaf index is in `[0, 2^TREE_DEPTH)`.
///
/// **Value:** `3` — chosen to match the membership this guest already verifies.
/// The golden `golden/n2_inputs.json` tree (the only inputs this guest is gated
/// on, derived byte-for-byte from the PoC `golden/poc_vectors.json`) has paths of
/// length 3 and leaves at indices 0 and 1, i.e. a depth-3 tree. Pinning the
/// constant here makes that depth an explicit, enforced pool parameter.
///
/// **Why it is consensus-critical (SEC Critical):** `merkle::root_from_path`
/// orients each level using only the LOW `path.len()` bits of `index`, but the
/// nullifier derives from `Fr::from(index)` over the FULL u64 (`note.rs`
/// `path_indices`). Without pinning, a note genuinely at leaf `p` passes
/// membership for EVERY `index ∈ {p, p+2^d, p+2·2^d, …}` (identical low bits ⇒
/// identical orientation) while minting a DIFFERENT nullifier each time ⇒ the
/// same committed note yields unlimited distinct valid nullifiers ⇒ unbounded
/// double-spend the contract cannot catch. Asserting `index < 2^TREE_DEPTH` AND
/// `path.len() == TREE_DEPTH` forces the membership orientation and the
/// nullifier's `path_indices` to share the SAME bits — they can no longer
/// diverge — and rejects depth-confusion witnesses loudly.
const TREE_DEPTH: usize = 3;

fn main() {
    // ── Read the private witness + public root (LE bytes → field elements) ────
    let input: GuestInput = env::read();
    let root: Fr = note::fr_from_le_bytes(&input.merkle_root);

    let n = input.notes.len();
    // A batch must aggregate at least one withdrawal; an empty batch is a
    // malformed input, not a meaningful proof.
    assert!(n > 0, "guest: batch must contain at least one note");

    let mut nullifiers: Vec<Fr> = Vec::with_capacity(n);
    let mut payouts: Vec<Payout> = Vec::with_capacity(n);

    // Running balance sum (in-claro amounts). Checked: overflow ⇒ panic.
    let mut total: u128 = 0;

    for w in &input.notes {
        // ── WITNESS-TRUST BOUNDARY (SEC Critical + Important) ─────────────────
        // Pin the witness to the pool's tree shape BEFORE deriving anything, so
        // the membership orientation (low bits of `index`) and the nullifier's
        // `path_indices` (Fr::from(FULL index)) cannot diverge. Strict asserts —
        // a malformed witness is rejected loudly, never silently masked.
        //
        // (Important) Path must be exactly pool-depth: a shorter/longer path
        // would verify membership at the wrong depth (e.g. a length-0 path makes
        // root_from_path return the commitment unchanged).
        assert!(
            w.path.len() == TREE_DEPTH,
            "guest: merkle path length {} != tree depth {} (depth not pinned)",
            w.path.len(),
            TREE_DEPTH
        );
        // (Critical) Index must be in the tree's leaf range. Without this, an
        // index congruent to the true leaf modulo 2^TREE_DEPTH passes membership
        // (identical low bits) but mints a DIFFERENT nullifier via Fr::from(index)
        // ⇒ unbounded double-spend. 1u64 << TREE_DEPTH is the leaf count.
        assert!(
            w.index < (1u64 << TREE_DEPTH),
            "guest: index {} out of range for tree depth {} (>= 2^{})",
            w.index,
            TREE_DEPTH,
            TREE_DEPTH
        );

        // Reconstruct the note. `amount` is bound into the commitment as
        // Fr::from(amount); blinding & secret come from the witness (LE).
        let note = Note {
            amount: Fr::from(w.amount),
            priv_key: note::fr_from_le_bytes(&w.secret),
            blinding: note::fr_from_le_bytes(&w.blinding),
            leaf_index: w.index,
        };
        let secret: Fr = note::fr_from_le_bytes(&w.secret);

        // (1) commitment (binds amount + blinding + pubkey(secret)).
        let commitment = note::commitment(&note);

        // (2) MEMBERSHIP: recompute the root from (commitment, path, index).
        // Forging amount/blinding changes `commitment` → recomputed root ≠ root.
        let path: Vec<Fr> = w.path.iter().map(|b| note::fr_from_le_bytes(b)).collect();
        let recomputed = zk_core::merkle::root_from_path(commitment, &path, w.index);
        assert!(
            recomputed == root,
            "guest: membership failed (recomputed root != public merkle_root)"
        );

        // [ASP hook — stretch] non-membership of the depositor in the deny-set
        // would go HERE (after membership), using w.asp_path + an asp_root.

        // (3) nullifier (two-step: sign then nullify; secret == note.priv_key).
        let nullifier = note::nullifier(&note, secret);
        nullifiers.push(nullifier);

        // (4) BALANCE CONSERVATION: checked running sum (no value forged at the
        // aggregate level; per-note amount already bound by membership in (2)).
        total = total
            .checked_add(w.amount)
            .expect("guest: balance overflow (sum of amounts exceeds u128)");

        payouts.push(Payout {
            recipient: w.recipient,
            amount: w.amount,
        });
    }

    // `total` is the conserved batch sum, asserted overflow-free above. It is the
    // sum of exactly the membership-verified payout amounts (each payout amount
    // == its note's committed amount), so balance is conserved by construction.
    let _ = total;

    // (5) DISTINCTNESS (AC3.2): the N nullifiers must be pairwise distinct.
    // O(N^2) is fine for the demo's small N and keeps the check explicit and
    // auditable (no hashing/sorting that could mask a subtle equality bug).
    for i in 0..n {
        for j in (i + 1)..n {
            assert!(
                nullifiers[i] != nullifiers[j],
                "guest: duplicate nullifier at indices {i} and {j} (distinctness)"
            );
        }
    }

    // (6) Commit the canonical journal as RAW bytes (no serde framing) so the
    // Soroban contract reads `receipt.journal` as exactly journal::encode(...).
    let journal_bytes = journal::encode(root, &nullifiers, &payouts);
    env::commit_slice(&journal_bytes);
}
