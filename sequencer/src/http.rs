//! The HTTP API (s3/02) — the connection layer the demo frontend talks to.
//!
//! The router is built over a [`SharedState`] (the sequencer + injected
//! prover/settler + config) and a spawned background [`run_pipeline`]. Endpoints:
//!
//! - `POST /submit`          → `{ "request_id": "<id>" }` IMMEDIATELY (never blocks).
//! - `GET  /status/:id`      → the request's state (+ `prover_phase`, `tx_hash`, …).
//! - `GET  /recent_batches`  → settled batches, most-recent-first (seeded, never empty).
//! - `GET  /config`          → network/explorer/n_target/rollup_id/verifier_id so
//!   the frontend NEVER hardcodes contract addresses.
//!
//! CORS is enabled (the frontend is a separate origin). The prover + settler are
//! INJECTED by the caller ([`serve`]) → the lib never picks a backend; the binary
//! does (preserving candado 1: the FixtureProver is unreachable from prod).

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tower_http::cors::{Any, CorsLayer};

use crate::pipeline::{run_pipeline, Config, ProverPhase, SharedState};
use crate::prover::Prover;
use crate::settler::Settler;
use crate::types::{RequestId, Status, WithdrawalIntent};
use crate::Sequencer;

/// The `POST /submit` body — the withdrawal intent fields (32-byte LE values as
/// hex strings; `amount` as a number; `path` as an array of hex siblings). This
/// is the wire shape the frontend sends; it deserializes into a [`WithdrawalIntent`].
#[derive(Debug, Deserialize)]
pub struct SubmitBody {
    /// Spending secret, 32B LE hex (`0x…` or bare).
    pub secret: String,
    /// Commitment blinding, 32B LE hex.
    pub blinding: String,
    /// In-claro amount (= payout).
    pub amount: u128,
    /// Payout recipient, 32B hex (the judge's address, copied to the journal).
    pub recipient: String,
    /// Merkle authentication path, one 32B LE hex sibling per level.
    pub path: Vec<String>,
    /// Leaf index of this note's commitment.
    pub index: u64,
    /// Public Merkle root, 32B LE hex.
    pub merkle_root: String,
}

/// Parse `0x…`/bare 32-byte hex into `[u8; 32]`.
fn hex32(s: &str) -> Result<[u8; 32], String> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    let v = hex::decode(s).map_err(|e| format!("bad hex {s:?}: {e}"))?;
    if v.len() != 32 {
        return Err(format!("expected 32 bytes, got {}", v.len()));
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&v);
    Ok(out)
}

impl SubmitBody {
    /// Convert the wire body into a [`WithdrawalIntent`] (validates hex + lengths).
    pub fn into_intent(self) -> Result<WithdrawalIntent, String> {
        let mut path = Vec::with_capacity(self.path.len());
        for (i, sib) in self.path.iter().enumerate() {
            path.push(hex32(sib).map_err(|e| format!("path[{i}]: {e}"))?);
        }
        Ok(WithdrawalIntent {
            secret: hex32(&self.secret).map_err(|e| format!("secret: {e}"))?,
            blinding: hex32(&self.blinding).map_err(|e| format!("blinding: {e}"))?,
            amount: self.amount,
            recipient: hex32(&self.recipient).map_err(|e| format!("recipient: {e}"))?,
            path,
            index: self.index,
            merkle_root: hex32(&self.merkle_root).map_err(|e| format!("merkle_root: {e}"))?,
        })
    }
}

/// `POST /submit` response — the pollable request id (as a string, opaque).
#[derive(Debug, Serialize)]
pub struct SubmitResponse {
    /// The request id (e.g. `"req-0"`) to poll `GET /status/:id` with.
    pub request_id: String,
}

