/*
 * View 1 — Submit / Withdraw. The hero brand moment.
 *
 * The "N withdrawals → 1 proof → 1 tx" aggregation viz is the centerpiece.
 * Connect Freighter → the judge's G-address becomes the withdrawal recipient →
 * "Submit withdrawal" POSTs a fresh demo intent to the sequencer → request_id is
 * instant. Honest copy: counterparties are unlinkable, amounts in the clear; the
 * operator-sees-mapping trust boundary is stated, not hidden.
 */

import { useState } from "react";
import { AggregationViz } from "../components/AggregationViz";
import { LivePulse } from "../components/LivePulse";
import { DataRow } from "../components/DataRow";
import { submitWithdrawal, SequencerError } from "../lib/api";
import type { SequencerConfig } from "../lib/api";
import { buildDemoIntent, DEMO_AMOUNT } from "../lib/note";
import { stroopsToXlm, truncateMiddle } from "../lib/format";

interface Props {
  config: SequencerConfig;
  address: string | null;
  connecting: boolean;
  onConnect: () => void;
  onSubmitted: (requestId: string) => void;
}

export function SubmitView({
  config,
  address,
  connecting,
  onConnect,
  onSubmitted,
}: Props) {
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function onSubmit() {
    if (!address) return;
    setError(null);
    setSubmitting(true);
    try {
      const body = buildDemoIntent(address);
      const { request_id } = await submitWithdrawal(body);
      onSubmitted(request_id);
    } catch (e) {
      const msg =
        e instanceof SequencerError || e instanceof Error
          ? e.message
          : "Submit failed.";
      setError(msg);
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <>
      {/* ── Hero (inverted black beat) ─────────────────────────────────── */}
      <section className="hero invert" aria-labelledby="hero-title">
        <div className="container hero-grid">
          <div className="hero-copy">
            <LivePulse
              tone="settled"
              label={`testnet · live — ${config.n_target} withdrawals → 1 verification`}
            />
            <h1 id="hero-title" className="hero-title">
              {config.n_target} private withdrawals,{" "}
              <span className="hero-em">settled in one transaction.</span>
            </h1>
            <p className="hero-lead">
              A privacy-pool rollup that aggregates {config.n_target} off-chain
              withdrawals into a single RISC&nbsp;Zero proof, verified on-chain in
              one Soroban transaction. Connect a testnet wallet and run it
              yourself — the result is a real settle tx, not a rendered demo.
            </p>
            <div className="hero-cta">
              {!address ? (
                <button
                  type="button"
                  className="btn btn--primary-invert"
                  onClick={onConnect}
                  disabled={connecting}
                >
                  {connecting ? "Connecting…" : "Connect testnet wallet"}
                </button>
              ) : (
                <button
                  type="button"
                  className="btn btn--primary-invert"
                  onClick={onSubmit}
                  disabled={submitting}
                >
                  {submitting ? "Submitting…" : "Submit withdrawal"}
                </button>
              )}
              <a className="btn btn--ghost-invert" href="#how">
                How it works
              </a>
            </div>
            {error && (
              <p className="form-error" role="alert">
                {error}
              </p>
            )}
          </div>

          <div className="hero-viz">
            <AggregationViz n={config.n_target} active={submitting} />
          </div>
        </div>
      </section>

      {/* ── Recipient / submit panel ───────────────────────────────────── */}
      <section className="container section" aria-labelledby="recipient-title" id="how">
        <div className="split">
          <div className="split-lead">
            <h2 id="recipient-title">Your wallet is the recipient</h2>
            <p className="prose">
              The operator custodies a demo note for you. When you submit, your
              connected address is copied into the withdrawal&apos;s recipient
              field — so the testnet funds land in your wallet once the batch
              settles on-chain.
            </p>
            <p className="prose prose--muted">
              Counterparties are <strong>unlinkable</strong>: on-chain, the
              settle breaks the deposit↔withdrawal link. Amounts are in the
              clear, not hidden. The operator running this sequencer sees the
              note↔recipient mapping — that trust boundary is by design, and
              we state it plainly.
            </p>
          </div>

          <div className="panel">
            <div className="panel-head">
              <h3>Withdrawal</h3>
              {address ? (
                <LivePulse tone="settled" label="wallet connected" />
              ) : (
                <span className="panel-flag">not connected</span>
              )}
            </div>

            {address ? (
              <DataRow
                label="Recipient"
                value={address}
                truncate
                copyable
                href={`https://stellar.expert/explorer/${config.network}/account/${address}`}
              />
            ) : (
              <div className="datarow datarow--empty">
                <span className="datarow-label">Recipient</span>
                <span className="datarow-value datarow-value--muted">
                  Connect a wallet to set the recipient
                </span>
              </div>
            )}
            <DataRow
              label="Amount"
              value={`${stroopsToXlm(DEMO_AMOUNT)} XLM`}
              plain
            />
            <DataRow label="Network" value={config.network} plain />
            <DataRow
              label="Rollup"
              value={config.rollup_id || "—"}
              truncate={Boolean(config.rollup_id)}
              copyable={Boolean(config.rollup_id)}
              href={
                config.rollup_id
                  ? `https://stellar.expert/explorer/${config.network}/contract/${config.rollup_id}`
                  : undefined
              }
            />
            <DataRow
              label="Verifier"
              value={truncateMiddle(config.verifier_id)}
              plain
            />

            <div className="panel-foot">
              {!address ? (
                <button
                  type="button"
                  className="btn btn--primary"
                  onClick={onConnect}
                  disabled={connecting}
                >
                  {connecting ? "Connecting…" : "Connect testnet wallet"}
                </button>
              ) : (
                <button
                  type="button"
                  className="btn btn--primary"
                  onClick={onSubmit}
                  disabled={submitting}
                >
                  {submitting ? "Submitting…" : "Submit withdrawal"}
                </button>
              )}
              <p className="panel-note">
                Submitting returns a request id instantly. Proving runs
                asynchronously — you&apos;ll watch it settle.
              </p>
            </div>
            {error && (
              <p className="form-error" role="alert">
                {error}
              </p>
            )}
          </div>
        </div>
      </section>
    </>
  );
}
