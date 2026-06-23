---
name: pattern-stellar-cli-exit-codes
description: stellar contract invoke exit-code semantics and how the sequencer/gates depend on them.
metadata:
  type: reference
---

# stellar CLI exit-code semantics (testnet settle seam)

**Fact (empirically verified, stellar v25.2.0 on this machine, 2026-06-23):**
- `stellar contract invoke` returns exit **1** on a contract/host error (e.g. "contract not found"),
  exit **2** on argparse error, exit **0** on success.
- Captured real revert output in repo: `deployments/testnet.json:111` →
  `"replay": "Error(Contract,#3) NullifierSpent"` (the on-chain revert surfaced by the CLI).

**How the project depends on it:**
- Live `StellarCliSettler::settle` gates on `output.status.success()` (settler.rs:182) — treats a
  non-zero CLI exit as a failed settle. Sound for simulation-time reverts (NullifierSpent, unknown
  root, verify-fail), which fail before/at submission.
- Gate scripts go further and RPC-confirm: `seq_demo.sh:117-124` and `seq_http_gate.sh` resolve the
  tx hash via `getTransaction` and assert `status==SUCCESS` + balance drained. The LIVE server does
  NOT do this RPC re-check — that is the EJE-3 residual gap.

**Open question to verify if it ever matters:** does `--send=yes` return non-zero when a tx PASSES
simulation but FAILS at ledger inclusion? Not exhaustively tested here (would need a fresh
deploy + an artifically-failing submit). Until confirmed, prefer the RPC getTransaction re-check in
the live settler for a hard binding to on-chain success.
