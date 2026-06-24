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
import { readFileSync, readdirSync, statSync } from "node:fs";
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

describe("honesty lint — no aggregate overclaim in ANY public-facing copy", () => {
  const FORBIDDEN = /\b(flat|plano|constant)\b|barely\s+(moves|grows|changes)/i;

  it("the aggregate claim copy is honest", () => {
    expect(AGGREGATE_CLAIM.length).toBeGreaterThan(0);
    expect(AGGREGATE_CLAIM).not.toMatch(FORBIDDEN);
  });

  /*
   * Deny-by-default scan over EVERY judge-readable file: the README, all docs,
   * and all frontend copy (pages, demo views, components, lib). A line carrying a
   * forbidden term passes ONLY if it also contains one of these audited honest
   * markers — an explicit negation, the ~constant SNARK SUBcomponent, Poseidon2
   * round-constants, flat-top hexagon geometry, or a "never …" meta-note. Adding a
   * new honest use must be a deliberate allowlist edit. Anything else FAILS — so
   * a hidden "constant in N" / "flat" / "barely moves" can't ship.
   */
  const ALLOW = [
    "flat-top", // hexagon geometry, not cost
    "not flat", // explicit negation (en)
    "not** flat", // explicit negation (markdown bold)
    "no es plano", // explicit negation (es)
    "~constant", // the SNARK verify SUBcomponent (~constant) — honest, also covers ~constante
    "round constant", // Poseidon2 round constants (crypto, not cost)
    "round-constant",
    "params constant",
    "every constant",
    "never flat", // meta-comments: "GROWS … never flat/constant"
    'never "flat',
    'never "constant',
  ].map((s) => s.toLowerCase());

  const isTest = (f: string) => /\.test\.tsx?$/.test(f);
  const walk = (dir: string, out: string[] = []): string[] => {
    for (const e of readdirSync(dir)) {
      const full = resolve(dir, e);
      if (statSync(full).isDirectory()) {
        if (e !== "node_modules" && e !== "dist") walk(full, out);
      } else if (/\.(tsx?|md)$/.test(e) && !isTest(full)) {
        out.push(full);
      }
    }
    return out;
  };

  it("every public file: any unallowlisted flat/plano/constant/barely-moves FAILS", () => {
    const root = resolve(here, "../../..");
    const files = [
      ...walk(resolve(here, "..")), // frontend/src/**
      ...walk(resolve(root, "docs")), // docs/**.md
      resolve(root, "README.md"),
    ];
    const violations: string[] = [];
    for (const f of files) {
      readFileSync(f, "utf8")
        .split("\n")
        .forEach((line, i) => {
          if (!FORBIDDEN.test(line)) return;
          const low = line.toLowerCase();
          if (ALLOW.some((a) => low.includes(a))) return;
          violations.push(`${f.replace(`${root}/`, "")}:${i + 1}  ${line.trim().slice(0, 90)}`);
        });
    }
    expect(violations, `overclaim(s) in public copy:\n${violations.join("\n")}`).toEqual([]);
  });
});
