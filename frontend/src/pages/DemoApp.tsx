/*
 * The DEMO surface (`/demo`) — the functional pipeline, reached from the
 * landing's "Launch demo" CTA. This is the proven single-page flow, unchanged in
 * substance: it loads /config first (contract ids never hardcoded), talks ONLY
 * to the sequencer HTTP API + the wallet, and runs the real async pipeline:
 *
 *   Submit (hero) ──submit──▶ Status (poll) ──settled──▶ Result
 *                                   └─failed──▶ Status(failed) ──retry──▶ Submit→…
 *
 * Cero mocks: every request id, status, settle tx hash and recent batch comes
 * from the sequencer; the only client-side fixture is the optional preview
 * affordances below (render-only, never an on-chain artifact).
 */

import { useCallback, useEffect, useMemo, useState } from "react";
import { SubmitView } from "../views/SubmitView";
import { StatusView } from "../views/StatusView";
import { ResultView } from "../views/ResultView";
import { Header } from "../components/Header";
import { Footer } from "../components/Footer";
import { useStatusPolling } from "../lib/usePolling";
import {
  getConfig,
  getRecentBatches,
  type RecentBatch,
  type SequencerConfig,
} from "../lib/api";
import { connectWallet } from "../lib/wallet";
import { isValidAddress } from "../lib/note";
import { Link } from "../lib/router";

type Phase = "loading" | "ready" | "config-error";

/**
 * Optional `?previewAddress=G…` lets a headless browser (screenshot / CI) exercise
 * the real pipeline with a recipient set, exactly as a connected wallet would. It
 * ONLY pre-fills the recipient address — every sequencer call stays real. Ignored
 * unless it is a valid Stellar public key.
 */
function previewAddressFromUrl(): string | null {
  if (typeof window === "undefined") return null;
  const p = new URLSearchParams(window.location.search).get("previewAddress");
  return p && isValidAddress(p) ? p : null;
}

/**
 * Optional `?previewSettled=<tx>` renders the settled Result view from a REAL,
 * already-on-chain tx hash WITHOUT running a new settle. It is a presentation
 * affordance for capturing the settled screen (the live on-chain settle is the
 * orchestrator's gate, not the frontend's): it fabricates nothing — the hash it
 * shows must be a real testnet tx, and it links straight to the explorer. The
 * recipient shown is the previewAddress (or the historic recipient).
 */
function previewSettledFromUrl(): string | null {
  if (typeof window === "undefined") return null;
  const p = new URLSearchParams(window.location.search).get("previewSettled");
  return p && /^[0-9a-f]{64}$/i.test(p) ? p : null;
}

export function DemoApp() {
  const [phase, setPhase] = useState<Phase>("loading");
  const [config, setConfig] = useState<SequencerConfig | null>(null);
  const [configError, setConfigError] = useState<string | null>(null);

  const [address, setAddress] = useState<string | null>(previewAddressFromUrl);
  const [connecting, setConnecting] = useState(false);
  const [walletError, setWalletError] = useState<string | null>(null);

  const [requestId, setRequestId] = useState<string | null>(null);
  const [recent, setRecent] = useState<RecentBatch[]>([]);

  const previewSettled = useMemo(previewSettledFromUrl, []);

  const poll = useStatusPolling(requestId);
  const settledTx =
    poll.status?.state === "settled" ? poll.status.tx_hash ?? null : null;

  // ── load config (gates the app) ──────────────────────────────────────────
  useEffect(() => {
    const controller = new AbortController();
    getConfig(controller.signal)
      .then((c) => {
        setConfig(c);
        setPhase("ready");
      })
      .catch((e: Error) => {
        if (e.name === "AbortError") return;
        setConfigError(e.message);
        setPhase("config-error");
      });
    return () => controller.abort();
  }, []);

  // ── recent batches (instant panel; refresh on terminal state) ────────────
  const refreshRecent = useCallback(() => {
    getRecentBatches()
      .then(setRecent)
      .catch(() => {
        /* keep whatever we had; the panel is supplementary */
      });
  }, []);

  useEffect(() => {
    if (phase === "ready") refreshRecent();
  }, [phase, refreshRecent]);

  useEffect(() => {
    if (settledTx) refreshRecent();
  }, [settledTx, refreshRecent]);

  // ── wallet ───────────────────────────────────────────────────────────────
  const onConnect = useCallback(async () => {
    setWalletError(null);
    setConnecting(true);
    try {
      const addr = await connectWallet();
      if (addr) setAddress(addr);
    } catch (e) {
      setWalletError(
        e instanceof Error ? e.message : "Could not connect the wallet.",
      );
    } finally {
      setConnecting(false);
    }
  }, []);

  // ── flow transitions ─────────────────────────────────────────────────────
  const onSubmitted = useCallback((id: string) => {
    setRequestId(id);
    window.scrollTo({ top: 0, behavior: "smooth" });
  }, []);

  const onReset = useCallback(() => setRequestId(null), []);
  const onRetry = useCallback(() => setRequestId(null), []);

  const view = useMemo<"submit" | "status" | "result">(() => {
    if (previewSettled) return "result";
    if (!requestId) return "submit";
    if (poll.status?.state === "settled" && poll.status.tx_hash) return "result";
    return "status";
  }, [previewSettled, requestId, poll.status]);

  if (phase === "loading") {
    return (
      <>
        <Header surface="demo" network="testnet" address={null} onConnect={() => {}} connecting={false} />
        <main className="container section center-screen">
          <p className="loading-note">Connecting to the sequencer…</p>
        </main>
      </>
    );
  }

  if (phase === "config-error" || !config) {
    return (
      <>
        <Header surface="demo" network="testnet" address={null} onConnect={() => {}} connecting={false} />
        <main className="container section center-screen">
          <div className="booterror" role="alert">
            <h2>The sequencer is unreachable</h2>
            <p className="prose prose--muted">
              The demo reads its network, contract ids and batch target from the
              sequencer&apos;s <span className="mono">/config</span> endpoint — it
              hardcodes nothing. Start the sequencer and reload.
            </p>
            {configError && <p className="failbox-reason mono">{configError}</p>}
            <div className="booterror-actions">
              <button
                type="button"
                className="btn btn--primary"
                onClick={() => window.location.reload()}
              >
                Reload
              </button>
              <Link to="/" className="btn btn--ghost">
                Back to landing
              </Link>
            </div>
          </div>
        </main>
        <Footer surface="demo" config={null} />
      </>
    );
  }

  return (
    <>
      <Header
        surface="demo"
        network={config.network}
        address={address}
        onConnect={onConnect}
        connecting={connecting}
      />
      <main id="main">
        {walletError && view === "submit" && (
          <div className="container">
            <p className="form-error form-error--top" role="alert">
              {walletError}
            </p>
          </div>
        )}

        {view === "submit" && (
          <SubmitView
            config={config}
            address={address}
            connecting={connecting}
            onConnect={onConnect}
            onSubmitted={onSubmitted}
          />
        )}

        {view === "status" && requestId && (
          <StatusView
            config={config}
            requestId={requestId}
            status={poll.status}
            reconnecting={poll.reconnecting}
            error={poll.error}
            provingSince={poll.provingSince}
            recent={recent}
            onRetry={onRetry}
            onReset={onReset}
          />
        )}

        {view === "result" && (
          <ResultView
            config={config}
            txHash={previewSettled ?? settledTx ?? ""}
            recipient={address}
            recent={recent}
            onAgain={onReset}
          />
        )}
      </main>
      <Footer surface="demo" config={config} />
    </>
  );
}
