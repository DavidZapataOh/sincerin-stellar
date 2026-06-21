# s1/05 — N=2 On-Chain Verification (ELIMINATORIA)

**Task:** s1/05 — Verify the N=2 receipt ON-CHAIN on testnet
**Date:** 2026-06-20
**Status:** SUCCESS — ELIMINATORIA PASSED

---

## Pipeline

This is the **own end-to-end pipeline receipt** (not the spike's committed vector):
- `s1/03` — Guest N=2 logic (Poseidon2/BN254 + Merkle membership + nullifier derivation + balance conservation)
- `s1/04` — Groth16 wrap (RISC Zero prover 3.0.5, Docker STARK→Groth16)
- `s1/05` — On-chain verify (this task)

---

## Invocation

```bash
stellar contract invoke \
  --id CBQFQLSBYXUYLD2Q5EWHVNNI6VO33NAVRDUDIGJNMC5TUAINK5BXO2LJ \
  --source spikekey \
  --network testnet \
  --send=yes \
  -- verify \
  --seal <seal_hex_520_chars> \
  --image_id 3f036c528c18ed4cd66a898839d343fd4a51bb8735acec0577c6c6389c5d4d22 \
  --journal 0ec6ebba7901dd80e9605bac4e2aab5f62344175b2fd4aedc38e070d5bf90e89
```

Note: `--journal` is the **sha256 DIGEST** of `out/receipt/journal.bin`, not the journal itself.

---

## Receipt Artifacts

| Artifact | Value |
|---|---|
| `image_id` | `3f036c528c18ed4cd66a898839d343fd4a51bb8735acec0577c6c6389c5d4d22` |
| `seal selector` | `73c457ba` (Groth16/BN254) |
| `seal size` | 260 bytes (520 hex chars) |
| `journal.bin size` | 196 bytes |
| `journal sha256` | `0ec6ebba7901dd80e9605bac4e2aab5f62344175b2fd4aedc38e070d5bf90e89` |

---

## On-Chain Result (Gate run)

| Field | Value |
|---|---|
| **TX hash** | `7c9c51d924283c531169199cc96caae1398e07772f75057a3bf1d4df91a6f35b` |
| **Explorer** | https://stellar.expert/explorer/testnet/tx/7c9c51d924283c531169199cc96caae1398e07772f75057a3bf1d4df91a6f35b |
| **RPC status** | `SUCCESS` |
| **Ledger** | 3198629 |
| **Fee charged** | 36882 stroops (0.0036882 XLM) |
| **cpu_insn** | 31,506,765 (~31.5M = 7.9% of 400M budget) |
| **mem_bytes** | 1,540,662 (~1.5 MB) |
| **Verifier** | `CBQFQLSBYXUYLD2Q5EWHVNNI6VO33NAVRDUDIGJNMC5TUAINK5BXO2LJ` |
| **Signer** | `spikekey` (`GCVW272JWSJIOZUD3ZGT4MNFF45AHXHH53EMPBUG67K5LX735FTKV5T5`) |

Also submitted an earlier test tx (same result):
- TX: `7a1410d72f187016e61d8387b39b1b0c26204643bb567ddfed2573e3fe9ee170`
- Explorer: https://stellar.expert/explorer/testnet/tx/7a1410d72f187016e61d8387b39b1b0c26204643bb567ddfed2573e3fe9ee170

---

## Compat Verdict

**3.0.5 prover seal verified on 3.0.0 verifier VK: CONFIRMED.**

The RISC Zero prover at version 3.0.5 produces Groth16/BN254 seals (selector `73c457ba`) that are accepted by the deployed verifier whose `parameters.json` is at version 3.0.0. The VK/control-root is stable across patch versions (3.0.x). This is now **empirically proven on-chain** — no redeploy of the verifier is needed.

---

## ELIMINATORIA

The ZK verification happened ON-CHAIN in testnet (RPC `getTransaction` status `SUCCESS`). This is NOT a local/simulated verify. The `GATE_OK` was produced by `bash scripts/verify_onchain.sh && echo GATE_OK` with fresh output including the real tx hash above.

---

## Benchmark Note

On-chain cost for the N=2 verification:
- cpu_insn: ~31.5M (7.9% of the 400M ledger budget — lower than the spike's ~35.3M, within expected variance)
- Cost is **constant in N** (the verifier always verifies 1 Groth16 proof regardless of N)
- This anchors the s3 benchmark table for the on-chain axis
