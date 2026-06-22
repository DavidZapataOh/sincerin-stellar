//! Unit tests for the sequencer's PURE logic: the state machine, the
//! nullifier lock, batch assembly/trigger, and the collision rebuild.
//!
//! These tests use REAL intents loaded from `golden/n8_inputs.json` so the
//! nullifiers the lock/collision logic compute are byte-identical to what the
//! real prover puts in the journal (the same vectors the N=8 receipt was proven
//! over). The on-chain byte-compat proof (`host execute` on the rebuilt input)
//! lives in the gate (`scripts/seq_demo.sh`); these are the fast in-process
//! invariants.

use super::*;
use crate::types::{Status, WithdrawalIntent};

/// Parse `0x…` 32-byte little-endian hex into `[u8; 32]`.
fn le32(s: &str) -> [u8; 32] {
    let s = s.strip_prefix("0x").unwrap_or(s);
    let v = hex::decode(s).expect("hex");
    assert_eq!(v.len(), 32, "expected 32 bytes from {s:?}");
    let mut out = [0u8; 32];
    out.copy_from_slice(&v);
    out
}

/// Load the 8 REAL intents from `golden/n8_inputs.json`.
fn load_n8_intents() -> Vec<WithdrawalIntent> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../golden/n8_inputs.json");
    let raw = std::fs::read_to_string(path).expect("read n8_inputs.json");
    let v: serde_json::Value = serde_json::from_str(&raw).expect("parse");
    let root = le32(v["merkle_root_le"].as_str().unwrap());
    v["notes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| WithdrawalIntent {
            secret: le32(n["secret_le"].as_str().unwrap()),
            blinding: le32(n["blinding_le"].as_str().unwrap()),
            amount: n["amount"].as_u64().unwrap() as u128,
            recipient: le32(n["recipient"].as_str().unwrap()),
            path: n["path_le"]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| le32(s.as_str().unwrap()))
                .collect(),
            index: n["index"].as_u64().unwrap(),
            merkle_root: root,
        })
        .collect()
}

/// The expected nullifiers (LE) from the n8 fixture — the oracle the lock and
/// collision logic must match.
fn expected_nullifiers() -> Vec<[u8; 32]> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../golden/n8_inputs.json");
    let raw = std::fs::read_to_string(path).unwrap();
    let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
    v["expected_nullifiers_le"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| le32(s.as_str().unwrap()))
        .collect()
}

// ── nullifier derivation matches the fixture oracle ──────────────────────────
#[test]
fn nullifier_of_matches_fixture_oracle() {
    let intents = load_n8_intents();
    let expected = expected_nullifiers();
    for (i, intent) in intents.iter().enumerate() {
        assert_eq!(
            batch::nullifier_of(intent),
            expected[i],
            "nullifier[{i}] must match the fixture oracle (== journal)"
        );
    }
}

// ── state machine: submit returns a pollable id, starts Pending ──────────────
#[test]
fn submit_returns_pending_id() {
    let mut seq = Sequencer::new(8);
    let intents = load_n8_intents();
    let id = seq.submit_withdrawal(intents[0].clone()).unwrap();
    assert_eq!(seq.get_status(id), Some(Status::Pending));
}

#[test]
fn get_status_unknown_id_is_none() {
    let seq = Sequencer::new(8);
    assert_eq!(seq.get_status(RequestId(999)), None);
}

// ── lock: a note cannot enter two batches (lock-by-nullifier) ────────────────
#[test]
fn duplicate_note_is_rejected_by_lock() {
    let mut seq = Sequencer::new(8);
    let intents = load_n8_intents();
    let first = seq.submit_withdrawal(intents[0].clone()).unwrap();
    // Submitting the SAME note again must be rejected (locked), not enqueued.
    let err = seq.submit_withdrawal(intents[0].clone()).unwrap_err();
    assert_eq!(err, SubmitError::AlreadyReserved(first));
    // Only ONE reservation exists.
    assert_eq!(seq.reserved_count(), 1);
    assert_eq!(seq.pending_count(), 1);
}

// ── trigger: a batch assembles only at N ─────────────────────────────────────
#[test]
fn batch_triggers_at_n_not_before() {
    let mut seq = Sequencer::new(8);
    let intents = load_n8_intents();
    for intent in intents.iter().take(7) {
        seq.submit_withdrawal(intent.clone()).unwrap();
    }
    assert!(!seq.ready_to_batch(), "7 < 8: not ready");
    assert!(seq.try_assemble_batch().is_none());

    seq.submit_withdrawal(intents[7].clone()).unwrap();
    assert!(seq.ready_to_batch(), "8 == N: ready");
    let batch = seq.try_assemble_batch().expect("assemble at N");
    assert_eq!(batch.request_ids.len(), 8);
    assert_eq!(batch.merkle_root, intents[0].merkle_root);
}

