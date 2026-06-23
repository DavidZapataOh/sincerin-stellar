//! The on-chain settle seam (s3/02) — the ONLY frontier between the sequencer and
//! the deployed rollup contract.
//!
//! The sequencer hands a proven batch to a [`Settler`], which sends the REAL
//! on-chain `settle_batch` and returns the settle transaction hash. Local vs the
//! deployed contract is pure config; the state machine never changes.
//!
//! - [`StellarCliSettler`] (production, the REAL settle): shells out to
//!   `stellar contract invoke … settle_batch --seal {hex} --image_id {hex}
//!   --journal_bytes {hex}` — byte-for-byte the same invoke `scripts/seq_demo.sh`
//!   runs — and parses the 64-hex tx hash from the output. There is NO mock settle
//!   in the product path.
//!
//! The exact `stellar` argv it constructs and the tx-hash parser are split out as
//! PURE functions ([`settle_argv`], [`parse_tx_hash`]) so they are unit-tested
//! without spawning a process or touching the chain.

use async_trait::async_trait;

use crate::types::ProvedBatch;

/// Errors the settle step can surface. The driver maps these to a `Failed` status
/// (the note's reservation is released, so the user can re-submit).
#[derive(Debug)]
pub enum SettleError {
    /// The `stellar` process could not be spawned or exited non-zero.
    Invoke(String),
    /// The invoke ran but no 64-hex transaction hash was found in its output.
    NoTxHash(String),
    /// The invoke produced a tx hash, but the on-chain transaction did NOT apply
    /// successfully (RPC `getTransaction` returned a status other than `SUCCESS`).
    /// Carries the real (resolvable) hash and the observed status — the funds did
    /// not move, so this must NEVER be reported as settled.
    OnchainFailed {
        /// The real (resolvable) settle tx hash that did not apply successfully.
        hash: String,
        /// The observed on-chain status (e.g. `FAILED`, `NOT_FOUND`).
        status: String,
    },
}

impl core::fmt::Display for SettleError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            SettleError::Invoke(s) => write!(f, "settle invoke: {s}"),
            SettleError::NoTxHash(s) => write!(f, "settle produced no tx hash: {s}"),
            SettleError::OnchainFailed { hash, status } => {
                write!(f, "settle tx {hash} did not succeed on-chain (status {status})")
            }
        }
    }
}

impl std::error::Error for SettleError {}

/// The on-chain settle interface. `settle` sends the REAL `settle_batch` and
/// returns the settle tx hash (hex). The sequencer always calls it off the submit
/// path (the background pipeline), so the user never blocks.
#[async_trait]
pub trait Settler: Send + Sync {
    /// Send `proved` on-chain via `settle_batch` and return the tx hash (hex).
    /// Implementations MUST perform a real on-chain settle (no fabricated hash) on
    /// the product path.
    async fn settle(&self, proved: &ProvedBatch) -> Result<String, SettleError>;

    /// A short label for logging/telemetry (e.g. "stellar-cli(testnet)").
    fn backend_label(&self) -> String;
}

/// Build the exact `stellar` argv for a `settle_batch` invoke against `rollup_id`,
/// signed by `source`, on `network`. Mirrors `scripts/seq_demo.sh` byte-for-byte:
///
/// ```text
/// stellar contract invoke --id <rollup_id> --source <source> --network <network> \
///   --send=yes -- settle_batch --seal <hex> --image_id <hex> --journal_bytes <hex>
/// ```
///
/// The seal/image_id/journal are hex-encoded (no `0x` prefix — the same form the
/// script passes). Returns the argv WITHOUT the leading `stellar` program name
/// (the caller spawns `Command::new("stellar")`).
pub fn settle_argv(
    rollup_id: &str,
    source: &str,
    network: &str,
    proved: &ProvedBatch,
) -> Vec<String> {
    let seal_hex = hex::encode(&proved.seal);
    let image_id_hex = hex::encode(proved.image_id);
    let journal_hex = hex::encode(&proved.journal);
    vec![
        "contract".into(),
        "invoke".into(),
        "--id".into(),
        rollup_id.into(),
        "--source".into(),
        source.into(),
        "--network".into(),
        network.into(),
        "--send=yes".into(),
        "--".into(),
        "settle_batch".into(),
        "--seal".into(),
        seal_hex,
        "--image_id".into(),
        image_id_hex,
        "--journal_bytes".into(),
        journal_hex,
    ]
}

