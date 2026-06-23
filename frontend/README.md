# Sincerin — demo frontend (s3/02)

The face of the deployed **sequencer**. A judge connects a testnet wallet,
submits a withdrawal, and watches a batch of N withdrawals aggregate into **one
RISC Zero proof** that settles on Stellar testnet in **one Soroban transaction**.

The UI talks **only** to the sequencer HTTP API + the wallet. It never calls the
rollup contract, holds no proof artifacts, and hardcodes no contract address
(network / rollup id / verifier id / `n_target` all come from `GET /config`).

## Stack

Vite + React + TypeScript · `@creit.tech/stellar-wallets-kit` (Freighter,
testnet) · `@stellar/stellar-base` (StrKey decode of the recipient). Fonts:
Clash Display + General Sans (Fontshare), Martian Mono (Google).

## The three views

1. **Submit / Withdraw** (`views/SubmitView.tsx`) — the hero with the monochrome
   `AggregationViz` (N → 1 proof → 1 tx). Connect → the judge's G-address becomes
   the recipient → `POST /submit` → instant `request_id`.
2. **Status** (`views/StatusView.tsx`) — polls `GET /status/:id`:
   `pending → batched → starting prover → proving → settled`, with the live
   heartbeat pulse during proving and the batch filling toward `n_target`.
   `failed` → legible reason + Retry.
3. **Result + Recent** (`views/ResultView.tsx`) — the real settle tx hash +
   explorer link, "N withdrawals in 1 verification", funds credited, the
   `RecentBatches` panel (`GET /recent_batches`), and the inline two-axis
   `BenchChart` (measured on-chain numbers).

## Run

```bash
# 1. Start the sequencer (s3/02 gate binary — FixtureProver, no GPU):
#    needs a deployed rollup id to settle against.
FIXTURE_PROVE_DELAY=20 ROLLUP_ID=<C…> SIGNER=<key> NETWORK=testnet BIND=127.0.0.1:8787 \
  cargo run -p sequencer --features test-fixture --bin seq_demo_http

# 2. Point the frontend at it and run:
echo 'VITE_SEQUENCER_URL=http://localhost:8787' > .env.local
npm install
npm run dev          # http://localhost:5173
npm run build        # production build (must pass)
```

`VITE_SEQUENCER_URL` defaults to `http://localhost:8787` (the gate binary's
default bind). A `?previewAddress=G…` query param pre-fills the recipient for
headless screenshots/CI exactly as a connected wallet would — it only sets the
recipient; every sequencer call stays real.

## Honesty (AC3.3)

Counterparties are **unlinkable** on-chain — never "hidden/confidential
amounts" (amounts are in the clear). The operator running the sequencer receives
the note secrets and sees the note↔recipient mapping; that trust boundary is
stated in the UI, not hidden.
