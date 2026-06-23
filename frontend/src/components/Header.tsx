/*
 * App header — wordmark + live testnet pulse + wallet connect/address.
 * Sticky, hairline-ruled, monochrome.
 */

import { LivePulse } from "./LivePulse";
import { truncateMiddle } from "../lib/format";

interface Props {
  network: string;
  address: string | null;
  connecting: boolean;
  onConnect: () => void;
}

export function Header({ network, address, connecting, onConnect }: Props) {
  return (
    <>
      <a href="#main" className="skip-link">
        Skip to content
      </a>
      <header className="appbar">
        <div className="container appbar-inner">
          <a href="/" className="wordmark" aria-label="Sincerin — home">
            <span className="wordmark-mark" aria-hidden="true">
              <Glyph />
            </span>
            <span className="wordmark-text">Sincerin</span>
          </a>

          <div className="appbar-right">
            <span className="appbar-net">
              <LivePulse tone="settled" label={`${network} · connected`} />
            </span>
            {address ? (
              <span className="appbar-addr mono" title={address}>
                {truncateMiddle(address, 6, 5)}
              </span>
            ) : (
              <button
                type="button"
                className="btn btn--sm"
                onClick={onConnect}
                disabled={connecting}
              >
                {connecting ? "Connecting…" : "Connect wallet"}
              </button>
            )}
          </div>
        </div>
      </header>
    </>
  );
}

/* Wordmark glyph: N→1 aggregation, miniature. */
function Glyph() {
  return (
    <svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round">
      <line x1="3" y1="5" x2="11" y2="12" />
      <line x1="3" y1="12" x2="11" y2="12" />
      <line x1="3" y1="19" x2="11" y2="12" />
      <line x1="11" y1="12" x2="21" y2="12" />
      <circle cx="21" cy="12" r="1.6" fill="currentColor" stroke="none" />
    </svg>
  );
}