/// Parse the settle transaction hash out of `stellar`'s combined output
/// (stdout+stderr): the FIRST 64-hex run that is NOT `exclude`.
///
/// CRITICAL: `stellar contract invoke` echoes the `--image_id <hex>` arg, and the
/// image_id is ALSO exactly 32 bytes → 64 hex — the same width as a tx hash, and it
/// appears BEFORE the tx hash. A naive "first 64-hex" therefore returns the
/// image_id (`cbeab7aa…`), which a judge would click into a 404 on the explorer.
/// `exclude` is the image_id hex; we skip it so the OTHER 64-hex run (the real tx
/// hash) is returned. The seal/journal hex are far longer than 64 (single runs) so
/// they never match `run == 64`.
///
/// Returns `None` if no qualifying 64-char lowercase-hex run is present.
pub fn parse_tx_hash(output: &str, exclude: &str) -> Option<String> {
    let bytes = output.as_bytes();
    let is_hex = |b: u8| b.is_ascii_digit() || (b'a'..=b'f').contains(&b);
    let mut i = 0;
    while i < bytes.len() {
        if is_hex(bytes[i]) {
            let start = i;
            while i < bytes.len() && is_hex(bytes[i]) {
                i += 1;
            }
            if i - start == 64 {
                let run = &output[start..i];
                if run != exclude {
                    return Some(run.to_string());
                }
            }
        } else {
            i += 1;
        }
    }
    None
}

/// Decide the settle tx hash from a raw `stellar` invoke result. PURE.
///
/// EJE-1 gate: a NON-success exit (`ok == false`) NEVER yields a hash. A failed
/// invoke's diagnostics can carry OTHER 64-hex values (notably the contract-
/// computed `sha256(journal_bytes)` digest) that would 404 on the explorer if
/// mis-reported as a settle tx — so success is required BEFORE parsing. On
/// success, the image_id (echoed as `--image_id`, also 64 hex) is excluded so the
/// real tx hash is returned.
pub fn parse_settle_output(
    ok: bool,
    combined: &str,
    image_id_hex: &str,
) -> Result<String, SettleError> {
    if !ok {
        return Err(SettleError::Invoke(format!(
            "stellar invoke exited non-zero: {}",
            combined.trim()
        )));
    }
    match parse_tx_hash(combined, image_id_hex) {
        Some(h) => Ok(h),
        None => Err(SettleError::NoTxHash(combined.trim().to_string())),
    }
}

/// Read `.result.status` out of an RPC `getTransaction` JSON response. PURE.
/// Returns `None` if the body isn't JSON or has no `result.status` string (e.g. an
/// RPC-level `error`) — the caller treats that as "not confirmed".
pub fn parse_rpc_status(json: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    v.get("result")?
        .get("status")?
        .as_str()
        .map(|s| s.to_string())
}

/// Map an on-chain `getTransaction` status to the final settle result. PURE.
///
/// EJE-3 invariant: ONLY `SUCCESS` yields a settled tx hash. Any other status
/// (`FAILED`, `NOT_FOUND`, `PENDING`, `UNKNOWN`, …) is an error → the driver marks
/// the batch `Failed`, releases the lock, and the note can be re-submitted. The
/// funds path is never reported settled on a tx that didn't actually apply.
pub fn finalize_status(hash: String, status: &str) -> Result<String, SettleError> {
    if status == "SUCCESS" {
        Ok(hash)
    } else {
        Err(SettleError::OnchainFailed {
            hash,
            status: status.to_string(),
        })
    }
}

/// Default Soroban RPC URL for a known network — mirrors the gate scripts
/// (`scripts/seq_demo.sh`, `scripts/seq_http_gate.sh`). Unknown networks fall back
/// to the testnet RPC (the only network the product path settles on today).
pub fn default_rpc_url(network: &str) -> String {
    match network {
        "futurenet" => "https://rpc-futurenet.stellar.org",
        _ => "https://soroban-testnet.stellar.org",
    }
    .to_string()
}

