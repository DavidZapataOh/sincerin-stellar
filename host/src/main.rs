//! Host binary — Confidential Payments Rollup.
//!
//! Subcommand `prove` (s1/04 — CONTEXT.md D5 / AC5.1, AC5.2):
//!   1. Load the golden N=2 inputs from `golden/n2_inputs.json`.
//!   2. Build a [`GuestInput`] (field elements as 32-byte LE, matching
//!      `zk_core::witness` / the PoC `scalar_to_bytes`).
//!   3. Execute the guest inside the RISC Zero zkVM and **prove to Groth16/BN254**
//!      (`ProverOpts::groth16()`) — the STARK→SNARK wrap runs the risc0
//!      groth16-prover container, so Docker must be running (on ARM it runs x86
//!      under emulation).
//!   4. Assert `receipt.verify(ROLLUP_GUEST_ID)` locally before serializing.
//!   5. Serialize the artifacts the Soroban verifier consumes:
//!        - `out/receipt/seal.hex`     ← `encode_seal(&receipt)` (selector ‖ proof)
//!        - `out/receipt/image_id.hex` ← 32-byte `ROLLUP_GUEST_ID`
//!        - `out/receipt/journal.bin`  ← `receipt.journal.bytes`
//!                                       (== `zk_core::journal::encode(...)`)
//!
//! **REAL proof, not dev-mode.** A dev-mode (`RISC0_DEV_MODE=1`) "receipt" is a
//! `Fake` inner receipt; `encode_seal` then emits a 4-byte `0xFFFFFFFF` selector
//! plus a 32-byte claim digest (= 36 bytes) instead of a ~260-byte Groth16 seal.
//! We reject that here (the wrap MUST be a `Groth16` inner receipt), so this
//! binary can never silently produce a fake receipt for the on-chain path.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use methods::{ROLLUP_GUEST_ELF, ROLLUP_GUEST_ID};
use risc0_ethereum_contracts::encode_seal;
use risc0_zkvm::{default_prover, Digest, ExecutorEnv, InnerReceipt, ProverOpts, Receipt};
use serde::Deserialize;
use zk_core::witness::{GuestInput, NoteWitness};

/// Path (relative to the workspace root) of the golden N=2 inputs.
const GOLDEN_N2: &str = "golden/n2_inputs.json";
/// Output directory for the serialized receipt artifacts.
const RECEIPT_DIR: &str = "out/receipt";

// ── Golden JSON shape (mirrors host/tests/guest_exec.rs) ──────────────────────
// All field elements are `0x…` 32-byte LITTLE-ENDIAN hex (the PoC
// `scalar_to_bytes` convention, == `zk_core::witness`).

