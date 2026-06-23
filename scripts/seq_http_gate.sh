#!/usr/bin/env bash
# seq_http_gate.sh — s3/02 NO-GPU GATE, end-to-end through the HTTP API.
#
# Proves the WHOLE frontend connection layer works against REAL on-chain settle,
# WITHOUT a GPU: frontend → real sequencer HTTP API (`seq_demo_http`) →
# FixtureProver (LOADS the real N=8 receipt, never fabricates) → REAL
# StellarCliSettler → a REAL settle tx on testnet → a REAL tx hash.
#
# CERO MOCKS (plan §"Cero mocks"): real verifier CBQF…, real N=8 receipt
# (out/bench/n8), real on-chain settle. No dev-mode, no mock settler. Fresh pool +
# rollup per run so the settle really lands and the 8 nullifiers are unspent.
#
# THE LOAD-BEARING ASSERTION (the bug we just fixed in parse_tx_hash):
#   the settled tx_hash MUST be the REAL settle tx, NOT the deployed guest
#   image_id (cbeab7aa…). `stellar contract invoke` echoes `--image_id <hex>`
#   which is also 64 hex and appears BEFORE the tx hash; a naive parser returned
#   the image_id → a judge clicks into a 404. We assert tx_hash != image_id AND
#   resolve it by RPC to status SUCCESS.
#
# Flow:
#   1. Build seq_demo_http (release, test-fixture) + host + rollup wasm.
#   2. Deploy FRESH pool(levels=3) + seed the fixture's 8 commitments; assert
#      get_root == fixture root. Deploy FRESH rollup (real verifier) + fund 8028.
#   3. Start seq_demo_http (ROLLUP_ID=fresh, N_target=8, FIXTURE_PROVE_DELAY=20).
#      Wait for GET /config to echo the fresh rollup id.
#   4. POST 8 distinct-nullifier intents sharing one root (golden n8 notes) →
#      first request_id in <2s (assert). The batch assembles at N=8.
#   5. Poll GET /status/:id until settled (or failed → dump reason + fail).
#   6. Assert tx_hash != image_id; RPC getTransaction → SUCCESS; /recent_batches
#      shows the real hash with n:8 (most-recent-first), NOT a cbeab7aa entry.
#   7. Print GATE_PASS + the real tx hash + explorer link.
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

# ── config (mirrors scripts/seq_demo.sh + deploy_settle_n8.sh) ─────────────────
NETWORK="${NETWORK:-testnet}"
SIGNER="${SIGNER:-spikekey}"
VERIFIER="CBQFQLSBYXUYLD2Q5EWHVNNI6VO33NAVRDUDIGJNMC5TUAINK5BXO2LJ"
TOKEN="CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC"
POOL_WASM="${ROOT}/deployments/artifacts/pool.wasm"
ROLLUP_WASM="${ROOT}/target/wasm32v1-none/release/rollup.wasm"
RECEIPT="${ROOT}/out/bench/n8"
RPC_URL="https://soroban-testnet.stellar.org"
LEVELS=3; MAX_DEPOSIT=1000000000000; SUM_AMOUNTS=8028
N_TARGET=8
FIXTURE_PROVE_DELAY="${FIXTURE_PROVE_DELAY:-20}"   # honest dev delay before the REAL receipt
BIND_HOST="127.0.0.1"; BIND_PORT="${BIND_PORT:-8799}"
BIND="${BIND_HOST}:${BIND_PORT}"
API="http://${BIND}"
ADMIN="$(stellar keys address "${SIGNER}")"

# The deployed guest image_id — the value the tx hash must NOT equal (the bug).
IMAGE_ID="cbeab7aa6ce69944e10cca8c7ed94d15aae297f2580752f07a15c6cab6ba0d46"

