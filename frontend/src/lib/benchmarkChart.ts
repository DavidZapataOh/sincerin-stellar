/*
 * benchmarkChart — PURE SVG-string generators for the two benchmark charts.
 * Shared by the /benchmarks React page (rendered inline) AND the README export
 * script, so the chart a judge sees in the app and on GitHub are byte-identical
 * and can never drift. No React, no DOM. Colors are literal (the standalone SVG
 * has no CSS tokens) and B&W-on-white with one functional red for the INFEASIBLE
 * baseline — the only state that earns color (it's a failure: "won't fit").
 *
 * Star chart (crossoverChartSVG): on-chain instructions vs N. The baseline (N
 * standalone verifies) crosses the 400M one-tx budget at N≈12 and becomes
 * infeasible; the aggregate stays low and always fits, but GROWS (never flat).
 * Secondary (provingChartSVG): off-chain proving time vs N, CPU(dev, hours) vs
 * GPU(prod, minutes), on a log axis so 5m and 20h both read.
 */

import {
  ROWS,
  BUDGET,
  BASELINE_VERIFY,
  crossoverN,
  CROSSOVER_N_INT,
  formatProving,
} from "./benchmark";

const C = {
  ink: "#222222",
  muted: "#7a7a7a",
  faintInk: "#9a9a9a",
  line: "#e6e6e6",
  bg: "#ffffff",
  fail: "#c5371f",
  failFill: "rgba(197,55,31,0.07)",
};

const fmtM = (insn: number) => `${Math.round(insn / 1e6)}M`;

