# Security Reviewer — Memory Index

Durable ledger for the SEC role on the Confidential Payments Rollup (Stellar/Soroban + RISC Zero).
`MEMORY.md` is the index; one line per entry. Details live in the linked files.

## Findings log
- [Settler tx-hash parsing + Failed transition](finding-settler-txhash-2026-06-23.md) — SEC pass of settler.rs/pipeline.rs; parse_tx_hash hardening, 28 tests green, open RPC-confirm gap (EJE 3).
- [EJE-3 RPC-confirm fix](finding-eje3-rpc-confirm-2026-06-23.md) — EJE-3 Medium CLOSED: settle() now RPC-confirms getTransaction==SUCCESS before settled; 37 tests green; PASS CLEAN.

## Patterns / recurring
- [stellar CLI exit semantics](pattern-stellar-cli-exit-codes.md) — `stellar contract invoke` returns non-zero on host error / argparse; live settler relies on `output.status.success()`; gate scripts additionally RPC-confirm getTransaction==SUCCESS.
