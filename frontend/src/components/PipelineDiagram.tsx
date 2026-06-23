/*
 * The pipeline (how it works): deposit → withdraw intents → sequencer batches
 * → 1 proof → settle_batch atomic. A monochrome editorial flow, not a grid of
 * identical cards — each step is a numbered node on a connecting rail (the
 * numbers earn their place here: it IS an ordered sequence). Horizontal rail on
 * desktop, vertical on mobile. Distinct glyph per step.
 */

const STEPS = [
  {
    k: "deposit",
    title: "Deposit",
    body: "Funds enter the privacy pool as a commitment — a leaf in the Merkle tree. The deposit is on-chain and public.",
    glyph: <DepositGlyph />,
  },
  {
    k: "intent",
    title: "Withdraw intents",
    body: "Each withdrawal is an off-chain intent: a note secret, a recipient, a Merkle path, a fresh nullifier. The sequencer collects them.",
    glyph: <IntentGlyph />,
  },
  {
    k: "batch",
    title: "Sequencer batches",
    body: "At N=8, the sequencer seals the batch and re-executes every withdrawal's validity — membership, nullifier, balance — in one RISC Zero guest.",
    glyph: <BatchGlyph />,
  },
  {
    k: "prove",
    title: "One proof",
    body: "The guest run is wrapped into a single Groth16/BN254 receipt. Minutes of real proving — the same proof, whatever N is.",
    glyph: <ProveGlyph />,
  },
  {
    k: "settle",
    title: "settle_batch · atomic",
    body: "Soroban verifies the one proof, then marks N nullifiers and runs N transfers — all-or-nothing. A replay fails with NullifierSpent.",
    glyph: <SettleGlyph />,
  },
] as const;

export function PipelineDiagram() {
  return (
    <ol className="pipeline" aria-label="The rollup pipeline, in five ordered steps">
      {STEPS.map((s, i) => (
        <li key={s.k} className={`pipe-step pipe-step--${s.k}`}>
          <div className="pipe-node">
            <span className="pipe-index mono" aria-hidden="true">
              {i + 1}
            </span>
            <span className="pipe-glyph" aria-hidden="true">
              {s.glyph}
            </span>
          </div>
          <div className="pipe-body">
            <h3 className="pipe-title">{s.title}</h3>
            <p className="pipe-text">{s.body}</p>
          </div>
          {i < STEPS.length - 1 && (
            <span className="pipe-rail" aria-hidden="true" />
          )}
        </li>
      ))}
    </ol>
  );
}

/* ── glyphs (1.6 hairline, monochrome) ─────────────────────────────────── */
function DepositGlyph() {
  return (
    <svg viewBox="0 0 28 28" width="26" height="26" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round">
      <path d="M14 4 V15" />
      <path d="M9.5 10.5 L14 15 L18.5 10.5" />
      <path d="M5 17 V22 H23 V17" />
    </svg>
  );
}
function IntentGlyph() {
  return (
    <svg viewBox="0 0 28 28" width="26" height="26" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round">
      <rect x="6" y="5" width="16" height="18" />
      <path d="M9.5 10 H18.5 M9.5 14 H18.5 M9.5 18 H15" />
    </svg>
  );
}
function BatchGlyph() {
  return (
    <svg viewBox="0 0 28 28" width="26" height="26" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round">
      <path d="M5 8 L9 8 M5 14 L9 14 M5 20 L9 20" />
      <path d="M9 8 C16 8 16 14 22 14 M9 14 H22 M9 20 C16 20 16 14 22 14" />
      <circle cx="22.5" cy="14" r="2" fill="currentColor" stroke="none" />
    </svg>
  );
}
function ProveGlyph() {
  return (
    <svg viewBox="0 0 28 28" width="26" height="26" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinejoin="round">
      <polygon points="14,4 23,9 23,19 14,24 5,19 5,9" />
      <text x="14" y="17.5" textAnchor="middle" fontFamily="var(--font-data)" fontSize="8.5" fontWeight="600" fill="currentColor" stroke="none">zk</text>
    </svg>
  );
}
function SettleGlyph() {
  return (
    <svg viewBox="0 0 28 28" width="26" height="26" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="14" cy="14" r="10" />
      <path d="M9.5 14.5 L12.5 17.5 L18.5 10.5" />
    </svg>
  );
}
