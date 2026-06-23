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
import { EVIDENCE, EXPLORER, txUrl } from "../lib/evidence";

/** Off-site links. `soon` items aren't live yet — shown honestly, not faked. */
const SOCIAL_X = "https://x.com/sincerinZK";

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
        <div className="container appfoot-top">
          <div className="appfoot-brand">
            <Link to="/" className="appfoot-word">
              Sincerin
            </Link>
            <p className="appfoot-tag">
              The confidential payments rollup on Stellar — many private payments
              settle for the cost of about one.
            </p>
            <span className="appfoot-status">
              <span className="appfoot-dot" aria-hidden="true" />
              Live on Stellar testnet
            </span>
          </div>

          <nav className="appfoot-cols" aria-label="Footer">
            <div className="appfoot-nav">
              <p className="appfoot-nav-head">Product</p>
              <Link to="/demo">Launch demo</Link>
              <a href={txUrl(EVIDENCE.settleTx)} target="_blank" rel="noreferrer noopener">
                See a real settle
              </a>
              <a href={EXPLORER.repo} target="_blank" rel="noreferrer noopener">
                GitHub
              </a>
            </div>
            <div className="appfoot-nav">
              <p className="appfoot-nav-head">Learn</p>
              <span className="appfoot-soon">
                Documentation <em>soon</em>
              </span>
              <span className="appfoot-soon">
                Whitepaper <em>soon</em>
              </span>
              <span className="appfoot-soon">
                Blog <em>soon</em>
              </span>
            </div>
            <div className="appfoot-nav">
              <p className="appfoot-nav-head">Follow</p>
              <a href={SOCIAL_X} target="_blank" rel="noreferrer noopener">
                X / @sincerinZK
              </a>
            </div>
          </nav>
        </div>

        <div className="container appfoot-base">
          <span className="appfoot-wip">WIP · not audited · testnet only · open source</span>
          <span className="appfoot-copy">© 2026 Sincerin</span>
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
