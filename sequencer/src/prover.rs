//! The proving seam — the ONLY frontier between the sequencer and proving
//! hardware (plan §2).
//!
//! The sequencer builds a `GuestInput`, hands it to a `Prover`, and gets back a
//! [`ProvedBatch`]. Local vs remote is pure config (`PROVER_BACKEND`); the
//! sequencer's state machine, lock, and collision logic never change.
//!
//! - [`LocalProver`] (production, built always): shells out to `host prove`
//!   (`RISC0_DEV_MODE=0`, REAL Groth16). Multi-hour on a Mac.
//! - [`RemoteProver`] (production, s3/05): POSTs the inputs to a RunPod serverless
//!   GPU worker running the REAL `host prove` (native CUDA), polls, reconstructs
//!   the receipt. ~5 min. The HTTP transport is a seam so it's unit-tested with a
//!   worker-fake (no GPU); production always uses the real RunPod API.
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

/// Build + validate a [`ProvedBatch`] from raw parts. Shared by the local file
/// load and the remote (GPU worker) path so BOTH enforce the same cero-mocks
/// invariants: a real Groth16 seal (>64 bytes), a 32-byte image_id, a non-empty
/// journal, and — critically — a REJECTION of dev-mode seals (selector
/// `0xffffffff`). A fabricated/dev-mode receipt can never become a `ProvedBatch`.
pub fn build_proved_batch(
    seal: Vec<u8>,
    image_id_vec: Vec<u8>,
    journal: Vec<u8>,
) -> Result<ProvedBatch, ProverError> {
    if seal.len() <= 64 {
        return Err(ProverError::Artifact(format!(
            "seal too short ({} bytes) — not a real Groth16 seal",
            seal.len()
        )));
    }
    // dev-mode rejection: a RISC Zero dev-mode receipt's seal selector is
    // 0xffffffff. REAL proofs only — never on any product path (local or remote).
    if seal[0..4] == [0xff, 0xff, 0xff, 0xff] {
        return Err(ProverError::Artifact(
            "dev-mode seal (selector ffffffff) — REAL Groth16 proofs only".to_string(),
        ));
    }
    if image_id_vec.len() != 32 {
        return Err(ProverError::Artifact(format!(
            "image_id is {} bytes, expected 32",
            image_id_vec.len()
        )));
    }
    if journal.is_empty() {
        return Err(ProverError::Artifact("journal is empty".to_string()));
    }
    let mut image_id = [0u8; 32];
    image_id.copy_from_slice(&image_id_vec);
    Ok(ProvedBatch {
        seal,
        image_id,
        journal,
    })
}

/// Load a [`ProvedBatch`] from a directory holding `seal.hex`, `image_id.hex`,
/// `journal.bin` (the artifact layout `host prove` writes and the fixture stores).
pub fn load_proved_batch(dir: &Path) -> Result<ProvedBatch, ProverError> {
    let seal_hex = std::fs::read_to_string(dir.join("seal.hex"))
        .map_err(|e| ProverError::Artifact(format!("read seal.hex: {e}")))?;
    let seal = hex::decode(seal_hex.trim())
        .map_err(|e| ProverError::Artifact(format!("decode seal.hex: {e}")))?;
    let image_id_hex = std::fs::read_to_string(dir.join("image_id.hex"))
        .map_err(|e| ProverError::Artifact(format!("read image_id.hex: {e}")))?;
    let image_id_vec = hex::decode(image_id_hex.trim())
        .map_err(|e| ProverError::Artifact(format!("decode image_id.hex: {e}")))?;
    let journal = std::fs::read(dir.join("journal.bin"))
        .map_err(|e| ProverError::Artifact(format!("read journal.bin: {e}")))?;
    build_proved_batch(seal, image_id_vec, journal)
}

// ═════════════════════════════════════════════════════════════════════════════
// RemoteProver — production GPU path (s3/05). POSTs the SAME inputs JSON the
// LocalProver writes (lock 3, byte-identical) to a RunPod serverless worker that
// runs the REAL `host prove` (native CUDA, RISC0_DEV_MODE=0), polls for the
// receipt, and reconstructs a ProvedBatch through `build_proved_batch` (so the
// dev-mode/short-seal rejections apply identically). The HTTP transport is
// abstracted so the whole prover is unit-tested WITHOUT a GPU (the worker-fake);
// the production transport always talks to the real RunPod API — no mock on the
// product path.
// ═════════════════════════════════════════════════════════════════════════════

