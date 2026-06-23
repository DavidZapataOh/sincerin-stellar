/*
 * The LANDING surface (`/`). The brand page — editorial, B&W, long-scroll,
 * confident, with art direction that alternates white and inverted-black beats.
 * Speaks to the people who would USE Sincerin for private payments, then hands
 * off to the functional demo at /demo.
 *
 * Plain, benefit-driven copy: no instruction counts, no benchmark axes, no
 * Groth16/BN254/RISC Zero jargon AS body copy — those live in the docs/demo.
 * The only number-shaped artifacts are the signature "N → 1" beat and a single
 * confident "real settle on testnet" card. Honest constraint held throughout:
 * unlinkable, never "hidden amounts".
 *
 * Sections (top → bottom):
 *   1 Hero (inverted)            — Many private payments. One transaction.
 *   2 Ecosystem strip            — Runs on Stellar. Proofs by RISC Zero.
 *   3 Properties (live cards)    — many→one · unlinkable · settles on Stellar
 *   4 Signature showpiece        — the aggregation, big + animated
 *   5 Typographic beat (inverted)— N payments. 1 transaction.
 *   6 How it works               — Many payments in. One transaction out.
 *   7 Why it stays cheap         — the one-proof economics + honest line
 *   8 Real settle card           — a verifiable on-chain settlement
 *   9 Closing (inverted)         — See it settle. Right now.
 */

import { Link } from "../lib/router";
import { AggregationFlow } from "../components/AggregationFlow";
import { FlowField } from "../components/FlowField";
import { EcosystemStrip } from "../components/EcosystemStrip";
import { Properties } from "../components/Properties";
import { ProofCrystal } from "../components/ProofCrystal";
import { SettleCard } from "../components/SettleCard";
import { Counter } from "../components/Counter";
import { LivePulse } from "../components/LivePulse";
import { Reveal } from "../components/Reveal";
import { EVIDENCE, txUrl } from "../lib/evidence";

export function Landing() {
  return (
    <main id="main" className="land">
      <Hero />
      <Ecosystem />
      <Properties />
      <Showpiece />
      <TypeBeat />
      <HowItWorks />
      <Scales />
      <Settle />
      <Closing />
    </main>
  );
}

/* ── 1 · Hero (inverted black beat) ─────────────────────────────────────── */
function Hero() {
  return (
    <section className="land-hero invert" aria-labelledby="land-hero-title">
      <FlowField density={0.85} />
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

        <div className="land-hero-viz">
          <AggregationFlow n={7} variant="hero" />
        </div>
      </div>
    </section>
  );
}

/* ── 2 · Ecosystem strip ────────────────────────────────────────────────── */
function Ecosystem() {
  return (
    <section className="land-eco" aria-label="What Sincerin runs on">
      <div className="container">
        <Reveal>
          <EcosystemStrip />
        </Reveal>
      </div>
    </section>
  );
}

/* ── 4 · Signature showpiece — the aggregation, big + animated ──────────── */
function Showpiece() {
  return (
    <section className="land-showpiece section" aria-labelledby="showpiece-title">
      <div className="container land-showpiece-head">
        <Reveal>
          <p className="land-kicker">The aggregation</p>
          <h2 id="showpiece-title" className="land-showpiece-title">
            Many payments flow in. One settles on Stellar.
          </h2>
          <p className="land-showpiece-lead">
            Each private payment is its own stream. Sincerin bundles them into a
            single proof and lands them as one on-chain settlement — watch it
            converge.
          </p>
        </Reveal>
      </div>
      <Reveal className="land-showpiece-stage">
        <AggregationFlow n={7} variant="showpiece" />
      </Reveal>
    </section>
  );
}

/* ── 5 · Typographic beat (inverted) — N payments. 1 transaction. ───────── */
function TypeBeat() {
  return (
    <section className="land-beat invert" aria-labelledby="beat-title">
      <FlowField density={1.1} />
      <div className="container land-beat-inner">
        <Reveal className="land-beat-grid">
          <p className="land-beat-eq" aria-hidden="true">
            <span className="land-beat-term">
              <span className="land-beat-num">
                <Counter to={8} />
              </span>
              <span className="land-beat-word">payments</span>
            </span>
            <span className="land-beat-op">→</span>
            <span className="land-beat-term">
              <span className="land-beat-num land-beat-num--one">1</span>
              <span className="land-beat-word">transaction</span>
            </span>
          </p>
          <h2 id="beat-title" className="sr-only">
            Eight private payments, one transaction.
          </h2>
          <p className="land-beat-sub">
            That ratio is the whole point. The batch grows; the on-chain cost of
            settling it doesn’t.
          </p>
        </Reveal>
      </div>
    </section>
  );
}

/* ── 6 · How it works (3 plain steps) ───────────────────────────────────── */
function HowItWorks() {
  const steps = [
    { title: "Queue payments", text: "Line up the private payments you want to make." },
    { title: "Prove them once", text: "Sincerin bundles the whole batch into a single proof." },
    { title: "Settle together", text: "They all land on Stellar in one transaction — unlinkable." },
  ];
  return (
    <section className="container section land-steps" aria-labelledby="steps-title">
      <Reveal className="land-steps-head">
        <p className="land-kicker">How it works</p>
        <h2 id="steps-title">Many payments in. One transaction out.</h2>
      </Reveal>

      <ol className="land-steps-grid">
        {steps.map((s, i) => (
          <Reveal as="li" key={s.title} className="land-step" delay={60 + i * 70}>
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

/* ── 7 · Why it stays cheap — the rotating proof crystal (inverted) ──────── */
function Scales() {
  return (
    <section className="land-crystal invert section" aria-labelledby="scales-title">
      <div className="container land-crystal-grid">
        <Reveal className="land-crystal-copy">
          <p className="land-kicker land-kicker--invert">One proof for the batch</p>
          <h2 id="scales-title">Why it stays cheap.</h2>
          <p className="land-crystal-lead">
            The whole batch is proven once and verified a single time on Stellar —
            so the on-chain cost barely moves whether you settle eight payments or
            eighty.
          </p>
          <p className="land-crystal-honest">
            Amounts settle in the open on Stellar; what stays private is who paid
            whom.
          </p>
        </Reveal>

        <Reveal className="land-crystal-viz" delay={120}>
          <ProofCrystal n={8} />
        </Reveal>
      </div>
    </section>
  );
}

/* ── 8 · Real settle (the verifiable payoff) ────────────────────────────── */
function Settle() {
  return (
    <section className="container section land-settle" aria-labelledby="settle-title">
      <div className="land-settle-grid">
        <Reveal className="land-settle-copy">
          <p className="land-kicker">Real, not rendered</p>
          <h2 id="settle-title">Already live on Stellar.</h2>
          <p className="prose">
            This isn’t a mock-up. A real batch of eight private payments has
            already aggregated into a single proof and settled on Stellar
            testnet — verified on-chain, in one transaction. Open it on the
            explorer.
          </p>
        </Reveal>
        <Reveal className="land-settle-card" delay={120}>
          <SettleCard />
        </Reveal>
      </div>
    </section>
  );
}

/* ── 9 · Closing CTA (inverted black) ───────────────────────────────────── */
function Closing() {
  return (
    <section className="land-closing invert" aria-labelledby="closing-title">
      <FlowField density={0.7} />
      <div className="container">
        <Reveal className="land-closing-inner">
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
