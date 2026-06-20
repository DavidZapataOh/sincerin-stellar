//! s1/03 — Guest validity logic, exercised via the RISC Zero **executor**
//! (no proving → fast). Realizes CONTEXT.md D1/AC1.1 + D3/AC3.1, AC3.2.
//!
//! TDD gate (`plans/s1/03-guest-n2-logic.md`):
//!   - valid N=2 case → committed journal == expected bytes (encode());
//!   - three invalid cases each PANIC:
//!     (a) bad merkle path, (b) duplicate nullifier, (c) broken balance.
//!
//! The executor runs the guest natively (re-execution of validity in Rust);
//! a guest `panic!`/`assert!` surfaces here as an `Err` from `execute`, which
//! these tests assert on. No SNARK is verified in-zkVM (zero pairings).

use methods::{ROLLUP_GUEST_ELF, ROLLUP_GUEST_ID};
use risc0_zkvm::{default_executor, ExecutorEnv};
use zk_core::journal::{self, Payout};
use zk_core::note::{self, Note};
use zk_core::poseidon2::Fr;
use zk_core::witness::{GuestInput, NoteWitness};

use serde::Deserialize;

/// Pool-wide Merkle tree depth (must match `methods/guest/src/main.rs::TREE_DEPTH`
/// and the depth of the `golden/n2_inputs.json` tree — every note's `path_le` has
/// exactly this many siblings). Pinned here so the SEC-Critical regression tests
/// reference the same invariant the guest enforces.
const TREE_DEPTH: usize = 3;

// ── Canonical N=2 inputs (golden/n2_inputs.json) ──────────────────────────────
// The valid-path deliverable for this task. note0 (leaf 0) + note1 (leaf 1) are
// real PoC-derived vectors in the SAME depth-3 tree, so membership holds against
// `merkle_root_le`. `expected_nullifiers_le` are the frozen PoC nullifiers — the
// host oracle (`expected_journal`) recomputes them and we assert equality, so a
// drift in either the guest OR the golden file is caught.

#[derive(Deserialize)]
struct N2Inputs {
    merkle_root_le: String,
    expected_nullifiers_le: Vec<String>,
    notes: Vec<JsonNote>,
}

#[derive(Deserialize)]
struct JsonNote {
    secret_le: String,
    blinding_le: String,
    amount: u128,
    recipient: String,
    index: u64,
    path_le: Vec<String>,
}

fn load_n2_inputs() -> N2Inputs {
    // CARGO_MANIFEST_DIR = host/ ; golden/ is a sibling at the workspace root.
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../golden/n2_inputs.json");
    let raw =
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    serde_json::from_str(&raw).expect("parse n2_inputs.json")
}

/// Parse `0x…` 32-byte little-endian hex into `[u8;32]`.
fn le32(s: &str) -> [u8; 32] {
    let s = s.strip_prefix("0x").unwrap_or(s);
    let v = hex::decode(s).expect("hex");
    assert_eq!(v.len(), 32, "expected 32 bytes, got {}", v.len());
    let mut out = [0u8; 32];
    out.copy_from_slice(&v);
    out
}

/// Build a `NoteWitness` (transport bytes, LE) from a JSON note.
fn witness_from(n: &JsonNote) -> NoteWitness {
    NoteWitness {
        secret: le32(&n.secret_le),
        blinding: le32(&n.blinding_le),
        amount: n.amount,
        recipient: le32(&n.recipient),
        path: n.path_le.iter().map(|s| le32(s)).collect(),
        index: n.index,
    }
}

/// The two valid witnesses from the golden file, in order.
fn valid_witnesses(g: &N2Inputs) -> Vec<NoteWitness> {
    assert_eq!(g.notes.len(), 2, "n2_inputs.json must hold exactly 2 notes");
    g.notes.iter().map(witness_from).collect()
}

/// Run the guest through the executor; return the committed journal bytes,
/// or the executor error (guest panic) on failure.
fn run_guest(input: &GuestInput) -> Result<Vec<u8>, String> {
    let env = ExecutorEnv::builder()
        .write(input)
        .map_err(|e| e.to_string())?
        .build()
        .map_err(|e| e.to_string())?;
    let session = default_executor()
        .execute(env, ROLLUP_GUEST_ELF)
        .map_err(|e| e.to_string())?;
    Ok(session.journal.bytes)
}

