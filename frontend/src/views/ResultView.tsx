/*
 * View 3 — Result + Recent. The settled payoff beat (inverted black section).
 *
 * On `settled`: the big real tx hash (mono) + explorer link (built from
 * /config explorer_base + tx), "N withdrawals in 1 verification", and
 * "funds credited to your wallet". Below: the Recent-batches panel and the
 * tasteful inline two-axis benchmark (measured numbers). Everything real:
 * the hash comes from the sequencer's on-chain settle, never fabricated.
 */

import { AggregationViz } from "../components/AggregationViz";
import { DataRow } from "../components/DataRow";
import { LivePulse } from "../components/LivePulse";
import { RecentBatches } from "../components/RecentBatches";
import { BenchChart } from "../components/BenchChart";
import type { RecentBatch, SequencerConfig } from "../lib/api";
import { stroopsToXlm } from "../lib/format";
import { DEMO_AMOUNT } from "../lib/note";

interface Props {
  config: SequencerConfig;
  txHash: string;
  recipient: string | null;
  recent: RecentBatch[];
  onAgain: () => void;
}

export function ResultView({
  config,
  txHash,
  recipient,
  recent,
  onAgain,
}: Props) {
  const explorerUrl = `${config.explorer_base}${txHash}`;
  const n = config.n_target;

  return (
    <>
      {/* ── Settled (inverted black beat — the "real" moment) ───────────── */}
      <section className="result invert" aria-labelledby="result-title">
        <div className="container result-grid">
          <div className="result-copy">
            <LivePulse tone="settled" label="settled on-chain · testnet" />
            <h2 id="result-title" className="result-title">
              <span className="result-count">{n}</span> withdrawals,{" "}
              <span className="result-em">one verification.</span>
            </h2>
            <p className="result-lead">
              The batch is settled on Stellar testnet — one Groth16 proof, one
              Soroban transaction. The testnet funds are credited to your wallet.
            </p>

            <div className="result-data">
              <DataRow
                label="Settle tx"
                value={txHash}
                truncate
                copyable
                href={explorerUrl}
              />
              {recipient && (
                <DataRow
                  label="Credited to"
                  value={recipient}
                  truncate
                  copyable
                  href={`https://stellar.expert/explorer/${config.network}/account/${recipient}`}
                />
              )}
              <DataRow
                label="Your amount"
                value={`${stroopsToXlm(DEMO_AMOUNT)} XLM`}
                plain
              />
              <DataRow label="On-chain cost" value="36.1M instr · ~9% of 1-tx budget" plain />
            </div>

            <div className="result-cta">
              <a
                className="btn btn--primary-invert"
                href={explorerUrl}
                target="_blank"
                rel="noreferrer noopener"
              >
                View on explorer
              </a>
              <button type="button" className="btn btn--ghost-invert" onClick={onAgain}>
                Run another withdrawal
              </button>
            </div>
          </div>

          <div className="result-viz">
            <AggregationViz n={n} settled />
          </div>
        </div>
      </section>

      {/* ── Recent + benchmark ──────────────────────────────────────────── */}
      <section className="container section result-below">
        <div className="result-below-grid">
          <RecentBatches config={config} batches={recent} />

          <div className="benchwrap">
            <div className="benchwrap-head">
              <h3>Why aggregate</h3>
              <p className="prose prose--muted">
                One verification, settled for all {n}. The aggregated cost stays
                roughly flat in N near 9% of the single-transaction budget;{" "}
                {n} individual pool verifications would grow linearly and exceed
                the 400M budget past N≈12 — so large batches are impossible{" "}
                <em>without</em> aggregation.
              </p>
            </div>
            <BenchChart />
            <p className="bench-foot">
              Measured: N=8 settle = 36,118,956 instructions (tx aedc1cc4…).
              N=16/32 projected (1 Groth16 verify + ~0.77M per note). Full
              two-axis benchmark in the project report.
            </p>
          </div>
        </div>
      </section>
    </>
  );
}