// ── state machine transitions: Pending → Batched → Proving → Settled ─────────
#[test]
fn state_machine_full_happy_path() {
    let mut seq = Sequencer::new(8);
    let intents = load_n8_intents();
    let ids: Vec<_> = intents
        .iter()
        .map(|i| seq.submit_withdrawal(i.clone()).unwrap())
        .collect();
    for id in &ids {
        assert_eq!(seq.get_status(*id), Some(Status::Pending));
    }

    let batch = seq.try_assemble_batch().unwrap();
    for id in &ids {
        assert_eq!(seq.get_status(*id), Some(Status::Batched));
    }

    seq.mark_proving(&batch);
    for id in &ids {
        assert_eq!(seq.get_status(*id), Some(Status::Proving));
    }

    seq.mark_settled(&batch, "deadbeef");
    for id in &ids {
        assert_eq!(
            seq.get_status(*id),
            Some(Status::Settled {
                tx_hash: "deadbeef".into()
            })
        );
    }
    // Locks released after settle.
    assert_eq!(seq.reserved_count(), 0);
}

#[test]
fn settled_status_is_terminal() {
    let s = Status::Settled {
        tx_hash: "x".into(),
    };
    assert!(s.is_terminal());
    assert!(!Status::Pending.is_terminal());
}

// ── collision rebuild (plan §5): drop spent, re-queue, rebuild N−1 ───────────
#[test]
fn collision_drops_spent_note_and_rebuilds_n_minus_1() {
    let mut seq = Sequencer::new(8);
    let intents = load_n8_intents();
    let ids: Vec<_> = intents
        .iter()
        .map(|i| seq.submit_withdrawal(i.clone()).unwrap())
        .collect();
    let batch = seq.try_assemble_batch().unwrap();

    // Pretend note index 3's nullifier is already spent on-chain.
    let spent_nf = batch::nullifier_of(&intents[3]);
    let outcome = seq.handle_collision(&batch, &[spent_nf]);

    // (e) dropped note returns to Pending, exactly one, no dup.
    assert_eq!(outcome.dropped, vec![ids[3]]);
    assert_eq!(seq.get_status(ids[3]), Some(Status::Pending));

    // rebuilt batch has the 7 survivors, same root.
    let rebuilt = outcome.rebuilt.expect("survivors remain");
    assert_eq!(rebuilt.request_ids.len(), 7);
    assert_eq!(rebuilt.merkle_root, batch.merkle_root, "(a) root UNCHANGED");
    assert!(!rebuilt.request_ids.contains(&ids[3]), "dropped excluded");

    // (c) the rebuilt GuestInput has exactly the survivors; the dropped
    // nullifier does NOT appear among the rebuilt notes.
    let gi = rebuilt.guest_input();
    assert_eq!(gi.notes.len(), 7);
    assert_eq!(gi.merkle_root, batch.merkle_root);
    for w in &gi.notes {
        let intent = WithdrawalIntent {
            secret: w.secret,
            blinding: w.blinding,
            amount: w.amount,
            recipient: w.recipient,
            path: w.path.clone(),
            index: w.index,
            merkle_root: gi.merkle_root,
        };
        assert_ne!(
            batch::nullifier_of(&intent),
            spent_nf,
            "(c) dropped nullifier must NOT appear in the rebuilt input"
        );
    }
    // (b) survivors' paths still recompute the SAME root (membership holds).
    // Checked rigorously by `host execute` in the gate; here we assert the root
    // field is preserved, which the executor then validates.
}

#[test]
fn collision_no_survivors_returns_none() {
    let mut seq = Sequencer::new(2);
    let intents = load_n8_intents();
    seq.submit_withdrawal(intents[0].clone()).unwrap();
    seq.submit_withdrawal(intents[1].clone()).unwrap();
    let batch = seq.try_assemble_batch().unwrap();
    let all_spent = vec![
        batch::nullifier_of(&intents[0]),
        batch::nullifier_of(&intents[1]),
    ];
    let outcome = seq.handle_collision(&batch, &all_spent);
    assert!(outcome.rebuilt.is_none(), "no survivors → None");
    assert_eq!(outcome.dropped.len(), 2);
}

#[test]
fn collision_no_spent_is_noop_rebuild() {
    let mut seq = Sequencer::new(8);
    let intents = load_n8_intents();
    for i in &intents {
        seq.submit_withdrawal(i.clone()).unwrap();
    }
    let batch = seq.try_assemble_batch().unwrap();
    // Nothing spent → all 8 survive, none dropped.
    let outcome = seq.handle_collision(&batch, &[]);
    assert!(outcome.dropped.is_empty());
    assert_eq!(outcome.rebuilt.unwrap().request_ids.len(), 8);
}

// ── GuestInput byte-shape: the inputs JSON we emit round-trips the witness ────
#[test]
fn guest_input_json_has_all_notes_and_root() {
    let intents = load_n8_intents();
    let gi = batch::guest_input_of(&intents);
    let json = batch::inputs_file_json(&gi);
    let v: serde_json::Value = serde_json::from_str(&json).expect("emitted JSON parses");
    assert_eq!(v["n"].as_u64().unwrap(), 8);
    assert_eq!(
        le32(v["merkle_root_le"].as_str().unwrap()),
        intents[0].merkle_root
    );
    assert_eq!(v["notes"].as_array().unwrap().len(), 8);
    // secret/path round-trip for note 0.
    assert_eq!(
        le32(v["notes"][0]["secret_le"].as_str().unwrap()),
        intents[0].secret
    );
    assert_eq!(v["notes"][0]["path_le"].as_array().unwrap().len(), 3);
}
