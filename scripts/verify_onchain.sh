#!/usr/bin/env bash
# verify_onchain.sh — submit the N=2 Groth16 receipt to the RISC Zero verifier on Stellar testnet.
# ELIMINATORIA: must execute a real on-chain tx (--send=yes), never simulation only.
# Exits 1 on any failure. Prints tx hash + explorer link on success.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
RECEIPT_DIR="${ROOT}/out/receipt"

VERIFIER="CBQFQLSBYXUYLD2Q5EWHVNNI6VO33NAVRDUDIGJNMC5TUAINK5BXO2LJ"
SIGNER="spikekey"
NETWORK="testnet"

EXPECTED_JOURNAL_DIGEST="0ec6ebba7901dd80e9605bac4e2aab5f62344175b2fd4aedc38e070d5bf90e89"

echo "=== s1/05 ELIMINATORIA: on-chain RISC Zero receipt verification ==="
echo "Verifier: ${VERIFIER}"
echo "Signer:   ${SIGNER}"
echo "Network:  ${NETWORK}"
echo ""

# --- Read receipt files ---
if [[ ! -f "${RECEIPT_DIR}/seal.hex" ]]; then
  echo "ERROR: ${RECEIPT_DIR}/seal.hex not found" >&2; exit 1
fi
if [[ ! -f "${RECEIPT_DIR}/image_id.hex" ]]; then
  echo "ERROR: ${RECEIPT_DIR}/image_id.hex not found" >&2; exit 1
fi
if [[ ! -f "${RECEIPT_DIR}/journal.bin" ]]; then
  echo "ERROR: ${RECEIPT_DIR}/journal.bin not found" >&2; exit 1
fi

SEAL_HEX="$(tr -d '[:space:]' < "${RECEIPT_DIR}/seal.hex")"
IMAGE_ID_HEX="$(tr -d '[:space:]' < "${RECEIPT_DIR}/image_id.hex")"

# Compute journal digest (sha256 of raw bytes) — verifier takes the DIGEST not the journal
JOURNAL_DIGEST="$(shasum -a 256 "${RECEIPT_DIR}/journal.bin" | awk '{print $1}')"

echo "seal (first 16 chars): ${SEAL_HEX:0:16}..."
echo "image_id:              ${IMAGE_ID_HEX}"
echo "journal sha256:        ${JOURNAL_DIGEST}"
echo ""

# Assert journal digest matches expected value
if [[ "${JOURNAL_DIGEST}" != "${EXPECTED_JOURNAL_DIGEST}" ]]; then
  echo "ERROR: journal digest mismatch!" >&2
  echo "  expected: ${EXPECTED_JOURNAL_DIGEST}" >&2
  echo "  got:      ${JOURNAL_DIGEST}" >&2
  exit 1
fi
echo "journal digest OK (matches expected)"

# Check seal selector (first 8 hex chars should be 73c457ba)
SEAL_SELECTOR="${SEAL_HEX:0:8}"
echo "seal selector: ${SEAL_SELECTOR} (expected: 73c457ba)"
if [[ "${SEAL_SELECTOR}" != "73c457ba" ]]; then
  echo "ERROR: seal selector mismatch — expected 73c457ba got ${SEAL_SELECTOR}" >&2
  exit 1
fi
echo "seal selector OK"
echo ""

# --- Invoke the verifier on-chain ---
echo "Submitting on-chain tx (--send=yes) ..."
echo "Command: stellar contract invoke --id ${VERIFIER} --source ${SIGNER} --network ${NETWORK} --send=yes -- verify --seal ${SEAL_HEX} --image_id ${IMAGE_ID_HEX} --journal ${JOURNAL_DIGEST}"
echo ""

INVOKE_OUTPUT="$(stellar contract invoke \
  --id "${VERIFIER}" \
  --source "${SIGNER}" \
  --network "${NETWORK}" \
  --send=yes \
  -- verify \
  --seal "${SEAL_HEX}" \
  --image_id "${IMAGE_ID_HEX}" \
  --journal "${JOURNAL_DIGEST}" 2>&1)" || {
  echo "ERROR: stellar contract invoke failed" >&2
  echo "${INVOKE_OUTPUT}" >&2
  exit 1
}

echo "Raw invoke output:"
echo "${INVOKE_OUTPUT}"
echo ""

# Extract tx hash from CLI output
TX_HASH="$(echo "${INVOKE_OUTPUT}" | grep -oE '[a-fA-F0-9]{64}' | head -1 || true)"

if [[ -z "${TX_HASH}" ]]; then
  echo "WARNING: could not extract tx hash from output; checking via RPC..."
fi

# Query RPC for transaction status
RPC_URL="https://soroban-testnet.stellar.org"

if [[ -n "${TX_HASH}" ]]; then
  echo "Querying RPC for tx status: ${TX_HASH}"
  RPC_RESPONSE="$(curl -s -X POST "${RPC_URL}" \
    -H "Content-Type: application/json" \
    -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"getTransaction\",\"params\":{\"hash\":\"${TX_HASH}\"}}")"
  echo "RPC response: ${RPC_RESPONSE}"

  TX_STATUS="$(echo "${RPC_RESPONSE}" | python3 -c "import sys,json; r=json.load(sys.stdin); print(r.get('result',{}).get('status','UNKNOWN'))" 2>/dev/null || echo "PARSE_ERROR")"
  echo ""
  echo "TX status: ${TX_STATUS}"

  if [[ "${TX_STATUS}" != "SUCCESS" ]]; then
    echo ""
    echo "ERROR: Transaction did NOT succeed. Status=${TX_STATUS}" >&2
    echo "RPC response: ${RPC_RESPONSE}" >&2
    exit 1
  fi

  EXPLORER_LINK="https://stellar.expert/explorer/testnet/tx/${TX_HASH}"
  echo ""
  echo "======================================================"
  echo "ON-CHAIN VERIFICATION: SUCCESS"
  echo "TX hash:  ${TX_HASH}"
  echo "Explorer: ${EXPLORER_LINK}"
  echo "Verifier: ${VERIFIER}"
  echo "3.0.5 prover seal VERIFIED on 3.0.0 verifier VK: CONFIRMED"
  echo "======================================================"
else
  echo "ERROR: could not extract tx hash from output" >&2
  echo "Full output: ${INVOKE_OUTPUT}" >&2
  exit 1
fi
