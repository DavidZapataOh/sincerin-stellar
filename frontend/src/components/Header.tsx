/*
 * App header — route-aware, one wordmark, monochrome, hairline-ruled, sticky.
 *
 *  - surface="landing": a marketing bar — wordmark + a "Launch demo" CTA.
 *  - surface="demo":    the live bar — wordmark (→ home) + testnet pulse +
 *                       wallet connect/address. Carries the loaded /config state.
 */

import { LivePulse } from "./LivePulse";
import { Link } from "../lib/router";
import { truncateMiddle } from "../lib/format";

type LandingProps = { surface: "landing" };
type DemoProps = {
  surface: "demo";
  network: string;
  address: string | null;
  connecting: boolean;
  onConnect: () => void;
};
type Props = LandingProps | DemoProps;

export function Header(props: Props) {
  return (
    <>
      <a href="#main" className="skip-link">
        Skip to content
      </a>
      <header className="appbar">
        <div className="container appbar-inner">
          <Link to="/" className="wordmark" aria-label="Sincerin — home">
            <span className="wordmark-mark" aria-hidden="true">
              <Glyph />
            </span>
            <span className="wordmark-text">Sincerin</span>
          </Link>

          {props.surface === "landing" ? (
            <nav className="appbar-right" aria-label="Primary">
              <Link to="/demo" className="btn btn--sm btn--solid-sm">
                Launch demo
              </Link>
            </nav>
          ) : (
            <div className="appbar-right">
              <Link to="/" className="appbar-back">
                <BackArrow />
                <span>Landing</span>
              </Link>
              <span className="appbar-net">
                <LivePulse tone="settled" label={`${props.network} · connected`} />
              </span>
              {props.address ? (
                <span className="appbar-addr mono" title={props.address}>
                  {truncateMiddle(props.address, 6, 5)}
                </span>
              ) : (
                <button
                  type="button"
                  className="btn btn--sm"
                  onClick={props.onConnect}
                  disabled={props.connecting}
                >
                  {props.connecting ? "Connecting…" : "Connect wallet"}
                </button>
              )}
            </div>
          )}
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

function BackArrow() {
  return (
    <svg viewBox="0 0 16 16" width="14" height="14" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M10 3 L5 8 L10 13" />
    </svg>
  );
}
