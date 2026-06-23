/*
 * s3/01 honesty gate (TDD). The benchmark deliverable MUST:
 *  - carry the exact measured/projected numbers from docs/proving-times.md
 *    (no re-measure, provenance honest per cell),
 *  - show the aggregate on-chain cost as STRICTLY GROWING (never flat/constant),
 *  - encode the real N≈12 crossover (baseline N×35M crosses the 400M budget),
 *  - never describe the aggregate as flat/plano/constant — in its own copy OR in
 *    the corrected hero crystal note labels.
 *
 * This gate FAILS the build if any of those break.
 */

import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
import {
  BUDGET,
  BASELINE_VERIFY,
  ROWS,
  aggregateSeries,
  baselineSeries,
  crossoverN,
  AGGREGATE_CLAIM,
} from "./benchmark";

const here = dirname(fileURLToPath(import.meta.url));
const byN = () => Object.fromEntries(ROWS.map((r) => [r.n, r]));

describe("data == docs/proving-times.md (no re-measure, provenance per cell)", () => {
  it("on-chain aggregate settle matches the doc cells", () => {
    const r = byN();
    expect(r[2].settleAggInsn).toBe(31_500_000);
    expect(r[8].settleAggInsn).toBe(36_118_956); // measured exact (tx aedc1cc4…)
    expect(r[16].settleAggInsn).toBe(43_800_000);
    expect(r[32].settleAggInsn).toBe(56_100_000);
  });

  it("measured cycle counts match the doc (all four measured)", () => {
    const r = byN();
    expect(r[2].cycles).toBe(30_670_848);
    expect(r[8].cycles).toBe(122_683_392);
    expect(r[16].cycles).toBe(263_192_576);
    expect(r[32].cycles).toBe(561_512_448);
  });

  it("provenance flags are honest: N=2/8 measured, N=16/32 projected", () => {
    const r = byN();
    expect(r[2].settleFlag).toBe("measured");
    expect(r[8].settleFlag).toBe("measured");
    expect(r[16].settleFlag).toBe("projected");
    expect(r[32].settleFlag).toBe("projected");
  });

  it("GPU production proving is the real RTX 3090 number (N=8 = 5m04s), CPU is hours", () => {
    const n8 = byN()[8];
    expect(n8.provingGpuSeconds).toBe(304); // 5m04s, measured PARADA 1
    expect(n8.provingGpuFlag).toBe("measured");
    expect(n8.provingCpuSeconds).toBe(15967); // 4h26m07s (Mac dev)
    // production is dramatically faster than dev — the story the judge must see
    expect(n8.provingCpuSeconds / n8.provingGpuSeconds!).toBeGreaterThan(40);
  });
});

describe("the aggregate is sub-linear, NOT flat", () => {
  it("the on-chain aggregate series is STRICTLY increasing", () => {
    const ys = aggregateSeries().map((p) => p.insn);
    expect(ys.length).toBeGreaterThanOrEqual(4);
    for (let i = 1; i < ys.length; i++) {
      expect(ys[i]).toBeGreaterThan(ys[i - 1]);
    }
    // and it grows materially across the range (not a rounding wobble)
    expect(ys[ys.length - 1]).toBeGreaterThan(ys[0] * 1.5);
  });
});

describe("the N≈12 crossover is real (the differentiator)", () => {
  it("baseline (N × 35M) crosses the 400M budget at N in [11,12]", () => {
    expect(crossoverN()).toBeGreaterThanOrEqual(11);
    expect(crossoverN()).toBeLessThanOrEqual(12);
    expect(11 * BASELINE_VERIFY).toBeLessThanOrEqual(BUDGET); // N=11 fits
    expect(12 * BASELINE_VERIFY).toBeGreaterThan(BUDGET); // N=12 does not
  });

  it("aggregate always fits the budget; baseline is infeasible for N>=16", () => {
    for (const r of ROWS) expect(r.settleAggInsn).toBeLessThan(BUDGET);
    const base = Object.fromEntries(baselineSeries().map((p) => [p.n, p.insn]));
    expect(base[16]).toBeGreaterThan(BUDGET);
    expect(base[32]).toBeGreaterThan(BUDGET);
  });
});

describe("honesty lint: never flat/plano/constant about the aggregate", () => {
  const FORBIDDEN = /\b(flat|plano|constant)\b/i;

  it("the aggregate claim copy is honest", () => {
    expect(AGGREGATE_CLAIM.length).toBeGreaterThan(0);
    expect(AGGREGATE_CLAIM).not.toMatch(FORBIDDEN);
  });

  it("the hero crystal note labels are honest (corrected)", () => {
    const src = readFileSync(resolve(here, "../components/ProofCrystal.tsx"), "utf8");
    const labels = [...src.matchAll(/crystal-note-l">([^<]*)</g)].map((m) => m[1]);
    expect(labels.length).toBeGreaterThan(0);
    for (const label of labels) expect(label).not.toMatch(FORBIDDEN);
  });

  it("no deliverable claims the AGGREGATE cost is constant/flat in N (the exact overclaim)", () => {
    // Precise patterns: these phrasings only ever describe the aggregate TOTAL —
    // the honest "~constant Groth16 verification" (the subcomponent) and explicit
    // negations ("not flat", "NO es plano") are intentionally NOT matched.
    const OVERCLAIM = /\b(constant in n|constante en n|constant cost regardless|flat in n|plano en n)\b/i;
    const files = [
      resolve(here, "../components/ProofCrystal.tsx"),
      resolve(here, "../../../README.md"),
      resolve(here, "../../../docs/proving-times.md"),
    ];
    for (const f of files) {
      expect(readFileSync(f, "utf8"), `${f} claims the aggregate is constant/flat in N`).not.toMatch(
        OVERCLAIM,
      );
    }
  });
});
