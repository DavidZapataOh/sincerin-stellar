//! Orchestration demo driver for the gate (`scripts/seq_demo.sh`).
//!
//! **Built ONLY with `--features test-fixture`** (`required-features` in
//! `Cargo.toml`) — a plain `cargo build` skips it, so the `FixtureProver` it uses
//! is structurally unreachable from production (LOCK 1).
//!
//! This binary exercises the sequencer's ORCHESTRATION with REAL artifacts:
//!   - `orchestrate`: submit the 8 N=8 intents, assert the state machine + the
//!     nullifier lock + the batch trigger at N, run the `FixtureProver` (which
//!     LOADS the real `out/bench/n8/` receipt — never hand-built), and emit the
//!     settle args (seal/image_id/journal hex) + the 8 nullifiers for the bash
//!     gate to drive the REAL on-chain `settle_batch` against the REAL verifier.
//!   - `collision --spent <nf_le,…>`: given the nullifiers that are now `is_spent`
//!     ON-CHAIN (the bash gate queries them after the settle), drop the spent
//!     notes, re-queue them (→ Pending), rebuild the N−1 `GuestInput`, and write
//!     it as an inputs file. The gate runs `host execute` on that file to PROVE
//!     it is byte-compatible (LOCK 3): if the executor accepts it, the real prover
//!     would too.
//!
//! **HONEST LABEL (LOCK 2):** this certifies ORCHESTRATION (state machine, lock,
//! trigger, collision) + a real on-chain settle of a real proof. It does NOT
//! certify that the sequencer GENERATES proofs (that is s3 — a real prove through
//! the sequencer on GPU). The fixture only replaces the 4-hour prove with the
//! already-generated real receipt.

use std::path::PathBuf;

use sequencer::batch;
use sequencer::prover::{FixtureProver, Prover};
use sequencer::types::{Status, WithdrawalIntent};
use sequencer::Sequencer;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("sequencer/ has a parent (workspace root)")
        .to_path_buf()
}

fn le32(s: &str) -> [u8; 32] {
    let s = s.strip_prefix("0x").unwrap_or(s);
    let v = hex::decode(s).unwrap_or_else(|e| panic!("bad hex {s:?}: {e}"));
    assert_eq!(v.len(), 32, "expected 32 bytes from {s:?}");
    let mut out = [0u8; 32];
    out.copy_from_slice(&v);
    out
}

