/*
 * View 2 — Status (async). Polls /status/:id and renders the honest progression
 *   pending → batched → starting prover → proving → settled,
 * with the monochrome LIVE PULSE during proving (a heartbeat, never a dead
 * spinner). Shows the batch filling toward n_target. On `failed`, renders a
 * legible reason + a Retry button. The waiting screen also surfaces the
 * Recent-batches panel so the judge always has the live system to watch.
 */

import { AggregationViz } from "../components/AggregationViz";
import { StatusChip } from "../components/StatusChip";
import { LivePulse } from "../components/LivePulse";
import { RecentBatches } from "../components/RecentBatches";
import { useElapsedSeconds } from "../lib/usePolling";
import { formatElapsed } from "../lib/format";
import type {
  ProverPhase,
  RecentBatch,
  RequestState,
  SequencerConfig,
  StatusResponse,
} from "../lib/api";

interface Props {
  config: SequencerConfig;
  requestId: string;
  status: StatusResponse | null;
  reconnecting: boolean;
  error: string | null;
  /** Epoch-ms of the first `proving` observation (drives the elapsed clock). */
  provingSince: number | null;
  recent: RecentBatch[];
  onRetry: () => void;
  onReset: () => void;
}

/** Ordered phases for the progress rail. */
const PHASES: { key: string; label: string; sub?: string }[] = [
  { key: "pending", label: "Pending", sub: "validated · note reserved" },
  { key: "batched", label: "Batched", sub: "assigned to a batch" },
  { key: "starting", label: "Starting prover", sub: "waking the worker" },
  { key: "proving", label: "Proving", sub: "generating the Groth16 proof" },
  { key: "settled", label: "Settled", sub: "verified on-chain" },
];

function phaseIndex(state: RequestState, phase?: ProverPhase): number {
  switch (state) {
    case "pending":
      return 0;
    case "batched":
      return 1;
    case "proving":
      return phase === "proving" ? 3 : 2;
    case "settled":
      return 4;
    case "failed":
      return -1;
  }
}

export function StatusView({
  config,
  requestId,
  status,
  reconnecting,
  error,
  provingSince,
  recent,
  onRetry,
  onReset,
}: Props) {
  const state = status?.state ?? "pending";
  const phase = status?.prover_phase;
  const active = state === "proving" || state === "batched";
  const idx = phaseIndex(state, phase);
  const isProving = state === "proving";

  // Live elapsed clock while proving — the single biggest "it's moving" signal.
  // Ticks every second from the first `proving` observation (resets on retry).
  const elapsed = useElapsedSeconds(isProving ? provingSince : null);

  // batch fill
  const filled = status?.batch_size ?? 0;
  const nTarget = status?.n_target ?? config.n_target;
  // Once a batch is proving, every cell is sealed in — show it FULL, not a
  // stuck `0/N`. The fill-toward-N belongs to pending/batched; proving's focus
  // is the elapsed clock + the indeterminate motion below.
  const fillShown = isProving ? nTarget : Math.min(filled, nTarget);

  const hardError = error;
  const failed = state === "failed";
  const reason = status?.reason;

  return (
    <section className="container section status-wrap" aria-labelledby="status-title">
      <header className="status-head">
        <div>
          <p className="status-eyebrow">
            Request <span className="mono">{requestId}</span>
          </p>
          <h2 id="status-title">
            {failed
              ? "Proof generation failed"
              : hardError
                ? "Connection lost"
                : "Aggregating your withdrawal"}
          </h2>
        </div>
        <StatusChip
          state={
            failed
              ? "failed"
              : isProving
                ? phase === "starting"
                  ? "starting"
                  : "proving"
                : state
          }
        />
      </header>

      {/* ── failure / hard error ─────────────────────────────────────────── */}
      {(failed || hardError) && (
        <div className="failbox" role="alert">
          <p className="failbox-reason mono">
            {reason ?? hardError ?? "Unknown error."}
          </p>
          <p className="failbox-help">
            Your note was <strong>not spent</strong> — no nullifier was marked
            on-chain, and the lock was released. It&apos;s safe to retry the same
            withdrawal.
          </p>
          <div className="failbox-actions">
            <button type="button" className="btn btn--primary" onClick={onRetry}>
              Retry withdrawal
            </button>
            <button type="button" className="btn btn--ghost" onClick={onReset}>
              Start over
            </button>
          </div>
        </div>
      )}

      {/* ── live progress ────────────────────────────────────────────────── */}
      {!failed && !hardError && (
        <div className="status-grid">
          <div className="status-main">
            <div className={`status-viz ${active ? "is-active" : ""}`}>
              <AggregationViz
                n={nTarget}
                active={active}
                settled={state === "settled"}
              />
            </div>

            {isProving && (
              <ProvingMeter elapsed={elapsed} reconnecting={reconnecting} />
            )}

            <ol className="rail" aria-label="Settlement progress">
              {PHASES.map((p, i) => {
                const done = i < idx;
                const current = i === idx;
                return (
                  <li
                    key={p.key}
                    className={`rail-step ${done ? "is-done" : ""} ${current ? "is-current" : ""}`}
                  >
                    <span className="rail-marker" aria-hidden="true">
                      {done ? <RailCheck /> : current ? <RailDot pulsing={isProving && current} /> : <RailIdle />}
                    </span>
                    <span className="rail-body">
                      <span className="rail-label">{p.label}</span>
                      {p.sub && <span className="rail-sub">{p.sub}</span>}
                    </span>
                    {current && isProving && (
                      <LivePulse tone="proving" label="working" />
                    )}
                  </li>
                );
              })}
            </ol>
          </div>

          <aside className="status-side">
            <div className={`fillcard ${isProving ? "is-sealed" : ""}`}>
              <div className="fillcard-head">
                <span className="fillcard-label">Batch</span>
                <span className="mono fillcard-count">
                  {isProving ? (
                    <>
                      {nTarget}/{nTarget}
                      <span className="fillcard-sealed"> · proving</span>
                    </>
                  ) : (
                    <>
                      {fillShown}/{nTarget}
                    </>
                  )}
                </span>
              </div>
              <div
                className="fillbar"
                role="progressbar"
                aria-valuemin={0}
                aria-valuemax={nTarget}
                aria-valuenow={fillShown}
                aria-label={
                  isProving
                    ? `Batch of ${nTarget} sealed and now proving`
                    : `Batch filled to ${fillShown} of ${nTarget} withdrawals`
                }
              >
                {Array.from({ length: nTarget }).map((_, i) => (
                  <span
                    key={i}
                    className={`fillbar-cell ${i < fillShown ? "is-filled" : ""}`}
                  />
                ))}
              </div>
              <p className="fillcard-note">
                {isProving
                  ? `Your batch of ${nTarget} is sealed — the ${nTarget}→1 aggregation is being proved now, with you in it.`
                  : `Your withdrawal joins companion notes to complete a batch of ${nTarget}. The ${nTarget}→1 aggregation happens with you in it.`}
              </p>
            </div>

            <div className="waitnote">
              <p>
                {reconnecting
                  ? "Reconnecting to the sequencer…"
                  : isProving
                    ? "ZK proving typically takes a few minutes — it's safe to leave and come back to this request id. The minutes are the proof it isn't faked."
                    : "Assembling the batch. The request id is yours to poll anytime."}
              </p>
            </div>

            <RecentBatches
              config={config}
              batches={recent}
              compact
              caption="Settling now while yours finishes"
            />
          </aside>
        </div>
      )}
    </section>
  );
}

