#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# build-host.sh — STAGE 1 of the production worker bake.
#
# Builds the PRODUCTION `host` binary with image_id **cbeab7aa…0d46** — the
# reproducible `r0.1.88.0` Docker guest build that the on-chain `settle_batch`
# contract BINDS. (NOT ROLLUP_LOCAL_GUEST, which gives a different, path-dependent
# id the contract would REJECT — see methods/build.rs:41-48.)
#
# Runs on an x86 host WITH a real Docker daemon (a GH Actions runner, or a VM —
# NOT a RunPod pod, which has no daemon). **No GPU needed to BUILD** (only nvcc,
# from the CUDA devel image; Docker for the guest build; `host execute` is the
# CPU executor). The native CUDA wrap + the guest cycles are identical to PARADA 1
# (only the image_id build method differs) → prove time stays ~5 min.
#
#   bash worker/build-host.sh          # builds the CURRENTLY CHECKED-OUT commit
#
# The repo must already be checked out at the target commit (CI's actions/checkout,
# or your `git clone -b sdd/s3-05`). Outputs worker/dist/{host,risc0-home}, which
# worker/Dockerfile (Stage 2) COPYs into the slim runtime image.
#
# Docker-out-of-Docker note: the build runs inside a cuda:12.4.1-devel container
# (nvcc + ABI match with the runtime image) with the host's Docker socket AND the
# checkout mounted at the SAME path ($ROOT:$ROOT). That same-path mount is what
# lets risc0's nested `r0.1.88.0` guest-build container mount the source — the host
# daemon resolves the path on the host, where $ROOT exists.
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
EXPECTED_IMAGE_ID="cbeab7aa6ce69944e10cca8c7ed94d15aae297f2580752f07a15c6cab6ba0d46"
mkdir -p "$ROOT/worker/dist"
echo "Stage 1: building production host from $(git -C "$ROOT" rev-parse --short HEAD) (expect image_id $EXPECTED_IMAGE_ID)"

docker run --rm \
  -e EXPECTED_IMAGE_ID="$EXPECTED_IMAGE_ID" \
  -v /var/run/docker.sock:/var/run/docker.sock \
  -v "$ROOT:$ROOT" -w "$ROOT" \
  nvidia/cuda:12.4.1-devel-ubuntu22.04 bash -euo pipefail -c '
    export DEBIAN_FRONTEND=noninteractive
    apt-get update -y && apt-get install -y --no-install-recommends \
      build-essential libssl-dev pkg-config curl git ca-certificates \
      protobuf-compiler clang libclang-dev llvm-dev docker.io
    export LIBCLANG_PATH=/usr/lib/llvm-14/lib
    ls "$LIBCLANG_PATH"/libclang*.so* >/dev/null
    curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y; . "$HOME/.cargo/env"
    curl -L https://risczero.com/install | bash; export PATH="$HOME/.risc0/bin:$PATH"
    rzup install rust && rzup install cargo-risczero 3.0.5 && rzup install r0vm 3.0.5 && rzup install risc0-groth16
    git config --global --add safe.directory "$PWD" || true
    # enable the risc0-zkvm cuda feature + pin the CUDA stack (the guest-builder tag
    # r0.1.88.0 and risc0 =3.0.5 are already pinned in build.rs / Cargo.toml).
    sed -i "s#risc0-zkvm\( *\)= { workspace = true }#risc0-zkvm\1= { workspace = true, features = [\"cuda\"] }#" host/Cargo.toml
    grep -q "features = \[\"cuda\"\]" host/Cargo.toml
    cargo fetch
    cargo update -p sppark --precise 0.1.12
    cargo update -p blst --precise 0.3.15
    grep -A1 "name = \"sppark\"" Cargo.lock | grep -q "\"0.1.12\""
    # ── the production build: NO ROLLUP_LOCAL_GUEST → r0.1.88.0 Docker guest build
    #    (the nested docker run resolves $PWD on the host via the same-path mount).
    cargo build --release -p host
    # verify the EMBEDDED image_id == the contract-bound one with `host execute`
    # (the CPU executor — FAST, NO GPU). Abort BEFORE shipping if it differs.
    IMG="$(./target/release/host execute --inputs golden/n8_inputs.json 2>&1 \
            | grep -i "guest image-id" | grep -oiE "[a-f0-9]{64}" | head -1 || true)"
    if [ "$IMG" != "$EXPECTED_IMAGE_ID" ]; then
      echo "FATAL: built guest image-id is $IMG, NOT the contract-bound $EXPECTED_IMAGE_ID."
      echo "  → a worker with the wrong image_id has EVERY settle REJECTED on-chain. Aborting."
      echo "  → did ROLLUP_LOCAL_GUEST leak in, or did the r0.1.88.0 Docker guest build not run?"
      exit 1
    fi
    echo "OK: embedded guest image-id == $EXPECTED_IMAGE_ID (contract-bound)"
    cp target/release/host worker/dist/host
    cp -r "$HOME/.risc0" worker/dist/risc0-home
    git checkout -- host/Cargo.toml Cargo.lock 2>/dev/null || true   # leave the checkout clean
  '
echo ""
echo "Stage 1 done → worker/dist/{host,risc0-home}. Next: docker build worker/ && push."
