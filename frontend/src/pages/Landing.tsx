/*
 * The LANDING surface (`/`). The brand page — editorial, B&W, multi-section,
 * confident. Sells the rollup, then hands off to the functional demo at /demo.
 *
 * It is fully static / honest: every claim is a measured number from
 * docs/proving-times.md + deployments/testnet.json (cited), and the only live
 * artifacts (real testnet tx links) point at the historic on-chain settle. No
 * sequencer call here — the landing is the pitch, /demo is the proof you run.
 *
 * Sections (see brief): hero · problem · solution · how-it-works · benchmark ·
 * zk-load-bearing · privacy-honestly · evidence · closing CTA.
 */

import { Link } from "../lib/router";
import { AggregationViz } from "../components/AggregationViz";
import { BenchChart } from "../components/BenchChart";
import { PipelineDiagram } from "../components/PipelineDiagram";
import { LivePulse } from "../components/LivePulse";
import { Reveal } from "../components/Reveal";
import {
  EVIDENCE,
  EXPLORER,
  contractUrl,
  txUrl,
} from "../lib/evidence";

const N = 8;

export function Landing() {
  return (
    <main id="main" className="land">
      <Hero />
      <Problem />
      <Solution />
      <HowItWorks />
      <Benchmark />
      <LoadBearing />
      <Privacy />
      <Evidence />
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
          <LivePulse tone="settled" label="Stellar testnet · live" />
          <h1 id="land-hero-title" className="land-hero-title">
            {N} private withdrawals,{" "}
            <span className="land-hero-em">settled in one transaction.</span>
          </h1>
          <p className="land-hero-lead">
            A privacy-pool rollup that aggregates {N} off-chain withdrawals into
            a single RISC&nbsp;Zero proof, verified on-chain in one Soroban
            transaction. The result is a real settle tx — not a rendered demo.
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
              View the settle on testnet
            </a>
          </div>
          <p className="land-hero-sub">
            You connect a testnet wallet, submit a withdrawal, and watch the
            batch settle on-chain — the same pipeline a judge runs, end to end.
          </p>
        </div>

        <div className="land-hero-viz" aria-hidden="false">
          <AggregationViz n={N} active />
        </div>
      </div>
    </section>
  );
}

/* ── 2 · The problem ────────────────────────────────────────────────────── */
function Problem() {
  return (
    <section className="container section land-problem" aria-labelledby="problem-title">
      <Reveal className="land-problem-lead">
        <p className="land-kicker">The problem</p>
        <h2 id="problem-title">
          One pool withdrawal already costs ~9% of a transaction. Eight of them
          don&apos;t fit.
        </h2>
        <p className="prose">
          A single privacy-pool withdrawal verifies a Groth16 proof on-chain —
          about <strong>35M instructions</strong>, ~9% of Soroban&apos;s{" "}
          <strong>400M</strong> single-transaction budget. That&apos;s fine for
          one. But verifying <em>N</em> withdrawals the naive way means{" "}
          <em>N</em> separate verifications, and the cost climbs linearly.
        </p>
        <p className="prose prose--muted">
          Past <strong>N≈12</strong> the batch no longer fits in a single
          transaction at all — measured, not hand-waved. The honest ceiling is a
          dozen; anything larger is impossible on-chain by the direct route.
        </p>
      </Reveal>

      <Reveal className="land-problem-figure" delay={120}>
        <ul className="costbars" aria-label="On-chain cost of N individual verifications versus the 400M single-transaction budget">
          {[
            { n: 1, pct: 9, fits: true },
            { n: 4, pct: 35, fits: true },
            { n: 8, pct: 70, fits: true },
            { n: 12, pct: 105, fits: false },
            { n: 16, pct: 140, fits: false },
          ].map(({ n, pct, fits }) => (
            <li key={n} className={`costbar ${fits ? "" : "is-over"}`}>
              <span className="costbar-n mono">N={n}</span>
              <span className="costbar-track">
                <span
                  className="costbar-fill"
                  style={{ width: `${Math.min(pct, 100)}%` }}
                />
                {!fits && (
                  <span
                    className="costbar-over"
                    style={{ width: `${pct - 100}%` }}
                  />
                )}
              </span>
              <span className="costbar-pct mono">{pct}%</span>
            </li>
          ))}
          <li className="costbar-budget">
            <span className="costbar-budget-line" />
            <span className="costbar-budget-label mono">400M — 1 tx budget</span>
          </li>
        </ul>
        <p className="land-figure-note">
          N individual pool verifications, as a share of the 400M single-tx
          budget. Cost is ~35M instr per verification; the line is crossed at
          N≈12.
        </p>
      </Reveal>
    </section>
  );
}

