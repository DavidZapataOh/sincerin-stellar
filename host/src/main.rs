//! Host binary — Confidential Payments Rollup.
//!
//! Subcommands:
//!   - `prove   [--inputs <json>] [--out <dir>]` (s1/04, s2/04 — D5/AC5.1,AC5.2):
//!     load the inputs (default `golden/n2_inputs.json`), prove to Groth16/BN254,
//!     verify locally, and serialize `{seal.hex,image_id.hex,journal.bin}` under
//!     `<dir>` (default `out/receipt`).
//!   - `execute --inputs <json>` (s2/04 — FAST, no proving): run the inputs
//!     through the RISC Zero **executor** at the matching depth and print N
//!     nullifiers + N payouts from the committed journal. Proves the witness is
//!     valid (membership + distinctness) BEFORE the multi-hour Groth16 prove.
//!   - `gen-inputs --n <N> --out <json> [--recipients hex,hex,…] [--seed <u64>]`
//!     (s2/04): emit a self-consistent depth=ceil(log2 N) Merkle tree with N
//!     fresh notes (distinct secrets/blindings ⇒ distinct nullifiers) whose
//!     commitments/paths the guest accepts under the executor.
//!   - `image-id`: print the DEPLOYED guest image id (cbeab7aa…) as 32B hex.
//!
//! ## Depth → guest selection (consensus-critical)
//! The on-chain `settle_batch` binds the DEPLOYED guest (depth 3, image_id
//! `cbeab7aa…`). Inputs at depth 3 (paths of length 3 — the N=8 demo and the N=2
//! golden) MUST prove/execute against that DEPLOYED guest so the receipt verifies
//! on-chain. Inputs at depth 4/5 (N=16/32, proving-time-only, NEVER settled) use
//! the separate `rollup-guest-bench` guest built at the matching depth via
//! `ROLLUP_TREE_DEPTH` (its image_id differs — that is fine, it is never bound).
//! The depth is inferred from `notes[0].path.len()`; a depth-4/5 inputs file run
//! against a default (depth-3) bench build is rejected loudly by the guest's
//! `path.len() == TREE_DEPTH` assert, so a mis-built bench guest cannot silently
//! produce a wrong-depth proof.
//!
//! **REAL proof, not dev-mode.** A dev-mode (`RISC0_DEV_MODE=1`) "receipt" is a
//! `Fake` inner receipt; we reject that here so this binary can never silently
//! produce a fake receipt for the on-chain path.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use methods::{
    ROLLUP_GUEST_BENCH_ELF, ROLLUP_GUEST_BENCH_ID, ROLLUP_GUEST_ELF, ROLLUP_GUEST_ID,
};
use risc0_ethereum_contracts::encode_seal;
use risc0_zkvm::{
    default_executor, default_prover, Digest, ExecutorEnv, InnerReceipt, ProverOpts, Receipt,
};
use serde::Deserialize;
use zk_core::journal::{self, DecodedJournal};
use zk_core::merkle;
use zk_core::note::{self, Note};
use zk_core::poseidon2::Fr;
use zk_core::witness::{GuestInput, NoteWitness};

/// Path (relative to the workspace root) of the golden N=2 inputs (default).
const GOLDEN_N2: &str = "golden/n2_inputs.json";
/// Output directory for the serialized receipt artifacts (default).
const RECEIPT_DIR: &str = "out/receipt";
/// The DEPLOYED guest's depth (image_id cbeab7aa…); depth-3 inputs settle here.
const DEPLOYED_TREE_DEPTH: usize = 3;

// ── Inputs JSON shape (mirrors host/tests/guest_exec.rs) ──────────────────────
// All field elements are `0x…` 32-byte LITTLE-ENDIAN hex (the PoC
// `scalar_to_bytes` convention, == `zk_core::witness`).

