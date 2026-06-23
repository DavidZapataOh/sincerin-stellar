/*
 * Status chip — the one place a pill radius is allowed (DESIGN.md).
 * State is communicated by ICON + LABEL + position first; the hue is reinforcement
 * (WCAG / colorblind-safe — never color alone). Each state has a distinct glyph.
 */

import type { RequestState } from "../lib/api";

type ChipState = RequestState | "starting";

const META: Record<
  ChipState,
  { label: string; glyph: React.ReactNode; live?: boolean }
> = {
  pending: { label: "Pending", glyph: <DotGlyph /> },
  batched: { label: "Batched", glyph: <StackGlyph /> },
  starting: { label: "Starting prover", glyph: <SpinGlyph />, live: true },
  proving: { label: "Proving", glyph: <PulseGlyph />, live: true },
  settled: { label: "Settled on-chain", glyph: <CheckGlyph /> },
  failed: { label: "Failed", glyph: <CrossGlyph /> },
};

export function StatusChip({ state }: { state: ChipState }) {
  const m = META[state];
  return (
    <span className={`chip chip--${state}`} data-state={state}>
      <span className="chip-glyph" aria-hidden="true">
        {m.glyph}
      </span>
      <span className="chip-label">{m.label}</span>
    </span>
  );
}

function DotGlyph() {
  return (
    <svg viewBox="0 0 16 16" width="14" height="14">
      <circle cx="8" cy="8" r="3.5" fill="currentColor" />
    </svg>
  );
}
function StackGlyph() {
  return (
    <svg viewBox="0 0 16 16" width="14" height="14" stroke="currentColor" fill="none" strokeWidth="1.6">
      <rect x="3" y="3" width="10" height="3.2" />
      <rect x="3" y="9.8" width="10" height="3.2" />
    </svg>
  );
}
function SpinGlyph() {
  return (
    <svg viewBox="0 0 16 16" width="14" height="14" className="chip-spin">
      <path d="M8 2 a6 6 0 1 1 -6 6" stroke="currentColor" strokeWidth="1.8" fill="none" strokeLinecap="round" />
    </svg>
  );
}
function PulseGlyph() {
  return (
    <svg viewBox="0 0 16 16" width="14" height="14" stroke="currentColor" fill="none" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round">
      <path className="chip-pulse-path" d="M1 8 h3 l1.6 -4 l2.4 8 l1.8 -5 l1.4 1 H15" />
    </svg>
  );
}
function CheckGlyph() {
  return (
    <svg viewBox="0 0 16 16" width="14" height="14" stroke="currentColor" fill="none" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M3 8.5 L6.5 12 L13 4.5" />
    </svg>
  );
}
function CrossGlyph() {
  return (
    <svg viewBox="0 0 16 16" width="14" height="14" stroke="currentColor" fill="none" strokeWidth="2" strokeLinecap="round">
      <path d="M4 4 L12 12 M12 4 L4 12" />
    </svg>
  );
}
