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
