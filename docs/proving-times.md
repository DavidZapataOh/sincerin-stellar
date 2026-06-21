# Proving-time benchmark — N × proving wall-clock (CONTEXT.md D2 / AC2.1, AC2.2)

How the proving cost of the rollup grows with **N** (number of withdrawals
aggregated in one batch). Depth scales with N: `depth = ceil(log2 N)`, so each
note's Merkle path has exactly `depth` siblings and a full tree has `2^depth = N`
leaves.

- **N=8 (depth 3)** is the **on-chain demo**: proved with the **DEPLOYED guest**
  (`image_id cbeab7aa6ce69944e10cca8c7ed94d15aae297f2580752f07a15c6cab6ba0d46`),
  the binary the `settle_batch` contract binds, and **settled on testnet**.
- **N=16 (depth 4)** and **N=32 (depth 5)** are **PROVING-ONLY** (informative):
  proved with the separate proving-only `rollup-guest-bench` guest built at the
  matching depth (`ROLLUP_TREE_DEPTH=4|5`). Their image_ids differ from the
  deployed guest and are **never deployed or settled** — they exist solely to
  measure how proving scales with N.

The **on-chain settle cost** is constant in N (~35.3M instr ≈ 8.8% of 400M — one
Groth16 verification + N cheap nullifier/transfer ops); this table measures the
**off-chain proving** cost, the other axis.

## Method (reproducible)

```bash
# 1. (once) input sets are committed: golden/n{8,16,32}_inputs.json
#    regen if needed: cargo run -p host --release -- gen-inputs --n <N> --out golden/n<N>_inputs.json [--recipients …]
# 2. run the bench (REAL Groth16 proves — multi-hour; the controller runs this):
bash scripts/proving_times.sh           # N=8,16,32
# or a subset:  bash scripts/proving_times.sh 8 16
```

Each prove is a **real** Groth16/BN254 STARK→SNARK wrap (Docker, `RISC0_DEV_MODE=0`
forced — never a dev-mode fake). The script times wall-clock per N and appends a
row below. Receipts go to `out/bench/n<N>/` so the canonical N=8 settle receipt
in `out/receipt/` is never clobbered.

- **cycles** = `prove_info.stats.total_cycles` (the executor cycle count the host
  prints), captured from the prover log.
- **proving wall-clock** = end-to-end `prove` time (execute + STARK prove +
  STARK→Groth16 wrap), `date`-measured around the host invocation.
- Hardware / RISC Zero version should be recorded with the run (see below).

> Run environment (fill in when the controller runs the proves):
> - host: `<cpu / ram / os>`
> - RISC Zero: `risc0-zkvm =3.0.5`, groth16-prover container `r0.1.88.0`
> - date: `<YYYY-MM-DD>`

## Results

| N | depth | cycles | proving wall-clock | on-chain settle? |
|---|-------|--------|--------------------|------------------|
<!-- rows appended by scripts/proving_times.sh — one per N. Pending the controller's proves. -->