#[derive(Deserialize)]
struct InputsFile {
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

/// Build the `GuestInput` the guest reads from an inputs file. This is the SAME
/// struct, with the SAME 32-byte-LE serialization, that the executor test feeds
/// the guest — so the proven journal equals the executor-checked one.
fn guest_input_from(g: &InputsFile) -> GuestInput {
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

/// Resolve a path relative to the workspace root regardless of CWD.
fn workspace_path(rel: &str) -> PathBuf {
    let manifest = env!("CARGO_MANIFEST_DIR");
    Path::new(manifest)
        .parent()
        .expect("host/ has a parent (workspace root)")
        .join(rel)
}

/// If `p` is relative, anchor it to the workspace root; otherwise use it as-is.
fn resolve(p: &str) -> PathBuf {
    let pb = PathBuf::from(p);
    if pb.is_absolute() {
        pb
    } else {
        workspace_path(p)
    }
}

/// Load + parse an inputs file, returning the `GuestInput` and the inferred depth.
fn load_inputs(path: &Path) -> Result<(GuestInput, usize), String> {
    let raw = fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let g: InputsFile =
        serde_json::from_str(&raw).map_err(|e| format!("parse {}: {e}", path.display()))?;
    let input = guest_input_from(&g);
    if input.notes.is_empty() {
        return Err(format!("{}: inputs file has no notes", path.display()));
    }
    let depth = input.notes[0].path.len();
    // Sanity: all paths share the same length (the guest also asserts this).
    for (i, w) in input.notes.iter().enumerate() {
        if w.path.len() != depth {
            return Err(format!(
                "{}: note[{i}] path length {} != note[0] length {depth} (inconsistent depth)",
                path.display(),
                w.path.len()
            ));
        }
    }
    Ok((input, depth))
}

/// Select the guest ELF + image id for a given inferred tree `depth`.
/// depth 3 → DEPLOYED guest (cbeab7aa…, on-chain). depth 4/5 → bench guest
/// (proving-only); the bench guest must have been BUILT at that depth
/// (`ROLLUP_TREE_DEPTH=<depth>`), which the guest's own asserts enforce at run.
fn guest_for_depth(depth: usize) -> Result<(&'static [u8], [u32; 8], &'static str), String> {
    match depth {
        DEPLOYED_TREE_DEPTH => Ok((ROLLUP_GUEST_ELF, ROLLUP_GUEST_ID, "deployed(depth3)")),
        4 | 5 => Ok((
            ROLLUP_GUEST_BENCH_ELF,
            ROLLUP_GUEST_BENCH_ID,
            "bench(proving-only)",
        )),
        other => Err(format!(
            "unsupported tree depth {other} (inputs paths have {other} siblings); \
             only 3 [deployed], 4 [N=16 proving], 5 [N=32 proving] are supported"
        )),
    }
}

/// Decode + pretty-print a committed journal (root, N nullifiers, N payouts).
fn print_journal(label: &str, bytes: &[u8]) -> Result<DecodedJournal, String> {
    let d = journal::decode(bytes).map_err(|e| format!("journal decode: {e:?}"))?;
    println!(
        "[{label}] journal OK: {} bytes, N={} nullifiers, N={} payouts",
        bytes.len(),
        d.nullifiers.len(),
        d.payouts.len()
    );
    println!("[{label}]   root        0x{}", hex::encode(note::fr_to_le_bytes(&d.root)));
    for (i, nf) in d.nullifiers.iter().enumerate() {
        println!("[{label}]   nullifier[{i:>2}] 0x{}", hex::encode(note::fr_to_le_bytes(nf)));
    }
    for (i, p) in d.payouts.iter().enumerate() {
        println!(
            "[{label}]   payout[{i:>2}]    recipient=0x{} amount={}",
            hex::encode(p.recipient),
            p.amount
        );
    }
    Ok(d)
}

// ═════════════════════════════════════════════════════════════════════════════
// execute — run the inputs through the RISC Zero executor (FAST, no proving).
// ═════════════════════════════════════════════════════════════════════════════
fn run_execute(inputs: &str) -> Result<(), String> {
    let path = resolve(inputs);
    let (input, depth) = load_inputs(&path)?;
    let (elf, id, which) = guest_for_depth(depth)?;
    let n = input.notes.len();
    println!(
        "[execute] {} note(s), inferred depth {depth} → guest {which}",
        n
    );
    println!("[execute]   guest image-id 0x{}", hex::encode(Digest::from(id).as_bytes()));

    let env = ExecutorEnv::builder()
        .write(&input)
        .map_err(|e| format!("ExecutorEnv::write: {e}"))?
        .build()
        .map_err(|e| format!("ExecutorEnv::build: {e}"))?;
    let session = default_executor()
        .execute(env, elf)
        .map_err(|e| format!("executor rejected the witness (guest panic): {e}"))?;

    let d = print_journal("execute", &session.journal.bytes)?;
    if d.nullifiers.len() != n || d.payouts.len() != n {
        return Err(format!(
            "journal N mismatch: {} notes in, {} nullifiers / {} payouts out",
            n,
            d.nullifiers.len(),
            d.payouts.len()
        ));
    }
    // Distinct nullifiers (the guest also asserts this; double-check here).
    for i in 0..d.nullifiers.len() {
        for j in (i + 1)..d.nullifiers.len() {
            if d.nullifiers[i] == d.nullifiers[j] {
                return Err(format!("nullifier[{i}] == nullifier[{j}] (not distinct)"));
            }
        }
    }
    println!("[execute] OK: witness valid under the executor (N={n}, depth={depth}, distinct nullifiers).");
    Ok(())
}

// ═════════════════════════════════════════════════════════════════════════════
// prove — load inputs, prove to Groth16/BN254, verify, serialize artifacts.
// ═════════════════════════════════════════════════════════════════════════════
fn run_prove(inputs: &str, out: &str) -> Result<(), String> {
    // ── Guard: a real Groth16 wrap is required; dev-mode is a fake. ───────────
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

    // ── 1. Load inputs + select guest by inferred depth. ──────────────────────
    let path = resolve(inputs);
    let (input, depth) = load_inputs(&path)?;
    let (elf, id, which) = guest_for_depth(depth)?;
    println!(
        "[prove] loaded {} note(s) (depth {depth}) from {} → guest {which}",
        input.notes.len(),
        path.display()
    );
    if depth != DEPLOYED_TREE_DEPTH {
        println!(
            "[prove] NOTE: depth {depth} is PROVING-ONLY (bench guest, image-id \
             0x{}); this receipt is NOT settle-able on-chain.",
            hex::encode(Digest::from(id).as_bytes())
        );
    }

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
        .prove_with_opts(env, elf, &ProverOpts::groth16())
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

    // ── 4b. Verify the receipt against the SELECTED guest image ID, locally. ──
    receipt
        .verify(id)
        .map_err(|e| format!("receipt.verify(image_id) FAILED: {e}"))?;
    println!("[prove] receipt.verify(image_id): OK");

    // ── 5. Serialize artifacts. ───────────────────────────────────────────────
    let out_dir = resolve(out);
    fs::create_dir_all(&out_dir).map_err(|e| format!("mkdir {}: {e}", out_dir.display()))?;

    let seal = encode_seal(&receipt).map_err(|e| format!("encode_seal: {e}"))?;
    if seal.len() <= 64 {
        return Err(format!(
            "encode_seal produced only {} bytes — too short to be a Groth16 seal \
             (expected ~260). Refusing to write a fake/short seal.",
            seal.len()
        ));
    }
    let seal_hex = hex::encode(&seal);
    write_artifact(&out_dir.join("seal.hex"), seal_hex.as_bytes())?;

    let image_id_bytes = Digest::from(id);
    let image_id_hex = hex::encode(image_id_bytes.as_bytes());
    write_artifact(&out_dir.join("image_id.hex"), image_id_hex.as_bytes())?;

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

// ═════════════════════════════════════════════════════════════════════════════
// gen-inputs — emit a self-consistent depth=ceil(log2 N) tree with N fresh notes.
// ═════════════════════════════════════════════════════════════════════════════

/// Deterministic 32-byte field-ish seed material from (tag, seed, index).
/// Uses Poseidon2 over (seed, index, tag) so the bytes are < field order and the
/// derivation is reproducible. NOT a security RNG — these are demo witnesses.
fn derive_fr(seed: u64, index: u64, tag: u64) -> Fr {
    zk_core::poseidon2::hash(&[Fr::from(seed), Fr::from(index), Fr::from(tag)])
}

/// Build the full Merkle tree from N leaf commitments and return
/// `(root, paths)` where `paths[i]` is the authentication path (leaf level first)
/// for leaf `i`. The tree has depth `ceil(log2 N)`; N must be a power of two so
/// the tree is full and every leaf has a real path (the guest requires
/// `path.len() == depth` and `index < 2^depth`).
fn build_tree(leaves: &[Fr]) -> (Fr, Vec<Vec<Fr>>, usize) {
    let n = leaves.len();
    assert!(n.is_power_of_two() && n >= 2, "N must be a power of two >= 2");
    let depth = n.trailing_zeros() as usize;

    // levels[0] = leaves, levels[k+1] = compressed pairs of levels[k].
    let mut levels: Vec<Vec<Fr>> = Vec::with_capacity(depth + 1);
    levels.push(leaves.to_vec());
    for _ in 0..depth {
        let prev = levels.last().unwrap();
        let mut next = Vec::with_capacity(prev.len() / 2);
        for pair in prev.chunks(2) {
            next.push(merkle::compress(pair[0], pair[1]));
        }
        levels.push(next);
    }
    let root = levels[depth][0];

    // Authentication path for each leaf: at level k, sibling of node at the
    // leaf's position-shifted index. is_right uses bit k of the leaf index.
    let mut paths: Vec<Vec<Fr>> = Vec::with_capacity(n);
    for leaf_idx in 0..n {
        let mut path = Vec::with_capacity(depth);
        let mut idx = leaf_idx;
        // One sibling per level (leaf level first); `level_nodes` is levels[k].
        for level_nodes in levels.iter().take(depth) {
            let sibling = idx ^ 1; // flip the low bit at this level
            path.push(level_nodes[sibling]);
            idx >>= 1;
        }
        paths.push(path);
    }
    (root, paths, depth)
}

fn run_gen_inputs(
    n: usize,
    out: &str,
    recipients: Option<Vec<[u8; 32]>>,
    seed: u64,
    amount_base: u128,
) -> Result<(), String> {
    if !n.is_power_of_two() || n < 2 {
        return Err(format!("--n {n} must be a power of two >= 2 (so the tree is full)"));
    }
    let depth = n.trailing_zeros() as usize;

    // Per-leaf fresh secrets/blindings (distinct ⇒ distinct nullifiers).
    let mut secrets: Vec<Fr> = Vec::with_capacity(n);
    let mut blindings: Vec<Fr> = Vec::with_capacity(n);
    let mut amounts: Vec<u128> = Vec::with_capacity(n);
    let mut recips: Vec<[u8; 32]> = Vec::with_capacity(n);

    for i in 0..n {
        secrets.push(derive_fr(seed, i as u64, 0x5EC2E7)); // "secret"
        blindings.push(derive_fr(seed, i as u64, 0xB11D)); // "blind"
        amounts.push(amount_base + i as u128); // small, distinct amounts
        let r = match &recipients {
            Some(rs) => *rs
                .get(i)
                .ok_or_else(|| format!("--recipients has {} entries, need {n}", rs.len()))?,
            // Proving-only default: deterministic 32-byte recipient (NEVER settled).
            None => note::fr_to_le_bytes(&derive_fr(seed, i as u64, 0x5EC1)), // "reci"
        };
        recips.push(r);
    }

    // Commitments via zk-core (byte-identical to the PoC).
    let leaves: Vec<Fr> = (0..n)
        .map(|i| {
            let note = Note {
                amount: Fr::from(amounts[i]),
                priv_key: secrets[i],
                blinding: blindings[i],
                leaf_index: i as u64,
            };
            note::commitment(&note)
        })
        .collect();

    let (root, paths, tree_depth) = build_tree(&leaves);
    assert_eq!(tree_depth, depth, "tree depth mismatch");

    // Emit JSON in the exact shape host/tests + prove read (LE hex everywhere).
    let mut s = String::new();
    s.push_str("{\n");
    s.push_str(&format!(
        "  \"description\": \"s2/04 PROVING input set: {n} fresh notes at depth {depth} (full tree, leaves 0..{}). Distinct secrets/blindings ⇒ {n} distinct nullifiers (no spent-nullifier reuse). All field elements 32-byte LITTLE-ENDIAN hex (PoC scalar_to_bytes); commitments/paths computed by zk-core (byte-identical to the PoC). Internally consistent: each note's root_from_path(commitment, path, index) == merkle_root_le, so the guest's membership assert passes under the executor.\",\n",
        n - 1
    ));
    s.push_str(&format!(
        "  \"provenance\": \"generated by `host gen-inputs --n {n} --seed {seed}` (zk-core prover crypto). Recipients: {}.\",\n",
        if recipients.is_some() {
            "REAL funded testnet ed25519 keys (see deployments/testnet.json recipients_n8) — settle-able"
        } else {
            "DETERMINISTIC 32-byte values (PROVING-ONLY, NEVER settled on-chain)"
        }
    ));
    s.push_str(&format!("  \"n\": {n},\n"));
    s.push_str(&format!("  \"depth\": {depth},\n"));
    s.push_str(&format!("  \"merkle_root_le\": \"0x{}\",\n", hex::encode(note::fr_to_le_bytes(&root))));

    // expected_nullifiers_le — host oracle, so a drift in guest OR file is caught.
    s.push_str("  \"expected_nullifiers_le\": [\n");
    for i in 0..n {
        let note = Note {
            amount: Fr::from(amounts[i]),
            priv_key: secrets[i],
            blinding: blindings[i],
            leaf_index: i as u64,
        };
        let nf = note::nullifier(&note, secrets[i]);
        let comma = if i + 1 < n { "," } else { "" };
        s.push_str(&format!("    \"0x{}\"{}\n", hex::encode(note::fr_to_le_bytes(&nf)), comma));
    }
    s.push_str("  ],\n");

    s.push_str("  \"notes\": [\n");
    for i in 0..n {
        s.push_str("    {\n");
        s.push_str(&format!("      \"label\": \"note{i}\",\n"));
        s.push_str(&format!("      \"secret_le\": \"0x{}\",\n", hex::encode(note::fr_to_le_bytes(&secrets[i]))));
        s.push_str(&format!("      \"blinding_le\": \"0x{}\",\n", hex::encode(note::fr_to_le_bytes(&blindings[i]))));
        s.push_str(&format!("      \"amount\": {},\n", amounts[i]));
        s.push_str(&format!("      \"recipient\": \"0x{}\",\n", hex::encode(recips[i])));
        s.push_str(&format!("      \"index\": {i},\n"));
        s.push_str("      \"path_le\": [\n");
        for (k, sib) in paths[i].iter().enumerate() {
            let comma = if k + 1 < paths[i].len() { "," } else { "" };
            s.push_str(&format!("        \"0x{}\"{}\n", hex::encode(note::fr_to_le_bytes(sib)), comma));
        }
        s.push_str("      ]\n");
        let comma = if i + 1 < n { "," } else { "" };
        s.push_str(&format!("    }}{}\n", comma));
    }
    s.push_str("  ]\n");
    s.push_str("}\n");

    let out_path = resolve(out);
    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
    }
    fs::write(&out_path, s.as_bytes()).map_err(|e| format!("write {}: {e}", out_path.display()))?;
    println!(
        "[gen-inputs] wrote {n} fresh notes (depth {depth}) → {}",
        out_path.display()
    );
    println!("[gen-inputs]   root 0x{}", hex::encode(note::fr_to_le_bytes(&root)));
    println!("[gen-inputs] verifying internal consistency (root_from_path per note)...");
    // Self-check: every note's path recomputes the root (the guest's membership).
    for i in 0..n {
        let recomputed = merkle::root_from_path(leaves[i], &paths[i], i as u64);
        if recomputed != root {
            return Err(format!("note[{i}] membership self-check FAILED (path inconsistent)"));
        }
    }
    println!("[gen-inputs] OK: all {n} notes recompute the root (membership self-consistent).");
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

// ── tiny flag parser (no clap dep) ────────────────────────────────────────────
/// Get the value following `--flag` in `args`, if present.
fn flag_value(args: &[String], flag: &str) -> Option<String> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1).cloned())
}