/// `GET /status/:id` response — the API state machine projected to JSON.
#[derive(Debug, Serialize)]
pub struct StatusResponse {
    /// `pending | batched | proving | settled | failed`.
    pub state: &'static str,
    /// Present only while `state == proving`: `starting | proving`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prover_phase: Option<&'static str>,
    /// Present only when `state == settled`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_hash: Option<String>,
    /// Present only when `state == failed`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// How many requests are currently in flight toward the next batch.
    pub batch_size: usize,
    /// The configured batch target (N).
    pub n_target: usize,
}

/// `GET /config` response — everything the frontend needs so it NEVER hardcodes a
/// contract address.
#[derive(Debug, Serialize)]
pub struct ConfigResponse {
    /// e.g. `"testnet"`.
    pub network: String,
    /// Explorer base for tx links.
    pub explorer_base: String,
    /// The batch target (N).
    pub n_target: usize,
    /// The deployed rollup id.
    pub rollup_id: String,
    /// The deployed verifier id (e.g. `CBQF…`).
    pub verifier_id: String,
}

/// Build the axum [`Router`] over `state`. Exposed for testing (no server bound).
pub fn router(state: Arc<SharedState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);
    Router::new()
        .route("/submit", post(submit))
        .route("/status/{id}", get(status))
        .route("/recent_batches", get(recent_batches))
        .route("/config", get(config))
        .layer(cors)
        .with_state(state)
}

/// Serve the HTTP API on `addr` with an INJECTED prover + settler. Spawns the
/// background pipeline, then runs the server until the process exits.
///
/// The lib NEVER selects a backend; the binary passes the boxed prover/settler →
/// candado 1 (the FixtureProver stays unreachable from the production binary).
pub async fn serve(
    addr: std::net::SocketAddr,
    prover: Box<dyn Prover>,
    settler: Box<dyn Settler>,
    config: Config,
) -> Result<(), String> {
    let state = SharedState::new(Sequencer::new(config.n_target), prover, settler, config);

    // background pipeline (assembles, proves, settles) — never blocks /submit.
    tokio::spawn(run_pipeline(state.clone()));

    let app = router(state);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| format!("bind {addr}: {e}"))?;
    axum::serve(listener, app)
        .await
        .map_err(|e| format!("serve: {e}"))
}

// ── handlers ──────────────────────────────────────────────────────────────────

async fn submit(
    State(state): State<Arc<SharedState>>,
    Json(body): Json<SubmitBody>,
) -> impl IntoResponse {
    let intent = match body.into_intent() {
        Ok(i) => i,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": e})),
            )
                .into_response()
        }
    };
    let mut seq = state.seq.lock().await;
    match seq.submit_withdrawal(intent) {
        Ok(id) => (
            StatusCode::OK,
            Json(SubmitResponse {
                request_id: id.to_string(),
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::CONFLICT,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Parse a `req-<n>` (or bare `<n>`) id string into a [`RequestId`].
fn parse_id(s: &str) -> Option<RequestId> {
    let n = s.strip_prefix("req-").unwrap_or(s);
    n.parse::<u64>().ok().map(RequestId)
}

async fn status(
    State(state): State<Arc<SharedState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Some(req_id) = parse_id(&id) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "bad id"})),
        )
            .into_response();
    };
    let (status, batch_size, n_target) = {
        let seq = state.seq.lock().await;
        let st = seq.get_status(req_id);
        (st, seq.pending_count(), state.config.n_target)
    };
    let Some(st) = status else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "unknown request_id"})),
        )
            .into_response();
    };

    let prover_phase = if matches!(st, Status::Proving) {
        state
            .prover_phase
            .lock()
            .await
            .get(&req_id)
            .map(|p| match p {
                ProverPhase::Starting => "starting",
                ProverPhase::Proving => "proving",
            })
    } else {
        None
    };

    let resp = match &st {
        Status::Pending => StatusResponse {
            state: "pending",
            prover_phase: None,
            tx_hash: None,
            reason: None,
            batch_size,
            n_target,
        },
        Status::Batched => StatusResponse {
            state: "batched",
            prover_phase: None,
            tx_hash: None,
            reason: None,
            batch_size,
            n_target,
        },
        Status::Proving => StatusResponse {
            state: "proving",
            // default to "starting" if the phase map has not been set yet.
            prover_phase: Some(prover_phase.unwrap_or("starting")),
            tx_hash: None,
            reason: None,
            batch_size,
            n_target,
        },
        Status::Settled { tx_hash } => StatusResponse {
            state: "settled",
            prover_phase: None,
            tx_hash: Some(tx_hash.clone()),
            reason: None,
            batch_size,
            n_target,
        },
        Status::Failed { reason } => StatusResponse {
            state: "failed",
            prover_phase: None,
            tx_hash: None,
            reason: Some(reason.clone()),
            batch_size,
            n_target,
        },
    };
    (StatusCode::OK, Json(resp)).into_response()
}

