/*
 * EcosystemStrip — the "built on" logo wall. The real marks of the stack
 * Sincerin runs on (Stellar · Soroban · RISC Zero · BN254), normalised to one
 * monochrome treatment + a uniform optical size so the row reads EVEN on the
 * strict B&W brand. The marks have opposite polarities (RISC Zero is white-on-
 * black; the rest are dark-on-light) — per-mark filters in landing.css bring
 * them to a single black-on-white reading. Files live in public/logos/.
 */

interface Item {
  src: string;
  name: string;
  cls?: string;
}

const ITEMS: Item[] = [
  { src: "/logos/stellar.png", name: "Stellar" },
  { src: "/logos/soroban.png", name: "Soroban" },
  { src: "/logos/risc0.png", name: "RISC Zero", cls: "ecostrip-logo--risc0" },
  { src: "/logos/bn254.png", name: "BN254" },
];

export function EcosystemStrip() {
  return (
    <div className="ecostrip">
      <p className="ecostrip-lead">Runs on Stellar. Proofs by RISC&nbsp;Zero.</p>
      <ul className="ecostrip-list">
        {ITEMS.map((it) => (
          <li className="ecostrip-item" key={it.name}>
            <span className="ecostrip-media">
              <img
                className={`ecostrip-logo ${it.cls ?? ""}`}
                src={it.src}
                alt={it.name}
                title={it.name}
                loading="lazy"
                decoding="async"
              />
            </span>
          </li>
        ))}
      </ul>
    </div>
  );
}