#[derive(Deserialize)]
struct N2Inputs {
    merkle_root_le: String,
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

/// Parse `0x…` 32-byte little-endian hex into `[u8; 32]`.
fn le32(s: &str) -> [u8; 32] {
    let s = s.strip_prefix("0x").unwrap_or(s);
    let v = hex::decode(s).unwrap_or_else(|e| panic!("invalid hex {s:?}: {e}"));
    assert_eq!(v.len(), 32, "expected 32 bytes, got {} from {s:?}", v.len());
    let mut out = [0u8; 32];
    out.copy_from_slice(&v);
    out
}

/// Build the `GuestInput` the guest reads from the golden file. This is the
/// SAME struct, with the SAME 32-byte-LE serialization, that the executor test
/// (`host/tests/guest_exec.rs`) feeds the guest — so the proven journal equals
/// the executor-checked one.
fn guest_input_from_golden(g: &N2Inputs) -> GuestInput {
    let notes = g
        .notes
        .iter()
        .map(|n| NoteWitness {
            secret: le32(&n.secret_le),
            blinding: le32(&n.blinding_le),
            amount: n.amount,
            recipient: le32(&n.recipient),
            path: n.path_le.iter().map(|s| le32(s)).collect(),
            index: n.index,
        })
        .collect();
    GuestInput {
        notes,
        merkle_root: le32(&g.merkle_root_le),
    }
}

/// Resolve a path relative to the workspace root regardless of the CWD `cargo
/// run` is invoked from. `CARGO_MANIFEST_DIR` is `host/`; the workspace root is
/// its parent.
fn workspace_path(rel: &str) -> PathBuf {
    let manifest = env!("CARGO_MANIFEST_DIR");
    Path::new(manifest)
        .parent()
        .expect("host/ has a parent (workspace root)")
        .join(rel)
}

fn run_prove() -> Result<(), String> {
    // ── Guard: a real Groth16 wrap is required; dev-mode is a fake. ───────────
    // `RISC0_DEV_MODE` truthy ("1"/"true") makes default_prover return the dev
    // prover (a Fake receipt). We fail loudly rather than emit a fake seal.
    if let Ok(v) = std::env::var("RISC0_DEV_MODE") {
        let v = v.trim().to_ascii_lowercase();
        if v == "1" || v == "true" || v == "yes" {
            return Err(format!(
                "RISC0_DEV_MODE={v:?} is set — this would produce a FAKE (dev-mode) \
                 receipt, not a real Groth16 proof. Unset it (or set it to 0) and \
                 re-run. The on-chain path requires a real Groth16 seal (AC5.1)."
            ));
        }
    }

    // ── 1. Load golden inputs. ────────────────────────────────────────────────
    let golden = workspace_path(GOLDEN_N2);
    let raw = fs::read_to_string(&golden)
        .map_err(|e| format!("read {}: {e}", golden.display()))?;
    let g: N2Inputs =
        serde_json::from_str(&raw).map_err(|e| format!("parse {}: {e}", golden.display()))?;
    let input = guest_input_from_golden(&g);
    println!(
        "[prove] loaded {} note(s) from {}",
        input.notes.len(),
        golden.display()
    );

    // ── 2. Build the ExecutorEnv with the private witness. ────────────────────
    let env = ExecutorEnv::builder()
        .write(&input)
        .map_err(|e| format!("ExecutorEnv::write(GuestInput): {e}"))?
        .build()
        .map_err(|e| format!("ExecutorEnv::build: {e}"))?;

    // ── 3. Prove → Groth16/BN254 (STARK → SNARK wrap via Docker). ─────────────
    println!(
        "[prove] proving guest to Groth16/BN254 (STARK->SNARK wrap via Docker; \
         first run pulls a multi-GB image and takes several minutes)..."
    );
    let prover = default_prover();
    let prove_info = prover
        .prove_with_opts(env, ROLLUP_GUEST_ELF, &ProverOpts::groth16())
        .map_err(|e| format!("prove_with_opts(groth16): {e}"))?;
    let receipt: Receipt = prove_info.receipt;
    println!(
        "[prove] proving done: {} total cycles",
        prove_info.stats.total_cycles
    );

    // ── 4a. Reject a Fake inner receipt — must be a real Groth16 wrap. ────────
    match &receipt.inner {
        InnerReceipt::Groth16(_) => {}
        other => {
            return Err(format!(
                "expected a Groth16 inner receipt, got {}. This is NOT a real proof \
                 (likely dev-mode/Bonsai mock). Refusing to serialize a fake seal.",
                inner_kind(other)
            ));
        }
    }

    // ── 4b. Verify the receipt against the guest image ID, locally. ───────────
    receipt
        .verify(ROLLUP_GUEST_ID)
        .map_err(|e| format!("receipt.verify(ROLLUP_GUEST_ID) FAILED: {e}"))?;
    println!("[prove] receipt.verify(ROLLUP_GUEST_ID): OK");

    // ── 5. Serialize artifacts. ───────────────────────────────────────────────
    let out_dir = workspace_path(RECEIPT_DIR);
    fs::create_dir_all(&out_dir).map_err(|e| format!("mkdir {}: {e}", out_dir.display()))?;

    // seal.hex ← encode_seal (selector ‖ Groth16 proof) — what the verifier router
    // consumes. For a Groth16 receipt this is 4 (selector) + 256 (proof) = 260 B.
    let seal = encode_seal(&receipt).map_err(|e| format!("encode_seal: {e}"))?;
    if seal.len() <= 64 {
        // A real Groth16 seal is ~260 bytes; a Fake one is 36 (4 + 32). Belt &
        // suspenders on top of the InnerReceipt::Groth16 check above.
        return Err(format!(
            "encode_seal produced only {} bytes — too short to be a Groth16 seal \
             (expected ~260). Refusing to write a fake/short seal.",
            seal.len()
        ));
    }
    let seal_hex = hex::encode(&seal);
    write_artifact(&out_dir.join("seal.hex"), seal_hex.as_bytes())?;

    // image_id.hex ← canonical 32-byte image id (Digest from the [u32;8] words).
    let image_id_bytes = Digest::from(ROLLUP_GUEST_ID);
    let image_id_hex = hex::encode(image_id_bytes.as_bytes());
    write_artifact(&out_dir.join("image_id.hex"), image_id_hex.as_bytes())?;

    // journal.bin ← committed journal bytes == zk_core::journal::encode(...).
    let journal = receipt.journal.bytes.clone();
    write_artifact(&out_dir.join("journal.bin"), &journal)?;

    println!();
    println!("[prove] artifacts written to {}", out_dir.display());
    println!("[prove]   seal.hex      {} bytes ({} hex chars)", seal.len(), seal_hex.len());
    println!("[prove]   image_id.hex  32 bytes  ({image_id_hex})");
    println!("[prove]   journal.bin   {} bytes", journal.len());
    println!(
        "[prove]   seal[0..8]    {}  (first bytes = verifier selector)",
        hex::encode(&seal[..seal.len().min(8)])
    );
    println!("[prove] OK: real Groth16 receipt verified and serialized.");
    Ok(())
}

/// Write `bytes` to `path`, mapping any IO error to a `String`.
fn write_artifact(path: &Path, bytes: &[u8]) -> Result<(), String> {
    fs::write(path, bytes).map_err(|e| format!("write {}: {e}", path.display()))
}

/// Human-readable tag for a non-Groth16 inner receipt (for the error message).
fn inner_kind(inner: &InnerReceipt) -> &'static str {
    match inner {
        InnerReceipt::Composite(_) => "Composite (un-wrapped STARK)",
        InnerReceipt::Succinct(_) => "Succinct (un-wrapped STARK)",
        InnerReceipt::Groth16(_) => "Groth16",
        InnerReceipt::Fake(_) => "Fake (dev-mode)",
        _ => "unknown",
    }
}

fn main() -> ExitCode {
    let cmd = std::env::args().nth(1);
    match cmd.as_deref() {
        Some("prove") => match run_prove() {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("[prove] ERROR: {e}");
                ExitCode::FAILURE
            }
        },
        other => {
            eprintln!(
                "host: unknown subcommand {:?}\n\nusage:\n  cargo run -p host --release -- prove\n\n\
                 `prove` loads {GOLDEN_N2}, proves the rollup guest to Groth16/BN254 \
                 (Docker required), verifies it locally, and writes {RECEIPT_DIR}/\
                 {{seal.hex,image_id.hex,journal.bin}}.",
                other.unwrap_or("<none>")
            );
            ExitCode::FAILURE
        }
    }
}
