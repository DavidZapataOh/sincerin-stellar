/*
 * EcosystemStrip — the "built on" wall (the reference logo-strip pattern, made
 * Sincerin's own). The real stack Sincerin runs on, as a clean monochrome
 * word/glyph strip: Stellar · Soroban · RISC Zero · BN254. No third-party brand
 * logos pulled in — each is a small in-house glyph + wordmark in our type, which
 * stays honest (we don't imply endorsement) and on-brand (strict B&W, sharp).
 */

interface Item {
  glyph: React.ReactNode;
  name: string;
  role: string;
}

const ITEMS: Item[] = [
  {
    name: "Stellar",
    role: "settlement",
    glyph: (
      <svg viewBox="0 0 28 28" width="26" height="26" fill="none" aria-hidden="true">
        <circle cx="14" cy="14" r="12.4" stroke="currentColor" strokeWidth="1.4" />
        <path d="M4 18 L24 9.2" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
        <path d="M4 11.5 L24 18.8" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
      </svg>
    ),
  },
  {
    name: "Soroban",
    role: "smart contracts",
    glyph: (
      <svg viewBox="0 0 28 28" width="26" height="26" fill="none" aria-hidden="true">
        <rect x="4" y="4" width="20" height="20" stroke="currentColor" strokeWidth="1.4" />
        <path d="M9 14 L13 18 L19 10" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" />
      </svg>
    ),
  },
  {
    name: "RISC Zero",
    role: "the proof",
    glyph: (
      <svg viewBox="0 0 28 28" width="26" height="26" fill="none" aria-hidden="true">
        <polygon points="14,3 24,9 24,19 14,25 4,19 4,9" stroke="currentColor" strokeWidth="1.4" />
        <text x="14" y="14" textAnchor="middle" dy="0.34em" fontSize="8" fontFamily="var(--font-data)" fontWeight="600" fill="currentColor">zk</text>
      </svg>
    ),
  },
  {
    name: "BN254",
    role: "Groth16 curve",
    glyph: (
      <svg viewBox="0 0 28 28" width="26" height="26" fill="none" aria-hidden="true">
        <circle cx="14" cy="14" r="11" stroke="currentColor" strokeWidth="1.4" />
        <path d="M3.5 16 C 9 8, 19 8, 24.5 16" stroke="currentColor" strokeWidth="1.4" fill="none" />
        <circle cx="9" cy="12.7" r="1.5" fill="currentColor" />
        <circle cx="19" cy="12.7" r="1.5" fill="currentColor" />
      </svg>
    ),
  },
];

export function EcosystemStrip() {
  return (
    <div className="ecostrip">
      <p className="ecostrip-lead">
        Runs on Stellar. Proofs by RISC&nbsp;Zero.
      </p>
      <ul className="ecostrip-list">
        {ITEMS.map((it) => (
          <li className="ecostrip-item" key={it.name}>
            <span className="ecostrip-glyph">{it.glyph}</span>
            <span className="ecostrip-text">
              <span className="ecostrip-name">{it.name}</span>
              <span className="ecostrip-role">{it.role}</span>
            </span>
          </li>
        ))}
      </ul>
    </div>
  );
}
