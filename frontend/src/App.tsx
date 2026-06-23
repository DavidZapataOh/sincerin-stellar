/*
 * App shell — orchestrates the three views and the async flow:
 *
 *   Submit (hero) ──submit──▶ Status (poll) ──settled──▶ Result
 *                                   └─failed──▶ Status(failed) ──retry──▶ Submit→…
 *
 * Loads /config first (so contract addresses / network / n_target are never
 * hardcoded). Loads /recent_batches up front so the Recent panel is instant.
 * Talks ONLY to the sequencer HTTP API + the wallet — never the rollup contract.
 */

import { useCallback, useEffect, useMemo, useState } from "react";
import { SubmitView } from "./views/SubmitView";
import { StatusView } from "./views/StatusView";
import { ResultView } from "./views/ResultView";
import { Header } from "./components/Header";
import { Footer } from "./components/Footer";
import { useStatusPolling } from "./lib/usePolling";
import {
  getConfig,
  getRecentBatches,
  type RecentBatch,
  type SequencerConfig,
} from "./lib/api";
import { connectWallet } from "./lib/wallet";
import { isValidAddress } from "./lib/note";

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

export function App() {
  const [phase, setPhase] = useState<Phase>("loading");
  const [config, setConfig] = useState<SequencerConfig | null>(null);
  const [configError, setConfigError] = useState<string | null>(null);

  const [address, setAddress] = useState<string | null>(previewAddressFromUrl);
  const [connecting, setConnecting] = useState(false);
  const [walletError, setWalletError] = useState<string | null>(null);

  const [requestId, setRequestId] = useState<string | null>(null);
  const [recent, setRecent] = useState<RecentBatch[]>([]);

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
    if (!requestId) return "submit";
    if (poll.status?.state === "settled" && poll.status.tx_hash) return "result";
    return "status";
  }, [requestId, poll.status]);

  if (phase === "loading") {
    return (
      <>
        <Header network="testnet" address={null} onConnect={() => {}} connecting={false} />
        <main className="container section center-screen">
          <p className="loading-note">Connecting to the sequencer…</p>
        </main>
      </>
    );
  }

  if (phase === "config-error" || !config) {
    return (
      <>
        <Header network="testnet" address={null} onConnect={() => {}} connecting={false} />
        <main className="container section center-screen">
          <div className="booterror" role="alert">
            <h2>The sequencer is unreachable</h2>
            <p className="prose prose--muted">
              The frontend reads its network, contract ids and batch target from
              the sequencer&apos;s <span className="mono">/config</span> endpoint —
              it hardcodes nothing. Start the sequencer and reload.
            </p>
            {configError && <p className="failbox-reason mono">{configError}</p>}
            <button
              type="button"
              className="btn btn--primary"
              onClick={() => window.location.reload()}
            >
              Reload
            </button>
          </div>
        </main>
        <Footer config={null} />
      </>
    );
  }

  return (
    <>
      <Header
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

        {view === "result" && settledTx && (
          <ResultView
            config={config}
            txHash={settledTx}
            recipient={address}
            recent={recent}
            onAgain={onReset}
          />
        )}
      </main>
      <Footer config={config} />
    </>
  );
}
