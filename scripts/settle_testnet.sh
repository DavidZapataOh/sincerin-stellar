#!/usr/bin/env bash
# settle_testnet.sh — s2/03 PART C: submit the N=2 batch to the rollup on testnet.
#
# ELIMINATORIA: executes a REAL on-chain tx (--send=yes). The whole settle is
# atomic (AC4.2): rollup verifies the Groth16 receipt → asserts root ∈ pool →
# marks BOTH nullifiers → transfers BOTH payouts, or the entire tx reverts.
#
# DO NOT run this until the receipt exists (out/receipt/{seal.hex,image_id.hex,
# journal.bin}) — i.e. after the controller's `make prove`. PART A leaves this
# script in place but does NOT invoke it (there is no receipt yet).
#
# Verifies, via RPC + balances, that the gate holds:
#   - settle tx status == SUCCESS;
#   - both recipients credited by exactly their payout amounts;
#   - replay of the SAME batch reverts (both nullifiers now spent).
# Prints the tx hash + explorer link. Exits non-zero on any failure.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
RECEIPT_DIR="${ROOT}/out/receipt"
DEPLOY="${ROOT}/deployments/testnet.json"

NETWORK="testnet"
SIGNER="spikekey"
RPC_URL="https://soroban-testnet.stellar.org"

jqget() { python3 -c "import sys,json;print(json.load(open('${DEPLOY}'))${1})"; }

ROLLUP="$(jqget "['rollup']")"
TOKEN="$(jqget "['token']")"
RECIP_A="$(jqget "['recipients']['note0_leaf0']['G']")"
RECIP_B="$(jqget "['recipients']['note1_leaf1']['G']")"
AMT_A="$(jqget "['recipients']['note0_leaf0']['amount']")"
AMT_B="$(jqget "['recipients']['note1_leaf1']['amount']")"
SUM_AMT=$(( AMT_A + AMT_B ))

echo "=== s2/03 PART C — settle N=2 on ${NETWORK} ==="
echo "rollup:     ${ROLLUP}"
echo "recipientA: ${RECIP_A} (+${AMT_A})"
echo "recipientB: ${RECIP_B} (+${AMT_B})"
echo "rollup debit expected: -${SUM_AMT}"
echo ""

for f in seal.hex image_id.hex journal.bin; do
  [[ -f "${RECEIPT_DIR}/${f}" ]] || { echo "ERROR: ${RECEIPT_DIR}/${f} not found — run \`make prove\` first." >&2; exit 1; }
done

SEAL_HEX="$(tr -d '[:space:]' < "${RECEIPT_DIR}/seal.hex")"
IMAGE_ID_HEX="$(tr -d '[:space:]' < "${RECEIPT_DIR}/image_id.hex")"
JOURNAL_HEX="$(xxd -p "${RECEIPT_DIR}/journal.bin" | tr -d '[:space:]')"

echo "seal[0..8]:  ${SEAL_HEX:0:16}..."
echo "image_id:    ${IMAGE_ID_HEX}"
echo "journal len: $(( ${#JOURNAL_HEX} / 2 )) bytes"

# Extract the N nullifiers from the journal (layout: root 32B ‖ N u32-LE ‖ N×32B
# nullifiers ‖ N×(recip 32B ‖ amount 16B)). These are the values the rollup marks
# spent; we print them and (after settle) prove the replay reverts on them.
read -r NF0 NF1 <<<"$(python3 - "${RECEIPT_DIR}/journal.bin" <<'PY'
import sys,struct
d=open(sys.argv[1],'rb').read()
n=struct.unpack_from('<I',d,32)[0]
print(*[d[36+i*32:36+(i+1)*32].hex() for i in range(n)])
PY
)"
echo "nullifier[0]: ${NF0}"
echo "nullifier[1]: ${NF1}"
echo ""

# Recipient + rollup balances BEFORE (stroops).
bal() { stellar contract invoke --id "${TOKEN}" --source "${SIGNER}" --network "${NETWORK}" \
  -- balance --id "$1" 2>/dev/null | tail -1 | tr -d '"[:space:]'; }
