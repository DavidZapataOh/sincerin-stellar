//! The proving seam — the ONLY frontier between the sequencer and proving
//! hardware (plan §2).
//!
//! The sequencer builds a `GuestInput`, hands it to a `Prover`, and gets back a
//! [`ProvedBatch`]. Local vs remote is pure config (`PROVER_BACKEND`); the
//! sequencer's state machine, lock, and collision logic never change.
//!
//! - [`LocalProver`] (production, built always): shells out to `host prove`
//!   (`RISC0_DEV_MODE=0`, REAL Groth16). Multi-hour on a Mac.
//! - `RemoteProver` (s3, NOT built here): POST inputs to a GPU prover, poll.
//! - [`FixtureProver`] (TEST-ONLY, `feature = "test-fixture"`): LOADS the real
//!   pre-generated N=8 receipt from `out/bench/n8/`. It is **structurally
//!   unreachable** from the production binary (which builds with no features).

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use zk_core::witness::GuestInput;

use crate::types::ProvedBatch;

/// Errors a [`Prover`] can surface. The sequencer maps these to a `Failed` status.
#[derive(Debug)]
pub enum ProverError {
    /// The proving backend itself failed (process error, non-zero exit, timeout).
    Backend(String),
    /// The produced artifacts were missing or malformed (e.g. empty seal).
    Artifact(String),
}

impl core::fmt::Display for ProverError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ProverError::Backend(s) => write!(f, "prover backend: {s}"),
            ProverError::Artifact(s) => write!(f, "prover artifact: {s}"),
        }
    }
}

impl std::error::Error for ProverError {}

/// The proving interface. `prove` may take hours; the sequencer always calls it
/// off the submit path (background batching loop), so the user never blocks.
#[async_trait]
pub trait Prover: Send + Sync {
    /// Produce a REAL Groth16 receipt for `input`. Implementations MUST NOT
    /// fabricate a receipt (no dev-mode, no hand-built journal).
    async fn prove(&self, input: GuestInput) -> Result<ProvedBatch, ProverError>;

    /// A short label for logging/telemetry (e.g. "local", "fixture(n8)").
    fn backend_label(&self) -> &'static str;
}

/// Production prover: shells out to the `host prove` subcommand, which performs a
/// real STARK→Groth16 wrap (Docker) and writes `{seal.hex,image_id.hex,journal.bin}`.
///
/// Construction takes the workspace root (so it can find the `host` binary +
/// write a temp inputs file). `RISC0_DEV_MODE` is forced to `0`.
pub struct LocalProver {
    /// Workspace root (contains `target/release/host`, `golden/`, etc.).
    workspace_root: PathBuf,
}

impl LocalProver {
    /// Build a `LocalProver` rooted at `workspace_root`.
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
        }
    }

    /// Serialize a `GuestInput` to the inputs-file JSON shape `host prove`/`execute`
    /// parse (LE hex everywhere). This is the SAME shape as `golden/*_inputs.json`,
    /// so the bytes the prover reads are byte-identical to what the sequencer built
    /// (lock 3).
    pub fn inputs_json(input: &GuestInput) -> String {
        crate::batch::inputs_file_json(input)
    }
}

#[async_trait]
impl Prover for LocalProver {
    async fn prove(&self, input: GuestInput) -> Result<ProvedBatch, ProverError> {
        use std::process::Command;

        let root = self.workspace_root.clone();
        // Write the inputs file the host reads.
        let inputs_path = root.join("out/seq/batch_inputs.json");
        if let Some(parent) = inputs_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| ProverError::Backend(format!("mkdir {}: {e}", parent.display())))?;
        }
        std::fs::write(&inputs_path, Self::inputs_json(&input))
            .map_err(|e| ProverError::Backend(format!("write inputs: {e}")))?;

        let out_dir = root.join("out/seq/receipt");
        let host_bin = root.join("target/release/host");

        // Run `host prove --inputs <f> --out <d>` with RISC0_DEV_MODE=0 (REAL).
        let status = Command::new(&host_bin)
            .arg("prove")
            .arg("--inputs")
            .arg(&inputs_path)
            .arg("--out")
            .arg(&out_dir)
            .env("RISC0_DEV_MODE", "0")
            .current_dir(&root)
            .status()
            .map_err(|e| ProverError::Backend(format!("spawn host prove: {e}")))?;
        if !status.success() {
            return Err(ProverError::Backend(format!(
                "host prove exited with {status}"
            )));
        }
        load_proved_batch(&out_dir)
    }

    fn backend_label(&self) -> &'static str {
        "local(host prove, RISC0_DEV_MODE=0)"
    }
}

