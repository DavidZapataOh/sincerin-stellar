#!/usr/bin/env bash
# seq_http_demo.sh — s3/02 DEMO INTERACTIVO (sin GPU). Igual que seq_http_gate.sh
# pero NO auto-submitea ni sale: despliega un rollup fresco, arranca el sequencer
# (FixtureProver → settle REAL on-chain) + el frontend, y se queda VIVO para que
# juegues en el navegador. Ctrl-C apaga todo.
#
# Uso:   bash scripts/seq_http_demo.sh
# Luego: abre http://localhost:5173
#        - conecta Freighter (testnet), o agrega ?previewAddress=<tu G-address>
#        - "Submit withdrawal" → ~6s arma el batch → proving (~25s) → SETTLED (tx REAL)
#
# CERO MOCKS: verifier real CBQF…, receipt N=8 real, settle on-chain real. La nota
# del juez (FixtureProver) settlea el batch FIJO N=8 (los recipients del fixture);
# el recipient arbitrario del juez es s3/05 (RemoteProver/GPU). El tx es REAL.
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

NETWORK="${NETWORK:-testnet}"; SIGNER="${SIGNER:-spikekey}"
VERIFIER="CBQFQLSBYXUYLD2Q5EWHVNNI6VO33NAVRDUDIGJNMC5TUAINK5BXO2LJ"
TOKEN="CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC"
POOL_WASM="${ROOT}/deployments/artifacts/pool.wasm"
ROLLUP_WASM="${ROOT}/target/wasm32v1-none/release/rollup.wasm"
RECEIPT="${ROOT}/out/bench/n8"
LEVELS=3; MAX_DEPOSIT=1000000000000; SUM_AMOUNTS=8028
N_TARGET=8; BATCH_TIMEOUT="${BATCH_TIMEOUT:-6}"
FIXTURE_PROVE_DELAY="${FIXTURE_PROVE_DELAY:-25}"   # demo: ~25s (no los 5 min reales)
BIND="127.0.0.1:${BIND_PORT:-8799}"; API="http://${BIND}"
FE_PORT="${FE_PORT:-5173}"
ADMIN="$(stellar keys address "${SIGNER}")"
EXPECTED_ROOT_DEC=9477110651562997593162969197308257692712826173766156000136958452708141189893
C=(2098441622598076782746511037081986855819330548846085959436867638769621116777 \
   21850405880430287235981682780826813804754627803832801673792234743332955240539 \
   18045205374491365785174578378606250270069615164305461249410963774719418282513 \
   16178099244925787357733210809213436994899948690551900306934395880707990780509 \
   20815269016325629426393636467174170632206143373393341791705306631012837546302 \
   19189738753259764291719179573442822446694061001295892209456045593398536626564 \
   2694129937400855065380482240116444891444498468902275455052190903817872907992 \
   1685480781901591428795149257982532325864526028206086168943935810355343183620)

SEQ_PID=""; FE_PID=""
cleanup(){ echo; echo "apagando…"; [[ -n "${SEQ_PID}" ]] && kill "${SEQ_PID}" 2>/dev/null||true; [[ -n "${FE_PID}" ]] && kill "${FE_PID}" 2>/dev/null||true; pkill -f 'target/release/seq_demo_http' 2>/dev/null||true; pkill -f vite 2>/dev/null||true; }
trap cleanup EXIT INT TERM

echo "== [1/4] build seq_demo_http + rollup wasm =="
( cd "${ROOT}" && cargo build -p sequencer --features test-fixture --bin seq_demo_http --release >/dev/null 2>&1 )
( cd "${ROOT}" && stellar contract build --package rollup >/dev/null 2>&1 || cargo build -p rollup --release --target wasm32v1-none >/dev/null 2>&1 )
SEQ_HTTP="${ROOT}/target/release/seq_demo_http"

echo "== [2/4] deploy FRESH pool + seed 8 commitments + rollup + fund (testnet) =="
POOL_ID="$(stellar contract deploy --wasm "${POOL_WASM}" --source "${SIGNER}" --network "${NETWORK}" -- \
  --admin "${ADMIN}" --token "${TOKEN}" --verifier "${VERIFIER}" --asp_membership "${ADMIN}" \
  --asp_non_membership "${ADMIN}" --maximum_deposit_amount "${MAX_DEPOSIT}" --levels "${LEVELS}" 2>&1 | tail -1)"
seed(){ stellar contract invoke --id "${POOL_ID}" --source "${SIGNER}" --network "${NETWORK}" --send=yes -- seed_two_leaves --leaf_1 "$1" --leaf_2 "$2" >/dev/null; }
seed "${C[0]}" "${C[1]}"; seed "${C[2]}" "${C[3]}"; seed "${C[4]}" "${C[5]}"; seed "${C[6]}" "${C[7]}"
ROOT_DEC="$(stellar contract invoke --id "${POOL_ID}" --source "${SIGNER}" --network "${NETWORK}" -- get_root 2>&1 | tail -1 | tr -d '"[:space:]')"
[[ "${ROOT_DEC}" == "${EXPECTED_ROOT_DEC}" ]] || { echo "ERROR: root mismatch"; exit 2; }
ROLLUP_ID="$(stellar contract deploy --wasm "${ROLLUP_WASM}" --source "${SIGNER}" --network "${NETWORK}" -- \
  --verifier "${VERIFIER}" --pool "${POOL_ID}" --token "${TOKEN}" 2>&1 | tail -1)"
stellar contract invoke --id "${TOKEN}" --source "${SIGNER}" --network "${NETWORK}" --send=yes -- \
  transfer --from "${ADMIN}" --to "${ROLLUP_ID}" --amount "${SUM_AMOUNTS}" >/dev/null
echo "   pool=${POOL_ID}  rollup=${ROLLUP_ID}  (funded ${SUM_AMOUNTS})"

echo "== [3/4] start sequencer (${API}) + frontend (:${FE_PORT}) =="
mkdir -p "${ROOT}/out/seq"
ROLLUP_ID="${ROLLUP_ID}" SIGNER="${SIGNER}" NETWORK="${NETWORK}" N_TARGET="${N_TARGET}" \
  BATCH_TIMEOUT="${BATCH_TIMEOUT}" FIXTURE_PROVE_DELAY="${FIXTURE_PROVE_DELAY}" BIND="${BIND}" \
  "${SEQ_HTTP}" >"${ROOT}/out/seq/demo.seq.log" 2>&1 & SEQ_PID=$!
for _ in $(seq 1 40); do curl -fsS "${API}/config" >/dev/null 2>&1 && break; sleep 0.5; done
( cd "${ROOT}/frontend" && VITE_SEQUENCER_URL="${API}" npm run dev -- --port "${FE_PORT}" >"${ROOT}/out/seq/demo.fe.log" 2>&1 ) & FE_PID=$!
for _ in $(seq 1 60); do curl -fsS "http://localhost:${FE_PORT}" >/dev/null 2>&1 && break; sleep 0.5; done

echo
echo "== [4/4] LISTO — abre:  http://localhost:${FE_PORT} =="
echo "   • conecta Freighter (testnet)  —o—  http://localhost:${FE_PORT}/?previewAddress=$(stellar keys address "${SIGNER}")"
echo "   • Submit withdrawal → ~${BATCH_TIMEOUT}s batch → proving ~${FIXTURE_PROVE_DELAY}s → SETTLED (tx REAL en stellar.expert)"
echo "   • settle: FixtureProver = batch FIJO N=8 (recipient arbitrario = s3/05/GPU). tx 100% real."
echo
echo "   Ctrl-C para apagar todo."
wait
