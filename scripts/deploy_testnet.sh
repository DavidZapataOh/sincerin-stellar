#!/usr/bin/env bash
# deploy_testnet.sh — s2/03 PART A: deploy pool + rollup to Stellar testnet and
# seed the pool so its Merkle root equals the zkVM guest's expected merkle_root.
#
# Reproducible from a clean checkout (given a funded `spikekey` + the two built
# wasms). Writes all contract ids to deployments/testnet.json.
#
# Pipeline:
#   1. upload+deploy the PoC pool (pool.wasm, levels=3) with the XLM SAC as token.
#   2. SEED the pool: reconstruct the FULL golden depth-3 tree via 4×
#      seed_two_leaves so get_last_root() == golden merkle_root.
#      (note0@0, note1@1, note2@3, note3@6; remaining leaves = pool zero leaf.)
#      NOTE: the s1/04 receipt only AGGREGATES note0+note1, but the root it is
#      proven against is the FULL 4-note golden tree's root — so all 4 golden
#      commitments must be planted or the on-chain root will not match (verified
#      locally: seeding only note0+note1 yields a different root).
#   3. confirm on-chain get_root() == expected golden root (BE U256). STOP if not.
#   4. deploy the rollup (__constructor: verifier, pool, token=XLM SAC).
#   5. fund the rollup with >= sum(amounts) = 1_000_042 stroops of XLM.
#
# Does NOT run `make prove` (controller does that) and does NOT settle (PART C).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
POOL_REPO="/Users/david/Projects/sincerin-stellar/stellar-private-payments"
# Prefer the vendored pool.wasm (self-contained in this worktree); fall back to a
# fresh build in the PoC repo. The vendored wasm is built from the PoC pool with
# the admin-gated `seed_two_leaves` entrypoint added (see
# deployments/artifacts/pool-seed_two_leaves.patch) — the Merkle/Poseidon2 logic
# is the unmodified PoC code, so on-chain roots stay byte-identical to the guest.
POOL_WASM="${ROOT}/deployments/artifacts/pool.wasm"
[[ -f "${POOL_WASM}" ]] || POOL_WASM="${POOL_REPO}/target/wasm32v1-none/release/pool.wasm"
ROLLUP_WASM="${ROOT}/target/wasm32v1-none/release/rollup.wasm"
OUT="${ROOT}/deployments/testnet.json"

NETWORK="testnet"
SIGNER="spikekey"   # funder + pool admin + rollup deployer

# Pre-confirmed deployed RISC Zero verifier (s1/05). Do NOT redeploy.
VERIFIER="CBQFQLSBYXUYLD2Q5EWHVNNI6VO33NAVRDUDIGJNMC5TUAINK5BXO2LJ"
# Native XLM Stellar Asset Contract (SAC) on testnet.
TOKEN="CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC"

# ── Pool tree parameters (MUST equal guest TREE_DEPTH=3) ──────────────────────
LEVELS=3
MAX_DEPOSIT=1000000000000   # generous cap; unused by seed_two_leaves
# Golden commitments as U256 field-element decimals (= int(commitment_le, LE)).
ZERO_LEAF=16820622405745174042249830601237189755928192602553897283642901160942722677198
C0=5434317342687061651116115748844809812731476864863865128804760412168769371349   # leaf 0 (note0)
C1=18000051224769994697789132266280444351182659227408604649257086024028987495333  # leaf 1 (note1)
C2=9904632811179042992553492644800165599780960913068989983029745245705980139351   # leaf 3 (note2)
C3=5384613036826241196188144739208061787082658193535111934314871654089459874292   # leaf 6 (note3)
# Expected golden root as BE U256 decimal (= int(merkle_root_le, LE)). What the
# pool returns from get_root() as a numeric U256.
EXPECTED_ROOT_DEC=11367819772409864252604588583363811900671116702745926458574914206832615695518
SUM_AMOUNTS=1000042  # note0(1_000_000) + note1(42), stroops to fund the rollup.

ADMIN="$(stellar keys address "${SIGNER}")"

echo "=== s2/03 PART A — deploy + seed on ${NETWORK} ==="
echo "signer/admin: ${SIGNER} = ${ADMIN}"
echo "verifier:     ${VERIFIER} (reused, s1/05)"
echo "token (XLM):  ${TOKEN}"
echo "pool wasm:    ${POOL_WASM}"
echo "rollup wasm:  ${ROLLUP_WASM}"
echo ""

[[ -f "${POOL_WASM}"   ]] || { echo "ERROR: pool.wasm missing — run: (cd ${POOL_REPO} && stellar contract build --package pool)" >&2; exit 1; }
[[ -f "${ROLLUP_WASM}" ]] || { echo "ERROR: rollup.wasm missing — run: (cd ${ROOT} && stellar contract build --package rollup)" >&2; exit 1; }

# ── 1. Deploy the pool ────────────────────────────────────────────────────────
# ASP membership/non-membership addresses are STORED but never dereferenced in the
# seed/root-read/settle path (only transact()/get_asp_*_root() call them). We pass
# the admin account address as an inert placeholder for both — keeps the demo from
# depositing the ASP contracts it does not exercise.
echo "[1/5] deploying pool (levels=${LEVELS}) ..."
POOL_ID="$(stellar contract deploy \
  --wasm "${POOL_WASM}" \
  --source "${SIGNER}" --network "${NETWORK}" \
  -- \
  --admin "${ADMIN}" \
  --token "${TOKEN}" \
  --verifier "${VERIFIER}" \
  --asp_membership "${ADMIN}" \
  --asp_non_membership "${ADMIN}" \
  --maximum_deposit_amount "${MAX_DEPOSIT}" \
  --levels "${LEVELS}" 2>&1 | tail -1)"