/* ── 1 · the crossover (the differentiator, in one image) ─────────────────── */
export function crossoverChartSVG({ w = 760, h = 480 }: { w?: number; h?: number } = {}): string {
  const m = { l: 66, r: 30, t: 44, b: 70 };
  const pw = w - m.l - m.r;
  const ph = h - m.t - m.b;
  const Nmax = 34;
  const Ymax = 600_000_000;
  const x = (n: number) => m.l + (n / Nmax) * pw;
  const y = (insn: number) => m.t + ph - (Math.min(insn, Ymax) / Ymax) * ph;

  const xN = crossoverN(); // 11.43
  const yB = y(BUDGET);
  const baselineExitN = Ymax / BASELINE_VERIFY; // 17.14 (where it leaves the top)

  const out: string[] = [];
  out.push(
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${w} ${h}" width="${w}" height="${h}" font-family="ui-sans-serif, system-ui, sans-serif" role="img" aria-label="On-chain instructions versus batch size N: the baseline of N standalone verifications crosses the 400M one-transaction budget around N=12 and becomes infeasible, while the aggregated Sincerin settle stays well inside the budget but grows sub-linearly.">`,
  );
  out.push(`<rect x="0" y="0" width="${w}" height="${h}" fill="${C.bg}"/>`);

  // infeasible zone (above the budget line)
  out.push(`<rect x="${m.l}" y="${m.t}" width="${pw}" height="${yB - m.t}" fill="${C.failFill}"/>`);

  // y gridlines + labels (0..600M by 100M)
  for (let v = 0; v <= Ymax; v += 100_000_000) {
    const yy = y(v);
    out.push(`<line x1="${m.l}" y1="${yy.toFixed(1)}" x2="${m.l + pw}" y2="${yy.toFixed(1)}" stroke="${C.line}" stroke-width="1"/>`);
    out.push(`<text x="${m.l - 10}" y="${(yy + 4).toFixed(1)}" text-anchor="end" font-size="12" fill="${C.muted}">${fmtM(v)}</text>`);
  }

  // axes
  out.push(`<line x1="${m.l}" y1="${m.t}" x2="${m.l}" y2="${m.t + ph}" stroke="${C.ink}" stroke-width="1.5"/>`);
  out.push(`<line x1="${m.l}" y1="${m.t + ph}" x2="${m.l + pw}" y2="${m.t + ph}" stroke="${C.ink}" stroke-width="1.5"/>`);

  // x ticks (N = 2, 8, 12, 16, 32)
  for (const n of [2, 8, CROSSOVER_N_INT, 16, 32]) {
    const xx = x(n);
    out.push(`<line x1="${xx.toFixed(1)}" y1="${m.t + ph}" x2="${xx.toFixed(1)}" y2="${m.t + ph + 5}" stroke="${C.ink}" stroke-width="1"/>`);
    out.push(`<text x="${xx.toFixed(1)}" y="${m.t + ph + 20}" text-anchor="middle" font-size="12" fill="${C.muted}">${n}</text>`);
  }
  out.push(`<text x="${(m.l + pw / 2).toFixed(1)}" y="${h - 16}" text-anchor="middle" font-size="12.5" fill="${C.ink}">batch size · N withdrawals</text>`);

  // budget line
  out.push(`<line x1="${m.l}" y1="${yB.toFixed(1)}" x2="${m.l + pw}" y2="${yB.toFixed(1)}" stroke="${C.ink}" stroke-width="1.5" stroke-dasharray="6 4"/>`);
  out.push(`<text x="${m.l + pw - 4}" y="${(yB - 8).toFixed(1)}" text-anchor="end" font-size="12" font-weight="600" fill="${C.ink}">400M · one-transaction budget</text>`);

  // infeasible label
  out.push(`<text x="${m.l + 14}" y="${m.t + 22}" font-size="12.5" fill="${C.fail}" font-weight="600">Doesn’t fit in one transaction</text>`);

  // baseline: feasible part (ink) then infeasible part (red dashed) + exit arrow
  out.push(`<line x1="${x(0).toFixed(1)}" y1="${y(0).toFixed(1)}" x2="${x(xN).toFixed(1)}" y2="${yB.toFixed(1)}" stroke="${C.ink}" stroke-width="2.4"/>`);
  out.push(`<line x1="${x(xN).toFixed(1)}" y1="${yB.toFixed(1)}" x2="${x(baselineExitN).toFixed(1)}" y2="${y(Ymax).toFixed(1)}" stroke="${C.fail}" stroke-width="2.4" stroke-dasharray="2 5"/>`);
  // exit arrow + offchart value
  const exX = x(baselineExitN);
  out.push(`<path d="M ${exX.toFixed(1)} ${(m.t + 8).toFixed(1)} l -4 8 l 8 0 z" fill="${C.fail}"/>`);
  out.push(`<text x="${(exX + 10).toFixed(1)}" y="${(m.t + 14).toFixed(1)}" font-size="11.5" fill="${C.fail}">→ 1,120M at N=32</text>`);
  out.push(`<text x="${x(6).toFixed(1)}" y="${y(245_000_000).toFixed(1)}" font-size="12" fill="${C.ink}" transform="rotate(-31 ${x(6).toFixed(1)} ${y(245_000_000).toFixed(1)})">baseline · N separate verifications</text>`);

  // crossover guide + marker (THE focus)
  out.push(`<line x1="${x(xN).toFixed(1)}" y1="${yB.toFixed(1)}" x2="${x(xN).toFixed(1)}" y2="${(m.t + ph).toFixed(1)}" stroke="${C.fail}" stroke-width="1" stroke-dasharray="3 3"/>`);
  out.push(`<circle cx="${x(xN).toFixed(1)}" cy="${yB.toFixed(1)}" r="5" fill="${C.bg}" stroke="${C.fail}" stroke-width="2.4"/>`);
  out.push(`<text x="${x(xN).toFixed(1)}" y="${(m.t + ph + 38).toFixed(1)}" text-anchor="middle" font-size="13" font-weight="700" fill="${C.fail}">N ≈ ${CROSSOVER_N_INT} — aggregation becomes the only way</text>`);

  // aggregate: measured solid (N2→N8), projected dashed (N8→N16→N32)
  const pts = ROWS.map((r) => ({ n: r.n, insn: r.settleAggInsn, flag: r.settleFlag }));
  for (let i = 1; i < pts.length; i++) {
    const a = pts[i - 1], b = pts[i];
    const dashed = b.flag === "projected";
    out.push(
      `<line x1="${x(a.n).toFixed(1)}" y1="${y(a.insn).toFixed(1)}" x2="${x(b.n).toFixed(1)}" y2="${y(b.insn).toFixed(1)}" stroke="${C.ink}" stroke-width="2.6"${dashed ? ` stroke-dasharray="5 4"` : ""}/>`,
    );
  }
  for (const p of pts) {
    out.push(
      p.flag === "measured"
        ? `<circle cx="${x(p.n).toFixed(1)}" cy="${y(p.insn).toFixed(1)}" r="4.5" fill="${C.ink}"/>`
        : `<circle cx="${x(p.n).toFixed(1)}" cy="${y(p.insn).toFixed(1)}" r="4.5" fill="${C.bg}" stroke="${C.ink}" stroke-width="2"/>`,
    );
  }
  out.push(`<text x="${x(32).toFixed(1)}" y="${(y(56_100_000) - 12).toFixed(1)}" text-anchor="end" font-size="12.5" font-weight="600" fill="${C.ink}">aggregated · always fits, grows sub-linearly</text>`);

  // legend (measured vs projected)
  const lgY = m.t + 4;
  out.push(`<circle cx="${m.l + 14}" cy="${lgY}" r="4" fill="${C.ink}"/><text x="${m.l + 24}" y="${lgY + 4}" font-size="11.5" fill="${C.muted}">measured</text>`);
  out.push(`<circle cx="${m.l + 96}" cy="${lgY}" r="4" fill="${C.bg}" stroke="${C.ink}" stroke-width="2"/><text x="${m.l + 106}" y="${lgY + 4}" font-size="11.5" fill="${C.muted}">projected</text>`);

  out.push(`</svg>`);
  return out.join("");
}

