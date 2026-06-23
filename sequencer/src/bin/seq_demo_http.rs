//! `seq_demo_http` — the s3/02 gate's HTTP server, built ONLY with
//! `--features test-fixture` (`required-features` in `Cargo.toml`).
//!
//! **LOCK 1 (candado 1):** a plain `cargo build` skips this binary entirely, so the
//! `FixtureProver` it wires is structurally UNREACHABLE from production. The prod
//! `sequencer` bin uses `LocalProver`/`RemoteProver` only.
//!
//! What it does: serve the SAME HTTP router as production (`sequencer::http::serve`)
//! but with the [`FixtureProver`] injected — which LOADS the real pre-generated
//! N=8 receipt (`out/bench/n8/`) — plus the REAL [`StellarCliSettler`]. So the gate
//! exercises the full frontend (3 views + async UX + recent-batches + `failed`)
//! against a REAL on-chain settle of a REAL receipt, with NO GPU.
//!
//! Honest dev/demo knob `FIXTURE_PROVE_DELAY` (seconds) makes the fixture sleep
//! that long BEFORE returning the real receipt — exercising the `proving` state
//! visually. It is NOT a fake proof (receipt + settle + verifier all real).
//!
//! Env: `ROLLUP_ID` (the freshly-deployed rollup `C…` the gate settles against),
//! `SIGNER`/`NETWORK` (settle source/network), `BIND` (default `127.0.0.1:8787`),
//! `N_TARGET`/`BATCH_TIMEOUT`, `FIXTURE_PROVE_DELAY`.

use std::path::PathBuf;

use sequencer::pipeline::Config;
use sequencer::prover::{FixtureProver, Prover};
use sequencer::settler::{Settler, StellarCliSettler};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("sequencer/ has a parent (workspace root)")
        .to_path_buf()
}

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

#[tokio::main]
async fn main() -> std::process::ExitCode {
    let cfg = build_config();

    // FixtureProver: LOADS the real N=8 receipt (never fabricates). The honest
    // FIXTURE_PROVE_DELAY knob exercises the `proving` UX without a GPU.
    let receipt_dir = workspace_root().join("out/bench/n8");
    let prover: Box<dyn Prover> = Box::new(FixtureProver::new(&receipt_dir));

    // REAL settler — the gate runs a REAL on-chain settle (cero-mocks intact).
    let rollup_id = match std::env::var("ROLLUP_ID") {
        Ok(id) => id,
        Err(_) => {
            eprintln!(
                "[seq_demo_http] ERROR: ROLLUP_ID env required (the deployed rollup C… to settle \
                 against). The gate deploys a fresh rollup and passes it here."
            );
            return std::process::ExitCode::FAILURE;
        }
    };
    let source = std::env::var("SIGNER").unwrap_or_else(|_| "spikekey".to_string());
    let settler: Box<dyn Settler> =
        Box::new(StellarCliSettler::new(rollup_id, source, &cfg.network));

    let delay = FixtureProver::prove_delay();
    println!("=== seq_demo_http — s3/02 gate HTTP server (FixtureProver, no GPU) ===");
    println!(
        "[seq_demo_http] prover : {} (FIXTURE_PROVE_DELAY={}s — honest dev delay before the REAL receipt)",
        prover.backend_label(),
        delay.as_secs()
    );
    println!("[seq_demo_http] settler: {}", settler.backend_label());

    let addr: std::net::SocketAddr = match std::env::var("BIND")
        .unwrap_or_else(|_| "127.0.0.1:8787".to_string())
        .parse()
    {
        Ok(a) => a,
        Err(e) => {
            eprintln!("[seq_demo_http] ERROR: bad BIND addr: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    println!(
        "[seq_demo_http] HTTP API on http://{addr}  (POST /submit · GET /status/:id · \
         GET /recent_batches · GET /config) — N_target={}",
        cfg.n_target
    );

    match sequencer::http::serve(addr, prover, settler, cfg).await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[seq_demo_http] ERROR: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}
