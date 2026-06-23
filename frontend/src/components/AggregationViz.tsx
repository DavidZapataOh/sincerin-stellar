/*
 * The hero brand visual: "N withdrawals → 1 proof → 1 tx".
 *
 * A custom monochrome SVG (DESIGN.md "the aggregation visual" = the centerpiece
 * imagery; generated data-viz, not stock). N withdrawal leaves on the left
 * converge through a single proof node into one on-chain settle on the right.
 * Precise, hairline, B&W. Animates on mount; `active` drives the live "flow"
 * pulse along the converging paths. Honours prefers-reduced-motion.
 */

import { useId } from "react";

interface Props {
  /** Number of withdrawal leaves (the batch target N). */
  n: number;
  /** When true, the convergence flow animates (proving / live). */
  active?: boolean;
  /** When true, the settle node reads as settled (the green payoff pop). */
  settled?: boolean;
}

export function AggregationViz({ n, active = false, settled = false }: Props) {
  const uid = useId().replace(/:/g, "");
  // Geometry — a 720×420 viewBox, leaves stacked left, proof node centre,
  // settle node right.
  const W = 720;
  const H = 420;
  const leafX = 96;
  const proofX = 372;
  const settleX = 612;
  const midY = H / 2;
  const count = Math.max(2, Math.min(n, 8));

  const leaves = Array.from({ length: count }, (_, i) => {
    const span = H - 96;
    const y = 48 + (span * i) / (count - 1);
    return { y, i };
  });

  return (
    <svg
      className={`aggviz ${active ? "is-active" : ""} ${settled ? "is-settled" : ""}`}
      viewBox={`0 0 ${W} ${H}`}
      role="img"
      aria-label={`${count} withdrawals converge into one proof, then settle on-chain in a single transaction.`}
      preserveAspectRatio="xMidYMid meet"
    >
      <defs>
        <marker
          id={`${uid}-arrow`}
          viewBox="0 0 10 10"
          refX="8"
          refY="5"
          markerWidth="6"
          markerHeight="6"
          orient="auto-start-reverse"
        >
          <path d="M0,0 L10,5 L0,10 z" className="aggviz-arrowhead" />
        </marker>
      </defs>

      {/* converging paths: each leaf → proof node */}
      <g className="aggviz-paths">
        {leaves.map(({ y, i }) => {
          const d = `M ${leafX + 22} ${y} C ${proofX - 120} ${y}, ${proofX - 120} ${midY}, ${proofX - 28} ${midY}`;
          return (
            <path
              key={i}
              d={d}
              className="aggviz-link"
              style={{ animationDelay: `${i * 90}ms` }}
            />
          );
        })}
      </g>

      {/* proof → settle */}
      <path
        d={`M ${proofX + 30} ${midY} L ${settleX - 34} ${midY}`}
        className="aggviz-link aggviz-link--trunk"
        markerEnd={`url(#${uid}-arrow)`}
      />

      {/* flow particles along the converging links (active only) */}
      {active &&
        leaves.map(({ y, i }) => {
          const d = `M ${leafX + 22} ${y} C ${proofX - 120} ${y}, ${proofX - 120} ${midY}, ${proofX - 28} ${midY}`;
          return (
            <circle key={`p${i}`} r="3.5" className="aggviz-particle">
              <animateMotion
                dur="1.8s"
                repeatCount="indefinite"
                begin={`${i * 0.18}s`}
                path={d}
                rotate="auto"
              />
            </circle>
          );
        })}

      {/* leaf nodes (withdrawals) */}
      <g className="aggviz-leaves">
        {leaves.map(({ y, i }) => (
          <g key={i} transform={`translate(${leafX}, ${y})`}>
            <rect x="-22" y="-13" width="44" height="26" className="aggviz-leaf" />
            <line x1="-12" y1="-3" x2="12" y2="-3" className="aggviz-leaf-line" />
            <line x1="-12" y1="4" x2="6" y2="4" className="aggviz-leaf-line" />
          </g>
        ))}
      </g>

      {/* proof node (1 RISC Zero receipt) */}
      <g transform={`translate(${proofX}, ${midY})`} className="aggviz-proof">
        <polygon
          points="0,-34 30,-17 30,17 0,34 -30,17 -30,-17"
          className="aggviz-proof-shape"
        />
        <text className="aggviz-proof-glyph" textAnchor="middle" dy="0.36em">
          zk
        </text>
      </g>

      {/* settle node (1 on-chain tx) */}
      <g transform={`translate(${settleX}, ${midY})`} className="aggviz-settle">
        <circle r="34" className="aggviz-settle-ring" />
        <circle r="34" className="aggviz-settle-pulse" />
        <path
          d="M -13 1 L -4 11 L 14 -10"
          className="aggviz-settle-check"
          fill="none"
        />
      </g>

      {/* labels */}
      <text x={leafX} y={H - 16} className="aggviz-label" textAnchor="middle">
        {count} withdrawals
      </text>
      <text x={proofX} y={H - 16} className="aggviz-label" textAnchor="middle">
        1 proof
      </text>
      <text x={settleX} y={H - 16} className="aggviz-label" textAnchor="middle">
        1 transaction
      </text>
    </svg>
  );
}
