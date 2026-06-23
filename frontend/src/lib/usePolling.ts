/*
 * Poll GET /status/:id until a terminal state. Self-cancelling, backoff-free
 * fixed interval (the proving wait is minutes; 1.5s is plenty and keeps the UI
 * feeling alive). Transient fetch errors during a long prove do NOT kill the
 * poll — they surface as a soft "reconnecting" note and the loop keeps trying.
 */

import { useEffect, useRef, useState } from "react";
import { getStatus, type StatusResponse } from "./api";

const INTERVAL_MS = 1500;
const MAX_TRANSIENT_ERRORS = 6; // ~9s of failed polls before we give up

/** The states that count as "the prover is working" for the elapsed clock. */
function isWorking(s: StatusResponse | null): boolean {
  return s?.state === "proving";
}

export interface PollState {
  status: StatusResponse | null;
  /** A soft, recoverable connection wobble (kept polling). */
  reconnecting: boolean;
  /** A hard error (sequencer unreachable past tolerance, or unknown id). */
  error: string | null;
  /**
   * Epoch-ms of the FIRST `proving` observation for this request id, or null
   * until then. Drives the live elapsed-time counter. Reset to null whenever the
   * polled request id changes (i.e. on retry / start-over), because the effect
   * re-runs and seeds a fresh state — so the clock never carries across requests.
   */
  provingSince: number | null;
}

export function useStatusPolling(requestId: string | null): PollState {
  const [state, setState] = useState<PollState>({
    status: null,
    reconnecting: false,
    error: null,
    provingSince: null,
  });
  const transientErrors = useRef(0);

  useEffect(() => {
    if (!requestId) {
      setState({
        status: null,
        reconnecting: false,
        error: null,
        provingSince: null,
      });
      return;
    }

    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | null = null;
    const controller = new AbortController();
    transientErrors.current = 0;

    const tick = async () => {
      try {
        const status = await getStatus(requestId, controller.signal);
        if (cancelled) return;
        transientErrors.current = 0;
        setState((s) => ({
          status,
          reconnecting: false,
          error: null,
          // Latch the first `proving` observation; keep it stable thereafter.
          provingSince:
            s.provingSince ?? (isWorking(status) ? Date.now() : null),
        }));
        if (status.state === "settled" || status.state === "failed") return; // terminal
      } catch (e) {
        if (cancelled || (e as Error).name === "AbortError") return;
        transientErrors.current += 1;
        if (transientErrors.current >= MAX_TRANSIENT_ERRORS) {
          setState((s) => ({
            ...s,
            reconnecting: false,
            error:
              "Lost the connection to the sequencer. Your note was not spent — retry when it's back.",
          }));
          return;
        }
        setState((s) => ({ ...s, reconnecting: true }));
      }
      if (!cancelled) timer = setTimeout(tick, INTERVAL_MS);
    };

    tick();
    return () => {
      cancelled = true;
      controller.abort();
      if (timer) clearTimeout(timer);
    };
  }, [requestId]);

  return state;
}

/**
 * Live, 1-second-ticking elapsed seconds since `since` (epoch-ms), or 0 when
 * `since` is null. This is the single biggest "it's moving" signal during the
 * minutes-long prove — a truthful wall clock, not a faked progress bar. The
 * interval is torn down whenever `since` clears (terminal / retry), so it never
 * ticks past the proving window.
 */
export function useElapsedSeconds(since: number | null): number {
  const [seconds, setSeconds] = useState(() =>
    since ? Math.max(0, Math.floor((Date.now() - since) / 1000)) : 0,
  );

  useEffect(() => {
    if (since == null) {
      setSeconds(0);
      return;
    }
    // Seed immediately (don't wait a full second for the first paint).
    setSeconds(Math.max(0, Math.floor((Date.now() - since) / 1000)));
    const id = setInterval(() => {
      setSeconds(Math.max(0, Math.floor((Date.now() - since) / 1000)));
    }, 1000);
    return () => clearInterval(id);
  }, [since]);

  return seconds;
}
