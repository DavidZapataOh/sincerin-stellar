//! PROVING-ONLY bench guest — Confidential Payments Rollup validity check at a
//! compile-time-selected tree depth (3/4/5). **NEVER deployed or settled.**
//!
//! This is a faithful copy of the DEPLOYED guest (`methods/guest/src/main.rs`),
//! differing ONLY in that `TREE_DEPTH` is selected by a Cargo feature instead of
//! a hardcoded `3`. It exists so the proving-time benchmark can prove N=16
//! (depth 4) and N=32 (depth 5) without touching the deployed guest's source —
//! whose image_id (`cbeab7aa…`) is bound on-chain and must stay byte-identical
//! (risc0's image_id is ELF-derived and span-sensitive, so any source edit to the
//! deployed guest would change it; see methods/guest-bench/Cargo.toml).
//!
//! The validity logic, crypto (`zk-core`, byte-identical to the PoC) and the
//! SEC-critical witness-trust asserts are IDENTICAL to the deployed guest at
//! every depth — only the pinned `TREE_DEPTH` changes. A depth-3 bench build
//! (no feature) therefore exercises the SAME validity as the deployed guest
//! (its image_id differs only because it is a different crate/binary).

use risc0_zkvm::guest::env;

use zk_core::journal::{self, Payout};
use zk_core::note::{self, Note};
use zk_core::poseidon2::Fr;
use zk_core::witness::GuestInput;

/// Pool-wide Merkle tree depth, selected at compile time by a feature.
/// default (no feature) = 3 (mirrors the deployed guest); `td4` = 4 (N=16);
/// `td5` = 5 (N=32). Exactly one (or none) may be active.
#[cfg(not(any(feature = "td4", feature = "td5")))]
const TREE_DEPTH: usize = 3;
#[cfg(all(feature = "td4", not(feature = "td5")))]
const TREE_DEPTH: usize = 4;
#[cfg(feature = "td5")]
const TREE_DEPTH: usize = 5;

#[cfg(all(feature = "td4", feature = "td5"))]
compile_error!("bench guest: features `td4` and `td5` are mutually exclusive — pick one tree depth");

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