/// A receipt as the GPU worker returns it over the wire — hex everywhere (the
/// journal is small; hex avoids a base64 dependency).
#[derive(Clone, Debug)]
pub struct RemoteReceipt {
    /// The Groth16 seal, hex-encoded.
    pub seal_hex: String,
    /// The guest image_id (32 bytes), hex-encoded.
    pub image_id_hex: String,
    /// The journal bytes, hex-encoded.
    pub journal_hex: String,
}

/// Transport seam: submit the inputs to the GPU worker and wait for the receipt.
/// Production = RunPod serverless (curl). Tests inject a fake — so the
/// `RemoteProver` decode/validate/candado logic runs with zero GPU spend.
#[async_trait]
pub trait ProveTransport: Send + Sync {
    /// Submit `input_json` (the inputs-file JSON) and block until a receipt or error.
    async fn run(&self, input_json: &str) -> Result<RemoteReceipt, ProverError>;
}

/// Parsed RunPod job state (from a `/status` response). PURE.
#[derive(Debug)]
pub enum RunPodStatus {
    /// Job accepted, waiting in the queue.
    Queued,
    /// Worker is proving.
    InProgress,
    /// Done — carries the receipt.
    Completed(RemoteReceipt),
    /// Terminal failure (FAILED/CANCELLED/TIMED_OUT, or a malformed COMPLETED).
    Failed(String),
    /// Transient / unrecognised status — keep polling.
    Unknown(String),
}

/// Read the job id out of a RunPod `/run` response. PURE.
pub fn parse_submit_id(json: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(json)
        .ok()?
        .get("id")?
        .as_str()
        .map(|s| s.to_string())
}

/// Map a RunPod `/status` response to a [`RunPodStatus`]. PURE. A `COMPLETED`
/// without the three receipt fields is treated as a failure (never a silent pass).
pub fn parse_status(json: &str) -> RunPodStatus {
    let v = match serde_json::from_str::<serde_json::Value>(json) {
        Ok(v) => v,
        Err(_) => return RunPodStatus::Unknown(format!("non-json: {}", trunc(json, 80))),
    };
    match v.get("status").and_then(|s| s.as_str()).unwrap_or("") {
        "COMPLETED" => {
            let out = v.get("output");
            let get = |k: &str| {
                out.and_then(|o| o.get(k))
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string())
            };
            match (get("seal_hex"), get("image_id_hex"), get("journal_hex")) {
                (Some(seal_hex), Some(image_id_hex), Some(journal_hex)) => {
                    RunPodStatus::Completed(RemoteReceipt {
                        seal_hex,
                        image_id_hex,
                        journal_hex,
                    })
                }
                _ => RunPodStatus::Failed(format!(
                    "COMPLETED but output missing seal/image_id/journal: {}",
                    trunc(json, 120)
                )),
            }
        }
        "FAILED" | "CANCELLED" | "TIMED_OUT" => RunPodStatus::Failed(
            v.get("error")
                .and_then(|e| e.as_str())
                .unwrap_or("worker reported failure")
                .to_string(),
        ),
        "IN_QUEUE" => RunPodStatus::Queued,
        "IN_PROGRESS" => RunPodStatus::InProgress,
        other => RunPodStatus::Unknown(other.to_string()),
    }
}

