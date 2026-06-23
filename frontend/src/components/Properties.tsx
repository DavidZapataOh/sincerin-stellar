/*
 * Properties — three LIVE concept cards that demonstrate Sincerin in motion
 * (not text that asserts it). The motion language is borrowed from the kind of
 * line-art "solutions" illustrations that read as alive without shouting: a
 * single clear looping motif per card, monochrome, sharp, with the data in mono.
 *
 *   1 Many → one        — payments charge in and collapse into one proof.
 *   2 Unlinkable        — the sender↔recipient mapping keeps re-shuffling, so
 *                         who paid whom can't be traced. (The brand's core.)
 *   3 Settles on Stellar— a check draws + the ONE state-green pulse fires, tied
 *                         to a real on-chain tx hash.
 *
 * Every illustration is fully DRAWN by default; the loop only enhances it, so it
 * stays legible under prefers-reduced-motion and in headless renders. Strict
 * B&W; state-green appears only on the settle card — the on-chain payoff.
 */

import type { ReactNode } from "react";
import { Reveal } from "./Reveal";
import { EVIDENCE } from "../lib/evidence";

interface Prop {
  key: string;
  title: string;
  blurb: string;
  viz: ReactNode;
}

const PROPS: Prop[] = [
  {
    key: "aggregate",
    title: "Many payments, one transaction",
    blurb: "A whole batch of private payments is bundled into a single proof and settles together.",
    viz: <VizAggregate />,
  },
  {
    key: "unlinkable",
    title: "Unlinkable",
    blurb: "The link between who paid and who got paid is broken — the trail can't be followed.",
    viz: <VizUnlinkable />,
  },
  {
    key: "settle",
    title: "Real, final, on Stellar",
    blurb: "Verified on-chain in one transaction — no bridges, no wrapped tokens, no middlemen.",
    viz: <VizSettle />,
  },
];

export function Properties() {
  return (
    <section className="container section land-props" aria-labelledby="props-title">
      <Reveal className="land-props-head">
        <p className="land-kicker">What it does</p>
        <h2 id="props-title">Private payments that actually scale.</h2>
      </Reveal>

      <ul className="land-props-grid">
        {PROPS.map((p, i) => (
          <Reveal as="li" key={p.key} className="prop-card" delay={i * 90}>
            <span className="prop-viz">{p.viz}</span>
            <h3 className="prop-title">{p.title}</h3>
            <p className="prop-blurb">{p.blurb}</p>
          </Reveal>
        ))}
      </ul>
    </section>
  );
}

/* ── 1 · Many → one ──────────────────────────────────────────────────────────
   Four "payment" receipts stacked top; each charges a hairline down into a
   single hexagon proof, which pulses once the batch is in. Reads: many in, one
   out. Horizontal-stack framing keeps it distinct from the hero's stream diagram. */
function VizAggregate() {
  const rows = [0, 1, 2, 3];
  const top = 22;
  const gap = 15;
  const busY = top + rows.length * gap + 2; // where the stack feeds out
  const hexCY = 118;
  return (
    <svg className="cc" viewBox="0 0 200 150" role="img" aria-label="Four private payments bundle into a single proof.">
      {/* the receipts (payments) — each ticks in sequence */}
      <g className="cc-receipts">
        {rows.map((i) => (
          <g key={i} className="cc-receipt" style={{ ["--i" as string]: i }}>
            <rect x="58" y={top + i * gap} width="84" height="10" rx="1.5" />
            <line x1="64" y1={top + i * gap + 5} x2="96" y2={top + i * gap + 5} />
          </g>
        ))}
      </g>

      {/* one clean channel feeds the bundled batch down into the proof */}
      <circle className="cc-junction" cx="100" cy={busY} r="2.4" />
      <path
        className="cc-charge"
        style={{ ["--i" as string]: 0 }}
        d={`M 100 ${busY} L 100 ${hexCY - 19}`}
      />

      {/* the single proof (hexagon) + pulse */}
      <g transform={`translate(100 ${hexCY})`} className="cc-proof">
        <circle className="cc-proof-pulse" r="18" />
        <polygon className="cc-proof-shape" points={hexPoints(17)} />
        <text className="cc-proof-glyph" textAnchor="middle" dy="0.34em">
          1
        </text>
      </g>
    </svg>
  );
}

/* ── 2 · Unlinkable ──────────────────────────────────────────────────────────
   A bipartite mesh between senders (left) and recipients (right). Subsets of the
   connectors fade in and out on staggered timers, so the visible pairing keeps
   re-shuffling and no fixed link can be read. The dots stay; only the mapping
   is uncertain. */
function VizUnlinkable() {
  const ys = [30, 60, 90, 120];
  const lx = 34;
  const rx = 166;
  // a partial mesh: each sender reaches two recipients (offset), 8 candidate links
  const links: Array<[number, number]> = [];
  ys.forEach((_, i) => {
    links.push([i, (i + 1) % 4]);
    links.push([i, (i + 2) % 4]);
  });
  return (
    <svg className="cc" viewBox="0 0 200 150" role="img" aria-label="The mapping between senders and recipients keeps re-shuffling and cannot be traced.">
      <g className="cc-mesh">
        {links.map(([a, b], i) => (
          <line
            key={i}
            className="cc-link"
            style={{ ["--i" as string]: i }}
            x1={lx}
            y1={ys[a]}
            x2={rx}
            y2={ys[b]}
          />
        ))}
      </g>
      <g className="cc-ends">
        {ys.map((y, i) => (
          <circle key={`l${i}`} className="cc-end" cx={lx} cy={y} r="4.5" />
        ))}
        {ys.map((y, i) => (
          <circle key={`r${i}`} className="cc-end" cx={rx} cy={y} r="4.5" />
        ))}
      </g>
      <text className="cc-axis" x={lx} y="14" textAnchor="middle">
        in
      </text>
      <text className="cc-axis" x={rx} y="14" textAnchor="middle">
        out
      </text>
    </svg>
  );
}

/* ── 3 · Settles on Stellar ──────────────────────────────────────────────────
   The on-chain payoff: a check draws itself inside the settle node and the ONE
   state-green pulse fires. A real tx hash anchors it — honest, not rendered. */
function VizSettle() {
  const short = `${EVIDENCE.settleTx.slice(0, 8)}…${EVIDENCE.settleTx.slice(-4)}`;
  return (
    <svg className="cc cc--settle" viewBox="0 0 200 150" role="img" aria-label="A real batch settled on Stellar in a single transaction.">
      <g transform="translate(100 62)" className="cc-settle">
        <circle className="cc-settle-pulse" r="34" />
        <circle className="cc-settle-ring" r="30" />
        <path className="cc-settle-check" d="M -13 1 L -4 11 L 14 -10" fill="none" />
      </g>
      <text className="cc-hash" x="100" y="126" textAnchor="middle">
        {short}
      </text>
      <text className="cc-axis cc-axis--settle" x="100" y="142" textAnchor="middle">
        settled · 1 tx
      </text>
    </svg>
  );
}

/* flat-top hexagon points for radius r, centred on origin */
function hexPoints(r: number): string {
  const pts: string[] = [];
  for (let k = 0; k < 6; k++) {
    const a = (Math.PI / 3) * k - Math.PI / 6;
    pts.push(`${(Math.cos(a) * r).toFixed(1)},${(Math.sin(a) * r).toFixed(1)}`);
  }
  return pts.join(" ");
}