async fn recent_batches(State(state): State<Arc<SharedState>>) -> impl IntoResponse {
    let recent = state.recent_batches.lock().await.clone();
    Json(recent)
}

async fn config(State(state): State<Arc<SharedState>>) -> impl IntoResponse {
    let c = &state.config;
    Json(ConfigResponse {
        network: c.network.clone(),
        explorer_base: c.explorer_base.clone(),
        n_target: c.n_target,
        rollup_id: c.rollup_id.clone(),
        verifier_id: c.verifier_id.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::HISTORIC_N8_TX;
    use crate::prover::{Prover, ProverError};
    use crate::settler::{SettleError, Settler};
    use crate::types::ProvedBatch;
    use async_trait::async_trait;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt; // for `oneshot`
    use zk_core::witness::GuestInput;

    // ── marked unit-test doubles (#[cfg(test)] only) ─────────────────────────
    struct FakeProver;
    #[async_trait]
    impl Prover for FakeProver {
        async fn prove(&self, _: GuestInput) -> Result<ProvedBatch, ProverError> {
            Ok(ProvedBatch {
                seal: vec![0xAA; 100],
                image_id: [0x11; 32],
                journal: vec![0x01],
            })
        }
        fn backend_label(&self) -> &'static str {
            "fake"
        }
    }
    struct FakeSettler;
    #[async_trait]
    impl Settler for FakeSettler {
        async fn settle(&self, _: &ProvedBatch) -> Result<String, SettleError> {
            Ok("settletx0000000000000000000000000000000000000000000000000000abcd".into())
        }
        fn backend_label(&self) -> String {
            "fake".into()
        }
    }

    fn test_state(n_target: usize) -> Arc<SharedState> {
        let mut cfg = Config::testnet_defaults();
        cfg.n_target = n_target;
        cfg.rollup_id = "CROLLUPTEST".into();
        SharedState::new(
            Sequencer::new(n_target),
            Box::new(FakeProver),
            Box::new(FakeSettler),
            cfg,
        )
    }

    fn submit_body_json(seed: u8) -> String {
        let h = |b: u8| format!("0x{}", hex::encode([b; 32]));
        serde_json::json!({
            "secret": h(seed),
            "blinding": h(seed.wrapping_add(1)),
            "amount": 100,
            "recipient": h(seed.wrapping_add(2)),
            "path": [h(seed.wrapping_add(3))],
            "index": seed as u64,
            "merkle_root": h(9),
        })
        .to_string()
    }

    async fn body_json(resp: axum::response::Response) -> serde_json::Value {
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    // ── POST /submit returns a request_id immediately ────────────────────────
    #[tokio::test]
    async fn post_submit_returns_request_id() {
        let app = router(test_state(8));
        let req = Request::builder()
            .method("POST")
            .uri("/submit")
            .header("content-type", "application/json")
            .body(Body::from(submit_body_json(1)))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let v = body_json(resp).await;
        assert_eq!(v["request_id"], "req-0");
    }

    // ── GET /status/:id reflects pending → settled (driven manually) ─────────
    #[tokio::test]
    async fn status_transitions_pending_batched_proving_settled() {
        let state = test_state(1);

        // submit via HTTP → request_id.
        let resp = router(state.clone())
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/submit")
                    .header("content-type", "application/json")
                    .body(Body::from(submit_body_json(2)))
                    .unwrap(),
            )
            .await
            .unwrap();
        let id = body_json(resp).await["request_id"]
            .as_str()
            .unwrap()
            .to_string();

        // pending.
        let get_status = |st: Arc<SharedState>, id: String| async move {
            let resp = router(st)
                .oneshot(
                    Request::builder()
                        .uri(format!("/status/{id}"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            body_json(resp).await
        };

        let v = get_status(state.clone(), id.clone()).await;
        assert_eq!(v["state"], "pending");
        assert_eq!(v["n_target"], 1);

        // assemble → batched.
        let batch = {
            let mut seq = state.seq.lock().await;
            seq.try_assemble_batch().unwrap()
        };
        let v = get_status(state.clone(), id.clone()).await;
        assert_eq!(v["state"], "batched");

        // drive to settled (uses the fakes).
        let tx = crate::pipeline::drive_batch(&state, batch).await.unwrap();
        let v = get_status(state.clone(), id.clone()).await;
        assert_eq!(v["state"], "settled");
        assert_eq!(v["tx_hash"], tx);
    }

    // ── status exposes prover_phase while Proving ────────────────────────────
    #[tokio::test]
    async fn status_exposes_prover_phase_when_proving() {
        let state = test_state(1);
        let (req_id, batch) = {
            let mut seq = state.seq.lock().await;
            let id = seq
                .submit_withdrawal(submit_body(3).into_intent().unwrap())
                .unwrap();
            let b = seq.try_assemble_batch().unwrap();
            seq.mark_proving(&b);
            (id, b)
        };
        // set the phase to "starting" (what the pipeline does first).
        {
            let mut map = state.prover_phase.lock().await;
            map.insert(req_id, ProverPhase::Starting);
        }
        let resp = router(state.clone())
            .oneshot(
                Request::builder()
                    .uri(format!("/status/{req_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let v = body_json(resp).await;
        assert_eq!(v["state"], "proving");
        assert_eq!(v["prover_phase"], "starting");
        let _ = batch;
    }

    fn submit_body(seed: u8) -> SubmitBody {
        serde_json::from_str(&submit_body_json(seed)).unwrap()
    }

    // ── GET /recent_batches non-empty + includes the historic hash ───────────
    #[tokio::test]
    async fn recent_batches_includes_historic_hash() {
        let app = router(test_state(8));
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/recent_batches")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let v = body_json(resp).await;
        let arr = v.as_array().unwrap();
        assert!(!arr.is_empty(), "recent_batches must never be empty");
        assert!(
            arr.iter().any(|b| b["tx_hash"] == HISTORIC_N8_TX),
            "must include the historic N=8 hash"
        );
        assert!(arr[0]["explorer_url"]
            .as_str()
            .unwrap()
            .ends_with(HISTORIC_N8_TX));
    }

    // ── GET /config exposes contract addresses (so the FE hardcodes nothing) ─
    #[tokio::test]
    async fn config_exposes_contract_addresses() {
        let app = router(test_state(8));
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/config")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let v = body_json(resp).await;
        assert_eq!(v["network"], "testnet");
        assert_eq!(v["n_target"], 8);
        assert_eq!(v["rollup_id"], "CROLLUPTEST");
        assert_eq!(
            v["verifier_id"],
            "CBQFQLSBYXUYLD2Q5EWHVNNI6VO33NAVRDUDIGJNMC5TUAINK5BXO2LJ"
        );
        assert_eq!(
            v["explorer_base"],
            "https://stellar.expert/explorer/testnet/tx/"
        );
    }

    // ── status of an unknown id is 404 (not a hang) ──────────────────────────
    #[tokio::test]
    async fn status_unknown_id_is_404() {
        let app = router(test_state(8));
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/status/req-999")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}
