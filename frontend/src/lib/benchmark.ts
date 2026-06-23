/*
 * STUB (TDD red) — intentionally wrong so the s3/01 honesty gate fails first:
 * zero data, and an AGGREGATE_CLAIM that violates the lint ("flat", "constant").
 * Replaced by the real source-of-truth module in the green step.
 */

export const BUDGET = 0;
export const BASELINE_VERIFY = 0;

export type Flag = "measured" | "projected";
export interface BenchRow {
  n: number;
  depth: number;
  cycles: number;
  provingCpuSeconds: number;
  provingCpuFlag: Flag;
  provingGpuSeconds: number | null;
  provingGpuFlag: Flag | null;
  settleAggInsn: number;
  settleFlag: Flag;
  baselineInsn: number;
}

export const ROWS: BenchRow[] = [];

export function aggregateSeries(): Array<{ n: number; insn: number }> {
  return [];
}
export function baselineSeries(): Array<{ n: number; insn: number }> {
  return [];
}
export function crossoverN(): number {
  return 0;
}

export const AGGREGATE_CLAIM =
  "The on-chain cost is flat — constant in N.";
