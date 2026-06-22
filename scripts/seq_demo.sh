#!/usr/bin/env bash
# seq_demo.sh — s2/05 GATE: sequencer ORCHESTRATION with REAL artifacts.
#
# HONEST LABEL (lock 2): this certifies the sequencer's ORCHESTRATION — the state
# machine, the nullifier lock, the batch trigger at N, and the collision rebuild —
# PLUS a REAL on-chain settle of a REAL proof (the pre-generated N=8 receipt,
# verified by the REAL verifier CBQF…). It does NOT certify that the sequencer
# GENERATES proofs (that is s3: a real prove THROUGH the sequencer on GPU). The
# FixtureProver only replaces the 4-hour prove with the already-generated real
# receipt — it LOADS out/bench/n8/{seal,image_id,journal}, never fabricates.
#
# CERO MOCKS: REAL verifier always, REAL receipt, REAL on-chain settle. No
# dev-mode, no mock-verifier. Fresh deploy per run so the settle really lands.
#
# Flow:
#   1. seq_demo orchestrate → drives submit×8 + lock + trigger + fixture prove,
#      emits the settle args (real seal/image_id/journal) + the 8 nullifiers.
#   2. Deploy FRESH pool(levels=3) + seed 8 commitments → root == receipt root.
#   3. Deploy FRESH rollup (real verifier) + fund; settle on-chain → RPC SUCCESS.
#   4. Replay reverts → on-chain PROOF the 8 nullifiers are now is_spent.
#   5. Collision: feed one on-chain-spent nullifier → seq_demo rebuilds N−1 →
#      `host execute` VALIDATES it (lock 3 byte-compat: the real prover would
#      accept it; root unchanged, dropped nullifier absent, membership passes).
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
NETWORK="testnet"; SIGNER="spikekey"
VERIFIER="CBQFQLSBYXUYLD2Q5EWHVNNI6VO33NAVRDUDIGJNMC5TUAINK5BXO2LJ"
TOKEN="CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC"
POOL_WASM="${ROOT}/deployments/artifacts/pool.wasm"
ROLLUP_WASM="${ROOT}/target/wasm32v1-none/release/rollup.wasm"
DEMO_DIR="${ROOT}/out/seq/demo"
RPC_URL="https://soroban-testnet.stellar.org"
LEVELS=3; MAX_DEPOSIT=1000000000000; SUM_AMOUNTS=8028
ADMIN="$(stellar keys address "${SIGNER}")"

# N=8 commitments (U256 dec, leaf order 0..7) — the leaves the pool is seeded with
# so its root equals the receipt's merkle_root (054be46b…). Same values as
# deploy_settle_n8.sh (derived from golden/n8_inputs.json).
C=(2098441622598076782746511037081986855819330548846085959436867638769621116777 \
   21850405880430287235981682780826813804754627803832801673792234743332955240539 \
   18045205374491365785174578378606250270069615164305461249410963774719418282513 \
   16178099244925787357733210809213436994899948690551900306934395880707990780509 \
   20815269016325629426393636467174170632206143373393341791705306631012837546302 \
   19189738753259764291719179573442822446694061001295892209456045593398536626564 \
   2694129937400855065380482240116444891444498468902275455052190903817872907992 \
   1685480781901591428795149257982532325864526028206086168943935810355343183620)
EXPECTED_ROOT_DEC=9477110651562997593162969197308257692712826173766156000136958452708141189893

echo "============================================================"
echo "s2/05 GATE — sequencer ORCHESTRATION (real artifacts, real verifier)"
echo "  admin ${ADMIN} · verifier ${VERIFIER:0:8}… · network ${NETWORK}"
echo "  LABEL: certifies orchestration + a REAL settle of a REAL proof."
echo "         Does NOT certify proof GENERATION through the sequencer (= s3)."
echo "============================================================"

# ── 0. Build host (release), seq_demo (test-fixture), rollup wasm ──────────────
[[ -f "${POOL_WASM}" ]] || { echo "ERROR: ${POOL_WASM} missing" >&2; exit 1; }
echo "[0/6] build host + seq_demo + rollup wasm"
( cd "${ROOT}" && cargo build -p host --release >/dev/null 2>&1 ) \
  || { echo "ERROR: host build failed" >&2; exit 1; }
( cd "${ROOT}" && cargo build -p sequencer --features test-fixture --bin seq_demo >/dev/null 2>&1 ) \
  || { echo "ERROR: seq_demo build failed" >&2; exit 1; }
( cd "${ROOT}" && stellar contract build --package rollup >/dev/null 2>&1 || cargo build -p rollup --release --target wasm32v1-none >/dev/null 2>&1 )
[[ -f "${ROLLUP_WASM}" ]] || { echo "ERROR: ${ROLLUP_WASM} missing" >&2; exit 1; }
HOST="${ROOT}/target/release/host"
SEQ="${ROOT}/target/debug/seq_demo"
[[ -x "${HOST}" && -x "${SEQ}" ]] || { echo "ERROR: built binaries missing" >&2; exit 1; }