# N=8 commitments (U256 dec, leaf order 0..7) — seed so the pool root == the
# receipt's merkle_root. Same values as seq_demo.sh / deploy_settle_n8.sh.
C=(2098441622598076782746511037081986855819330548846085959436867638769621116777 \
   21850405880430287235981682780826813804754627803832801673792234743332955240539 \
   18045205374491365785174578378606250270069615164305461249410963774719418282513 \
   16178099244925787357733210809213436994899948690551900306934395880707990780509 \
   20815269016325629426393636467174170632206143373393341791705306631012837546302 \
   19189738753259764291719179573442822446694061001295892209456045593398536626564 \
   2694129937400855065380482240116444891444498468902275455052190903817872907992 \
   1685480781901591428795149257982532325864526028206086168943935810355343183620)
EXPECTED_ROOT_DEC=9477110651562997593162969197308257692712826173766156000136958452708141189893

SEQ_PID=""
cleanup() { [[ -n "${SEQ_PID}" ]] && kill "${SEQ_PID}" 2>/dev/null || true; }
trap cleanup EXIT

echo "============================================================"
echo "s3/02 NO-GPU GATE — frontend HTTP API → FixtureProver → REAL settle"
echo "  admin ${ADMIN} · verifier ${VERIFIER:0:8}… · network ${NETWORK}"
echo "  API on ${API} · N_target=${N_TARGET} · FIXTURE_PROVE_DELAY=${FIXTURE_PROVE_DELAY}s"
echo "  the settle tx hash MUST be REAL, never the image_id ${IMAGE_ID:0:8}…"
echo "============================================================"

# ── 1. Build seq_demo_http (release) + host + rollup wasm ──────────────────────
[[ -f "${POOL_WASM}" ]] || { echo "ERROR: ${POOL_WASM} missing" >&2; exit 1; }
for f in seal.hex image_id.hex journal.bin; do
  [[ -f "${RECEIPT}/${f}" ]] || { echo "ERROR: ${RECEIPT}/${f} missing" >&2; exit 1; }
done
RECEIPT_IMG="$(tr -d '[:space:]' < "${RECEIPT}/image_id.hex")"
[[ "${RECEIPT_IMG}" == "${IMAGE_ID}" ]] \
  || { echo "ERROR: receipt image_id ${RECEIPT_IMG:0:12}… != deployed ${IMAGE_ID:0:12}…" >&2; exit 1; }

echo "[1/7] build seq_demo_http (release, test-fixture) + host + rollup wasm"
( cd "${ROOT}" && cargo build -p sequencer --features test-fixture --bin seq_demo_http --release >/dev/null 2>&1 ) \
  || { echo "ERROR: seq_demo_http build failed" >&2; exit 1; }
( cd "${ROOT}" && cargo build -p host --release >/dev/null 2>&1 ) \
  || { echo "ERROR: host build failed" >&2; exit 1; }
( cd "${ROOT}" && stellar contract build --package rollup >/dev/null 2>&1 \
    || cargo build -p rollup --release --target wasm32v1-none >/dev/null 2>&1 )
[[ -f "${ROLLUP_WASM}" ]] || { echo "ERROR: ${ROLLUP_WASM} missing" >&2; exit 1; }
SEQ_HTTP="${ROOT}/target/release/seq_demo_http"
[[ -x "${SEQ_HTTP}" ]] || { echo "ERROR: ${SEQ_HTTP} missing" >&2; exit 1; }
echo "      built ${SEQ_HTTP}"

# ── 2. Deploy FRESH pool + seed 8 commitments + assert root; rollup + fund ─────
echo ""
echo "[2/7] deploy FRESH pool (levels=${LEVELS}) + seed 8 commitments"
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

echo "      deploy FRESH rollup (real verifier ${VERIFIER:0:8}…) + fund ${SUM_AMOUNTS}"
ROLLUP_ID="$(stellar contract deploy --wasm "${ROLLUP_WASM}" --source "${SIGNER}" --network "${NETWORK}" -- \
  --verifier "${VERIFIER}" --pool "${POOL_ID}" --token "${TOKEN}" 2>&1 | tail -1)"
