/*
 * The ONLY seam between the frontend and the deployed sequencer.
 *
 * Everything the UI shows — request ids, status, settle tx hashes, recent
 * batches, the active rollup/verifier ids — comes from THIS API. The frontend
 * never talks to the rollup contract, never holds proof artifacts, never
 * hardcodes a contract address. (Cero mocks: see plan §"Cero mocks".)
 */

const BASE: string = (
  import.meta.env.VITE_SEQUENCER_URL ?? "http://localhost:8787"
).replace(/\/$/, "");

export const SEQUENCER_URL = BASE;

/** The withdrawal-intent wire body POST /submit expects (32B LE values as hex). */
export interface SubmitBody {
  secret: string;
  blinding: string;
  amount: number;
  recipient: string;
  path: string[];
  index: number;
  merkle_root: string;
}

export interface SubmitResponse {
  request_id: string;
}

export type RequestState =
  | "pending"
  | "batched"
  | "proving"
  | "settled"
  | "failed";

export type ProverPhase = "starting" | "proving";

export interface StatusResponse {
  state: RequestState;
  prover_phase?: ProverPhase;
  tx_hash?: string;
  reason?: string;
  batch_size: number;
  n_target: number;
}

export interface RecentBatch {
  tx_hash: string;
  n: number;
  explorer_url: string;
}

export interface SequencerConfig {
  network: string;
  explorer_base: string;
  n_target: number;
  rollup_id: string;
  verifier_id: string;
}

/** A typed, message-bearing error for any non-2xx response or network fault. */
export class SequencerError extends Error {
  constructor(
    message: string,
    readonly status?: number,
  ) {
    super(message);
    this.name = "SequencerError";
  }
}

async function request<T>(
  path: string,
  init?: RequestInit,
  signal?: AbortSignal,
): Promise<T> {
  let res: Response;
  try {
    res = await fetch(`${BASE}${path}`, {
      ...init,
      signal,
      headers: { "content-type": "application/json", ...init?.headers },
    });
  } catch (e) {
    if ((e as Error).name === "AbortError") throw e;
    throw new SequencerError(
      `Cannot reach the sequencer at ${BASE}. Is it running?`,
    );
  }
  if (!res.ok) {
    let detail = `${res.status} ${res.statusText}`;
    try {
      const body = (await res.json()) as { error?: string };
      if (body?.error) detail = body.error;
    } catch {
      /* non-JSON error body — keep the status line */
    }
    throw new SequencerError(detail, res.status);
  }
  return (await res.json()) as T;
}

export function getConfig(signal?: AbortSignal): Promise<SequencerConfig> {
  return request<SequencerConfig>("/config", undefined, signal);
}

export function submitWithdrawal(
  body: SubmitBody,
  signal?: AbortSignal,
): Promise<SubmitResponse> {
  return request<SubmitResponse>(
    "/submit",
    { method: "POST", body: JSON.stringify(body) },
    signal,
  );
}

export function getStatus(
  requestId: string,
  signal?: AbortSignal,
): Promise<StatusResponse> {
  return request<StatusResponse>(
    `/status/${encodeURIComponent(requestId)}`,
    undefined,
    signal,
  );
}

export function getRecentBatches(signal?: AbortSignal): Promise<RecentBatch[]> {
  return request<RecentBatch[]>("/recent_batches", undefined, signal);
}
