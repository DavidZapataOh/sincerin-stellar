#!/usr/bin/env bash
# Runs INSIDE nvidia/cuda:12.4.1-devel-ubuntu22.04 (launched by worker/build-host.sh)
# with the host Docker socket + the repo mounted at the same path. Builds the
# PRODUCTION host (image_id cbeab7aa) WITHOUT a GPU by forcing a fixed CUDA arch.
set -euo pipefail
export DEBIAN_FRONTEND=noninteractive
: "${EXPECTED_IMAGE_ID:?}"

apt-get update -y && apt-get install -y --no-install-recommends \
  build-essential libssl-dev pkg-config curl git ca-certificates \
  protobuf-compiler clang libclang-dev llvm-dev docker.io
export LIBCLANG_PATH=/usr/lib/llvm-14/lib
ls "$LIBCLANG_PATH"/libclang*.so* >/dev/null

# ── Force a fixed CUDA arch so kernels compile with NO GPU present (-arch=native
#    needs one). Replace nvcc IN-PLACE with a wrapper that rewrites -arch=native →
#    -arch=compute_86 (PTX-only → JITs at runtime on the 24GB serverless category:
#    A5000/3090 sm_86, L4 sm_89). Catches every nvcc call (PATH, $NVCC, absolute).
NVCC_REAL="$(command -v nvcc)"
mv "$NVCC_REAL" "${NVCC_REAL}.real"
cat > "$NVCC_REAL" <<EOF
#!/bin/bash
a=()
for x in "\$@"; do
  case "\$x" in
    -arch=native|--gpu-architecture=native) a+=("-arch=compute_86") ;;
    *) a+=("\$x") ;;
  esac
done
exec "${NVCC_REAL}.real" "\${a[@]}"
EOF
chmod +x "$NVCC_REAL"
echo "nvcc wrapper installed: -arch=native → -arch=compute_86 (no GPU needed to compile)"

# rust + RISC Zero toolchain (cargo-risczero/r0vm 3.0.5 + groth16; r0.1.88.0 guest
# builder + risc0 =3.0.5 are pinned in build.rs / Cargo.toml → image_id cbeab7aa).
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

# NO ROLLUP_LOCAL_GUEST → r0.1.88.0 Docker guest build (nested docker run resolves
# $PWD on the host via the same-path mount) → image_id cbeab7aa.
cargo build --release -p host

# verify the EMBEDDED image_id == the contract-bound one with `host execute`
# (the CPU executor — FAST, no GPU). Abort BEFORE shipping if it differs.
IMG="$(./target/release/host execute --inputs golden/n8_inputs.json 2>&1 \
        | grep -i 'guest image-id' | grep -oiE '[a-f0-9]{64}' | head -1 || true)"
if [ "$IMG" != "$EXPECTED_IMAGE_ID" ]; then
  echo "FATAL: built guest image-id is $IMG, NOT the contract-bound $EXPECTED_IMAGE_ID."
  echo "  → a worker with the wrong image_id has EVERY settle REJECTED on-chain. Aborting."
  exit 1
fi
echo "OK: embedded guest image-id == $EXPECTED_IMAGE_ID (contract-bound)"

cp target/release/host worker/dist/host
cp -r "$HOME/.risc0" worker/dist/risc0-home
git checkout -- host/Cargo.toml Cargo.lock 2>/dev/null || true
