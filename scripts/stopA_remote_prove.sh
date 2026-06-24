#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# stopA_remote_prove.sh — Stop A: submit ONE real N=8 prove to the RunPod
# serverless worker and verify it, WITHOUT a local GPU (this only calls the
# endpoint). Run AFTER the endpoint exists (guardrails confirmed) and the image
# is baked. It triggers ONE real GPU prove (~$0.10 on a 24GB worker).
#
# Verifies (the Stop-A acceptance criteria):
#   - which GPU ran           (3090 = direct PARADA-1 comparison; A5000/L4 = sibling)
#   - prove wall-clock ≈ 5m04s (PARADA-1, 304s)
#   - seal selector ≠ ffffffff (REAL Groth16, never dev-mode)
#   - seal length > 64 bytes
#   - image_id == the canonical deployed guest (byte-parity of the BAKED guest)
#
#   RUNPOD_ENDPOINT_ID=xxxx RUNPOD_API_KEY=yyyy bash scripts/stopA_remote_prove.sh
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

: "${RUNPOD_ENDPOINT_ID:?set RUNPOD_ENDPOINT_ID (the serverless endpoint id)}"
: "${RUNPOD_API_KEY:?set RUNPOD_API_KEY}"
BASE="${RUNPOD_BASE_URL:-https://api.runpod.ai/v2}"
INPUTS="golden/n8_inputs.json"
# the deployed guest image_id (== PARADA-1's, since methods/ + zk-core are 0-commits
# since e807be1). A baked guest that differs would change this.
CANON_IMAGE_ID="cbeab7aa6ce69944e10cca8c7ed94d15aae297f2580752f07a15c6cab6ba0d46"
PARADA1_SECS=304 # 5m04s on RTX 3090

[ -f "$INPUTS" ] || { echo "ERROR: $INPUTS not found — run from the repo root." >&2; exit 1; }

BODY="$(python3 -c "import json;print(json.dumps({'input':json.load(open('$INPUTS'))}))")"
echo "submitting N=8 prove to RunPod endpoint $RUNPOD_ENDPOINT_ID ..."
RESP="$(curl -s -X POST "$BASE/$RUNPOD_ENDPOINT_ID/run" \
  -H "Authorization: Bearer $RUNPOD_API_KEY" -H 'Content-Type: application/json' -d "$BODY")"
JOB_ID="$(echo "$RESP" | python3 -c "import sys,json;print(json.load(sys.stdin).get('id',''))" 2>/dev/null || true)"
[ -n "$JOB_ID" ] || { echo "ERROR: no job id in /run response: $RESP" >&2; exit 1; }
echo "job: $JOB_ID — polling (a real prove is ~5 min + cold start) ..."

OUT=""
while :; do
  sleep 10
  S="$(curl -s "$BASE/$RUNPOD_ENDPOINT_ID/status/$JOB_ID" -H "Authorization: Bearer $RUNPOD_API_KEY")"
  ST="$(echo "$S" | python3 -c "import sys,json;print(json.load(sys.stdin).get('status','?'))" 2>/dev/null || echo PARSE_ERR)"
  echo "  status: $ST"
  case "$ST" in
    COMPLETED) OUT="$S"; break ;;
    FAILED | CANCELLED | TIMED_OUT) echo "ERROR: job $ST" >&2; echo "$S" >&2; exit 1 ;;
  esac
done

get() { echo "$OUT" | python3 -c "import sys,json;print((json.load(sys.stdin).get('output') or {}).get('$1',''))"; }
GPU="$(get gpu)"; CC="$(get compute_cap)"; PSEC="$(get prove_seconds)"
SEAL="$(get seal_hex)"; IMG="$(get image_id_hex)"
SEL="$(printf '%s' "$SEAL" | cut -c1-8)"; SEAL_BYTES=$(( ${#SEAL} / 2 ))

echo ""
echo "================ STOP A — RESULT ================"
printf 'GPU:                 %s (CC %s)\n' "${GPU:-?}" "${CC:-?}"
printf 'prove wall-clock:    %ss   (PARADA-1: %ss = 5m04s on RTX 3090)\n' "${PSEC:-?}" "$PARADA1_SECS"
printf 'seal selector:       %s   %s\n' "${SEL:-?}" "$([ "$SEL" != ffffffff ] && echo '≠ ffffffff ✓ REAL' || echo 'ffffffff ✗ DEV-MODE')"
printf 'seal length:         %s bytes\n' "$SEAL_BYTES"
printf 'image_id:            %s\n' "${IMG:-?}"
printf 'image_id == canonical guest: %s\n' "$([ "$IMG" = "$CANON_IMAGE_ID" ] && echo 'YES ✓ (byte-parity)' || echo 'NO ✗')"
echo ""

FAIL=0
[ "$SEL" != ffffffff ] || { echo "FAIL: dev-mode seal"; FAIL=1; }
[ "$SEAL_BYTES" -gt 64 ] || { echo "FAIL: seal too short ($SEAL_BYTES B)"; FAIL=1; }
[ "$IMG" = "$CANON_IMAGE_ID" ] || { echo "FAIL: image_id ≠ canonical guest — the BAKED guest differs from PARADA-1/deployed!"; FAIL=1; }
PSEC_INT="${PSEC%.*}"
if [ "${PSEC_INT:-999999}" -le 600 ]; then
  echo "VERDICT: prove ≤10min ✅  (${PSEC}s vs PARADA-1 304s)"
else
  echo "VERDICT: prove >10min ⚠️  — STOP, show David before Stop B"; FAIL=1
fi
case "$GPU" in
  *3090*) echo "GPU = RTX 3090 → direct PARADA-1 comparison." ;;
  *) echo "NOTE: GPU = '${GPU:-?}' (24GB-category sibling, not a 3090) — time may differ from 304s; expected, not a failure." ;;
esac
echo "STOP_A_DONE prove_seconds=${PSEC:-?} gpu='${GPU:-?}' selector=${SEL:-?} image_id_ok=$([ "$IMG" = "$CANON_IMAGE_ID" ] && echo 1 || echo 0)"
exit $FAIL