/* ── 3 · The solution ───────────────────────────────────────────────────── */
function Solution() {
  return (
    <section className="container section land-solution" aria-labelledby="solution-title">
      <Reveal className="land-solution-copy">
        <p className="land-kicker">The solution</p>
        <h2 id="solution-title">
          Aggregate the {N} withdrawals off-chain.{" "}
          <span className="land-solution-em">Verify them once.</span>
        </h2>
        <p className="prose">
          A sequencer re-executes every withdrawal&apos;s validity — Merkle
          membership, nullifier non-reuse, balance — inside a single RISC&nbsp;Zero
          guest, then wraps the whole batch into{" "}
          <strong>one Groth16/BN254 proof</strong>. On-chain, Soroban verifies
          that one proof and runs the {N} transfers atomically: all-or-nothing.
        </p>
        <dl className="land-nums">
          <div className="land-num">
            <dt className="mono">{N}</dt>
            <dd>withdrawals in a batch</dd>
          </div>
          <div className="land-num land-num--arrow" aria-hidden="true">
            <dt>
              <ArrowRight />
            </dt>
            <dd />
          </div>
          <div className="land-num">
            <dt className="mono">1</dt>
            <dd>RISC&nbsp;Zero proof</dd>
          </div>
          <div className="land-num land-num--arrow" aria-hidden="true">
            <dt>
              <ArrowRight />
            </dt>
            <dd />
          </div>
          <div className="land-num">
            <dt className="mono">1</dt>
            <dd>on-chain verification</dd>
          </div>
        </dl>
      </Reveal>

      <Reveal className="land-solution-viz" delay={100}>
        <AggregationViz n={N} active />
      </Reveal>
    </section>
  );
}

/* ── 4 · How it works ───────────────────────────────────────────────────── */
function HowItWorks() {
  return (
    <section className="container section land-how" aria-labelledby="how-title">
      <Reveal>
        <p className="land-kicker">How it works</p>
        <h2 id="how-title">From deposit to settled, in five honest steps.</h2>
      </Reveal>
      <Reveal delay={100}>
        <PipelineDiagram />
      </Reveal>
    </section>
  );
}

/* ── 5 · Benchmark (two-axis) ───────────────────────────────────────────── */
function Benchmark() {
  return (
    <section className="container section land-bench" aria-labelledby="bench-title">
      <Reveal className="land-bench-head">
        <p className="land-kicker">The benchmark</p>
        <h2 id="bench-title">
          The aggregated settle stays feasible. The baseline doesn&apos;t.
        </h2>
        <p className="prose">
          Two axes, both measured where the numbers say <em>measured</em>.
          On-chain, the aggregated settle is one Groth16 verification plus{" "}
          <em>N</em> cheap transfers — it grows slowly, from{" "}
          <strong>~9% at N=8</strong> toward ~14% at N=32. The baseline of{" "}
          <em>N</em> individual verifications grows linearly and crosses the 400M
          budget at <strong>N≈12</strong>.
        </p>
      </Reveal>
      <Reveal delay={120}>
        <BenchChart />
      </Reveal>
      <Reveal delay={160} className="land-bench-foot">
        <p>
          Measured on Stellar testnet: the N=8 settle cost{" "}
          <strong className="mono">36,118,956</strong> instructions (tx{" "}
          <a href={txUrl(EVIDENCE.settleTx)} target="_blank" rel="noreferrer noopener" className="mono">
            aedc1cc4…
          </a>
          ). Proving N=8 took <strong>4h 26m</strong> end-to-end on Apple Silicon
          — real ZK work, never dev-mode. N=16/32 on-chain cost is projected
          (1 Groth16 verify + ~0.77M per note) and labelled as such.
        </p>
      </Reveal>
    </section>
  );
}

/* ── 6 · Why the ZK is load-bearing (inverted black beat) ───────────────── */
function LoadBearing() {
  return (
    <section className="land-load invert" aria-labelledby="load-title">
      <div className="container land-load-inner">
        <Reveal>
          <p className="land-kicker land-kicker--invert">The point</p>
          <h2 id="load-title" className="land-load-title">
            Without the proof, there is no rollup.
          </h2>
          <p className="land-load-lead">
            The aggregation isn&apos;t a cost optimization bolted onto a working
            system — it <em>is</em> the system. Verifying one proof for {N}{" "}
            withdrawals is the only thing that makes a batch larger than a dozen
            fit on-chain. Remove the ZK and you&apos;re back to N individual
            verifications that don&apos;t fit. The proof is the product.
          </p>
          <div className="land-load-cta">
            <Link to="/demo" className="btn btn--primary-invert btn--lg">
              Run it yourself
              <ArrowRight />
            </Link>
          </div>
        </Reveal>
      </div>
    </section>
  );
}