/* ── 2 · off-chain proving — dev CPU (hours) vs prod GPU (minutes), log axis ── */
export function provingChartSVG({ w = 760, h = 360 }: { w?: number; h?: number } = {}): string {
  const m = { l: 66, r: 30, t: 40, b: 64 };
  const pw = w - m.l - m.r;
  const ph = h - m.t - m.b;
  const Nmax = 34;
  // log scale: 60s (1m) .. 86400s (24h)
  const lmin = Math.log10(60);
  const lmax = Math.log10(86_400);
  const x = (n: number) => m.l + (n / Nmax) * pw;
  const y = (s: number) => m.t + ph - ((Math.log10(s) - lmin) / (lmax - lmin)) * ph;

  const ticks: Array<[number, string]> = [
    [60, "1m"],
    [600, "10m"],
    [3600, "1h"],
    [36_000, "10h"],
  ];

  const out: string[] = [];
  out.push(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${w} ${h}" width="${w}" height="${h}" font-family="ui-sans-serif, system-ui, sans-serif" role="img" aria-label="Off-chain proving time versus batch size N on a log scale: development on CPU (Mac, x86-on-ARM emulation) takes hours and grows with N, while production on a single RTX 3090 GPU proved N=8 in 5 minutes 4 seconds — about 52 times faster.">`);
  out.push(`<rect x="0" y="0" width="${w}" height="${h}" fill="${C.bg}"/>`);

  for (const [s, label] of ticks) {
    const yy = y(s);
    out.push(`<line x1="${m.l}" y1="${yy.toFixed(1)}" x2="${m.l + pw}" y2="${yy.toFixed(1)}" stroke="${C.line}" stroke-width="1"/>`);
    out.push(`<text x="${m.l - 10}" y="${(yy + 4).toFixed(1)}" text-anchor="end" font-size="12" fill="${C.muted}">${label}</text>`);
  }
  out.push(`<line x1="${m.l}" y1="${m.t}" x2="${m.l}" y2="${m.t + ph}" stroke="${C.ink}" stroke-width="1.5"/>`);
  out.push(`<line x1="${m.l}" y1="${m.t + ph}" x2="${m.l + pw}" y2="${m.t + ph}" stroke="${C.ink}" stroke-width="1.5"/>`);
  for (const n of [2, 8, 16, 32]) {
    const xx = x(n);
    out.push(`<line x1="${xx.toFixed(1)}" y1="${m.t + ph}" x2="${xx.toFixed(1)}" y2="${m.t + ph + 5}" stroke="${C.ink}" stroke-width="1"/>`);
    out.push(`<text x="${xx.toFixed(1)}" y="${m.t + ph + 20}" text-anchor="middle" font-size="12" fill="${C.muted}">${n}</text>`);
  }
  out.push(`<text x="${(m.l + pw / 2).toFixed(1)}" y="${h - 14}" text-anchor="middle" font-size="12.5" fill="${C.ink}">batch size · N withdrawals</text>`);

  // CPU line (measured N2,N8 solid; projected N16,N32 dashed)
  const cpu = ROWS.map((r) => ({ n: r.n, s: r.provingCpuSeconds, flag: r.provingCpuFlag }));
  for (let i = 1; i < cpu.length; i++) {
    const a = cpu[i - 1], b = cpu[i];
    const dashed = b.flag === "projected";
    out.push(`<line x1="${x(a.n).toFixed(1)}" y1="${y(a.s).toFixed(1)}" x2="${x(b.n).toFixed(1)}" y2="${y(b.s).toFixed(1)}" stroke="${C.ink}" stroke-width="2.4"${dashed ? ` stroke-dasharray="5 4"` : ""}/>`);
  }
  for (const p of cpu) {
    out.push(
      p.flag === "measured"
        ? `<circle cx="${x(p.n).toFixed(1)}" cy="${y(p.s).toFixed(1)}" r="4.5" fill="${C.ink}"/>`
        : `<circle cx="${x(p.n).toFixed(1)}" cy="${y(p.s).toFixed(1)}" r="4.5" fill="${C.bg}" stroke="${C.ink}" stroke-width="2"/>`,
    );
  }
  out.push(`<text x="${x(32).toFixed(1)}" y="${(y(70_800) - 10).toFixed(1)}" text-anchor="end" font-size="12.5" fill="${C.muted}">CPU · development (Mac)</text>`);

  // GPU production point (N=8 = 304s), emphasized
  const gx = x(8), gy = y(304);
  out.push(`<circle cx="${gx.toFixed(1)}" cy="${gy.toFixed(1)}" r="6.5" fill="${C.ink}"/>`);
  out.push(`<circle cx="${gx.toFixed(1)}" cy="${gy.toFixed(1)}" r="11" fill="none" stroke="${C.ink}" stroke-width="1.4"/>`);
  out.push(`<text x="${(gx + 16).toFixed(1)}" y="${(gy - 8).toFixed(1)}" font-size="13" font-weight="700" fill="${C.ink}">production GPU · ${formatProving(304)}</text>`);
  out.push(`<text x="${(gx + 16).toFixed(1)}" y="${(gy + 8).toFixed(1)}" font-size="11.5" fill="${C.muted}">RTX 3090 · ~52× faster than the Mac at N=8</text>`);

  // legend
  const lgY = m.t + 2;
  out.push(`<circle cx="${m.l + 14}" cy="${lgY}" r="4" fill="${C.ink}"/><text x="${m.l + 24}" y="${lgY + 4}" font-size="11.5" fill="${C.muted}">measured</text>`);
  out.push(`<circle cx="${m.l + 96}" cy="${lgY}" r="4" fill="${C.bg}" stroke="${C.ink}" stroke-width="2"/><text x="${m.l + 106}" y="${lgY + 4}" font-size="11.5" fill="${C.muted}">projected</text>`);

  out.push(`</svg>`);
  return out.join("");
}
