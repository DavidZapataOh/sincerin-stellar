/*
 * Two-axis benchmark — the project's differentiator, inline.
 *
 * All numbers are MEASURED on-chain values (docs/proving-times.md "Eje 2"):
 *   N=2 settle 31.5M (~7.9%), N=8 settle 36,118,956 (~9.0%, tx aedc1cc4…).
 *   Projected N=16 ~43.8M, N=32 ~56.1M (1 Groth16 verify + 0.77M/note).
 * Baseline = N individual pool verifications (N × ~35M) — crosses the 400M
 * single-tx budget at N≈12, so large batches are impossible WITHOUT aggregation.
 *
 * Honest by construction: measured vs projected are labelled distinctly; the
 * full chart is s3/01 — this is the tasteful inline version (plan §3.3).
 */

const BUDGET = 400; // M instructions — Soroban single-tx limit.

interface Point {
  n: number;
  aggregated: number; // M instr — settle, ~flat
  baseline: number; // M instr — N × ~35M
  measured: boolean;
}

const DATA: Point[] = [
  { n: 2, aggregated: 31.5, baseline: 70, measured: true },
  { n: 8, aggregated: 36.1, baseline: 280, measured: true },
  { n: 16, aggregated: 43.8, baseline: 560, measured: false },
  { n: 32, aggregated: 56.1, baseline: 1120, measured: false },
];

export function BenchChart() {
  const W = 560;
  const H = 300;
  const padL = 52;
  const padR = 18;
  const padT = 20;
  const padB = 44;
  const maxY = 400; // clip the y-axis at the budget so the "fits/doesn't fit" line reads
  const maxN = 34;

  const x = (n: number) => padL + ((n - 1) / (maxN - 1)) * (W - padL - padR);
  const y = (v: number) =>
    padT + (1 - Math.min(v, maxY) / maxY) * (H - padT - padB);

  const aggLine = DATA.map((d) => `${x(d.n)},${y(d.aggregated)}`).join(" ");
  // baseline drawn only up to where it crosses the budget (N≈12), then clipped.
  const baseLine = DATA.filter((d) => d.baseline <= maxY * 1.6)
    .map((d) => `${x(d.n)},${y(d.baseline)}`)
    .join(" ");

  // crossing point of baseline with the 400M budget: 35·N = 400 ⇒ N ≈ 11.4
  const crossN = 400 / 35;

  return (
    <figure className="bench">
      <svg
        className="bench-svg"
        viewBox={`0 0 ${W} ${H}`}
        role="img"
        aria-label="On-chain cost by batch size N. Aggregated settle stays roughly flat near 9% of the 400M single-transaction budget; N individual verifications grow linearly and exceed the budget past N≈12."
        preserveAspectRatio="xMidYMid meet"
      >
        {/* budget ceiling */}
        <line
          x1={padL}
          y1={y(BUDGET)}
          x2={W - padR}
          y2={y(BUDGET)}
          className="bench-budget"
        />
        <text x={W - padR} y={y(BUDGET) - 7} className="bench-budget-label" textAnchor="end">
          400M — 1 tx budget
        </text>

        {/* axes */}
        <line x1={padL} y1={padT} x2={padL} y2={H - padB} className="bench-axis" />
        <line x1={padL} y1={H - padB} x2={W - padR} y2={H - padB} className="bench-axis" />

        {/* y ticks */}
        {[0, 100, 200, 300, 400].map((v) => (
          <g key={v}>
            <line x1={padL - 4} y1={y(v)} x2={padL} y2={y(v)} className="bench-axis" />
            <text x={padL - 9} y={y(v) + 3.5} className="bench-tick" textAnchor="end">
              {v}M
            </text>
          </g>
        ))}

        {/* baseline (N individual verifications) — dashed, clipped at budget */}
        <polyline points={baseLine} className="bench-baseline" />
        {/* crossing marker */}
        <line
          x1={x(crossN)}
          y1={y(0)}
          x2={x(crossN)}
          y2={y(BUDGET)}
          className="bench-cross"
        />
        <text x={x(crossN)} y={H - padB + 30} className="bench-cross-label" textAnchor="middle">
          baseline exceeds budget · N≈12
        </text>

        {/* aggregated settle — solid, the flat line that is the whole point */}
        <polyline points={aggLine} className="bench-agg" />
        {DATA.map((d) => (
          <g key={d.n}>
            <circle
              cx={x(d.n)}
              cy={y(d.aggregated)}
              r="4.5"
              className={`bench-dot ${d.measured ? "is-measured" : "is-projected"}`}
            />
            <text x={x(d.n)} y={y(d.aggregated) - 12} className="bench-dotlabel" textAnchor="middle">
              {d.aggregated}M
            </text>
          </g>
        ))}

        {/* x ticks */}
        {DATA.map((d) => (
          <text key={d.n} x={x(d.n)} y={H - padB + 16} className="bench-tick" textAnchor="middle">
            N={d.n}
          </text>
        ))}
      </svg>

      <figcaption className="bench-legend">
        <span className="bench-key">
          <span className="bench-key-swatch bench-key-swatch--agg" /> Aggregated settle
          <span className="bench-key-note"> · 1 Groth16 verify, ~flat in N</span>
        </span>
        <span className="bench-key">
          <span className="bench-key-swatch bench-key-swatch--base" /> N individual
          verifications<span className="bench-key-note"> · N × ~35M</span>
        </span>
        <span className="bench-key bench-key--prov">
          <span className="bench-key-swatch bench-key-swatch--measured" /> measured
          <span className="bench-key-swatch bench-key-swatch--projected" /> projected
        </span>
      </figcaption>
    </figure>
  );
}
