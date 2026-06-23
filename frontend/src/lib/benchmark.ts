/*
 * Benchmark — the single source of truth for the two-axis benchmark (s3/01),
 * transcribed EXACTLY from docs/proving-times.md. Nothing is re-measured here;
 * every cell carries its provenance ('measured' | 'projected'). The /benchmarks
 * page and the README export both render from THIS module, and the honesty gate
 * (benchmark.test.ts) asserts these values against the doc + the invariants:
 *   - the aggregate on-chain cost GROWS with N (sub-linear, never flat/constant),
 *   - the baseline (N loose Groth16 verifies) crosses the 400M budget at N≈12.
 *
 * Axis 1 (off-chain proving) GROWS with N. Two tracks, honestly separated:
 *   - CPU (Mac, x86-on-ARM emulation) = the DEV numbers (hours),
 *   - GPU (RTX 3090, native CUDA)     = the PRODUCTION number (minutes; N=8 = 5m04s).
 * Axis 2 (on-chain settle) grows sub-linearly and stays well inside the budget.
 */

/** The Soroban single-transaction instruction budget. */
export const BUDGET = 400_000_000;
/** Cost of ONE standalone Groth16 verification (the per-withdrawal baseline). */
export const BASELINE_VERIFY = 35_000_000;

export type Flag = "measured" | "projected";

export interface BenchRow {
  /** Batch size (withdrawals aggregated into one receipt). */
  n: number;
  /** Merkle depth = ceil(log2 N). */
  depth: number;
  /** Executor-padded cycles — REAL (all four measured). */
  cycles: number;
  /** Proving wall-clock on CPU (Mac, development), seconds. */
  provingCpuSeconds: number;
  provingCpuFlag: Flag;
  /** Optional note (e.g. N=16's wrap failure — no end-to-end CPU time). */
  provingCpuNote?: string;
  /** Proving wall-clock on GPU (RTX 3090, production), seconds — null if unmeasured. */
  provingGpuSeconds: number | null;
  provingGpuFlag: Flag | null;
  /** Aggregated on-chain settle cost (1 verify + N transfers), instructions. */
  settleAggInsn: number;
  settleFlag: Flag;
  /** Baseline: settling the same N as N standalone verifies = N × 35M. */
  baselineInsn: number;
}

/** The measured/projected table, verbatim from docs/proving-times.md. */
export const ROWS: BenchRow[] = [
  {
    n: 2,
    depth: 3,
    cycles: 30_670_848,
    provingCpuSeconds: 4_440, // ~1h 14m
    provingCpuFlag: "measured",
    provingGpuSeconds: null,
    provingGpuFlag: null,
    settleAggInsn: 31_500_000,
    settleFlag: "measured",
    baselineInsn: 2 * BASELINE_VERIFY,
  },
  {
    n: 8,
    depth: 3,
    cycles: 122_683_392,
    provingCpuSeconds: 15_967, // 4h 26m 07s
    provingCpuFlag: "measured",
    provingGpuSeconds: 304, // 5m 04s — RTX 3090, PARADA 1
    provingGpuFlag: "measured",
    settleAggInsn: 36_118_956, // measured exact — settle tx aedc1cc4…
    settleFlag: "measured",
    baselineInsn: 8 * BASELINE_VERIFY,
  },
  {
    n: 16,
    depth: 4,
    cycles: 263_192_576,
    provingCpuSeconds: 33_540, // ~9h 19m (projected)
    provingCpuFlag: "projected",
    provingCpuNote: "real prove FAILED at 7h38m in the Groth16 wrap — no end-to-end time",
    provingGpuSeconds: null,
    provingGpuFlag: null,
    settleAggInsn: 43_800_000, // 31.5M + 0.77M·N
    settleFlag: "projected",
    baselineInsn: 16 * BASELINE_VERIFY,
  },
  {
    n: 32,
    depth: 5,
    cycles: 561_512_448,
    provingCpuSeconds: 70_800, // ~19h 40m (projected)
    provingCpuFlag: "projected",
    provingGpuSeconds: null,
    provingGpuFlag: null,
    settleAggInsn: 56_100_000, // 31.5M + 0.77M·N
    settleFlag: "projected",
    baselineInsn: 32 * BASELINE_VERIFY,
  },
];

/** Aggregate on-chain cost per N (the curve that stays feasible). */
export function aggregateSeries(): Array<{ n: number; insn: number; flag: Flag }> {
  return ROWS.map((r) => ({ n: r.n, insn: r.settleAggInsn, flag: r.settleFlag }));
}

/** Baseline (N standalone verifies) per N (the curve that blows past the budget). */
export function baselineSeries(): Array<{ n: number; insn: number; feasible: boolean }> {
  return ROWS.map((r) => ({ n: r.n, insn: r.baselineInsn, feasible: r.baselineInsn <= BUDGET }));
}

/** The N at which the baseline crosses the budget (continuous): 400M / 35M ≈ 11.43. */
export function crossoverN(): number {
  return BUDGET / BASELINE_VERIFY;
}

/** First INTEGER batch size that no longer fits in one tx as loose verifies. */
export const CROSSOVER_N_INT = Math.floor(crossoverN()) + 1; // 12

/** A settle cost as a percentage of the 400M budget. */
export function pctOfBudget(insn: number): number {
  return (insn / BUDGET) * 100;
}

/** Human duration from seconds: "5m 04s", "4h 26m". */
export function formatProving(seconds: number): string {
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = Math.round(seconds % 60);
  if (h > 0) return `${h}h ${String(m).padStart(2, "0")}m`;
  return `${m}m ${String(s).padStart(2, "0")}s`;
}

/**
 * The headline honest claim about the aggregate — rendered on /benchmarks and
 * checked by the gate. It describes GROWTH, never "flat/constant".
 */
export const AGGREGATE_CLAIM =
  "Aggregated, the on-chain cost grows sub-linearly with N — from ~7.9% of a " +
  "block's budget at N=2 to ~14% at N=32 — staying well inside the 400M limit. " +
  "Settling the same withdrawals as N separate verifications grows linearly and " +
  "stops fitting in a single transaction around N=12.";