/// Load the 8 REAL N=8 intents from `golden/n8_inputs.json`.
fn load_n8_intents() -> Vec<WithdrawalIntent> {
    let path = workspace_root().join("golden/n8_inputs.json");
    let raw =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let v: serde_json::Value = serde_json::from_str(&raw).expect("parse n8_inputs.json");
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

/// `orchestrate --out-dir <d>`: full happy-path orchestration with the REAL
/// fixture receipt. Asserts the state machine + lock + trigger, then writes the
/// settle args for the bash gate. Exits non-zero on any failed assertion.
fn run_orchestrate(out_dir: &str) -> Result<(), String> {
    let out = PathBuf::from(out_dir);
    std::fs::create_dir_all(&out).map_err(|e| format!("mkdir {out_dir}: {e}"))?;

    let intents = load_n8_intents();
    let mut seq = Sequencer::new(8);

    println!("[orchestrate] === state machine + lock + trigger (real intents) ===");

    // ── submit 8 → each returns a pollable id, starts Pending ────────────────
    let mut ids = Vec::new();
    for (i, intent) in intents.iter().enumerate() {
        let id = seq
            .submit_withdrawal(intent.clone())
            .map_err(|e| format!("submit[{i}] rejected: {e}"))?;
        let st = seq.get_status(id);
        if st != Some(Status::Pending) {
            return Err(format!("submit[{i}] status {st:?} != Pending"));
        }
        println!("[orchestrate]   submit note{i} → {id} (Pending)");
        ids.push(id);
    }

    // ── LOCK: re-submitting note0 is rejected (cannot enter two batches) ─────
    match seq.submit_withdrawal(intents[0].clone()) {
        Err(sequencer::SubmitError::AlreadyReserved(holder)) => {
            if holder != ids[0] {
                return Err(format!("lock: rejected by {holder}, expected {}", ids[0]));
            }
            println!("[orchestrate]   LOCK OK: re-submit of note0 rejected (held by {holder})");
        }
        other => {
            return Err(format!(
                "lock FAILED: re-submit returned {other:?}, expected AlreadyReserved"
            ))
        }
    }
    if seq.reserved_count() != 8 || seq.pending_count() != 8 {
        return Err(format!(
            "lock: reserved {} pending {} (expected 8/8 — no duplicate enqueued)",
            seq.reserved_count(),
            seq.pending_count()
        ));
    }

    // ── TRIGGER: a batch assembles exactly at N=8 ────────────────────────────
    if !seq.ready_to_batch() {
        return Err("trigger: not ready at N=8".into());
    }
    let batch = seq
        .try_assemble_batch()
        .ok_or("trigger: try_assemble_batch returned None at N=8")?;
    if batch.request_ids.len() != 8 {
        return Err(format!(
            "trigger: batch has {} notes, expected 8",
            batch.request_ids.len()
        ));
    }
    for id in &batch.request_ids {
        if seq.get_status(*id) != Some(Status::Batched) {
            return Err(format!("trigger: {id} not Batched"));
        }
    }
    println!("[orchestrate]   TRIGGER OK: batch of 8 assembled (all → Batched)");

    // ── PROVE (fixture = REAL N=8 receipt, LOADED from disk) ─────────────────
    seq.mark_proving(&batch);
    for id in &batch.request_ids {
        if seq.get_status(*id) != Some(Status::Proving) {
            return Err(format!("{id} not Proving"));
        }
    }
    let gi = batch.guest_input();
    let receipt_dir = workspace_root().join("out/bench/n8");
    let prover = FixtureProver::new(&receipt_dir);
    println!("[orchestrate]   prover backend: {}", prover.backend_label());
    let proved = futures_block_on(prover.prove(gi))
        .map_err(|e| format!("fixture prove (load real receipt) failed: {e}"))?;

    // The fixture receipt's journal must equal the on-disk N=8 journal (sanity:
    // it really loaded the real file, not something fabricated).
    let real_journal = std::fs::read(receipt_dir.join("journal.bin"))
        .map_err(|e| format!("read real journal: {e}"))?;
    if proved.journal != real_journal {
        return Err("fixture journal != on-disk N=8 journal (NOT the real receipt)".into());
    }
    println!(
        "[orchestrate]   PROVE OK: loaded real receipt (seal {} B, journal {} B, image_id {})",
        proved.seal.len(),
        proved.journal.len(),
        &hex::encode(proved.image_id)[..12]
    );

    // ── emit settle args + the 8 nullifiers for the bash gate ────────────────
    std::fs::write(out.join("seal.hex"), hex::encode(&proved.seal))
        .map_err(|e| format!("write seal.hex: {e}"))?;
    std::fs::write(out.join("image_id.hex"), hex::encode(proved.image_id))
        .map_err(|e| format!("write image_id.hex: {e}"))?;
    std::fs::write(out.join("journal.bin"), &proved.journal)
        .map_err(|e| format!("write journal.bin: {e}"))?;

    // Nullifiers (LE hex, one per line) the gate uses to query on-chain is_spent.
    let mut nf_lines = String::new();
    for intent in &intents {
        nf_lines.push_str(&format!("0x{}\n", hex::encode(batch::nullifier_of(intent))));
    }
    std::fs::write(out.join("nullifiers.txt"), nf_lines)
        .map_err(|e| format!("write nullifiers.txt: {e}"))?;

    println!("[orchestrate]   wrote settle args + nullifiers to {out_dir}");
    println!("[orchestrate] ORCHESTRATE_OK");
    Ok(())
}

/// `collision --spent <nf_le,…> --out-rebuilt <f>`: drive the collision rebuild.
/// The bash gate passes the nullifiers that are NOW `is_spent` on-chain (queried
/// after the happy-path settle). We reconstruct the same batch, run
/// `handle_collision`, assert the state transitions, and write the rebuilt N−k
/// `GuestInput` as an inputs file for `host execute` (LOCK 3 byte-compat proof).
fn run_collision(spent_csv: &str, out_rebuilt: &str) -> Result<(), String> {
    let intents = load_n8_intents();
    let mut seq = Sequencer::new(8);
    let ids: Vec<_> = intents
        .iter()
        .map(|i| seq.submit_withdrawal(i.clone()).expect("submit"))
        .collect();
    let batch = seq.try_assemble_batch().ok_or("assemble at N=8")?;
    seq.mark_proving(&batch);

    let spent: Vec<[u8; 32]> = spent_csv
        .split(',')
        .filter(|s| !s.trim().is_empty())
        .map(|s| le32(s.trim()))
        .collect();
    if spent.is_empty() {
        return Err(
            "collision: --spent had no nullifiers (the gate must pass the on-chain is_spent set)"
                .into(),
        );
    }
    println!(
        "[collision] {} nullifier(s) reported is_spent ON-CHAIN; dropping + rebuilding",
        spent.len()
    );

    let outcome = seq.handle_collision(&batch, &spent);

    // (e) each dropped request returned to Pending exactly once (no loss/dup).
    let n_dropped = outcome.dropped.len();
    if n_dropped != spent.len() {
        return Err(format!(
            "collision: dropped {n_dropped} but {} nullifiers were spent",
            spent.len()
        ));
    }
    for id in &outcome.dropped {
        if seq.get_status(*id) != Some(Status::Pending) {
            return Err(format!("collision: dropped {id} not back to Pending"));
        }
    }
    // No duplicates among dropped.
    let mut seen = std::collections::BTreeSet::new();
    for id in &outcome.dropped {
        if !seen.insert(*id) {
            return Err(format!("collision: {id} dropped twice (duplicate)"));
        }
    }
    println!(
        "[collision]   STATE OK: {n_dropped} dropped → Pending (no loss/dup); survivors stay Batched"
    );

    // (a) rebuilt batch keeps the SAME root; survivors only.
    let rebuilt = outcome
        .rebuilt
        .ok_or("collision: no survivors (gate spent every note — pass a strict subset)")?;
    if rebuilt.merkle_root != batch.merkle_root {
        return Err("collision: rebuilt root CHANGED (must equal the pool root)".into());
    }
    let expected_survivors = 8 - spent.len();
    if rebuilt.request_ids.len() != expected_survivors {
        return Err(format!(
            "collision: rebuilt has {} notes, expected {expected_survivors}",
            rebuilt.request_ids.len()
        ));
    }
    // dropped ids excluded from the rebuilt batch.
    for d in &outcome.dropped {
        if rebuilt.request_ids.contains(d) {
            return Err(format!("collision: dropped {d} still in rebuilt batch"));
        }
        let _ = ids; // ids consumed only for submit; silence unused on some paths.
    }

    // (c) the dropped nullifiers do NOT appear in the rebuilt input.
    let gi = rebuilt.guest_input();
    let spent_set: std::collections::BTreeSet<[u8; 32]> = spent.iter().copied().collect();
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
        if spent_set.contains(&batch::nullifier_of(&intent)) {
            return Err("collision: a dropped nullifier reappears in the rebuilt input".into());
        }
    }
    println!(
        "[collision]   REBUILD OK: N−{} = {} survivors, root UNCHANGED, dropped nullifier(s) absent",
        spent.len(),
        expected_survivors
    );

    // Write the rebuilt inputs file for `host execute` (LOCK 3 byte-compat).
    let json = batch::inputs_file_json(&gi);
    let out_path = PathBuf::from(out_rebuilt);
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
    }
    std::fs::write(&out_path, json).map_err(|e| format!("write rebuilt inputs: {e}"))?;
    println!(
        "[collision]   wrote rebuilt N−{} inputs → {out_rebuilt}",
        spent.len()
    );
    println!(
        "[collision]   (the gate now runs `host execute` on this to PROVE byte-compat — lock 3)"
    );
    println!("[collision] COLLISION_OK");
    Ok(())
}

