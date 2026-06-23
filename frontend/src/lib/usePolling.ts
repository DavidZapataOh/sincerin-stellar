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

export interface PollState {
  status: StatusResponse | null;
  /** A soft, recoverable connection wobble (kept polling). */
  reconnecting: boolean;
  /** A hard error (sequencer unreachable past tolerance, or unknown id). */
  error: string | null;
}

export function useStatusPolling(requestId: string | null): PollState {
  const [state, setState] = useState<PollState>({
    status: null,
    reconnecting: false,
    error: null,
  });
  const transientErrors = useRef(0);

  useEffect(() => {
    if (!requestId) {
      setState({ status: null, reconnecting: false, error: null });
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
        setState({ status, reconnecting: false, error: null });
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