/// Production settler: shells out to the real `stellar contract invoke … settle_batch`
/// (identical to `scripts/seq_demo.sh`) and parses the tx hash. This is the ONLY
/// settler on the product path — there is no mock.
pub struct StellarCliSettler {
    /// The deployed rollup contract id (`C…`).
    rollup_id: String,
    /// The signer key/name passed to `--source`.
    source: String,
    /// The network passed to `--network` (e.g. `testnet`).
    network: String,
    /// Soroban RPC URL used to re-confirm `getTransaction == SUCCESS` before a
    /// batch is reported settled (derived from `network`; overridable for tests).
    rpc_url: String,
}

impl StellarCliSettler {
    /// Build a settler that settles against `rollup_id`, signing with `source`, on
    /// `network`. The RPC URL is derived from the network ([`default_rpc_url`]).
    pub fn new(
        rollup_id: impl Into<String>,
        source: impl Into<String>,
        network: impl Into<String>,
    ) -> Self {
        let network = network.into();
        let rpc_url = default_rpc_url(&network);
        Self {
            rollup_id: rollup_id.into(),
            source: source.into(),
            network,
            rpc_url,
        }
    }

    /// Override the Soroban RPC URL (e.g. a private endpoint). Builder form.
    pub fn with_rpc_url(mut self, rpc_url: impl Into<String>) -> Self {
        self.rpc_url = rpc_url.into();
        self
    }

    /// Re-confirm a submitted settle tx on-chain via RPC `getTransaction`, returning
    /// the observed status string. A tx may briefly read `NOT_FOUND`/`PENDING` right
    /// after submit, so we poll a few times with a short backoff and accept the first
    /// terminal status (`SUCCESS`/`FAILED`). Any transient/unreachable case falls
    /// through to the last-seen status (typically `NOT_FOUND`) → treated as not
    /// confirmed by [`finalize_status`]. Shells `curl` (same as the gate scripts; no
    /// new HTTP dependency).
    async fn confirm_status(&self, hash: &str) -> Result<String, SettleError> {
        let body = format!(
            r#"{{"jsonrpc":"2.0","id":1,"method":"getTransaction","params":{{"hash":"{hash}"}}}}"#
        );
        let mut last = String::from("UNKNOWN");
        for attempt in 0..5 {
            let out = tokio::process::Command::new("curl")
                .args([
                    "-s",
                    "-X",
                    "POST",
                    &self.rpc_url,
                    "-H",
                    "Content-Type: application/json",
                    "-d",
                    &body,
                ])
                .output()
                .await
                .map_err(|e| SettleError::Invoke(format!("spawn curl (getTransaction): {e}")))?;

            if out.status.success() {
                let stdout = String::from_utf8_lossy(&out.stdout);
                if let Some(status) = parse_rpc_status(&stdout) {
                    last = status.clone();
                    // SUCCESS/FAILED are terminal — stop polling immediately.
                    if status == "SUCCESS" || status == "FAILED" {
                        return Ok(status);
                    }
                }
            }
            // transient (NOT_FOUND / PENDING / curl error): brief backoff, then retry
            if attempt < 4 {
                tokio::time::sleep(std::time::Duration::from_millis(900)).await;
            }
        }
        Ok(last)
    }
}

#[async_trait]
impl Settler for StellarCliSettler {
    async fn settle(&self, proved: &ProvedBatch) -> Result<String, SettleError> {
        let argv = settle_argv(&self.rollup_id, &self.source, &self.network, proved);
        // Run blocking process on a blocking thread so we never stall the runtime.
        let output = tokio::process::Command::new("stellar")
            .args(&argv)
            .output()
            .await
            .map_err(|e| SettleError::Invoke(format!("spawn stellar: {e}")))?;

        let mut combined = String::new();
        combined.push_str(&String::from_utf8_lossy(&output.stdout));
        combined.push_str(&String::from_utf8_lossy(&output.stderr));

        // EJE 1: success gate + image_id exclusion (a non-zero exit NEVER yields a
        // hash — its diagnostics can carry the journal digest, another 64-hex).
        let hash = parse_settle_output(
            output.status.success(),
            &combined,
            &hex::encode(proved.image_id),
        )?;

        // EJE 3: the CLI exiting 0 means the command ran — NOT that the tx applied
        // in the ledger. Re-confirm on-chain via RPC and require SUCCESS before
        // declaring settled (same guarantee the gate scripts enforce). A FAILED /
        // NOT_FOUND tx → OnchainFailed → Failed (lock released, re-submit possible);
        // the funds path is never reported settled on a tx that didn't move funds.
        let status = self.confirm_status(&hash).await?;
        finalize_status(hash, &status)
    }