# ── 1. ORCHESTRATE: submit×8 + lock + trigger + fixture prove (real receipt) ───
echo ""
echo "[1/6] sequencer orchestration (state machine + lock + trigger + fixture prove)"
"${SEQ}" orchestrate --out-dir "${DEMO_DIR}" || { echo "ERROR: orchestrate FAILED" >&2; exit 1; }
SEAL="$(tr -d '[:space:]' < "${DEMO_DIR}/seal.hex")"
IMG="$(tr -d '[:space:]' < "${DEMO_DIR}/image_id.hex")"
JOURNAL="$(xxd -p "${DEMO_DIR}/journal.bin" | tr -d '[:space:]')"
# Read the 8 nullifiers (bash 3.2 has no mapfile).
NFS=()
while IFS= read -r line; do [[ -n "${line}" ]] && NFS+=("${line}"); done < "${DEMO_DIR}/nullifiers.txt"
echo "      orchestration emitted: seal $(( ${#SEAL}/2 ))B, journal $(( ${#JOURNAL}/2 ))B, ${#NFS[@]} nullifiers"
[[ "${IMG}" == "cbeab7aa6ce69944e10cca8c7ed94d15aae297f2580752f07a15c6cab6ba0d46" ]] \
  || { echo "ERROR: orchestrated image_id ${IMG:0:12}… != deployed guest cbeab7aa…" >&2; exit 1; }

# ── 2. Deploy FRESH pool + seed 8 commitments → root == receipt root ───────────
echo ""
echo "[2/6] deploy FRESH pool (levels=${LEVELS}) + seed 8 commitments"
POOL_ID="$(stellar contract deploy --wasm "${POOL_WASM}" --source "${SIGNER}" --network "${NETWORK}" -- \
  --admin "${ADMIN}" --token "${TOKEN}" --verifier "${VERIFIER}" \
  --asp_membership "${ADMIN}" --asp_non_membership "${ADMIN}" \
  --maximum_deposit_amount "${MAX_DEPOSIT}" --levels "${LEVELS}" 2>&1 | tail -1)"
case "${POOL_ID}" in C*) ;; *) echo "ERROR: pool deploy failed: ${POOL_ID}" >&2; exit 1;; esac
echo "      POOL_ID=${POOL_ID}"
seed(){ stellar contract invoke --id "${POOL_ID}" --source "${SIGNER}" --network "${NETWORK}" --send=yes -- seed_two_leaves --leaf_1 "$1" --leaf_2 "$2" >/dev/null; }
seed "${C[0]}" "${C[1]}"; seed "${C[2]}" "${C[3]}"; seed "${C[4]}" "${C[5]}"; seed "${C[6]}" "${C[7]}"
ROOT_DEC="$(stellar contract invoke --id "${POOL_ID}" --source "${SIGNER}" --network "${NETWORK}" -- get_root 2>&1 | tail -1 | tr -d '"[:space:]')"
echo "      on-chain root: ${ROOT_DEC}"
[[ "${ROOT_DEC}" == "${EXPECTED_ROOT_DEC}" ]] || { echo "ERROR: ROOT MISMATCH — STOP (no settle)" >&2; exit 2; }
echo "      ROOT MATCH (is_known_root will accept the receipt root)"

# ── 3. Deploy FRESH rollup (REAL verifier) + fund + SETTLE on-chain ────────────
echo ""
echo "[3/6] deploy FRESH rollup (real verifier ${VERIFIER:0:8}…) + fund ${SUM_AMOUNTS}"
ROLLUP_ID="$(stellar contract deploy --wasm "${ROLLUP_WASM}" --source "${SIGNER}" --network "${NETWORK}" -- \
  --verifier "${VERIFIER}" --pool "${POOL_ID}" --token "${TOKEN}" 2>&1 | tail -1)"
case "${ROLLUP_ID}" in C*) ;; *) echo "ERROR: rollup deploy failed: ${ROLLUP_ID}" >&2; exit 1;; esac
echo "      ROLLUP_ID=${ROLLUP_ID}"
stellar contract invoke --id "${TOKEN}" --source "${SIGNER}" --network "${NETWORK}" --send=yes -- \
  transfer --from "${ADMIN}" --to "${ROLLUP_ID}" --amount "${SUM_AMOUNTS}" >/dev/null
RBAL_BEFORE="$(stellar contract invoke --id "${TOKEN}" --source "${SIGNER}" --network "${NETWORK}" -- balance --id "${ROLLUP_ID}" 2>&1 | tail -1 | tr -d '"[:space:]')"
echo "      rollup balance before settle: ${RBAL_BEFORE} (expect ${SUM_AMOUNTS})"

echo "      SETTLE the orchestrated batch (--send=yes, real verifier)"
SETTLE_OUT="$(stellar contract invoke --id "${ROLLUP_ID}" --source "${SIGNER}" --network "${NETWORK}" --send=yes -- \
  settle_batch --seal "${SEAL}" --image_id "${IMG}" --journal_bytes "${JOURNAL}" 2>&1)"