echo "      POOL_ID=${POOL_ID}"
case "${POOL_ID}" in C*) ;; *) echo "ERROR: pool deploy did not return a C... id" >&2; exit 1;; esac

# ── 2. Seed the FULL golden tree (4 × seed_two_leaves) ────────────────────────
echo "[2/5] seeding golden tree (4 pair-inserts) ..."
seed() { # $1 left $2 right  (pair at leaves NextIndex, NextIndex+1)
  stellar contract invoke --id "${POOL_ID}" --source "${SIGNER}" --network "${NETWORK}" \
    --send=yes -- seed_two_leaves --leaf_1 "$1" --leaf_2 "$2" >/dev/null
}
seed "${C0}"        "${C1}"        # leaves 0,1 = note0, note1
seed "${ZERO_LEAF}" "${C2}"        # leaves 2,3 = zero, note2
seed "${ZERO_LEAF}" "${ZERO_LEAF}" # leaves 4,5 = zero, zero
seed "${C3}"        "${ZERO_LEAF}" # leaves 6,7 = note3, zero
echo "      seeded 8 leaves (4 pairs)."

# ── 3. Confirm on-chain root == expected golden root ──────────────────────────
echo "[3/5] reading on-chain pool root ..."
ROOT_OUT="$(stellar contract invoke --id "${POOL_ID}" --source "${SIGNER}" --network "${NETWORK}" \
  -- get_root 2>&1 | tail -1)"
# get_root returns a U256 JSON-encoded as a decimal string (quoted).
ROOT_DEC="$(echo "${ROOT_OUT}" | tr -d '"[:space:]')"
echo "      on-chain root (U256 dec): ${ROOT_DEC}"
echo "      expected root (U256 dec): ${EXPECTED_ROOT_DEC}"
if [[ "${ROOT_DEC}" != "${EXPECTED_ROOT_DEC}" ]]; then
  echo "" >&2
  echo "ROOT MISMATCH — STOP. Pool Poseidon2/endianness does NOT agree with the guest." >&2
  echo "  on-chain: ${ROOT_DEC}" >&2
  echo "  expected: ${EXPECTED_ROOT_DEC}" >&2
  exit 2
fi
echo "      ROOT MATCH: YES ✅ (pool Poseidon2 + endianness agree with the guest)"

# ── 4. Deploy the rollup ──────────────────────────────────────────────────────
echo "[4/5] deploying rollup ..."
ROLLUP_ID="$(stellar contract deploy \
  --wasm "${ROLLUP_WASM}" \
  --source "${SIGNER}" --network "${NETWORK}" \
  -- \
  --verifier "${VERIFIER}" \
  --pool "${POOL_ID}" \
  --token "${TOKEN}" 2>&1 | tail -1)"
echo "      ROLLUP_ID=${ROLLUP_ID}"
case "${ROLLUP_ID}" in C*) ;; *) echo "ERROR: rollup deploy did not return a C... id" >&2; exit 1;; esac

# ── 5. Fund the rollup with the payout total ──────────────────────────────────
# Transfer SUM_AMOUNTS stroops of XLM from the signer to the rollup contract so it
# can pay the two withdrawals. The rollup's current contract address is the C-id.
echo "[5/5] funding rollup with ${SUM_AMOUNTS} stroops XLM ..."
stellar contract invoke --id "${TOKEN}" --source "${SIGNER}" --network "${NETWORK}" \
  --send=yes -- transfer --from "${ADMIN}" --to "${ROLLUP_ID}" --amount "${SUM_AMOUNTS}" >/dev/null
RBAL="$(stellar contract invoke --id "${TOKEN}" --source "${SIGNER}" --network "${NETWORK}" \
  -- balance --id "${ROLLUP_ID}" 2>&1 | tail -1 | tr -d '"[:space:]')"
echo "      rollup XLM balance (stroops): ${RBAL}"

# ── Write deployments/testnet.json ────────────────────────────────────────────
cat > "${OUT}" <<JSON
{
  "network": "${NETWORK}",
  "deployed_by": "${SIGNER}",
  "admin": "${ADMIN}",
  "verifier": "${VERIFIER}",
  "token": "${TOKEN}",
  "token_note": "native XLM Stellar Asset Contract (SAC) on testnet",
  "pool": "${POOL_ID}",
  "pool_levels": ${LEVELS},
  "rollup": "${ROLLUP_ID}",
  "rollup_funded_stroops": "${RBAL}",
  "expected_merkle_root_le": "9e24c3e7b5c329b34a58f05a9840a90f051d6e5c97833c1d356f81323ef52119",
  "expected_merkle_root_u256_dec": "${EXPECTED_ROOT_DEC}",
  "onchain_root_match": "YES",
  "recipients": {
    "note0_leaf0": { "key": "lateo-agent-a", "G": "GB5MVC4HEWWBRF7TE3DVVS5F5K7EBJ37UMPKNDGXLL37SDTHLBIBINOL", "ed25519_hex": "7aca8b8725ac1897f326c75acba5eabe40a77fa31ea68cd75af7f90e67585014", "amount": 1000000 },
    "note1_leaf1": { "key": "lateo-agent-b", "G": "GCQUVGH54FBFO5PWI5FUK3FDCALUTS334JZIDKAZIRWAFJK7HCSAH5DF", "ed25519_hex": "a14a98fde1425775f6474b456ca3101749cb7be27281a819446c02a55f38a403", "amount": 42 }
  }
}
JSON
echo ""
echo "=== DONE — wrote ${OUT} ==="
cat "${OUT}"