A0="$(bal "${RECIP_A}")"; B0="$(bal "${RECIP_B}")"; R0="$(bal "${ROLLUP}")"
echo "balances before: A=${A0}  B=${B0}  rollup=${R0}"

# ── Submit settle_batch (REAL on-chain tx) ────────────────────────────────────
echo "submitting settle_batch (--send=yes) ..."
OUT="$(stellar contract invoke \
  --id "${ROLLUP}" --source "${SIGNER}" --network "${NETWORK}" --send=yes \
  -- settle_batch \
  --seal "${SEAL_HEX}" \
  --image_id "${IMAGE_ID_HEX}" \
  --journal_bytes "${JOURNAL_HEX}" 2>&1)" || { echo "ERROR: settle invoke failed" >&2; echo "${OUT}" >&2; exit 1; }
echo "${OUT}"

TX_HASH="$(echo "${OUT}" | grep -oE '[a-fA-F0-9]{64}' | head -1 || true)"
[[ -n "${TX_HASH}" ]] || { echo "ERROR: no tx hash in output" >&2; exit 1; }

RESP="$(curl -s -X POST "${RPC_URL}" -H 'Content-Type: application/json' \
  -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"getTransaction\",\"params\":{\"hash\":\"${TX_HASH}\"}}")"
STATUS="$(echo "${RESP}" | python3 -c "import sys,json;print(json.load(sys.stdin).get('result',{}).get('status','UNKNOWN'))" 2>/dev/null || echo PARSE_ERROR)"
echo "settle tx status: ${STATUS}"
[[ "${STATUS}" == "SUCCESS" ]] || { echo "ERROR: settle did not SUCCEED (${STATUS})" >&2; exit 1; }

# ── Verify recipients credited + rollup debited by exactly the amounts ────────
A1="$(bal "${RECIP_A}")"; B1="$(bal "${RECIP_B}")"; R1="$(bal "${ROLLUP}")"
echo "balances after:  A=${A1}  B=${B1}  rollup=${R1}"
[[ "$(( A1 - A0 ))" == "${AMT_A}" ]] || { echo "ERROR: recipient A delta $(( A1 - A0 )) != ${AMT_A}" >&2; exit 1; }
[[ "$(( B1 - B0 ))" == "${AMT_B}" ]] || { echo "ERROR: recipient B delta $(( B1 - B0 )) != ${AMT_B}" >&2; exit 1; }
# Rollup pays both withdrawals from its own balance: it must drop by exactly the sum.
[[ "$(( R0 - R1 ))" == "${SUM_AMT}" ]] || { echo "ERROR: rollup debit $(( R0 - R1 )) != ${SUM_AMT}" >&2; exit 1; }
echo "payouts credited exactly: A +${AMT_A}, B +${AMT_B}; rollup -${SUM_AMT} ✅"

# ── Verify double-spend protection: replaying the SAME batch must revert ───────
echo "replay check (same batch must revert — nullifiers now spent) ..."
if stellar contract invoke \
  --id "${ROLLUP}" --source "${SIGNER}" --network "${NETWORK}" --send=yes \
  -- settle_batch --seal "${SEAL_HEX}" --image_id "${IMAGE_ID_HEX}" --journal_bytes "${JOURNAL_HEX}" >/dev/null 2>&1; then
  echo "ERROR: replay SUCCEEDED — nullifiers not marked spent (double-spend hole!)" >&2
  exit 1
fi
echo "replay reverted as expected (both nullifiers spent) ✅"

echo ""
echo "======================================================"
echo "SETTLE N=2: SUCCESS"
echo "tx hash:    ${TX_HASH}"
echo "explorer:   https://stellar.expert/explorer/testnet/tx/${TX_HASH}"
echo "rollup:     ${ROLLUP}"
echo "nullifiers: ${NF0}"
echo "            ${NF1}  (both SPENT; replay reverted)"
echo "deltas:     A +${AMT_A}  B +${AMT_B}  rollup -${SUM_AMT}"
echo "======================================================"
echo GATE_OK