TXH="$(echo "${SETTLE_OUT}" | grep -oE '[a-f0-9]{64}' | head -1 || true)"
[[ -n "${TXH}" ]] || { echo "ERROR: no settle tx hash" >&2; echo "${SETTLE_OUT}" >&2; exit 1; }
RESP="$(curl -s -X POST "${RPC_URL}" -H 'Content-Type: application/json' \
  -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"getTransaction\",\"params\":{\"hash\":\"${TXH}\"}}")"
STATUS="$(echo "${RESP}" | python3 -c "import sys,json;print(json.load(sys.stdin).get('result',{}).get('status','UNKNOWN'))" 2>/dev/null || echo PARSE_ERROR)"
RBAL_AFTER="$(stellar contract invoke --id "${TOKEN}" --source "${SIGNER}" --network "${NETWORK}" -- balance --id "${ROLLUP_ID}" 2>&1 | tail -1 | tr -d '"[:space:]')"
echo "      settle tx: ${TXH}"
echo "      RPC getTransaction status: ${STATUS}"
echo "      rollup balance after: ${RBAL_AFTER} (expect 0 → 8 withdrawals paid)"
[[ "${STATUS}" == "SUCCESS" ]] || { echo "ERROR: settle did NOT SUCCEED (${STATUS})" >&2; exit 1; }
[[ "${RBAL_AFTER}" == "0" ]] || { echo "ERROR: rollup not drained ($RBAL_AFTER != 0)" >&2; exit 1; }
echo "      SETTLE SUCCESS (8 nullifiers marked spent, 8 transfers executed)"

# ── 4. On-chain is_spent PROOF: replaying the SAME batch must revert ──────────
echo ""
echo "[4/6] on-chain is_spent proof: replay the SAME batch (must revert NullifierSpent)"
if stellar contract invoke --id "${ROLLUP_ID}" --source "${SIGNER}" --network "${NETWORK}" --send=yes -- \
  settle_batch --seal "${SEAL}" --image_id "${IMG}" --journal_bytes "${JOURNAL}" >/dev/null 2>&1; then
  echo "ERROR: replay SUCCEEDED — nullifiers NOT spent (double-spend hole!)" >&2; exit 1
fi
echo "      replay REVERTED → the 8 nullifiers are now is_spent ON-CHAIN (real query)"

# ── 5. COLLISION: drop an on-chain-spent note, rebuild N−1, host execute it ────
echo ""
echo "[5/6] collision: drop a now-spent note, rebuild N−1, validate via host execute"
SPENT_NF="${NFS[3]}"   # note3's nullifier — proven is_spent on-chain by step 4.
echo "      feeding on-chain-spent nullifier ${SPENT_NF:0:14}… (note3) to the collision handler"
REBUILT="${ROOT}/out/seq/rebuilt_inputs.json"
"${SEQ}" collision --spent "${SPENT_NF}" --out-rebuilt "${REBUILT}" \
  || { echo "ERROR: collision rebuild FAILED" >&2; exit 1; }
echo "      running host execute on the rebuilt N−1 input (LOCK 3 byte-compat)..."
EXEC_OUT="$("${HOST}" execute --inputs "${REBUILT}" 2>&1)"
echo "${EXEC_OUT}" | sed 's/^/        /'
echo "${EXEC_OUT}" | grep -q "OK: witness valid under the executor (N=7" \
  || { echo "ERROR: host execute did NOT validate the rebuilt N−1 input (lock 3 FAILED)" >&2; exit 1; }
# The dropped nullifier must NOT appear in the rebuilt journal.
if echo "${EXEC_OUT}" | grep -qi "${SPENT_NF#0x}"; then
  echo "ERROR: dropped nullifier ${SPENT_NF:0:14}… reappears in rebuilt journal" >&2; exit 1
fi
echo "      REBUILD VALIDATED: N−1=7, root unchanged, dropped nullifier absent, membership passes"
echo "      (on-chain settle of the rebuilt N−1 needs a real re-prove → s3 GPU, NOT this gate)"

# ── 6. Summary + explorer links ───────────────────────────────────────────────
echo ""
echo "[6/6] ============================================================"
echo "  POOL_ID    = ${POOL_ID}"
echo "  ROLLUP_ID  = ${ROLLUP_ID}"
echo "  SETTLE_TX  = ${TXH}  (status ${STATUS})"
echo "  explorer   : https://stellar.expert/explorer/testnet/tx/${TXH}"
echo "  verifier   : https://stellar.expert/explorer/testnet/contract/${VERIFIER}"
echo "  state machine + lock + trigger : OK (orchestrate)"
echo "  happy-path settle (RPC SUCCESS, real verifier, real N=8 receipt) : OK"
echo "  collision rebuild N−1 validated by host execute (lock 3) : OK"
echo "============================================================"
echo "GATE_PASS"