    fn backend_label(&self) -> String {
        format!("stellar-cli({}, rollup {})", self.network, &self.rollup_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_proved() -> ProvedBatch {
        ProvedBatch {
            seal: vec![0xde, 0xad, 0xbe, 0xef],
            image_id: [0x11; 32],
            journal: vec![0x01, 0x02, 0x03],
        }
    }

    #[test]
    fn argv_mirrors_seq_demo_sh_byte_for_byte() {
        let proved = sample_proved();
        let argv = settle_argv("CROLLUP", "spikekey", "testnet", &proved);
        assert_eq!(
            argv,
            vec![
                "contract",
                "invoke",
                "--id",
                "CROLLUP",
                "--source",
                "spikekey",
                "--network",
                "testnet",
                "--send=yes",
                "--",
                "settle_batch",
                "--seal",
                "deadbeef",
                "--image_id",
                // 32 bytes of 0x11 → 64 hex chars.
                "1111111111111111111111111111111111111111111111111111111111111111",
                "--journal_bytes",
                "010203",
            ]
        );
    }

    #[test]
    fn argv_hex_encodes_each_field_no_0x_prefix() {
        let proved = sample_proved();
        let argv = settle_argv("C", "s", "n", &proved);
        // --seal value is at index 12, hex of the seal bytes, NO 0x prefix.
        assert_eq!(argv[12], "deadbeef");
        assert!(!argv[12].starts_with("0x"));
        assert_eq!(argv[14].len(), 64, "image_id is 32 bytes → 64 hex");
        assert_eq!(argv[16], "010203");
    }

    #[test]
    fn parse_tx_hash_finds_first_64_hex() {
        // A realistic stellar success line: the historic N=8 settle tx hash
        // (64 hex chars) on its own.
        let out = "ℹ️  Submitting transaction\n\
                   aedc1cc42f112d65913d4b1b5fb0e9b5636481e2f10a86f85ed21f5c0f605ea9\n\
                   ✅ Transaction succeeded\n";
        let h = parse_tx_hash(out, "").expect("hash present");
        assert_eq!(
            h,
            "aedc1cc42f112d65913d4b1b5fb0e9b5636481e2f10a86f85ed21f5c0f605ea9"
        );
        assert_eq!(h.len(), 64);
    }

    #[test]
    fn parse_tx_hash_skips_the_image_id() {
        // Realistic invoke output: `stellar` echoes the --image_id arg (the deployed
        // guest image_id, 64 hex) BEFORE the result tx hash (also 64 hex). The parser
        // MUST return the tx hash, not the image_id — else the explorer link 404s.
        let image_id = "cbeab7aa6ce69944e10cca8c7ed94d15aae297f2580752f07a15c6cab6ba0d46";
        let tx = "1f9fd5906a51332a790cfc2ce1ab135653cfafe5472ea5fac30a3a5bc0f04cb7";
        let out = format!(
            "invoke … --image_id {image_id} --journal_bytes 0a0b0c…\n\
             ℹ️  Submitting transaction\n{tx}\n✅ Transaction succeeded\n"
        );
        assert_eq!(parse_tx_hash(&out, image_id).as_deref(), Some(tx));
        // Sanity: WITHOUT the exclusion the bug returns the image_id (the regression).
        assert_eq!(parse_tx_hash(&out, "").as_deref(), Some(image_id));
    }

    #[test]
    fn parse_tx_hash_none_when_absent() {
        assert_eq!(parse_tx_hash("no hash here, just words", ""), None);
        // A short hex run (not 64) must NOT match.
        assert_eq!(parse_tx_hash("deadbeef cafe", ""), None);
    }

    #[test]
    fn parse_tx_hash_ignores_longer_hex_runs() {
        // The seal/journal hex are FAR longer than 64; they must not be mistaken
        // for the tx hash. A 128-hex run yields no 64-hex match.
        let long_hex = "a".repeat(128);
        assert_eq!(parse_tx_hash(&long_hex, ""), None);
        // But a 64-hex hash adjacent to words IS found.
        let mixed = format!("seal={} tx=", long_hex);
        let with_hash = format!("{}{}", mixed, "b".repeat(64));
        assert_eq!(parse_tx_hash(&with_hash, ""), Some("b".repeat(64)));
    }

    // ── EJE 1 · the success gate is testable as a pure function ─────────────
    #[test]
    fn parse_settle_output_rejects_spurious_hex_on_failure() {
        // A FAILED invoke whose diagnostics carry a 64-hex value (e.g. the
        // contract-computed journal digest) must NEVER yield a hash — it's Invoke,
        // never NoTxHash-with-a-digest, never Ok. (This is exactly Bug B's class.)
        let digest = "b".repeat(64);
        let out = format!("error: HostError … sha256(journal_bytes)={digest}\n");
        match parse_settle_output(false, &out, "") {
            Err(SettleError::Invoke(_)) => {}
            other => panic!("non-success must be Invoke err, got {other:?}"),
        }
    }

    #[test]
    fn parse_settle_output_returns_tx_hash_excluding_image_id() {
        let image_id = "c".repeat(64);
        let tx = "a".repeat(64);
        let out = format!("… --image_id {image_id} …\nSubmitting\n{tx}\n✅ succeeded\n");
        assert_eq!(parse_settle_output(true, &out, &image_id).unwrap(), tx);
    }

    #[test]
    fn parse_settle_output_no_hash_is_error() {
        assert!(matches!(
            parse_settle_output(true, "no hash here, just words", ""),
            Err(SettleError::NoTxHash(_))
        ));
    }

    // ── EJE 3 · only an on-chain SUCCESS may be reported settled ────────────
    #[test]
    fn finalize_status_success_yields_hash() {
        assert_eq!(
            finalize_status("deadhash".into(), "SUCCESS").unwrap(),
            "deadhash"
        );
    }

    #[test]
    fn finalize_status_failed_is_error_not_settled() {
        // A tx that did NOT apply on-chain must surface as OnchainFailed (→ Failed,
        // lock released, re-submit possible) — NEVER as a settled hash.
        match finalize_status("deadhash".into(), "FAILED") {
            Err(SettleError::OnchainFailed { hash, status }) => {
                assert_eq!(hash, "deadhash");
                assert_eq!(status, "FAILED");
            }
            other => panic!("FAILED must be OnchainFailed err, got {other:?}"),
        }
    }

    #[test]
    fn finalize_status_not_found_is_error() {
        assert!(matches!(
            finalize_status("h".into(), "NOT_FOUND"),
            Err(SettleError::OnchainFailed { .. })
        ));
    }

    #[test]
    fn finalize_status_unknown_is_error() {
        assert!(matches!(
            finalize_status("h".into(), "UNKNOWN"),
            Err(SettleError::OnchainFailed { .. })
        ));
    }

    #[test]
    fn parse_rpc_status_reads_result_status() {
        let ok = r#"{"jsonrpc":"2.0","id":1,"result":{"status":"SUCCESS","latestLedger":9}}"#;
        assert_eq!(parse_rpc_status(ok).as_deref(), Some("SUCCESS"));
        assert_eq!(
            parse_rpc_status(r#"{"result":{"status":"FAILED"}}"#).as_deref(),
            Some("FAILED")
        );
        assert_eq!(
            parse_rpc_status(r#"{"result":{"status":"NOT_FOUND"}}"#).as_deref(),
            Some("NOT_FOUND")
        );
    }

    #[test]
    fn parse_rpc_status_none_on_malformed_or_error() {
        assert_eq!(parse_rpc_status("not json at all"), None);
        assert_eq!(parse_rpc_status(r#"{"error":{"code":-32602}}"#), None);
    }
}