case "${ROLLUP_ID}" in C*) ;; *) echo "ERROR: rollup deploy failed: ${ROLLUP_ID}" >&2; exit 1;; esac
echo "      ROLLUP_ID=${ROLLUP_ID}"
stellar contract invoke --id "${TOKEN}" --source "${SIGNER}" --network "${NETWORK}" --send=yes -- \
  transfer --from "${ADMIN}" --to "${ROLLUP_ID}" --amount "${SUM_AMOUNTS}" >/dev/null
RBAL_BEFORE="$(stellar contract invoke --id "${TOKEN}" --source "${SIGNER}" --network "${NETWORK}" -- balance --id "${ROLLUP_ID}" 2>&1 | tail -1 | tr -d '"[:space:]')"
echo "      rollup balance before settle: ${RBAL_BEFORE} (expect ${SUM_AMOUNTS})"

# ── 3. Start seq_demo_http; wait for /config to echo the fresh rollup ──────────
echo ""
echo "[3/7] start seq_demo_http (FixtureProver + REAL StellarCliSettler)"
SEQ_LOG="${ROOT}/out/seq/seq_demo_http.gate.log"
mkdir -p "$(dirname "${SEQ_LOG}")"
ROLLUP_ID="${ROLLUP_ID}" SIGNER="${SIGNER}" NETWORK="${NETWORK}" \
  N_TARGET="${N_TARGET}" FIXTURE_PROVE_DELAY="${FIXTURE_PROVE_DELAY}" BIND="${BIND}" \
  "${SEQ_HTTP}" >"${SEQ_LOG}" 2>&1 &
SEQ_PID=$!
echo "      seq_demo_http pid ${SEQ_PID}, log ${SEQ_LOG}"

# Wait (≤20s) for /config to respond AND echo the fresh rollup id.
CONFIG_OK=""
for _ in $(seq 1 40); do
  CFG="$(curl -s "${API}/config" 2>/dev/null || true)"
  if [[ -n "${CFG}" ]]; then
    CFG_ROLLUP="$(echo "${CFG}" | python3 -c "import sys,json;print(json.load(sys.stdin).get('rollup_id',''))" 2>/dev/null || true)"
    if [[ "${CFG_ROLLUP}" == "${ROLLUP_ID}" ]]; then CONFIG_OK=1; break; fi
  fi
  kill -0 "${SEQ_PID}" 2>/dev/null || { echo "ERROR: seq_demo_http died on startup" >&2; cat "${SEQ_LOG}" >&2; exit 1; }
  sleep 0.5
done
[[ -n "${CONFIG_OK}" ]] || { echo "ERROR: /config never echoed ${ROLLUP_ID}" >&2; cat "${SEQ_LOG}" >&2; exit 1; }
echo "      GET /config OK → rollup_id matches the fresh deploy"
echo "      ${CFG}"

# ── 4. POST 8 distinct-nullifier intents sharing one root (golden n8) ──────────
echo ""
echo "[4/7] POST 8 withdrawal intents (8 distinct nullifiers, ONE shared root)"
# Emit the 8 submit bodies from golden/n8_inputs.json (distinct secrets/blindings
# ⇒ 8 distinct nullifiers; all share merkle_root_le). The FixtureProver ignores
# the GuestInput and returns the real N=8 receipt — the intents only drive the
# sequencer's assembly (batch of 8) and the API state machine, exactly as the FE.
BODIES_DIR="${ROOT}/out/seq/http_gate_bodies"
mkdir -p "${BODIES_DIR}"
python3 - "${ROOT}/golden/n8_inputs.json" "${BODIES_DIR}" <<'PY'
import json, sys
g = json.load(open(sys.argv[1])); outdir = sys.argv[2]
root = g["merkle_root_le"]
for i, nt in enumerate(g["notes"]):
    body = {
        "secret": nt["secret_le"], "blinding": nt["blinding_le"],
        "amount": nt["amount"], "recipient": nt["recipient"],
        "path": nt["path_le"], "index": nt["index"], "merkle_root": root,
    }
    json.dump(body, open(f"{outdir}/body{i}.json", "w"))