/// Recompute the expected journal independently of the guest (host-side oracle),
/// using the same zk-core primitives the guest uses.
fn expected_journal(witnesses: &[NoteWitness], root: Fr) -> Vec<u8> {
    let mut nullifiers = Vec::new();
    let mut payouts = Vec::new();
    for w in witnesses {
        let note = Note {
            amount: Fr::from(w.amount),
            priv_key: note::fr_from_le_bytes(&w.secret),
            blinding: note::fr_from_le_bytes(&w.blinding),
            leaf_index: w.index,
        };
        let secret = note::fr_from_le_bytes(&w.secret);
        nullifiers.push(note::nullifier(&note, secret));
        payouts.push(Payout { recipient: w.recipient, amount: w.amount });
    }
    journal::encode(root, &nullifiers, &payouts)
}

// ─────────────────────────────────────────────────────────────────────────────
// VALID N=2: committed journal == expected (AC3.1 layout + AC3.2 checks pass).
// ─────────────────────────────────────────────────────────────────────────────
#[test]
fn valid_n2_commits_expected_journal() {
    let g = load_n2_inputs();
    let root = note::fr_from_le_bytes(&le32(&g.merkle_root_le));
    let witnesses = valid_witnesses(&g);

    let input = GuestInput {
        notes: witnesses.clone(),
        merkle_root: le32(&g.merkle_root_le),
    };

    let journal_bytes = run_guest(&input).expect("valid N=2 must execute, not panic");
    let expected = expected_journal(&witnesses, root);

    assert_eq!(
        journal_bytes, expected,
        "committed journal must equal encode(root, nullifiers, payouts)"
    );

    // Cross-check: the journal must decode back to the same root + the golden
    // PoC nullifiers, proving (a) the canonical layout round-trips (s2 mirror)
    // and (b) the guest-derived nullifiers equal the frozen PoC values.
    let decoded = journal::decode(&journal_bytes).expect("journal must decode");
    assert_eq!(note::fr_to_le_bytes(&decoded.root), le32(&g.merkle_root_le));
    assert_eq!(decoded.nullifiers.len(), 2);
    for (i, exp_nf) in g.expected_nullifiers_le.iter().enumerate() {
        assert_eq!(
            note::fr_to_le_bytes(&decoded.nullifiers[i]),
            le32(exp_nf),
            "nullifier[{i}] must match the frozen PoC value"
        );
    }
    assert_eq!(decoded.payouts[0].amount, g.notes[0].amount);
    assert_eq!(decoded.payouts[1].amount, g.notes[1].amount);
    assert_eq!(decoded.payouts[0].recipient, le32(&g.notes[0].recipient));
    assert_eq!(decoded.payouts[1].recipient, le32(&g.notes[1].recipient));
}

