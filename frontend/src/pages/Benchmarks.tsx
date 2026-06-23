/*
 * /benchmarks — the two-axis benchmark surface (s3/01). The judge's rigorous
 * view: the N≈12 crossover chart (the differentiator in one image), the off-chain
 * proving chart (dev CPU hours vs production GPU minutes), and the full table
 * with per-cell provenance. Every number comes from src/lib/benchmark.ts (the
 * source of truth, == docs/proving-times.md); the honest aggregate claim is the
 * gated AGGREGATE_CLAIM. Kept off the marketing landing on purpose.
 */

import { Header } from "../components/Header";
import { Footer } from "../components/Footer";
import { Reveal } from "../components/Reveal";
import { crossoverChartSVG, provingChartSVG } from "../lib/benchmarkChart";
import {
  ROWS,
  AGGREGATE_CLAIM,
  CROSSOVER_N_INT,
  BUDGET,
  formatProving,
  pctOfBudget,
} from "../lib/benchmark";
import { EVIDENCE, txUrl } from "../lib/evidence";

const commas = (n: number) => n.toString().replace(/\B(?=(\d{3})+(?!\d))/g, ",");

function Chip({ flag }: { flag: "measured" | "projected" }) {
  return <span className={`bm-chip bm-chip--${flag}`}>{flag}</span>;
}

export function Benchmarks() {
  return (
    <>
      <Header surface="landing" />
      <main id="main" className="bm">
        <section className="container section bm-hero">
          <Reveal>
            <p className="land-kicker">Benchmark · two axes</p>
            <h1 className="bm-title">
              Aggregation makes on-chain settlement feasible — not just cheaper.
            </h1>
            <p className="bm-lead">
              Off-chain, proving grows with the batch. On-chain, settling N withdrawals as
              one proof grows slowly and stays inside the budget — while settling them as N
              separate verifications crosses the one-transaction limit around N&nbsp;=&nbsp;
              {CROSSOVER_N_INT} and stops being possible at all.
            </p>
          </Reveal>
        </section>

        {/* ── the star: the N≈12 crossover ─────────────────────────────────── */}
        <section className="container section bm-section">
          <Reveal className="bm-section-head">
            <h2>On-chain cost vs batch size</h2>
            <p className="bm-section-sub">{AGGREGATE_CLAIM}</p>
          </Reveal>
          <Reveal className="bm-chart" delay={80}>
            <div
              className="bm-chart-svg"
              // self-generated, trusted SVG string (no user input)
              dangerouslySetInnerHTML={{ __html: crossoverChartSVG() }}
            />
          </Reveal>
        </section>

        {/* ── secondary: proving, dev vs production ─────────────────────────── */}
        <section className="container section bm-section">
          <Reveal className="bm-section-head">
            <h2>Off-chain proving — minutes in production</h2>
            <p className="bm-section-sub">
              The Mac figures are a development artifact (x86-on-ARM emulation for the STARK→Groth16
              wrap). On a single RTX&nbsp;3090 with native CUDA, the real N=8 proof took{" "}
              <strong>{formatProving(304)}</strong> — about 52× faster. Production proving is minutes,
              not hours.
            </p>
          </Reveal>
          <Reveal className="bm-chart" delay={80}>
            <div className="bm-chart-svg" dangerouslySetInnerHTML={{ __html: provingChartSVG() }} />
          </Reveal>
        </section>

        {/* ── the full table, provenance per cell ──────────────────────────── */}
        <section className="container section bm-section">
          <Reveal className="bm-section-head">
            <h2>The numbers</h2>
            <p className="bm-section-sub">
              Every cell is measured or projected — never mixed without a label. The four cycle
              counts are measured; CPU time is measured for N=2/8, GPU time for N=8; N=16/32 settle
              and timings are projected.
            </p>
          </Reveal>
          <Reveal className="bm-table-wrap" delay={80}>
            <table className="bm-table">
              <thead>
                <tr>
                  <th>N</th>
                  <th>depth</th>
                  <th>cycles</th>
                  <th>proving · CPU (dev)</th>
                  <th>proving · GPU (prod)</th>
                  <th>settle (aggregated)</th>
                  <th>baseline · N×35M</th>
                </tr>
              </thead>
              <tbody>
                {ROWS.map((r) => {
                  const feasible = r.baselineInsn <= BUDGET;
                  return (
                    <tr key={r.n}>
                      <td className="bm-n">{r.n}</td>
                      <td>{r.depth}</td>
                      <td className="bm-num">{commas(r.cycles)}</td>
                      <td>
                        {formatProving(r.provingCpuSeconds)} <Chip flag={r.provingCpuFlag} />
                        {r.provingCpuNote && <span className="bm-note">{r.provingCpuNote}</span>}
                      </td>
                      <td>
                        {r.provingGpuSeconds != null ? (
                          <>
                            {formatProving(r.provingGpuSeconds)}{" "}
                            <Chip flag={r.provingGpuFlag ?? "measured"} />
                          </>
                        ) : (
                          <span className="bm-dash">—</span>
                        )}
                      </td>
                      <td>
                        <span className="bm-num">{(r.settleAggInsn / 1e6).toFixed(1)}M</span>{" "}
                        <span className="bm-muted">({pctOfBudget(r.settleAggInsn).toFixed(1)}%)</span>{" "}
                        <Chip flag={r.settleFlag} />
                      </td>
                      <td className={feasible ? "" : "bm-infeasible"}>
                        <span className="bm-num">{(r.baselineInsn / 1e6).toFixed(0)}M</span>{" "}
                        {feasible ? (
                          <span className="bm-muted">fits</span>
                        ) : (
                          <span className="bm-infeasible-tag">won’t fit</span>
                        )}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </Reveal>
        </section>

        {/* ── method / honesty ─────────────────────────────────────────────── */}
        <section className="container section bm-section bm-method">
          <Reveal>
            <h2>Method</h2>
            <p className="bm-section-sub">
              Cycles from the executor (validated: N=8 matches the prover total exactly). Proving
              measured with <code>RISC0_DEV_MODE=0</code> (never dev-mode). The N=8 settle is real and
              on testnet —{" "}
              <a href={txUrl(EVIDENCE.settleTx)} target="_blank" rel="noreferrer noopener">
                tx {EVIDENCE.settleTx.slice(0, 10)}…
              </a>{" "}
              — 8 withdrawals in one transaction. Full provenance in{" "}
              <a
                href="https://github.com/DavidZapataOh/sincerin-stellar/blob/main/docs/proving-times.md"
                target="_blank"
                rel="noreferrer noopener"
              >
                docs/proving-times.md
              </a>
              .
            </p>
          </Reveal>
        </section>
      </main>
      <Footer surface="landing" />
    </>
  );
}