print(f"wrote 8 bodies (root {root[:18]}…)")
PY

FIRST_ID=""
for i in 0 1 2 3 4 5 6 7; do
  START_MS=$(python3 -c "import time;print(int(time.time()*1000))")
  RESP="$(curl -s -X POST "${API}/submit" -H 'content-type: application/json' --data @"${BODIES_DIR}/body${i}.json")"
  END_MS=$(python3 -c "import time;print(int(time.time()*1000))")
  RID="$(echo "${RESP}" | python3 -c "import sys,json;print(json.load(sys.stdin).get('request_id',''))" 2>/dev/null || true)"
  [[ -n "${RID}" ]] || { echo "ERROR: submit ${i} returned no request_id: ${RESP}" >&2; exit 1; }
  if [[ -z "${FIRST_ID}" ]]; then
    FIRST_ID="${RID}"
    DT=$(( END_MS - START_MS ))
    echo "      submit[0] → ${RID} in ${DT}ms"
    [[ "${DT}" -lt 2000 ]] || { echo "ERROR: first submit took ${DT}ms (must be <2000ms)" >&2; exit 1; }
  fi
done
echo "      8 intents submitted; tracking ${FIRST_ID}"

# ── 5. Poll GET /status/:id until settled (or failed → dump + fail) ────────────
echo ""
echo "[5/7] poll GET /status/${FIRST_ID} until settled"
TX_HASH=""; LAST_STATE=""
# Generous budget: FIXTURE_PROVE_DELAY + settle (RPC) latency.
DEADLINE=$(( $(date +%s) + FIXTURE_PROVE_DELAY + 180 ))
while [[ "$(date +%s)" -lt "${DEADLINE}" ]]; do
  S="$(curl -s "${API}/status/${FIRST_ID}" 2>/dev/null || true)"
  STATE="$(echo "${S}" | python3 -c "import sys,json;print(json.load(sys.stdin).get('state',''))" 2>/dev/null || true)"
  if [[ "${STATE}" != "${LAST_STATE}" && -n "${STATE}" ]]; then
    PHASE="$(echo "${S}" | python3 -c "import sys,json;print(json.load(sys.stdin).get('prover_phase',''))" 2>/dev/null || true)"
    echo "      state=${STATE}${PHASE:+ (phase ${PHASE})}"
    LAST_STATE="${STATE}"
  fi
  case "${STATE}" in
    settled)
      TX_HASH="$(echo "${S}" | python3 -c "import sys,json;print(json.load(sys.stdin).get('tx_hash',''))" 2>/dev/null || true)"
      break;;
    failed)
      REASON="$(echo "${S}" | python3 -c "import sys,json;print(json.load(sys.stdin).get('reason',''))" 2>/dev/null || true)"
      echo "ERROR: request FAILED — reason: ${REASON}" >&2
      echo "--- seq_demo_http log tail ---" >&2; tail -40 "${SEQ_LOG}" >&2
      exit 1;;
  esac
  sleep 1
done
[[ -n "${TX_HASH}" ]] || { echo "ERROR: never reached settled before deadline" >&2; tail -40 "${SEQ_LOG}" >&2; exit 1; }
echo "      SETTLED with tx_hash=${TX_HASH}"

# ── 6. Assert the tx hash is REAL (≠ image_id) + RPC SUCCESS + recent_batches ──
echo ""
echo "[6/7] assert the tx hash is REAL (the parse_tx_hash fix), not the image_id"
if [[ "${TX_HASH}" == "${IMAGE_ID}" ]]; then
  echo "ERROR: tx_hash == image_id ${IMAGE_ID} — REGRESSION (parse_tx_hash bug)" >&2
  exit 1
