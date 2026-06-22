#!/usr/bin/env bash
# deploy_settle_n8.sh — s2/04 headline: deploy a fresh pool+rollup for the N=8
# batch, seed the 8 fresh commitments, and settle the N=8 receipt on testnet.
#
# Safety net: after seeding we assert on-chain get_root() == the receipt's
# merkle_root. If it mismatches we STOP before deploying/funding/settling — so a
# bad commitment can never reach a fund-moving settle.
#
# Values derived from golden/n8_inputs.json (committed): commitments are the
# leaf-0 siblings in each note's path (full 8-leaf tree), root = int(merkle_root_le,LE).
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
NETWORK="testnet"; SIGNER="spikekey"
VERIFIER="CBQFQLSBYXUYLD2Q5EWHVNNI6VO33NAVRDUDIGJNMC5TUAINK5BXO2LJ"
TOKEN="CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC"
POOL_WASM="${ROOT}/deployments/artifacts/pool.wasm"
ROLLUP_WASM="${ROOT}/target/wasm32v1-none/release/rollup.wasm"
RECEIPT="${ROOT}/out/bench/n8"
LEVELS=3; MAX_DEPOSIT=1000000000000
ADMIN="$(stellar keys address "${SIGNER}")"

# N=8 commitments (U256 dec), leaf order 0..7.
C=(2098441622598076782746511037081986855819330548846085959436867638769621116777 \
   21850405880430287235981682780826813804754627803832801673792234743332955240539 \
   18045205374491365785174578378606250270069615164305461249410963774719418282513 \
   16178099244925787357733210809213436994899948690551900306934395880707990780509 \
   20815269016325629426393636467174170632206143373393341791705306631012837546302 \
   19189738753259764291719179573442822446694061001295892209456045593398536626564 \
   2694129937400855065380482240116444891444498468902275455052190903817872907992 \
   1685480781901591428795149257982532325864526028206086168943935810355343183620)
EXPECTED_ROOT_DEC=9477110651562997593162969197308257692712826173766156000136958452708141189893
SUM_AMOUNTS=8028   # 1000+1001+…+1007

echo "=== s2/04 N=8 deploy + settle on ${NETWORK} (admin ${ADMIN}) ==="
[[ -f "${POOL_WASM}" ]] || { echo "ERROR: ${POOL_WASM} missing" >&2; exit 1; }
echo "[build] rollup wasm (pins image_id cbeab7aa…)"; stellar contract build --package rollup >/dev/null 2>&1 || (cd "${ROOT}" && cargo build -p rollup --release --target wasm32v1-none >/dev/null 2>&1)
[[ -f "${ROLLUP_WASM}" ]] || { echo "ERROR: ${ROLLUP_WASM} missing" >&2; exit 1; }
for f in seal.hex image_id.hex journal.bin; do [[ -f "${RECEIPT}/${f}" ]] || { echo "ERROR: ${RECEIPT}/${f} missing" >&2; exit 1; }; done

echo "[1/6] deploy pool (levels=${LEVELS})"
POOL_ID="$(stellar contract deploy --wasm "${POOL_WASM}" --source "${SIGNER}" --network "${NETWORK}" -- \
  --admin "${ADMIN}" --token "${TOKEN}" --verifier "${VERIFIER}" \
  --asp_membership "${ADMIN}" --asp_non_membership "${ADMIN}" \
  --maximum_deposit_amount "${MAX_DEPOSIT}" --levels "${LEVELS}" 2>&1 | tail -1)"
echo "      POOL_ID=${POOL_ID}"; case "${POOL_ID}" in C*) ;; *) echo "ERROR: pool deploy failed" >&2; exit 1;; esac

echo "[2/6] seed 8 commitments (4 pair-inserts)"
seed(){ stellar contract invoke --id "${POOL_ID}" --source "${SIGNER}" --network "${NETWORK}" --send=yes -- seed_two_leaves --leaf_1 "$1" --leaf_2 "$2" >/dev/null; }
seed "${C[0]}" "${C[1]}"; seed "${C[2]}" "${C[3]}"; seed "${C[4]}" "${C[5]}"; seed "${C[6]}" "${C[7]}"
echo "      seeded leaves 0..7"

