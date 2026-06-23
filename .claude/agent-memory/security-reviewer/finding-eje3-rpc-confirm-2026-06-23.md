---
name: finding-eje3-rpc-confirm-2026-06-23
description: EJE-3 RPC getTransaction==SUCCESS re-check fix verified in StellarCliSettler::settle — closes the Medium from the prior pass. 37 tests green.
metadata:
  type: project
---

# SEC re-pass — EJE-3 RPC-confirm fix (2026-06-23)

Scope: `sequencer/src/settler.rs` only (focused re-review pre-merge). Fix for the open
Medium from [[finding-settler-txhash-2026-06-23]]: live settler accepted `Settled` on
`output.status.success()` alone, without the RPC `getTransaction==SUCCESS` re-check the
gate scripts do. Implemented via TDD (red 6bf8924 / green 972a11f).

**Why:** the live settle seam moves funds; CLI exit 0 means the command ran, NOT that the
ledger tx applied. Without the RPC re-check a `--send=yes` that exits 0 but FAILED at
inclusion would be reported settled with a resolvable hash the explorer marks FAILED.

**How to apply:** if `settle()` / `confirm_status` / `finalize_status` change again, re-check
that ONLY `"SUCCESS"` returns Ok and that no path lets `confirm_status` return a spurious
SUCCESS (default `last="UNKNOWN"`, curl-failure never sets `last`).

## Verdict: PASS CLEAN
- EJE 3 CLOSED. `settle()` (settler.rs:298) is now: `parse_settle_output` (EJE-1 gate) →
  `confirm_status(hash)` (RPC getTransaction) → `finalize_status(hash,status)`.
  `finalize_status` (settler.rs:188) returns `Ok` IFF `status=="SUCCESS"`; every other string
  (FAILED/NOT_FOUND/PENDING/UNKNOWN/anything) → `Err(OnchainFailed{hash,status})`. Single `==`
  comparison, no fallthrough.
- No new false-settled path in `confirm_status` (settler.rs:256). SUCCESS is ONLY returned
  inside `if out.status.success()` AND `parse_rpc_status(...)==Some("SUCCESS")`. `parse_rpc_status`
  requires `.result.status` be a JSON string → an RPC-level `{"error":...}` or non-JSON body →
  `None` → never SUCCESS. curl spawn failure → `Err(Invoke)` (not Ok). curl exit!=0 → skipped,
  `last` unchanged. Default `last="UNKNOWN"` → finalize → OnchainFailed. Loop bounded `0..5`
  (cannot hang/infinite-loop). A success body cannot be forged on testnet RPC over TLS; even a
  hypothetical injected SUCCESS would need a string-literal `"status":"SUCCESS"` under `.result`.
- Driver unchanged + correct: `SettleError` (incl. new `OnchainFailed`) is all-Err → pipeline.rs:161-167
  maps to `fail_batch`→`mark_failed` (lib.rs:274) which removes the nullifier from `reserved`
  (re-submit possible). `mark_settled` (lib.rs:261) runs ONLY after `settle()` returns Ok
  (pipeline.rs:172). No double-spend / false-settled window.
- EJE 1 / EJE 2 no regression. The old inline success-gate was extracted verbatim into pure
  `parse_settle_output` (settler.rs:154): `ok==false` → `Invoke` (never a hash), image_id excluded
  on success. All `SettleError` variants remain `Err`.

## Liveness vs safety tradeoff (accepted)
False-NEGATIVE only: RPC down / tx truly applied but unconfirmed → batch reported Failed, lock
released. A re-submit reverts on-chain via `NullifierSpent` (settle_batch is atomic) → no
double-spend, no fund risk. This is a liveness cost, not a safety hole. Correct conservative bias.

## Tests: 37 passed / 0 failed (no features). 15 settler unit tests.
finalize_status_{success,failed,not_found,unknown}, parse_rpc_status_{reads,none_on_malformed_or_error},
parse_settle_output_{rejects_spurious_hex_on_failure,returns_tx_hash_excluding_image_id,no_hash_is_error},
parse_tx_hash_* (5), argv_* (2). Pure logic (EJE 1 + EJE 3) is fully unit-covered.
Residual (acceptable, documented): `confirm_status` IO itself (curl spawn-fail → Err, curl exit!=0
→ skip, transient retry loop) is NOT unit-tested — it's thin IO glue; the decision logic lives in the
pure `parse_rpc_status`/`finalize_status` and the gates exercise it end-to-end on-chain. Not a blocker.