fi
echo "      tx_hash != image_id ✓ (${TX_HASH:0:12}… != ${IMAGE_ID:0:12}…)"

# Resolve it on-chain: RPC getTransaction → SUCCESS (real settle on testnet).
STATUS=""
for _ in $(seq 1 30); do
  RESP="$(curl -s -X POST "${RPC_URL}" -H 'Content-Type: application/json' \
    -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"getTransaction\",\"params\":{\"hash\":\"${TX_HASH}\"}}")"
  STATUS="$(echo "${RESP}" | python3 -c "import sys,json;print(json.load(sys.stdin).get('result',{}).get('status','UNKNOWN'))" 2>/dev/null || echo PARSE_ERROR)"
  [[ "${STATUS}" == "SUCCESS" || "${STATUS}" == "FAILED" ]] && break
  sleep 2
done
echo "      RPC getTransaction status: ${STATUS}"
[[ "${STATUS}" == "SUCCESS" ]] || { echo "ERROR: settle tx not SUCCESS on testnet (${STATUS})" >&2; exit 1; }
EXPLORER="https://stellar.expert/explorer/${NETWORK}/tx/${TX_HASH}"
echo "      explorer: ${EXPLORER}"

# rollup drained (8 transfers executed) — corroborates the real settle.
RBAL_AFTER="$(stellar contract invoke --id "${TOKEN}" --source "${SIGNER}" --network "${NETWORK}" -- balance --id "${ROLLUP_ID}" 2>&1 | tail -1 | tr -d '"[:space:]')"
echo "      rollup balance after settle: ${RBAL_AFTER} (expect 0 → 8 withdrawals paid)"
[[ "${RBAL_AFTER}" == "0" ]] || { echo "ERROR: rollup not drained (${RBAL_AFTER} != 0)" >&2; exit 1; }

# /recent_batches shows the real tx with n:8, most-recent-first, NOT a cbeab7aa entry.
echo "      GET /recent_batches — assert the real settle is row 0 with n:8, no image_id"
RB="$(curl -s "${API}/recent_batches")"
python3 - "${TX_HASH}" "${IMAGE_ID}" <<PY
import sys, json
rb = json.loads('''${RB}''')
tx, img = sys.argv[1], sys.argv[2]
assert isinstance(rb, list) and rb, "recent_batches empty"
top = rb[0]
assert top["tx_hash"] == tx, f"row0 tx_hash {top['tx_hash']} != settle {tx}"
assert top["n"] == 8, f"row0 n {top['n']} != 8"
assert top["explorer_url"].endswith(tx), "row0 explorer_url mismatch"
assert all(b["tx_hash"] != img for b in rb), "image_id appears in recent_batches!"
print(f"      recent_batches[0] = {{tx_hash: {tx[:14]}…, n: 8}}  (no cbeab7aa entry) ✓")
print(f"      recent_batches has {len(rb)} rows (real settle + seeded historic)")
PY

# ── 7. GATE_PASS ───────────────────────────────────────────────────────────────
echo ""
echo "[7/7] ============================================================"
echo "  POOL_ID    = ${POOL_ID}"
echo "  ROLLUP_ID  = ${ROLLUP_ID}"
echo "  SETTLE_TX  = ${TX_HASH}  (RPC status SUCCESS)"
echo "  image_id   = ${IMAGE_ID}  (tx_hash is NOT this — the fix holds)"
echo "  explorer   : ${EXPLORER}"
echo "  verifier   : https://stellar.expert/explorer/${NETWORK}/contract/${VERIFIER}"
echo "  frontend → HTTP API → FixtureProver(real N=8 receipt) → REAL settle : OK"
echo "  async UX  : first /submit <2s, /status held proving, /recent_batches real"
echo "============================================================"
echo "GATE_PASS ${TX_HASH}"