fn trunc(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// Production transport: RunPod serverless run/status API via `curl` (same
/// no-new-dependency pattern as the settler's RPC re-check).
struct RunPodTransport {
    endpoint: String,
    api_key: String,
}

const POLL_INTERVAL_SECS: u64 = 10;
const POLL_TIMEOUT_SECS: u64 = 1200; // 20 min — generous over the ~5min prove + cold start

impl RunPodTransport {
    fn base_url() -> String {
        std::env::var("RUNPOD_BASE_URL").unwrap_or_else(|_| "https://api.runpod.ai/v2".to_string())
    }
    async fn curl(args: &[&str]) -> Result<String, ProverError> {
        let out = tokio::process::Command::new("curl")
            .args(args)
            .output()
            .await
            .map_err(|e| ProverError::Backend(format!("spawn curl: {e}")))?;
        if !out.status.success() {
            return Err(ProverError::Backend(format!(
                "curl exited {}: {}",
                out.status,
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    }
}

#[async_trait]
impl ProveTransport for RunPodTransport {
    async fn run(&self, input_json: &str) -> Result<RemoteReceipt, ProverError> {
        let base = Self::base_url();
        let auth = format!("Authorization: Bearer {}", self.api_key);
        // submit: POST {base}/{endpoint}/run  body {"input": <inputs json>}
        let body = format!(r#"{{"input":{input_json}}}"#);
        let run_url = format!("{base}/{}/run", self.endpoint);
        let submit = Self::curl(&[
            "-s", "-X", "POST", &run_url, "-H", &auth, "-H", "Content-Type: application/json", "-d",
            &body,
        ])
        .await?;
        let id = parse_submit_id(&submit).ok_or_else(|| {
            ProverError::Backend(format!("runpod /run returned no job id: {}", submit.trim()))
        })?;

        // poll: GET {base}/{endpoint}/status/{id} until terminal or timeout
        let status_url = format!("{base}/{}/status/{id}", self.endpoint);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(POLL_TIMEOUT_SECS);
        loop {
            if std::time::Instant::now() > deadline {
                return Err(ProverError::Backend(format!(
                    "runpod prove timed out after {POLL_TIMEOUT_SECS}s (job {id})"
                )));
            }
            tokio::time::sleep(std::time::Duration::from_secs(POLL_INTERVAL_SECS)).await;
            let sj = match Self::curl(&["-s", &status_url, "-H", &auth]).await {
                Ok(s) => s,
                Err(_) => continue, // transient network hiccup — keep polling until deadline
            };
            match parse_status(&sj) {
                RunPodStatus::Completed(r) => return Ok(r),
                RunPodStatus::Failed(msg) => {
                    return Err(ProverError::Backend(format!("runpod prove failed: {msg}")))
                }
                RunPodStatus::Queued | RunPodStatus::InProgress | RunPodStatus::Unknown(_) => {
                    continue
                }
            }
        }
    }
}

/// Production GPU prover. Constructed for `PROVER_BACKEND=remote`.
pub struct RemoteProver {
    transport: Box<dyn ProveTransport>,
}

impl RemoteProver {
    /// Build a prover that proves on a RunPod serverless endpoint, authenticated
    /// with `api_key`. The transport is the REAL RunPod API — never a fake.
    pub fn new(endpoint: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            transport: Box::new(RunPodTransport {
                endpoint: endpoint.into(),
                api_key: api_key.into(),
            }),
        }
    }

    /// TEST-ONLY: inject a fake transport so the decode/validate/candado logic runs
    /// without a GPU. Compiled only under `cfg(test)` — unreachable from production.
    #[cfg(test)]
    fn with_transport(transport: Box<dyn ProveTransport>) -> Self {
        Self { transport }
    }
}

#[async_trait]
impl Prover for RemoteProver {
    async fn prove(&self, input: GuestInput) -> Result<ProvedBatch, ProverError> {
        // The bytes the worker proves over are byte-identical to the local path
        // and to golden/*_inputs.json (lock 3).
        let input_json = crate::batch::inputs_file_json(&input);
        let r = self.transport.run(&input_json).await?;
        let seal = hex::decode(r.seal_hex.trim())
            .map_err(|e| ProverError::Artifact(format!("decode remote seal: {e}")))?;
        let image_id = hex::decode(r.image_id_hex.trim())
            .map_err(|e| ProverError::Artifact(format!("decode remote image_id: {e}")))?;
        let journal = hex::decode(r.journal_hex.trim())
            .map_err(|e| ProverError::Artifact(format!("decode remote journal: {e}")))?;
        // Same validation + dev-mode rejection as the local path.
        build_proved_batch(seal, image_id, journal)
    }

    fn backend_label(&self) -> &'static str {
        "remote(runpod gpu, host prove, RISC0_DEV_MODE=0)"
    }
}

#[cfg(test)]
mod remote_tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    enum FakeOutcome {
        Receipt(RemoteReceipt),
        Backend(String),
    }
    struct FakeTransport {
        outcome: FakeOutcome,
        captured: Arc<Mutex<Option<String>>>,
    }
    #[async_trait]
    impl ProveTransport for FakeTransport {
        async fn run(&self, input_json: &str) -> Result<RemoteReceipt, ProverError> {
            *self.captured.lock().unwrap() = Some(input_json.to_string());
            match &self.outcome {
                FakeOutcome::Receipt(r) => Ok(r.clone()),
                FakeOutcome::Backend(m) => Err(ProverError::Backend(m.clone())),
            }
        }
    }

    fn empty_input() -> GuestInput {
        GuestInput {
            notes: vec![],
            merkle_root: [0u8; 32],
        }
    }
    fn valid_receipt() -> RemoteReceipt {
        RemoteReceipt {
            seal_hex: "01".repeat(256), // 256 bytes, not dev-mode
            image_id_hex: "ab".repeat(32), // 32 bytes
            journal_hex: "0a0b0c".to_string(),
        }
    }
    fn prover_with(outcome: FakeOutcome) -> (RemoteProver, Arc<Mutex<Option<String>>>) {
        let captured = Arc::new(Mutex::new(None));
        let t = FakeTransport {
            outcome,
            captured: captured.clone(),
        };
        (RemoteProver::with_transport(Box::new(t)), captured)
    }

    #[tokio::test]
    async fn prove_decodes_a_valid_remote_receipt() {
        let (p, _) = prover_with(FakeOutcome::Receipt(valid_receipt()));
        let proved = p.prove(empty_input()).await.expect("valid receipt");
        assert_eq!(proved.seal.len(), 256);
        assert_eq!(proved.image_id, [0xabu8; 32]);
        assert_eq!(proved.journal, vec![0x0a, 0x0b, 0x0c]);
    }

    #[tokio::test]
    async fn prove_rejects_dev_mode_seal() {
        // a dev-mode seal selector is 0xffffffff — must NEVER become a ProvedBatch
        let dev = RemoteReceipt {
            seal_hex: format!("ffffffff{}", "00".repeat(128)),
            image_id_hex: "ab".repeat(32),
            journal_hex: "0a".to_string(),
        };
        let (p, _) = prover_with(FakeOutcome::Receipt(dev));
        let err = p.prove(empty_input()).await.unwrap_err();
        assert!(matches!(err, ProverError::Artifact(_)), "dev-mode → Artifact err");
        assert!(format!("{err}").contains("dev-mode"));
    }

    #[tokio::test]
    async fn prove_rejects_short_seal() {
        let short = RemoteReceipt {
            seal_hex: "01".repeat(32), // 32 bytes ≤ 64
            image_id_hex: "ab".repeat(32),
            journal_hex: "0a".to_string(),
        };
        let (p, _) = prover_with(FakeOutcome::Receipt(short));
        assert!(matches!(
            p.prove(empty_input()).await.unwrap_err(),
            ProverError::Artifact(_)
        ));
    }

    #[tokio::test]
    async fn prove_maps_backend_error() {
        let (p, _) = prover_with(FakeOutcome::Backend("gpu oom".to_string()));
        let err = p.prove(empty_input()).await.unwrap_err();
        assert!(matches!(err, ProverError::Backend(_)));
    }

    #[tokio::test]
    async fn prove_sends_byte_identical_inputs_json_lock3() {
        let input = empty_input();
        let (p, captured) = prover_with(FakeOutcome::Receipt(valid_receipt()));
        p.prove(input.clone()).await.unwrap();
        let sent = captured.lock().unwrap().clone().expect("captured");
        assert_eq!(sent, crate::batch::inputs_file_json(&input));
    }

    #[test]
    fn parse_submit_id_reads_id() {
        assert_eq!(
            parse_submit_id(r#"{"id":"abc-123","status":"IN_QUEUE"}"#).as_deref(),
            Some("abc-123")
        );
        assert_eq!(parse_submit_id(r#"{"status":"IN_QUEUE"}"#), None);
        assert_eq!(parse_submit_id("not json"), None);
    }

    #[test]
    fn parse_status_completed_yields_receipt() {
        let json = r#"{"status":"COMPLETED","output":{"seal_hex":"de","image_id_hex":"ad","journal_hex":"be"}}"#;
        match parse_status(json) {
            RunPodStatus::Completed(r) => {
                assert_eq!(r.seal_hex, "de");
                assert_eq!(r.image_id_hex, "ad");
                assert_eq!(r.journal_hex, "be");
            }
            other => panic!("expected Completed, got {other:?}"),
        }
    }

    #[test]
    fn parse_status_completed_without_output_is_failure() {
        // a COMPLETED missing the receipt fields must NOT silently pass
        assert!(matches!(
            parse_status(r#"{"status":"COMPLETED","output":{}}"#),
            RunPodStatus::Failed(_)
        ));
    }

    #[test]
    fn parse_status_other_states() {
        assert!(matches!(parse_status(r#"{"status":"IN_QUEUE"}"#), RunPodStatus::Queued));
        assert!(matches!(parse_status(r#"{"status":"IN_PROGRESS"}"#), RunPodStatus::InProgress));
        assert!(matches!(parse_status(r#"{"status":"FAILED","error":"oom"}"#), RunPodStatus::Failed(_)));
        assert!(matches!(parse_status("garbage"), RunPodStatus::Unknown(_)));
    }

    #[test]
    fn build_proved_batch_guards() {
        // short seal
        assert!(build_proved_batch(vec![1u8; 32], vec![0u8; 32], vec![1]).is_err());
        // dev-mode seal
        let mut dev = vec![0xffu8; 4];
        dev.extend(vec![0u8; 128]);
        assert!(build_proved_batch(dev, vec![0u8; 32], vec![1]).is_err());
        // bad image_id length
        assert!(build_proved_batch(vec![1u8; 100], vec![0u8; 31], vec![1]).is_err());
        // empty journal
        assert!(build_proved_batch(vec![1u8; 100], vec![0u8; 32], vec![]).is_err());
        // valid
        assert!(build_proved_batch(vec![1u8; 100], vec![0u8; 32], vec![1, 2]).is_ok());
    }
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

    impl FixtureProver {
        /// The honest dev/demo delay (seconds) read from `FIXTURE_PROVE_DELAY`.
        /// Defaults to 0 (no delay). This is NOT a fake proof — it only sleeps
        /// BEFORE returning the **real** receipt, so the async UX (the `proving`
        /// state) can be exercised without a GPU. The receipt, settle, and verifier
        /// are all real (cero-mocks intact).
        pub fn prove_delay() -> std::time::Duration {
            let secs = std::env::var("FIXTURE_PROVE_DELAY")
                .ok()
                .and_then(|s| s.trim().parse::<u64>().ok())
                .unwrap_or(0);
            std::time::Duration::from_secs(secs)
        }
    }

    #[async_trait]
    impl Prover for FixtureProver {
        async fn prove(&self, _input: GuestInput) -> Result<ProvedBatch, ProverError> {
            // Honest dev/demo delay: sleep BEFORE returning the real receipt so the
            // async UX (the `proving` state) is visible without a GPU. NOT a fake
            // proof — the receipt below is the real on-disk N=8 receipt.
            let delay = Self::prove_delay();
            if !delay.is_zero() {
                tokio::time::sleep(delay).await;
            }
            // LOAD the real file — never hand-build a ProvedBatch.
            load_proved_batch(&self.receipt_dir)
        }

        fn backend_label(&self) -> &'static str {
            "fixture(real N=8 receipt, test-only)"
        }
    }

    #[cfg(test)]
    mod fixture_tests {
        use super::*;

        /// With `FIXTURE_PROVE_DELAY` set, the fixture prove sleeps at least that
        /// long BEFORE returning the real receipt (it still returns the real one).
        #[tokio::test]
        async fn fixture_prove_delay_is_respected() {
            // Use a tiny but observable delay to keep the test fast.
            std::env::set_var("FIXTURE_PROVE_DELAY", "1");
            let dur = FixtureProver::prove_delay();
            assert_eq!(dur, std::time::Duration::from_secs(1));

            // The delay actually elapses on the prove path (real receipt loaded).
            let receipt_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .join("out/bench/n8");
            let prover = FixtureProver::new(&receipt_dir);
            let start = std::time::Instant::now();
            let proved = prover
                .prove(GuestInput {
                    notes: vec![],
                    merkle_root: [0u8; 32],
                })
                .await
                .expect("real receipt loads");
            assert!(
                start.elapsed() >= std::time::Duration::from_secs(1),
                "delay must elapse before returning"
            );
            // It is the REAL receipt (a real Groth16 seal is > 64 bytes).
            assert!(proved.seal.len() > 64, "real receipt, not fabricated");
            std::env::remove_var("FIXTURE_PROVE_DELAY");
        }

        /// With the env var unset, there is no delay (default 0).
        #[test]
        fn fixture_prove_delay_defaults_to_zero() {
            std::env::remove_var("FIXTURE_PROVE_DELAY");
            assert!(FixtureProver::prove_delay().is_zero());
        }
    }
}
