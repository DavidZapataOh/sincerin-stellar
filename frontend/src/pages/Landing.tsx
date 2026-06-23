/*
 * The LANDING surface (`/`). The brand page — editorial, B&W, multi-section,
 * confident. Speaks to the people who would USE Sincerin for private payments,
 * then hands off to the functional demo at /demo.
 *
 * Plain, benefit-driven copy: no instruction counts, no benchmark axes, no
 * Groth16/BN254/RISC Zero jargon as body copy — those live in the docs/demo.
 * The only number-shaped artifact here is a single confident "real settle on
 * testnet" link. Honest constraint held throughout: unlinkable, never "hidden
 * amounts".
 *
 * Sections: hero · why Sincerin · how it works · privacy that scales · closing.
 */

import { Link } from "../lib/router";
import { AggregationViz } from "../components/AggregationViz";
import { LivePulse } from "../components/LivePulse";
import { Reveal } from "../components/Reveal";
import { EVIDENCE, txUrl } from "../lib/evidence";

const N = 8;

export function Landing() {
  return (
    <main id="main" className="land">
      <Hero />
      <Why />
      <HowItWorks />
      <Scales />
      <Closing />
    </main>
  );
}

/* ── 1 · Hero (inverted black beat) ─────────────────────────────────────── */
function Hero() {
  return (
    <section className="land-hero invert" aria-labelledby="land-hero-title">
      <div className="container land-hero-grid">
        <div className="land-hero-copy">
          <LivePulse tone="settled" label="Confidential payments rollup · Stellar testnet" />
          <h1 id="land-hero-title" className="land-hero-title">
            Many private payments.{" "}
            <span className="land-hero-em">One transaction.</span>
          </h1>
          <p className="land-hero-lead">
            Sincerin is a confidential payments rollup on Stellar. It bundles
            private payments into a single proof and settles them all at once —
            so paying privately finally scales.
          </p>
          <div className="land-hero-cta">
            <Link to="/demo" className="btn btn--primary-invert btn--lg">
              Launch demo
              <ArrowRight />
            </Link>
            <a
              className="btn btn--ghost-invert btn--lg"
              href={txUrl(EVIDENCE.settleTx)}
              target="_blank"
              rel="noreferrer noopener"
            >
              See a real settle on testnet
            </a>
          </div>
        </div>

        <div className="land-hero-viz" aria-hidden="false">
          <AggregationViz n={N} active />
        </div>
      </div>
    </section>
  );
}

/* ── 2 · Why Sincerin (3 benefit cards) ─────────────────────────────────── */
function Why() {
  return (
    <section className="container section land-why" aria-labelledby="why-title">
      <Reveal className="land-why-head">
        <p className="land-kicker">Why Sincerin</p>
        <h2 id="why-title">Private payments that actually scale.</h2>
      </Reveal>

      <div className="land-why-grid">
        <Reveal className="benefit" delay={60}>
          <h3>Many payments, one transaction</h3>
          <p>
            Sincerin settles a whole batch of private payments together on
            Stellar — dozens at the cost of one.
          </p>
        </Reveal>
        <Reveal className="benefit" delay={120}>
          <h3>Unlinkable</h3>
          <p>
            Senders and recipients can&apos;t be tied together. Who paid whom
            stays private.
          </p>
        </Reveal>
        <Reveal className="benefit" delay={180}>
          <h3>Real, final, on Stellar</h3>
          <p>
            Settled directly on Stellar — no bridges, no wrapped tokens, no
            middlemen.
          </p>
        </Reveal>
      </div>
    </section>
  );
}

/* ── 3 · How it works (3 plain steps) ───────────────────────────────────── */
function HowItWorks() {
  const steps = [
    { title: "Queue payments", text: "Line up the private payments you want to make." },
    {
      title: "Prove them once",
      text: "Sincerin bundles the whole batch into a single proof.",
    },
    {
      title: "Settle together",
      text: "They all land on Stellar in one transaction — unlinkable.",
    },
  ];
  return (
    <section className="container section land-steps" aria-labelledby="steps-title">
      <Reveal className="land-steps-head">
        <p className="land-kicker">How it works</p>
        <h2 id="steps-title">Many payments in. One transaction out.</h2>
      </Reveal>

      <ol className="land-steps-grid">
        {steps.map((s, i) => (
          <Reveal as="li" key={s.title} className="land-step" delay={60 + i * 60}>
            <span className="land-step-n mono" aria-hidden="true">
              {i + 1}
            </span>
            <h3>{s.title}</h3>
            <p>{s.text}</p>
          </Reveal>
        ))}
      </ol>
    </section>
  );
}

/* ── 4 · Privacy that scales ────────────────────────────────────────────── */
function Scales() {
  return (
    <section className="container section land-scales" aria-labelledby="scales-title">
      <Reveal className="land-scales-copy">
        <p className="land-kicker">One proof for the batch</p>
        <h2 id="scales-title">Why it stays cheap.</h2>
        <p className="prose">
          Paying privately on-chain is expensive one payment at a time. Sincerin
          proves a whole batch at once and verifies it a single time — so each
          payment costs a fraction, and private payments work at real volume.
        </p>
        <p className="land-scales-honest">
          Amounts settle in the open on Stellar — what stays private is who paid
          whom.
        </p>
      </Reveal>

      <Reveal className="land-scales-viz" delay={100}>
        <AggregationViz n={N} active />
      </Reveal>
    </section>
  );
}

/* ── 5 · Closing CTA (inverted black) ───────────────────────────────────── */
function Closing() {
  return (
    <section className="land-closing invert" aria-labelledby="closing-title">
      <div className="container land-closing-inner">
        <Reveal>
          <h2 id="closing-title" className="land-closing-title">
            See it settle. Right now.
          </h2>
          <p className="land-closing-lead">
            Run a real private withdrawal on Stellar testnet — no signup, no
            wait.
          </p>
          <div className="land-closing-cta">
            <Link to="/demo" className="btn btn--primary-invert btn--lg">
              Launch demo
              <ArrowRight />
            </Link>
          </div>
        </Reveal>
      </div>
    </section>
  );
}

/* ── icons ──────────────────────────────────────────────────────────────── */
function ArrowRight() {
  return (
    <svg viewBox="0 0 18 18" width="17" height="17" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M3 9 H14" />
      <path d="M10 5 L14 9 L10 13" />
    </svg>
  );
}
