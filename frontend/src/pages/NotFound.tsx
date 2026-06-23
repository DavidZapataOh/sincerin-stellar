/* 404 — keep it on-brand and honest; route the visitor back to the two surfaces. */

import { Link } from "../lib/router";

export function NotFound() {
  return (
    <main id="main" className="container section center-screen">
      <div className="booterror">
        <p className="land-kicker">404</p>
        <h2>No such page.</h2>
        <p className="prose prose--muted">
          There are two surfaces here: the landing, and the demo you run against
          the live sequencer.
        </p>
        <div className="booterror-actions">
          <Link to="/" className="btn btn--primary">
            Back to landing
          </Link>
          <Link to="/demo" className="btn btn--ghost">
            Launch demo
          </Link>
        </div>
      </div>
    </main>
  );
}
