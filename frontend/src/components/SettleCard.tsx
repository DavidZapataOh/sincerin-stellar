/*
 * SettleCard — the "real settle" payoff, as one confident object (not a stats
 * dump). A single on-chain settlement: the historic N=8 batch that aggregated 8
 * private payments into ONE Stellar transaction. Carries the verified state
 * beat (icon + label + the one allowed state-green), the real tx hash in mono,
 * and a direct explorer link. Every value is a live, verifiable testnet id from
 * lib/evidence — nothing fabricated (DESIGN.md: "real, not rendered").
 */

import { EVIDENCE, txUrl } from "../lib/evidence";
import { truncateMiddle } from "../lib/format";
import { LivePulse } from "./LivePulse";

export function SettleCard() {
  const hash = EVIDENCE.settleTx;
  return (
    <a
      className="settlecard"
      href={txUrl(hash)}
      target="_blank"
      rel="noreferrer noopener"
      aria-label="View the real on-chain settlement on Stellar testnet (opens stellar.expert)"
    >
      <div className="settlecard-head">
        <LivePulse tone="settled" label="Settled on Stellar testnet" />
        <span className="settlecard-net mono">testnet</span>
      </div>

      <div className="settlecard-headline">
        <span className="settlecard-n mono">8</span>
        <span className="settlecard-n-label">private payments</span>
        <span className="settlecard-arrow" aria-hidden="true">
          <Merge />
        </span>
        <span className="settlecard-n mono">1</span>
        <span className="settlecard-n-label">transaction</span>
      </div>

      <dl className="settlecard-rows">
        <div className="settlecard-row">
          <dt>Transaction</dt>
          <dd className="mono">{truncateMiddle(hash, 10, 8)}</dd>
        </div>
        <div className="settlecard-row">
          <dt>Status</dt>
          <dd className="settlecard-status">
            <Check />
            Verified on-chain
          </dd>
        </div>
        <div className="settlecard-row">
          <dt>Proof</dt>
          <dd>1 RISC Zero receipt · verified once</dd>
        </div>
      </dl>

      <span className="settlecard-cta">
        View on stellar.expert
        <ExternalArrow />
      </span>
    </a>
  );
}

function Merge() {
  return (
    <svg viewBox="0 0 40 24" width="40" height="24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M2 5 C 14 5, 14 12, 22 12" />
      <path d="M2 19 C 14 19, 14 12, 22 12" />
      <path d="M22 12 H 38" />
      <path d="M34 8 L 38 12 L 34 16" />
    </svg>
  );
}

function Check() {
  return (
    <svg viewBox="0 0 18 18" width="15" height="15" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <circle cx="9" cy="9" r="7.5" strokeWidth="1.4" />
      <path d="M5.5 9.2 L8 11.5 L12.5 6.5" />
    </svg>
  );
}

function ExternalArrow() {
  return (
    <svg viewBox="0 0 16 16" width="14" height="14" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M5 11 L11 5" />
      <path d="M6 5 H11 V10" />
    </svg>
  );
}
