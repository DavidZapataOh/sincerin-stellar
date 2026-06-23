/*
 * AggregationFlow — Sincerin's SIGNATURE animated centerpiece.
 *
 * The product, drawn: MANY private payments stream in from the left, gather
 * into a single bundling lane, collapse into ONE proof, and settle as ONE
 * transaction. Continuously animated — flow along the streams, a periodic
 * "merge" gather, and a green "settle pulse" at the on-chain node.
 *
 * This is the brand motif from DESIGN.md ("the aggregation visual", generated
 * data-viz not stock). Monochrome, hairline, sharp. The ONE state-green appears
 * only at the settle node — the on-chain payoff. Two framings:
 *   variant="hero"     → compact, sits beside the hero copy.
 *   variant="showpiece"→ wide, full-bleed, the dedicated section.
 *
 * prefers-reduced-motion: the looping flow/pulse stop; the composition stays a
 * strong, fully-drawn static diagram (content is never gated on motion).
 */

import { useId } from "react";

interface Props {
  /** Number of incoming payment streams. Clamped to a legible 5–9. */
  n?: number;
  variant?: "hero" | "showpiece";
  /** When true the settle node reads settled (the green payoff). Loops anyway. */
  settled?: boolean;
}

export function AggregationFlow({ n = 7, variant = "showpiece", settled = false }: Props) {
  const uid = useId().replace(/:/g, "");
  const W = 1000;
  const H = variant === "hero" ? 560 : 460;
  const midY = H / 2;

  // Geometry: streams enter left, gather at a vertical "bundling lane", collapse
  // into the proof node, then a single trunk runs to the settle node.
  const inX = variant === "hero" ? 64 : 70;
  const gatherX = variant === "hero" ? 360 : 430; // where streams pinch together
  const proofX = variant === "hero" ? 470 : 560;
  const settleX = variant === "hero" ? 760 : 858;

  const count = Math.max(5, Math.min(n, 9));
  const spread = H - (variant === "hero" ? 150 : 130);
  const streams = Array.from({ length: count }, (_, i) => {
    const y = (H - spread) / 2 + (spread * i) / (count - 1);
    // a smooth S-curve from the entry point into the gather pinch (at midY)
    const c1 = inX + (gatherX - inX) * 0.42;
    const c2 = gatherX - (gatherX - inX) * 0.18;
    const d = `M ${inX} ${y} C ${c1} ${y}, ${c2} ${midY}, ${gatherX} ${midY}`;
    return { y, i, d };
  });

  const proofGap = variant === "hero" ? 40 : 46;
  const trunkD = `M ${proofX + proofGap} ${midY} L ${settleX - (variant === "hero" ? 42 : 48)} ${midY}`;
  const proofToGather = `M ${gatherX} ${midY} L ${proofX - proofGap} ${midY}`;
  // the single merged output: gather → settle, straight across (one clean run)
  const outPath = `M ${gatherX} ${midY} L ${settleX - (variant === "hero" ? 42 : 48)} ${midY}`;

  const proofR = variant === "hero" ? 40 : 48;
  const settleR = variant === "hero" ? 40 : 48;

  return (
    <svg
      className={`aggflow aggflow--${variant} ${settled ? "is-settled" : ""}`}
      viewBox={`0 0 ${W} ${H}`}
      role="img"
      aria-label={`${count} private payments stream in, bundle into a single proof, and settle on Stellar in one transaction.`}
      preserveAspectRatio="xMidYMid meet"
    >
      <defs>
        <marker
          id={`${uid}-tip`}
          viewBox="0 0 10 10"
          refX="8"
          refY="5"
          markerWidth="6.5"
          markerHeight="6.5"
          orient="auto-start-reverse"
        >
          <path d="M0,0 L10,5 L0,10 z" className="aggflow-tip" />
        </marker>
        {/* soft edge mask so streams fade in from the left edge */}
        <linearGradient id={`${uid}-fade`} x1="0" y1="0" x2="1" y2="0">
          <stop offset="0" stopColor="white" stopOpacity="0" />
          <stop offset="0.12" stopColor="white" stopOpacity="1" />
        </linearGradient>
        <mask id={`${uid}-mask`}>
          <rect x="0" y="0" width={W} height={H} fill={`url(#${uid}-fade)`} />
        </mask>
      </defs>

      {/* ── incoming payment streams ─────────────────────────────────────── */}
      <g className="aggflow-streams" mask={`url(#${uid}-mask)`}>
        {streams.map(({ d, i }) => (
          <path key={i} d={d} className="aggflow-stream" style={{ animationDelay: `${i * 90}ms` }} />
        ))}
      </g>

      {/* flow particles riding each stream (looping) */}
      <g className="aggflow-particles">
        {streams.map(({ d, i }) => (
          <circle key={`p${i}`} r={variant === "hero" ? 3.4 : 3.8} className="aggflow-particle">
            <animateMotion dur="2.4s" repeatCount="indefinite" begin={`${i * 0.26}s`} path={d} />
          </circle>
        ))}
      </g>

      {/* ── the bundling pinch → proof → settle trunk ────────────────────── */}
      <path d={proofToGather} className="aggflow-link aggflow-trunk" />
      <path d={trunkD} className="aggflow-link aggflow-trunk" markerEnd={`url(#${uid}-tip)`} />

      {/* one particle traveling the merged output → settle (the "settled" output) */}
      <g className="aggflow-out">
        <circle r={variant === "hero" ? 4 : 4.4} className="aggflow-particle aggflow-particle--out">
          <animateMotion dur="2.4s" repeatCount="indefinite" begin="1.1s" path={outPath} />
        </circle>
      </g>

      {/* the bundling-lane gather node (where streams pinch) */}
      <g transform={`translate(${gatherX}, ${midY})`} className="aggflow-gather">
        <circle r={variant === "hero" ? 6 : 7} className="aggflow-gather-dot" />
      </g>

      {/* stream entry nodes (payments) */}
      <g className="aggflow-ins">
        {streams.map(({ y, i }) => (
          <g key={i} transform={`translate(${inX}, ${y})`}>
            <circle r={variant === "hero" ? 4.5 : 5} className="aggflow-in" />
          </g>
        ))}
      </g>

      {/* proof node (1 RISC Zero receipt) — a hexagon */}
      <g transform={`translate(${proofX}, ${midY})`} className="aggflow-proof">
        <polygon
          points={hex(proofR)}
          className="aggflow-proof-shape"
        />
        <text className="aggflow-proof-glyph mono" textAnchor="middle" dy="0.34em">
          zk
        </text>
      </g>

      {/* settle node (1 on-chain tx) — the green payoff, looping pulse */}
      <g transform={`translate(${settleX}, ${midY})`} className="aggflow-settle">
        <circle r={settleR + 14} className="aggflow-settle-pulse" />
        <circle r={settleR} className="aggflow-settle-ring" />
        <path
          d={`M ${-settleR * 0.34} 1 L ${-settleR * 0.1} ${settleR * 0.28} L ${settleR * 0.36} ${-settleR * 0.26}`}
          className="aggflow-settle-check"
          fill="none"
        />
      </g>

      {/* labels (text, not mono — these are descriptors, the data is the count) */}
      <text x={inX + 8} y={H - (variant === "hero" ? 22 : 24)} className="aggflow-label" textAnchor="start">
        {count} private payments
      </text>
      <text x={proofX} y={H - (variant === "hero" ? 22 : 24)} className="aggflow-label" textAnchor="middle">
        1 proof
      </text>
      <text x={settleX} y={H - (variant === "hero" ? 22 : 24)} className="aggflow-label aggflow-label--settle" textAnchor="middle">
        1 transaction
      </text>
    </svg>
  );
}

/* flat-top hexagon points for radius r, centred on origin */
function hex(r: number): string {
  const pts: string[] = [];
  for (let k = 0; k < 6; k++) {
    const a = (Math.PI / 3) * k - Math.PI / 6;
    pts.push(`${(Math.cos(a) * r).toFixed(1)},${(Math.sin(a) * r).toFixed(1)}`);
  }
  return pts.join(" ");
}
