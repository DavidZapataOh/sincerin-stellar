/*
 * Footer — route-aware, carries the honest trust-boundary statement.
 *
 *  - surface="landing": the brand footer — restated trust boundary, the two
 *    surfaces, the repo link, and the honest "WIP · not audited · testnet" note.
 *    Contract ids come from the evidence constants (the historic on-chain ones).
 *  - surface="demo": the live footer — the trust statement + the contract ids
 *    read from the loaded /config (never hardcoded).
 */

import type { SequencerConfig } from "../lib/api";
import { truncateMiddle } from "../lib/format";
import { Link } from "../lib/router";
import { EVIDENCE, EXPLORER, contractUrl } from "../lib/evidence";

const TrustStatement = () => (
  <p>
    <strong>Unlinkable, not hidden.</strong> On-chain, the settle breaks the
    deposit↔withdrawal link between counterparties. Amounts are in the clear.
    The operator running this sequencer receives the note secrets and therefore
    sees the note↔recipient mapping — the unlinkability is public/on-chain, not
    against the operator. We state that boundary plainly.
  </p>
);

type Props =
  | { surface: "landing" }
  | { surface: "demo"; config: SequencerConfig | null };

export function Footer(props: Props) {
  if (props.surface === "landing") {
    return (
      <footer className="appfoot appfoot--landing">
        <div className="container appfoot-inner">
          <div className="appfoot-statement">
            <TrustStatement />
          </div>
          <nav className="appfoot-nav" aria-label="Footer">
            <span className="appfoot-nav-head">Surfaces</span>
            <Link to="/">Landing</Link>
            <Link to="/demo">Demo</Link>
            <a href={EXPLORER.repo} target="_blank" rel="noreferrer noopener">
              Repository
            </a>
          </nav>
          <dl className="appfoot-meta">
            <div>
              <dt>Network</dt>
              <dd>{EXPLORER.network}</dd>
            </div>
            <div>
              <dt>Rollup</dt>
              <dd>
                <a className="mono" href={contractUrl(EVIDENCE.rollup)} target="_blank" rel="noreferrer noopener">
                  {truncateMiddle(EVIDENCE.rollup)}
                </a>
              </dd>
            </div>
            <div>
              <dt>Verifier</dt>
              <dd>
                <a className="mono" href={contractUrl(EVIDENCE.verifier)} target="_blank" rel="noreferrer noopener">
                  {truncateMiddle(EVIDENCE.verifier)}
                </a>
              </dd>
            </div>
          </dl>
        </div>
        <div className="container appfoot-base">
          <span className="appfoot-wip">WIP · not audited · testnet only</span>
          <span className="appfoot-sig">Confidential Payments Rollup on Stellar</span>
        </div>
      </footer>
    );
  }

  const { config } = props;
  return (
    <footer className="appfoot">
      <div className="container appfoot-inner appfoot-inner--demo">
        <div className="appfoot-statement">
          <TrustStatement />
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
      <div className="container appfoot-base">
        <span className="appfoot-wip">WIP · not audited · testnet only</span>
        <Link to="/" className="appfoot-sig appfoot-sig--link">
          ← Back to landing
        </Link>
      </div>
    </footer>
  );
}
