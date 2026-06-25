#!/usr/bin/env bash
# Runs INSIDE nvidia/cuda:12.4.1-devel-ubuntu22.04 (launched by worker/build-host.sh)
# with --gpus all + the host Docker socket + the repo mounted at the same path.
# Builds the PRODUCTION host (image_id cbeab7aa) with NATIVE CUDA kernels for the
# VM's GPU arch (PARADA-1 style — GPU present), then EXECUTES a real validation
# prove on that GPU and refuses to ship unless it verifies. NO arch hack.
set -euo pipefail
export DEBIAN_FRONTEND=noninteractive
: "${EXPECTED_IMAGE_ID:?}"

apt-get update -y && apt-get install -y --no-install-recommends \
  build-essential libssl-dev pkg-config curl git ca-certificates \
  protobuf-compiler clang libclang-dev llvm-dev docker.io
export LIBCLANG_PATH=/usr/lib/llvm-14/lib
ls "$LIBCLANG_PATH"/libclang*.so* >/dev/null

# the GPU must be visible in here (the outer `docker run --gpus all` provides it),
# so -arch=native compiles SASS for THIS GPU's arch — the same arch we validate on.
nvidia-smi --query-gpu=name,compute_cap --format=csv,noheader

curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
. "$HOME/.cargo/env"
curl -L https://risczero.com/install | bash
export PATH="$HOME/.risc0/bin:$PATH"
rzup install rust && rzup install cargo-risczero 3.0.5 && rzup install r0vm 3.0.5 && rzup install risc0-groth16

git config --global --add safe.directory "$PWD" || true
sed -i 's#risc0-zkvm\( *\)= { workspace = true }#risc0-zkvm\1= { workspace = true, features = ["cuda"] }#' host/Cargo.toml
grep -q 'features = \["cuda"\]' host/Cargo.toml
cargo fetch
# Only sppark needs pinning (0.1.15 → illegal memory access). NOT blst: sppark doesn't
# use it, and c-kzg (via risc0-ethereum-contracts/encode_seal) requires blst ^0.3.16.
cargo update -p sppark --precise 0.1.12
grep -A1 'name = "sppark"' Cargo.lock | grep -q '"0.1.12"'
grep -A1 'name = "blst"' Cargo.lock | grep -qE '"0\.3\.(1[6-9]|[2-9][0-9])"' || { echo "blst < 0.3.16 — c-kzg would reject"; exit 1; }

# NO ROLLUP_LOCAL_GUEST → r0.1.88.0 Docker guest build → image_id cbeab7aa.
# -arch=native (GPU present) → SASS for this GPU's compute capability.
cargo build --release -p host

# (1) verify the EMBEDDED guest image_id (fast, no proving)
IMG="$(./target/release/host execute --inputs golden/n8_inputs.json 2>&1 \
        | grep -i 'guest image-id' | grep -oiE '[a-f0-9]{64}' | head -1 || true)"
[ "$IMG" = "$EXPECTED_IMAGE_ID" ] \
  || { echo "FATAL: guest image-id $IMG != $EXPECTED_IMAGE_ID — aborting."; exit 1; }
echo "OK: guest image-id == $EXPECTED_IMAGE_ID"

# (2) VALIDATION PROVE — actually EXECUTE a real N=8 Groth16 prove on THIS GPU
# (RISC0_DEV_MODE=0). This proves the CUDA kernels WORK, not just compile. Ship
# ONLY if: a real seal (≠ ffffffff), receipt.verify(image_id) OK, image_id cbeab7aa.
echo "VALIDATION PROVE (real N=8, RISC0_DEV_MODE=0, on this GPU) — ~5 min ..."
rm -rf /tmp/val && mkdir -p /tmp/val
RISC0_DEV_MODE=0 ./target/release/host prove --inputs golden/n8_inputs.json --out /tmp/val 2>&1 | tee /tmp/val.log
grep -q 'receipt.verify(image_id): OK' /tmp/val.log \
  || { echo "FATAL: validation prove did NOT verify on this GPU. NOT shipping a broken image."; exit 1; }
VSEAL="$(cut -c1-8 /tmp/val/seal.hex)"
[ "$VSEAL" != "ffffffff" ] \
  || { echo "FATAL: dev-mode seal from the validation prove. NOT shipping."; exit 1; }
VIMG="$(cat /tmp/val/image_id.hex)"
[ "$VIMG" = "$EXPECTED_IMAGE_ID" ] \
  || { echo "FATAL: validation prove image_id $VIMG != $EXPECTED_IMAGE_ID. NOT shipping."; exit 1; }
echo "OK: VALIDATION PROVE PASSED — real Groth16 seal ($VSEAL…), receipt.verify OK, image_id cbeab7aa."
echo "    → the kernels PROVE correctly on this GPU arch; production must use the SAME arch (single-arch endpoint category)."

cp target/release/host worker/dist/host
cp -r "$HOME/.risc0" worker/dist/risc0-home
git checkout -- host/Cargo.toml Cargo.lock 2>/dev/null || true