/// Minimal block_on for the single async call (`Prover::prove`) without pulling a
/// full async runtime. The fixture prove is synchronous I/O wrapped in async, so
/// the future resolves on the first poll; we drive it with a no-op waker via
/// `Box::pin` (no `unsafe` needed at the call site).
fn futures_block_on<F: std::future::Future>(fut: F) -> F::Output {
    use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

    fn noop_raw_waker() -> RawWaker {
        fn no_op(_: *const ()) {}
        fn clone(_: *const ()) -> RawWaker {
            noop_raw_waker()
        }
        let vtable = &RawWakerVTable::new(clone, no_op, no_op, no_op);
        RawWaker::new(std::ptr::null(), vtable)
    }
    // The no-op waker never reads the (null) data pointer; `Waker::from_raw` is
    // the only unsafe op and is encapsulated here.
    #[allow(unsafe_code)]
    let waker = unsafe { Waker::from_raw(noop_raw_waker()) };
    let mut cx = Context::from_waker(&waker);
    let mut fut = Box::pin(fut);
    loop {
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(v) => return v,
            // The fixture prove resolves immediately (sync I/O); we never park.
            Poll::Pending => continue,
        }
    }
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().map(String::as_str);
    let rest = if args.is_empty() { &[][..] } else { &args[1..] };

    let result = match cmd {
        Some("orchestrate") => {
            let out_dir = flag(rest, "--out-dir").unwrap_or_else(|| "out/seq/demo".to_string());
            run_orchestrate(&out_dir)
        }
        Some("collision") => {
            let spent = flag(rest, "--spent").unwrap_or_default();
            let out = flag(rest, "--out-rebuilt")
                .unwrap_or_else(|| "out/seq/rebuilt_inputs.json".to_string());
            run_collision(&spent, &out)
        }
        other => Err(format!(
            "usage: seq_demo orchestrate --out-dir <d> | seq_demo collision --spent <nf,…> --out-rebuilt <f>\n  got {other:?}"
        )),
    };

    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[seq_demo] ASSERTION FAILED: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}
