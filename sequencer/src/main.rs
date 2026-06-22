//! Production sequencer binary.
//!
//! **LOCK 1 (structural):** this binary builds with NO cargo features, so the
//! `test-fixture` module — and therefore `FixtureProver` — is not even compiled
//! here. The only provers reachable from production are `LocalProver` (real
//! `host prove`, `RISC0_DEV_MODE=0`) and, in s3, `RemoteProver` (GPU). Backend
//! selection is pure config (`PROVER_BACKEND=local|remote`); the sequencer's
//! state machine, lock, and collision logic never change between them.
//!
//! This MVP binary is a thin CLI around the library: it prints the trust-boundary
//! banner and the selected backend, then would run the async batching loop. The
//! orchestration is exercised end-to-end (with the REAL N=8 receipt + REAL
//! verifier) by `scripts/seq_demo.sh`, which drives the `seq_demo` binary built
//! ONLY with `--features test-fixture`.

use std::path::PathBuf;

use sequencer::prover::{LocalProver, Prover};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("sequencer/ has a parent (workspace root)")
        .to_path_buf()
}

/// Select the production prover from `PROVER_BACKEND` (default `local`).
/// `remote` is reserved for s3 (GPU) and is rejected here with a clear message —
/// NEVER does this binary fall back to a fixture or dev-mode.
fn select_prover() -> Result<Box<dyn Prover>, String> {
    let backend = std::env::var("PROVER_BACKEND").unwrap_or_else(|_| "local".to_string());
    match backend.as_str() {
        "local" => Ok(Box::new(LocalProver::new(workspace_root()))),
        "remote" => Err(
            "PROVER_BACKEND=remote (GPU) is wired in s3 (RemoteProver); not built in this MVP. \
             Use PROVER_BACKEND=local (real host prove)."
                .to_string(),
        ),
        other => Err(format!(
            "unknown PROVER_BACKEND={other:?}; use 'local' (or 'remote' in s3). \
             There is no fixture/dev-mode backend in the production binary."
        )),
    }
}

fn main() -> std::process::ExitCode {
    println!("=== Confidential Payments Rollup — sequencer (single-operator MVP) ===");
    println!(
        "TRUST BOUNDARY (AC4.4): the operator receives note secrets (Diseño B) and \
         therefore SEES the note↔recipient mapping. Unlinkability is ON-CHAIN/public, \
         NOT against the operator. See sequencer/README.md."
    );
    println!(
        "ZK LATENCY: the prove is multi-hour; every rollup batches+proves off-chain. \
         submit() returns a request_id immediately and never blocks — poll get_status()."
    );

    match select_prover() {
        Ok(p) => {
            println!("[sequencer] prover backend: {}", p.backend_label());
            println!(
                "[sequencer] MVP CLI ready. The async batching loop + on-chain settle are \
                 demonstrated end-to-end (REAL N=8 receipt, REAL verifier) by \
                 `bash scripts/seq_demo.sh`."
            );
            std::process::ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("[sequencer] ERROR: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}