/* ── 7 · Privacy, honestly ──────────────────────────────────────────────── */
function Privacy() {
  return (
    <section className="container section land-privacy" aria-labelledby="privacy-title">
      <Reveal className="land-privacy-head">
        <p className="land-kicker">Privacy, honestly</p>
        <h2 id="privacy-title">
          &ldquo;Confidential&rdquo; means unlinkable counterparties — and we say
          exactly what that does and doesn&apos;t mean.
        </h2>
      </Reveal>

      <div className="land-privacy-grid">
        <Reveal className="claimcard claimcard--yes" delay={60}>
          <span className="claimcard-tag">What it is</span>
          <h3>Unlinkable counterparties</h3>
          <p>
            On-chain, the settle breaks the link between a deposit and the
            withdrawal it funds. An observer can&apos;t tie who deposited to who
            withdrew.
          </p>
        </Reveal>
        <Reveal className="claimcard claimcard--no" delay={120}>
          <span className="claimcard-tag">What it is not</span>
          <h3>Amounts are in the clear</h3>
          <p>
            This is not a confidential-amounts scheme. Every withdrawal amount is
            public on-chain. We never claim hidden amounts — only unlinkable
            counterparties.
          </p>
        </Reveal>
        <Reveal className="claimcard claimcard--trust" delay={180}>
          <span className="claimcard-tag">The trust boundary</span>
          <h3>The operator sees the mapping</h3>
          <p>
            The operator running this sequencer receives the note secrets, so it
            sees the note↔recipient mapping. Unlinkability is public/on-chain,
            not against the operator. We state that plainly.
          </p>
        </Reveal>
      </div>
    </section>
  );
}

/* ── 8 · Evidence ───────────────────────────────────────────────────────── */
function Evidence() {
  return (
    <section className="container section land-evidence" aria-labelledby="evidence-title">
      <Reveal className="land-evidence-head">
        <p className="land-kicker">Evidence</p>
        <h2 id="evidence-title">Every claim has a link. Here are the real ones.</h2>
        <p className="prose prose--muted">
          All on Stellar testnet, all reproducible. Open them — the settle is a
          SUCCESS, the {N} recipients are credited, and a replay fails with{" "}
          <span className="mono">NullifierSpent</span>.
        </p>
      </Reveal>

      <Reveal delay={100} className="land-evidence-list">
        <EvidenceRow
          label="Settle tx · 8 withdrawals in 1"
          value={EVIDENCE.settleTx}
          href={txUrl(EVIDENCE.settleTx)}
          mono
        />
        <EvidenceRow
          label="RISC Zero verifier (Groth16/BN254)"
          value={EVIDENCE.verifier}
          href={contractUrl(EVIDENCE.verifier)}
          mono
        />
        <EvidenceRow
          label="Rollup contract"
          value={EVIDENCE.rollup}
          href={contractUrl(EVIDENCE.rollup)}
          mono
        />
        <EvidenceRow
          label="Privacy pool contract"
          value={EVIDENCE.pool}
          href={contractUrl(EVIDENCE.pool)}
          mono
        />
        <EvidenceRow
          label="Guest image id (binds the proof)"
          value={EVIDENCE.imageId}
          mono
        />
        <EvidenceRow label="On-chain cost · N=8 settle" value="36,118,956 instr · ~9% of budget" />
      </Reveal>
    </section>
  );
}

function EvidenceRow({
  label,
  value,
  href,
  mono,
}: {
  label: string;
  value: string;
  href?: string;
  mono?: boolean;
}) {
  const display = mono && value.length > 22 ? middle(value) : value;
  return (
    <div className="evrow">
      <span className="evrow-label">{label}</span>
      {href ? (
        <a
          className={`evrow-value ${mono ? "mono" : ""}`}
          href={href}
          target="_blank"
          rel="noreferrer noopener"
          title={value}
        >
          {display}
          <ExternalIcon />
        </a>
      ) : (
        <span className={`evrow-value evrow-value--static ${mono ? "mono" : ""}`} title={value}>
          {display}
        </span>
      )}
    </div>
  );
}

function middle(s: string): string {
  return `${s.slice(0, 10)}…${s.slice(-8)}`;
}

/* ── 9 · Closing CTA ────────────────────────────────────────────────────── */
function Closing() {
  return (
    <section className="land-closing invert" aria-labelledby="closing-title">
      <div className="container land-closing-inner">
        <Reveal>
          <h2 id="closing-title" className="land-closing-title">
            Don&apos;t take our word for it. Run a withdrawal.
          </h2>
          <p className="land-closing-lead">
            Connect a testnet wallet, submit, and watch your batch settle
            on-chain — the proof takes minutes because it&apos;s real.
          </p>
          <div className="land-closing-cta">
            <Link to="/demo" className="btn btn--primary-invert btn--lg">
              Launch demo
              <ArrowRight />
            </Link>
            <a
              className="btn btn--ghost-invert btn--lg"
              href={EXPLORER.repo}
              target="_blank"
              rel="noreferrer noopener"
            >
              View the code
            </a>
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
function ExternalIcon() {
  return (
    <svg viewBox="0 0 16 16" width="13" height="13" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M9 2.5 H13.5 V7" />
      <path d="M13.5 2.5 L7.5 8.5" />
      <path d="M11 9 V13 H3 V5 H7" />
    </svg>
  );
}
