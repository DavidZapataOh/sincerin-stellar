//! Production sequencer binary.
//!
//! **LOCK 1 (structural):** this binary builds with NO cargo features, so the
//! `test-fixture` module — and therefore `FixtureProver` — is not even compiled
//! here. The only provers reachable from production are `LocalProver` (real
//! `host prove`, `RISC0_DEV_MODE=0`) and `RemoteProver` (RunPod GPU, real
//! `host prove`, s3/05). Backend selection is pure config
//! (`PROVER_BACKEND=local|remote`); the sequencer's state machine, lock, and
//! collision logic never change between them.
//!
//! Subcommands:
//! - (default) — print the trust-boundary banner + selected backend.
//! - `serve` — wire `LocalProver` + `StellarCliSettler`, serve the HTTP API
//!   (s3/02) the frontend talks to. The on-chain settle is REAL
//!   (`stellar contract invoke … settle_batch`); there is NO fixture/dev-mode/mock
//!   anywhere on this path.
//!
//! Settle config is read from env: `ROLLUP_ID` (the deployed rollup `C…`),
//! `SIGNER` (the `--source` key), `NETWORK` (default `testnet`). HTTP bind from
//! `BIND` (default `127.0.0.1:8787`). Batch knobs from `N_TARGET`/`BATCH_TIMEOUT`.

use std::path::PathBuf;

use sequencer::pipeline::Config;
use sequencer::prover::{LocalProver, Prover, RemoteProver};
use sequencer::settler::{Settler, StellarCliSettler};

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
        "remote" => {
            // s3/05: the RunPod serverless GPU worker (real host prove, native CUDA).
            let endpoint = std::env::var("RUNPOD_ENDPOINT_ID").map_err(|_| {
                "RUNPOD_ENDPOINT_ID env required for PROVER_BACKEND=remote (the RunPod \
                 serverless endpoint id)"
                    .to_string()
            })?;
            let api_key = std::env::var("RUNPOD_API_KEY")
                .map_err(|_| "RUNPOD_API_KEY env required for PROVER_BACKEND=remote".to_string())?;
            Ok(Box::new(RemoteProver::new(endpoint, api_key)))
        }
        other => Err(format!(
            "unknown PROVER_BACKEND={other:?}; use 'local' (real host prove) or 'remote' \
             (RunPod GPU). There is no fixture/dev-mode backend in the production binary."
        )),
    }
}

/// Build the REAL settler from env (`ROLLUP_ID`, `SIGNER`, `NETWORK`). There is no
/// mock settler in production — this always shells out to `stellar contract invoke`.
fn select_settler(network: &str) -> Result<Box<dyn Settler>, String> {
    let rollup_id = std::env::var("ROLLUP_ID")
        .map_err(|_| "ROLLUP_ID env required (the deployed rollup C… to settle against)")?;
    let source = std::env::var("SIGNER").unwrap_or_else(|_| "spikekey".to_string());
    Ok(Box::new(StellarCliSettler::new(rollup_id, source, network)))
}

/// Build the runtime [`Config`] from env (`N_TARGET`, `BATCH_TIMEOUT`, `NETWORK`,
/// `ROLLUP_ID`).
fn build_config() -> Config {
    let mut cfg = Config::testnet_defaults();
    cfg.network = std::env::var("NETWORK").unwrap_or_else(|_| "testnet".to_string());
    cfg.explorer_base = format!("https://stellar.expert/explorer/{}/tx/", cfg.network);
    if let Ok(id) = std::env::var("ROLLUP_ID") {
        cfg.rollup_id = id;
    }
    if let Some(n) = std::env::var("N_TARGET").ok().and_then(|s| s.parse().ok()) {
        cfg.n_target = n;
    }
    if let Some(s) = std::env::var("BATCH_TIMEOUT")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
    {
        cfg.batch_timeout = std::time::Duration::from_secs(s);
    }
    cfg
}

fn print_banner() {
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
}

/// `sequencer serve` — wire LocalProver + StellarCliSettler, serve the HTTP API.
async fn run_serve() -> Result<(), String> {
    let cfg = build_config();
    let prover = select_prover()?;
    let settler = select_settler(&cfg.network)?;

    print_banner();
    println!("[sequencer] prover backend : {}", prover.backend_label());
    println!("[sequencer] settler backend: {}", settler.backend_label());

    let addr: std::net::SocketAddr = std::env::var("BIND")
        .unwrap_or_else(|_| "127.0.0.1:8787".to_string())
        .parse()
        .map_err(|e| format!("bad BIND addr: {e}"))?;
    println!(
        "[sequencer] HTTP API on http://{addr}  (POST /submit · GET /status/:id · \
         GET /recent_batches · GET /config) — N_target={}",
        cfg.n_target
    );
    sequencer::http::serve(addr, prover, settler, cfg).await
}

fn main() -> std::process::ExitCode {
    let cmd = std::env::args().nth(1);
    match cmd.as_deref() {
        Some("serve") => {
            // Build a multi-thread runtime only for the serving path.
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    eprintln!("[sequencer] ERROR: tokio runtime: {e}");
                    return std::process::ExitCode::FAILURE;
                }
            };
            match rt.block_on(run_serve()) {
                Ok(()) => std::process::ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("[sequencer] ERROR: {e}");
                    std::process::ExitCode::FAILURE
                }
            }
        }
        _ => {
            // Default: the MVP banner + backend selection sanity check (no serve).
            print_banner();
            match select_prover() {
                Ok(p) => {
                    println!("[sequencer] prover backend: {}", p.backend_label());
                    println!(
                        "[sequencer] MVP CLI ready. Run `sequencer serve` to expose the HTTP API \
                         (the frontend connection layer). The async batching loop + on-chain \
                         settle are also exercised by `bash scripts/seq_demo.sh`."
                    );
                    std::process::ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("[sequencer] ERROR: {e}");
                    std::process::ExitCode::FAILURE
                }
            }
        }
    }
}