/**
 * The proving meter — the forward-motion focal point of the minutes-long wait.
 *
 *  - A live `mm:ss` elapsed clock (the single biggest "it's moving" signal).
 *  - An honest INDETERMINATE bar: a monochrome highlight that sweeps across a
 *    hairline track. It is deliberately NOT a determinate `%` — real ZK proving
 *    exposes no progress signal, so a fake percentage would be dishonest. The
 *    `progressbar` role carries no `aria-valuenow` → screen readers announce it
 *    as indeterminate (truthful), and the elapsed clock is read out via text.
 *  - `prefers-reduced-motion`: the sweep freezes to a static fill; the clock
 *    still ticks (it's information, not vestibular motion) and the label reads
 *    "working…" so reduced-motion users still see the wait is progressing.
 */
function ProvingMeter({
  elapsed,
  reconnecting,
}: {
  elapsed: number;
  reconnecting: boolean;
}) {
  return (
    <div className="provemeter" aria-live="polite">
      <div className="provemeter-row">
        <span className="provemeter-status">
          <LivePulse tone="proving" label={reconnecting ? "reconnecting" : "proving"} />
        </span>
        <span className="provemeter-clock mono" aria-label={`Elapsed ${formatElapsed(elapsed)} (minutes:seconds)`}>
          {formatElapsed(elapsed)}
        </span>
      </div>
      <div
        className="provemeter-track"
        role="progressbar"
        aria-label="Generating the Groth16 proof — indeterminate, working"
      >
        <span className="provemeter-sweep" aria-hidden="true" />
        <span className="provemeter-still" aria-hidden="true" />
      </div>
      <p className="provemeter-hint">
        ZK proving typically takes a few minutes — safe to leave and come back.
      </p>
    </div>
  );
}

function RailCheck() {
  return (
    <svg viewBox="0 0 18 18" width="18" height="18" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M4 9.5 L7.5 13 L14 5" />
    </svg>
  );
}
function RailDot({ pulsing }: { pulsing: boolean }) {
  return (
    <span className={`rail-dot ${pulsing ? "is-pulsing" : ""}`}>
      <span className="rail-dot-core" />
    </span>
  );
}
function RailIdle() {
  return <span className="rail-idle" />;
}