/// Load a [`ProvedBatch`] from a directory holding `seal.hex`, `image_id.hex`,
/// `journal.bin` (the artifact layout `host prove` writes and the fixture stores).
pub fn load_proved_batch(dir: &Path) -> Result<ProvedBatch, ProverError> {
    let seal_hex = std::fs::read_to_string(dir.join("seal.hex"))
        .map_err(|e| ProverError::Artifact(format!("read seal.hex: {e}")))?;
    let seal = hex::decode(seal_hex.trim())
        .map_err(|e| ProverError::Artifact(format!("decode seal.hex: {e}")))?;
    if seal.len() <= 64 {
        return Err(ProverError::Artifact(format!(
            "seal too short ({} bytes) — not a real Groth16 seal",
            seal.len()
        )));
    }

    let image_id_hex = std::fs::read_to_string(dir.join("image_id.hex"))
        .map_err(|e| ProverError::Artifact(format!("read image_id.hex: {e}")))?;
    let image_id_vec = hex::decode(image_id_hex.trim())
        .map_err(|e| ProverError::Artifact(format!("decode image_id.hex: {e}")))?;
    if image_id_vec.len() != 32 {
        return Err(ProverError::Artifact(format!(
            "image_id is {} bytes, expected 32",
            image_id_vec.len()
        )));
    }
    let mut image_id = [0u8; 32];
    image_id.copy_from_slice(&image_id_vec);

    let journal = std::fs::read(dir.join("journal.bin"))
        .map_err(|e| ProverError::Artifact(format!("read journal.bin: {e}")))?;

    Ok(ProvedBatch {
        seal,
        image_id,
        journal,
    })
}

// ═════════════════════════════════════════════════════════════════════════════
// FixtureProver — TEST-ONLY. Compiled ONLY under `feature = "test-fixture"`, so
// the production binary (no features) cannot reference it. LOCK 1.
// ═════════════════════════════════════════════════════════════════════════════
#[cfg(feature = "test-fixture")]
pub use fixture::FixtureProver;

#[cfg(feature = "test-fixture")]
mod fixture {
    use super::*;

    /// Returns the **real** pre-generated N=8 receipt (`out/bench/n8/`), already
    /// SEC-approved. It does NOT fabricate anything — it LOADS the on-disk files
    /// the real prover produced. Used only to exercise the sequencer's
    /// orchestration without re-running the 4-hour prove.
    ///
    /// LOCK 1: this type only exists under `feature = "test-fixture"` — it is
    /// unreachable from the production binary.
    pub struct FixtureProver {
        /// Directory holding the real `out/bench/n8/{seal.hex,image_id.hex,journal.bin}`.
        receipt_dir: PathBuf,
    }

    impl FixtureProver {
        /// Build a fixture prover that loads the receipt from `receipt_dir`
        /// (e.g. `<workspace>/out/bench/n8`).
        pub fn new(receipt_dir: impl Into<PathBuf>) -> Self {
            Self {
                receipt_dir: receipt_dir.into(),
            }
        }
    }

    #[async_trait]
    impl Prover for FixtureProver {
        async fn prove(&self, _input: GuestInput) -> Result<ProvedBatch, ProverError> {
            // LOAD the real file — never hand-build a ProvedBatch.
            load_proved_batch(&self.receipt_dir)
        }

        fn backend_label(&self) -> &'static str {
            "fixture(real N=8 receipt, test-only)"
        }
    }
}