echo "[3/6] assert on-chain root == receipt root (SAFETY NET)"
ROOT_DEC="$(stellar contract invoke --id "${POOL_ID}" --source "${SIGNER}" --network "${NETWORK}" -- get_root 2>&1 | tail -1 | tr -d '"[:space:]')"
echo "      on-chain: ${ROOT_DEC}"; echo "      expected: ${EXPECTED_ROOT_DEC}"
[[ "${ROOT_DEC}" == "${EXPECTED_ROOT_DEC}" ]] || { echo "ROOT MISMATCH — STOP (no settle)." >&2; exit 2; }
echo "      ROOT MATCH ✅"

echo "[4/6] deploy rollup + fund ${SUM_AMOUNTS} stroops"
ROLLUP_ID="$(stellar contract deploy --wasm "${ROLLUP_WASM}" --source "${SIGNER}" --network "${NETWORK}" -- \
  --verifier "${VERIFIER}" --pool "${POOL_ID}" --token "${TOKEN}" 2>&1 | tail -1)"
echo "      ROLLUP_ID=${ROLLUP_ID}"; case "${ROLLUP_ID}" in C*) ;; *) echo "ERROR: rollup deploy failed" >&2; exit 1;; esac
stellar contract invoke --id "${TOKEN}" --source "${SIGNER}" --network "${NETWORK}" --send=yes -- transfer --from "${ADMIN}" --to "${ROLLUP_ID}" --amount "${SUM_AMOUNTS}" >/dev/null
RBAL_BEFORE="$(stellar contract invoke --id "${TOKEN}" --source "${SIGNER}" --network "${NETWORK}" -- balance --id "${ROLLUP_ID}" 2>&1 | tail -1 | tr -d '"[:space:]')"
echo "      rollup balance before settle: ${RBAL_BEFORE} (expect ${SUM_AMOUNTS})"

echo "[5/6] SETTLE N=8 (real --send=yes)"
SEAL="$(tr -d '[:space:]' < "${RECEIPT}/seal.hex")"
IMG="$(tr -d '[:space:]' < "${RECEIPT}/image_id.hex")"
JOURNAL="$(xxd -p "${RECEIPT}/journal.bin" | tr -d '[:space:]')"
echo "      journal ${#JOURNAL} hex chars ($(( ${#JOURNAL}/2 )) bytes), image_id ${IMG:0:12}…"
SETTLE_OUT="$(stellar contract invoke --id "${ROLLUP_ID}" --source "${SIGNER}" --network "${NETWORK}" --send=yes -- \
  settle_batch --seal "${SEAL}" --image_id "${IMG}" --journal_bytes "${JOURNAL}" 2>&1)"
echo "${SETTLE_OUT}" | grep -oE 'stellar.expert/explorer/testnet/tx/[a-f0-9]+' | head -1
TXH="$(echo "${SETTLE_OUT}" | grep -oE '[a-f0-9]{64}' | head -1)"
echo "      settle tx: ${TXH}"

echo "[6/6] verify effects"
sleep 6 2>/dev/null || true
STATUS="$(curl -s "https://horizon-testnet.stellar.org/transactions/${TXH}" 2>/dev/null | python3 -c "import sys,json;print(json.load(sys.stdin).get('successful'))" 2>/dev/null)"
RBAL_AFTER="$(stellar contract invoke --id "${TOKEN}" --source "${SIGNER}" --network "${NETWORK}" -- balance --id "${ROLLUP_ID}" 2>&1 | tail -1 | tr -d '"[:space:]')"
echo "      tx successful (Horizon): ${STATUS}"
echo "      rollup balance after: ${RBAL_AFTER} (expect 0 → las 8 retiradas pagadas)"
echo ""
echo "POOL_ID=${POOL_ID}"; echo "ROLLUP_ID=${ROLLUP_ID}"; echo "SETTLE_TX=${TXH}"
[[ "${STATUS}" == "True" && "${RBAL_AFTER}" == "0" ]] && echo "GATE_OK" || echo "GATE_FAIL"
