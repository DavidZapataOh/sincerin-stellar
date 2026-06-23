---
name: finding-settler-txhash-2026-06-23
description: SEC pass of sequencer Settler + pipeline (tx-hash parsing, Failed transition, fund atomicity). 28 tests green; one open RPC-confirm gap.
metadata:
  type: project
---

# SEC pass — Settler + sequencer pipeline (2026-06-23)

Scope: `sequencer/src/{settler,pipeline,lib,types,tests}.rs`. Pre-merge review after two
honesty bugs were fixed (Bug A: image_id mis-parsed as tx hash; Bug B: sha256(journal) on a
failed settle reported as settled).

**Why:** code moves funds (settles N withdrawals on-chain); a fake tx hash kills the "es real" demo claim.

**How to apply:** when re-reviewing the settle seam, start from these three axes and re-check the
open RPC-confirm gap before approving any change that touches `StellarCliSettler::settle` or `drive_batch`.

## Verdict: PASS-WITH-CONDITIONS
- EJE 1 (tx-hash parsing): PASS. `parse_tx_hash` (settler.rs:110) returns the first 64-run hex
  that != exclude(image_id). Success-gate at settler.rs:182 (`output.status.success()`) runs
  BEFORE parsing, so journal-digest (Bug B, a revert-path-only diagnostic) can't reach Settled.
  seal(520B)/journal(676B) hex runs are >64 → never match `run==64`. contract id is base32 (C…) not hex.
- EJE 2 (Failed transition): PASS. Every settle error → `Err` → `fail_batch` → `mark_failed`
  (lib.rs:274) which removes the nullifier from `reserved` → re-submit possible. SettleError has
  no Ok-bearing variant; `NoTxHash` is an Err. prove failure path identical.
- EJE 3 (fund atomicity): PASS-WITH-CONDITION. The on-chain `settle_batch` IS atomic (contract).
  But the live `StellarCliSettler` accepts settled on `output.status.success()` ALONE — it does
  NOT do the RPC `getTransaction==SUCCESS` re-check the gate scripts do (seq_demo.sh:117-124,
  seq_http_gate.sh step 6). Empirically the CLI returns non-zero on host error (verified v25.2.0),
  and the project already depends on that (seq_demo.sh:131 replay-revert assertion). Residual risk:
  a CLI/RPC path where `--send=yes` exits 0 yet the ledger tx ultimately failed would show a
  resolvable-looking hash that the explorer marks FAILED. Recommend the live settler also resolve
  the hash by RPC before Settled (Medium).

## No double-spend window
`mark_settled` (lib.rs:261) runs only AFTER `settle()` returns Ok (pipeline.rs:161-173). Nullifier
is never marked spent before on-chain confirmation. Lock released on settle (correct: now spent
on-chain) and on fail (correct: re-submittable).

## Note: handle_collision NOT wired into live pipeline
`handle_collision` (the on-chain is_spent anti-replay reconciliation) is only called by the gate
binary `seq_demo.rs` + unit tests, NOT by `drive_batch`. Live pipeline goes prove→settle→mark.
Acceptable for MVP because `settle_batch` is atomic (a colliding nullifier reverts the whole tx →
Failed → re-submit), but the in-memory N−1 rebuild optimization is gate-only. Document, not a blocker.

## Tests: 28 green (no features) + compiles with --features test-fixture.
Coverage gap: no test asserts a 64-hex SPURIOUS value on a FAILED (non-zero exit) output is
rejected — parse_tx_hash tests only feed success-shaped strings; the success-gate is only covered
indirectly. Recommend a unit test driving the success-gate with a non-zero ExitStatus is infeasible
(can't construct ExitStatus), so cover via the `combined` parse path + an integration test that a
failed CLI exit → SettleError::Invoke (not NoTxHash with a digest).
