#!/usr/bin/env bash
# proving_times.sh — s2/04 proving-time benchmark (two-axis: N × proving wall-clock).
#
# Proves the rollup guest for N=8 (depth 3, the DEPLOYED guest), N=16 (depth 4)
# and N=32 (depth 5), times the wall-clock of each `prove`, and appends one row
# per N to docs/proving-times.md. N=16/32 are PROVING-ONLY (depth 4/5 bench
# guest, never deployed/settled) — clearly labelled in the table.
#
# REAL proofs: each run is a Groth16/BN254 STARK→SNARK wrap (Docker required,
# RISC0_DEV_MODE=0 forced). Each prove takes a long time (≈ minutes→hours per N as
# N/depth grow); this is the multi-hour step the CONTROLLER runs, not a unit test.
#
# Reproducible: depth is selected by ROLLUP_TREE_DEPTH (host-side env; it switches
# ONLY the proving-only bench guest — the deployed depth-3 guest, image_id
# cbeab7aa…, is byte-identical regardless). Receipts go to a per-N out/ dir so the
# canonical out/receipt (the N=8 settle receipt) is NEVER clobbered.
#
# Usage:
#   bash scripts/proving_times.sh            # prove N=8,16,32 (default)
#   bash scripts/proving_times.sh 8 16       # prove only the listed N values
#   N_VALUES="8" bash scripts/proving_times.sh
#
# Each N must have a generated inputs file golden/n{N}_inputs.json (committed;
# produced by `cargo run -p host --release -- gen-inputs --n N --out …`).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${ROOT}"

DOC="${ROOT}/docs/proving-times.md"
HOST_BIN=(cargo run -p host --release --)

# Map N → depth (depth = ceil(log2 N)): 8→3, 16→4, 32→5.
depth_for_n() {
  case "$1" in
    8)  echo 3 ;;
    16) echo 4 ;;
    32) echo 5 ;;
    *)  echo "proving_times.sh: unsupported N=$1 (only 8/16/32)" >&2; exit 2 ;;
  esac
}

# N values to bench (CLI args > N_VALUES env > default 8 16 32).
if [[ $# -gt 0 ]]; then
  NS=("$@")
elif [[ -n "${N_VALUES:-}" ]]; then
  # shellcheck disable=SC2206
  NS=(${N_VALUES})
else
  NS=(8 16 32)
fi

# ── Prereqs ───────────────────────────────────────────────────────────────────
docker info >/dev/null 2>&1 || {
  echo "ERROR: Docker is not running — the STARK→Groth16 wrap requires it." >&2
  exit 1
}
echo "[bench] Docker OK"

# Ensure the doc + table header exist (idempotent).
mkdir -p "$(dirname "${DOC}")"
if [[ ! -f "${DOC}" ]] || ! grep -q '^| N |' "${DOC}"; then
  echo "ERROR: ${DOC} missing its results table header — restore the skeleton." >&2
  exit 1
fi

# Gate-critical: the DEPLOYED guest must be cbeab7aa… before we trust any timing.
echo "[bench] verifying DEPLOYED guest image_id (must be cbeab7aa…0d46)…"
DEPLOYED_ID="$(env -u ROLLUP_TREE_DEPTH "${HOST_BIN[@]}" image-id 2>/dev/null | tail -1)"
EXPECT_ID="cbeab7aa6ce69944e10cca8c7ed94d15aae297f2580752f07a15c6cab6ba0d46"
if [[ "${DEPLOYED_ID}" != "${EXPECT_ID}" ]]; then
  echo "ERROR: deployed image_id ${DEPLOYED_ID} != ${EXPECT_ID} — refusing to bench." >&2
  exit 1
fi
echo "[bench] deployed image_id OK: ${DEPLOYED_ID}"

# ── Bench loop ────────────────────────────────────────────────────────────────
for N in "${NS[@]}"; do
  DEPTH="$(depth_for_n "${N}")"
  INPUTS="golden/n${N}_inputs.json"
  OUTDIR="out/bench/n${N}"
  [[ -f "${INPUTS}" ]] || { echo "ERROR: ${INPUTS} not found (run gen-inputs --n ${N})." >&2; exit 1; }

  if [[ "${N}" == "8" ]]; then
    SETTLE="YES (deployed depth-3 guest, on testnet)"
    ENVPFX=(env -u ROLLUP_TREE_DEPTH)   # depth 3 = deployed guest
  else
    SETTLE="no (proving-only)"
    ENVPFX=(env "ROLLUP_TREE_DEPTH=${DEPTH}")  # bench guest at this depth
  fi

  echo ""
  echo "==> [bench] N=${N} depth=${DEPTH} (${SETTLE})  inputs=${INPUTS}  out=${OUTDIR}"
  echo "    (REAL Groth16 prove — this can take a long time)"

  mkdir -p "${OUTDIR}"
  LOG="${OUTDIR}/prove.log"

  START=$(date +%s)
  # RISC0_DEV_MODE=0 forces a real wrap. Tee the full prover output for the cycle
  # count + audit trail; the prove asserts a real Groth16 receipt itself.
  if RISC0_DEV_MODE=0 "${ENVPFX[@]}" "${HOST_BIN[@]}" \
        prove --inputs "${INPUTS}" --out "${OUTDIR}" 2>&1 | tee "${LOG}"; then
    STATUS="ok"
  else
    STATUS="FAILED"
  fi
  END=$(date +%s)
  ELAPSED=$(( END - START ))
  HMS="$(printf '%dh%02dm%02ds' $(( ELAPSED/3600 )) $(( (ELAPSED%3600)/60 )) $(( ELAPSED%60 )))"

  # Pull the total cycle count the host printed ("proving done: N total cycles").
  CYCLES="$(grep -oE 'proving done: [0-9]+ total cycles' "${LOG}" | grep -oE '[0-9]+' | head -1 || true)"
  CYCLES="${CYCLES:-n/a}"

  if [[ "${STATUS}" != "ok" ]]; then
    echo "ERROR: prove for N=${N} FAILED (see ${LOG})." >&2
    echo "| ${N} | ${DEPTH} | ${CYCLES} | ${HMS} (FAILED) | ${SETTLE} |" >> "${DOC}"
    exit 1
  fi

  echo "[bench] N=${N}: prove ok in ${HMS} (${ELAPSED}s), cycles=${CYCLES}"
  echo "| ${N} | ${DEPTH} | ${CYCLES} | ${HMS} | ${SETTLE} |" >> "${DOC}"
done

echo ""
echo "[bench] done — appended ${#NS[@]} row(s) to ${DOC}:"
grep -E '^\| (8|16|32) \|' "${DOC}" | tail -"${#NS[@]}"
echo "BENCH_OK"