fn usage() -> &'static str {
    "usage:\n  \
     cargo run -p host --release -- prove [--inputs <json>] [--out <dir>]\n  \
     cargo run -p host --release -- execute --inputs <json>\n  \
     cargo run -p host --release -- gen-inputs --n <N> --out <json> [--recipients hex,…] [--seed <u64>] [--amount-base <u128>]\n  \
     cargo run -p host --release -- image-id\n\n\
     prove:      proves the rollup guest to Groth16/BN254 (Docker), verifies it, writes <dir>/{seal.hex,image_id.hex,journal.bin}.\n\
     execute:    runs the inputs through the RISC Zero executor (FAST, no proving) and prints N nullifiers + payouts.\n\
     gen-inputs: emits a self-consistent depth=ceil(log2 N) tree of N fresh notes (default recipients are PROVING-ONLY).\n\
     depth 3 inputs use the DEPLOYED guest (cbeab7aa…); depth 4/5 use the proving-only bench guest (build with ROLLUP_TREE_DEPTH=<depth>)."
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().map(String::as_str);
    let rest = if args.is_empty() { &[][..] } else { &args[1..] };

    let result: Result<(), String> = match cmd {
        Some("prove") => {
            let inputs = flag_value(rest, "--inputs").unwrap_or_else(|| GOLDEN_N2.to_string());
            let out = flag_value(rest, "--out").unwrap_or_else(|| RECEIPT_DIR.to_string());
            run_prove(&inputs, &out)
        }
        Some("execute") => match flag_value(rest, "--inputs") {
            Some(inputs) => run_execute(&inputs),
            None => Err(format!("execute requires --inputs <json>\n\n{}", usage())),
        },
        Some("gen-inputs") => {
            let n = flag_value(rest, "--n")
                .and_then(|s| s.parse::<usize>().ok())
                .ok_or_else(|| format!("gen-inputs requires --n <N>\n\n{}", usage()));
            let out = flag_value(rest, "--out")
                .ok_or_else(|| format!("gen-inputs requires --out <json>\n\n{}", usage()));
            match (n, out) {
                (Ok(n), Ok(out)) => {
                    let seed = flag_value(rest, "--seed")
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(0xC0FFEE);
                    let amount_base = flag_value(rest, "--amount-base")
                        .and_then(|s| s.parse::<u128>().ok())
                        .unwrap_or(1000);
                    let recipients = flag_value(rest, "--recipients").map(|csv| {
                        csv.split(',')
                            .map(|h| le32(h.trim()))
                            .collect::<Vec<[u8; 32]>>()
                    });
                    run_gen_inputs(n, &out, recipients, seed, amount_base)
                }
                (Err(e), _) | (_, Err(e)) => Err(e),
            }
        }
        Some("image-id") => {
            // The DEPLOYED guest id (cbeab7aa…) — the one bound on-chain.
            let image_id_hex = hex::encode(Digest::from(ROLLUP_GUEST_ID).as_bytes());
            println!("{image_id_hex}");
            Ok(())
        }
        other => Err(format!("unknown subcommand {:?}\n\n{}", other.unwrap_or("<none>"), usage())),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[host] ERROR: {e}");
            ExitCode::FAILURE
        }
    }
}