// ─────────────────────────────────────────────────────────────────────────────
// INVALID (a): bad merkle path → membership recompute ≠ public root → PANIC.
// ─────────────────────────────────────────────────────────────────────────────
#[test]
fn invalid_bad_merkle_path_panics() {
    let g = load_n2_inputs();
    let mut w = valid_witnesses(&g);

    // Corrupt one sibling of note0's path: recomputed root won't match.
    w[0].path[0][0] ^= 0x01;

    let input = GuestInput {
        notes: w,
        merkle_root: le32(&g.merkle_root_le),
    };

    let res = run_guest(&input);
    assert!(
        res.is_err(),
        "bad merkle path must make the guest panic (membership assert), got Ok"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// INVALID (b): duplicate nullifier → distinctness assert → PANIC.
// Two identical notes derive the SAME nullifier (same secret/commitment/path).
// ─────────────────────────────────────────────────────────────────────────────
#[test]
fn invalid_duplicate_nullifier_panics() {
    let g = load_n2_inputs();
    let w0 = witness_from(&g.notes[0]);
    // Two copies of note0 → identical nullifier → AC3.2 distinctness violated.
    // Both copies have a valid membership path (same leaf), so the ONLY thing
    // that trips is the pairwise-distinct check.
    let input = GuestInput {
        notes: vec![w0.clone(), w0],
        merkle_root: le32(&g.merkle_root_le),
    };

    let res = run_guest(&input);
    assert!(
        res.is_err(),
        "duplicate nullifier must make the guest panic (distinctness assert), got Ok"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// INVALID (c): broken balance → committed amount ≠ membership-verified note
// amount. We tamper the witness `amount` so the recomputed commitment no longer
// matches the leaf under the root → membership FAILS (value is bound to the
// commitment; you cannot forge an amount without breaking membership). PANIC.
// ─────────────────────────────────────────────────────────────────────────────
#[test]
fn invalid_broken_balance_panics() {
    let g = load_n2_inputs();
    let mut w = valid_witnesses(&g);

    // Inflate note0's amount: commitment changes → membership recompute ≠ root.
    // (Balance is bound to the membership-verified commitment; value cannot be
    // forged without breaking membership.)
    w[0].amount += 1;

    let input = GuestInput {
        notes: w,
        merkle_root: le32(&g.merkle_root_le),
    };

    let res = run_guest(&input);
    assert!(
        res.is_err(),
        "forged amount must make the guest panic (commitment/membership bind), got Ok"
    );
}

// Touch the image ID so an accidental ELF/ID mismatch is loud, not silent.
#[test]
fn guest_id_is_present() {
    assert_eq!(ROLLUP_GUEST_ID.len(), 8, "image id is 8 u32 words (32 bytes)");
}

// ═════════════════════════════════════════════════════════════════════════════
// SEC Critical + Important — `index` / `path.len()` witness-trust boundary.
//
// ROOT CAUSE: `merkle::root_from_path` orients each level with the LOW
// `path.len()` bits of `index`, but the nullifier derives from `Fr::from(index)`
// over the FULL u64. So a note genuinely at leaf `p` (depth d = TREE_DEPTH)
// passes membership for EVERY `index ∈ {p, p+2^d, p+2·2^d, …}` (identical low
// bits ⇒ identical orientation) while `Fr::from(index)` differs each time ⇒ the
// SAME committed note mints unlimited DISTINCT valid nullifiers ⇒ unbounded
// double-spend the contract cannot catch (the nullifiers look genuinely
// distinct). The fix pins `path.len() == TREE_DEPTH` AND `index < 2^TREE_DEPTH`.
// ═════════════════════════════════════════════════════════════════════════════

/// Host-side oracle nullifier for a witness whose `index` field is overridden,
/// using the SAME zk-core primitives the guest uses. Demonstrates the
/// malleability (membership-orientation vs `Fr::from(index)` divergence) at the
/// crypto layer — independent of the executor.
fn nullifier_for_index(w: &NoteWitness, index: u64) -> Fr {
    let note = Note {
        amount: Fr::from(w.amount),
        priv_key: note::fr_from_le_bytes(&w.secret),
        blinding: note::fr_from_le_bytes(&w.blinding),
        leaf_index: index,
    };
    note::nullifier(&note, note::fr_from_le_bytes(&w.secret))
}

/// MALLEABILITY WITNESS (documents the hole at the crypto layer; not an executor
/// run). note0 is at leaf 0, depth TREE_DEPTH=3 ⇒ indices 0, 8, 16 all share the
/// low-3 bits (000) ⇒ all pass membership against the published root, yet each
/// yields a DIFFERENT nullifier. This is the unbounded double-spend: one
/// committed note, many valid-looking nullifiers. (The two *_panics tests below
/// prove the guest now rejects the out-of-range indices that enable this.)
#[test]
fn malleability_distinct_nullifiers_for_congruent_indices() {
    let g = load_n2_inputs();
    let root = note::fr_from_le_bytes(&le32(&g.merkle_root_le));
    let w0 = witness_from(&g.notes[0]);
    assert_eq!(w0.index, 0, "golden note0 is at leaf 0");
    assert_eq!(w0.path.len(), TREE_DEPTH, "golden tree depth is TREE_DEPTH");

    let commitment = {
        let note = Note {
            amount: Fr::from(w0.amount),
            priv_key: note::fr_from_le_bytes(&w0.secret),
            blinding: note::fr_from_le_bytes(&w0.blinding),
            leaf_index: w0.index,
        };
        note::commitment(&note)
    };
    let path: Vec<Fr> = w0.path.iter().map(note::fr_from_le_bytes).collect();

    let stride = 1u64 << TREE_DEPTH; // = 8
    let indices = [0u64, stride, 2 * stride]; // 0, 8, 16 — congruent mod 2^depth
    let mut nfs = Vec::new();
    for &idx in &indices {
        // Every congruent index passes membership (orientation uses only low bits)…
        let recomputed = zk_core::merkle::root_from_path(commitment, &path, idx);
        assert_eq!(
            recomputed, root,
            "index {idx} must pass membership (congruent low bits) — this IS the hole"
        );
        // …yet produces a DISTINCT nullifier (Fr::from uses the FULL u64).
        nfs.push(nullifier_for_index(&w0, idx));
    }
    assert_ne!(nfs[0], nfs[1], "indices 0 and 8 must differ (malleability)");
    assert_ne!(nfs[0], nfs[2], "indices 0 and 16 must differ (malleability)");
    assert_ne!(nfs[1], nfs[2], "indices 8 and 16 must differ (malleability)");
}

// ─────────────────────────────────────────────────────────────────────────────
// SEC Critical: out-of-range `index` (identical low bits ⇒ still passes
// membership today) MUST panic. RED before the fix (no panic — the nullifier is
// silently malleable, the double-spend), GREEN after the `index < 2^TREE_DEPTH`
// assert.
// ─────────────────────────────────────────────────────────────────────────────
#[test]
fn invalid_out_of_range_index_panics() {
    let g = load_n2_inputs();
    let mut w = valid_witnesses(&g);

    // note0 is at leaf p=0, depth TREE_DEPTH. Set index = p + 2^TREE_DEPTH: the
    // low TREE_DEPTH bits are unchanged, so membership orientation is identical
    // and it STILL passes membership — but Fr::from(index) differs ⇒ a different
    // nullifier. This is the double-spend the guest must now reject.
    assert_eq!(w[0].index, 0, "precondition: note0 at leaf 0");
    // leaf p + 2^TREE_DEPTH: identical low TREE_DEPTH bits ⇒ still passes
    // membership, but Fr::from(index) (and thus the nullifier) differs.
    w[0].index += 1u64 << TREE_DEPTH; // 0 + 8 = 8

    let input = GuestInput {
        notes: w,
        merkle_root: le32(&g.merkle_root_le),
    };

    let res = run_guest(&input);
    assert!(
        res.is_err(),
        "out-of-range index (>= 2^TREE_DEPTH) must make the guest panic; got Ok \
         — the nullifier is malleable (unbounded double-spend)"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// SEC Important: a Merkle path whose length != TREE_DEPTH MUST panic. A
// length-0 path makes `root_from_path` return the commitment unchanged, so any
// value equal to the published root "passes membership" — the guest must pin the
// path to exactly pool depth. RED before the fix, GREEN after the
// `path.len() == TREE_DEPTH` assert.
// ─────────────────────────────────────────────────────────────────────────────
#[test]
fn invalid_wrong_path_length_panics() {
    let g = load_n2_inputs();
    let mut w = valid_witnesses(&g);

    // Drop the last sibling → path.len() == TREE_DEPTH - 1 (wrong depth).
    let truncated: Vec<[u8; 32]> = w[0].path[..TREE_DEPTH - 1].to_vec();
    assert_ne!(truncated.len(), TREE_DEPTH, "precondition: wrong path length");
    w[0].path = truncated;

    let input = GuestInput {
        notes: w,
        merkle_root: le32(&g.merkle_root_le),
    };

    let res = run_guest(&input);
    assert!(
        res.is_err(),
        "path.len() != TREE_DEPTH must make the guest panic (depth not pinned); got Ok"
    );
}
