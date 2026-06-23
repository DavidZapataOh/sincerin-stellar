/*
 * Footer — the honest trust-boundary statement + the live contract ids read
 * from /config (never hardcoded). Restates: unlinkable on-chain, amounts in the
 * clear, operator sees the mapping.
 */

import type { SequencerConfig } from "../lib/api";
import { truncateMiddle } from "../lib/format";

export function Footer({ config }: { config: SequencerConfig | null }) {
  return (
    <footer className="appfoot">
      <div className="container appfoot-inner">
        <div className="appfoot-statement">
          <p>
            <strong>Unlinkable, not hidden.</strong> On-chain, the settle breaks
            the deposit↔withdrawal link between counterparties. Amounts are in the
            clear. The operator running this sequencer receives the note secrets
            and therefore sees the note↔recipient mapping — the unlinkability is
            public/on-chain, not against the operator. We state that boundary
            plainly.
          </p>
        </div>

        {config && (
          <dl className="appfoot-meta">
            <div>
              <dt>Network</dt>
              <dd>{config.network}</dd>
            </div>
            <div>
              <dt>Rollup</dt>
              <dd className="mono">
                {config.rollup_id ? truncateMiddle(config.rollup_id) : "—"}
              </dd>
            </div>
            <div>
              <dt>Verifier</dt>
              <dd className="mono">{truncateMiddle(config.verifier_id)}</dd>
            </div>
          </dl>
        )}
      </div>
    </footer>
  );
}
