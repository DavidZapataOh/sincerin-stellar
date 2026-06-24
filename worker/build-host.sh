#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# build-host.sh — STAGE 1 of the production worker bake.
#
# Builds the PRODUCTION `host` binary with image_id **cbeab7aa…0d46** — the
# reproducible `r0.1.88.0` Docker guest build that the on-chain `settle_batch`
# contract BINDS. (NOT ROLLUP_LOCAL_GUEST — that gives a different, path-dependent
# id that the contract would REJECT. See methods/build.rs:41-48.)
#
# Runs on an x86 box WITH Docker. **No GPU needed to BUILD** (only nvcc, from the
# CUDA devel image, + Docker for the guest build). Builds INSIDE the same
# nvidia/cuda:12.4.1-devel base as the runtime image (worker/Dockerfile) so the
# binary is ABI-compatible, with the box's Docker socket mounted so the nested
# r0.1.88.0 guest build runs. Outputs worker/dist/{host,risc0-home}.
#
#   GIT_SHA=<sha> bash worker/build-host.sh     # GIT_SHA defaults to HEAD
#
# The native CUDA wrap + the guest cycles are identical to PARADA 1 (only the
# image_id build method differs) → prove time stays ~5 min (confirmed at Stop A).
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

GIT_SHA="${GIT_SHA:-$(git rev-parse HEAD)}"
ROOT="$(git rev-parse --show-toplevel)"
EXPECTED_IMAGE_ID="cbeab7aa6ce69944e10cca8c7ed94d15aae297f2580752f07a15c6cab6ba0d46"
mkdir -p "$ROOT/worker/dist"

echo "building production host (image_id $EXPECTED_IMAGE_ID) from $GIT_SHA ..."
docker run --rm \
  -e GIT_SHA="$GIT_SHA" \
  -e EXPECTED_IMAGE_ID="$EXPECTED_IMAGE_ID" \
  -v /var/run/docker.sock:/var/run/docker.sock \
  -v "$ROOT/worker/dist:/out" \
  nvidia/cuda:12.4.1-devel-ubuntu22.04 bash -euo pipefail -c '
    export DEBIAN_FRONTEND=noninteractive
    apt-get update -y && apt-get install -y --no-install-recommends \
      build-essential libssl-dev pkg-config curl git ca-certificates \
      protobuf-compiler clang libclang-dev llvm-dev docker.io
    export LIBCLANG_PATH=/usr/lib/llvm-14/lib
    ls "$LIBCLANG_PATH"/libclang*.so* >/dev/null || { echo "no libclang"; exit 1; }
    curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y; . "$HOME/.cargo/env"
    curl -L https://risczero.com/install | bash; export PATH="$HOME/.risc0/bin:$PATH"
    rzup install rust && rzup install cargo-risczero 3.0.5 && rzup install r0vm 3.0.5 && rzup install risc0-groth16
    git clone https://github.com/DavidZapataOh/sincerin-stellar.git /src && cd /src && git checkout "$GIT_SHA"
    git log --oneline -1
    sed -i "s#risc0-zkvm\( *\)= { workspace = true }#risc0-zkvm\1= { workspace = true, features = [\"cuda\"] }#" host/Cargo.toml
    grep -q "features = \[\"cuda\"\]" host/Cargo.toml
    cargo fetch
    cargo update -p sppark --precise 0.1.12
    cargo update -p blst --precise 0.3.15
    grep -A1 "name = \"sppark\"" Cargo.lock | grep -q "\"0.1.12\""
    # ── the production build: NO ROLLUP_LOCAL_GUEST → r0.1.88.0 Docker guest build
    #    → image_id cbeab7aa (the contract-bound id). Docker socket is mounted above.
    cargo build --release -p host
    # verify the EMBEDDED image_id is the contract-bound one BEFORE we ship it.
    # `host execute` prints "[execute]   guest image-id 0x<id>" — FAST, no GPU.
    IMG="$(./target/release/host execute --inputs golden/n8_inputs.json 2>&1 \
            | grep -i "guest image-id" | grep -oiE "[a-f0-9]{64}" | head -1 || true)"
    if [ "$IMG" != "$EXPECTED_IMAGE_ID" ]; then
      echo "FATAL: built host guest image-id is $IMG, NOT the contract-bound $EXPECTED_IMAGE_ID."
      echo "  → a worker with the wrong image_id has EVERY settle REJECTED on-chain. Aborting."
      echo "  → did ROLLUP_LOCAL_GUEST leak in, or did the r0.1.88.0 Docker guest build not run?"
      exit 1
    fi
    echo "OK: embedded guest image-id == $EXPECTED_IMAGE_ID (contract-bound)"
    cp target/release/host /out/host
    cp -r "$HOME/.risc0" /out/risc0-home    # groth16 proving artifacts for the runtime image
  '
echo ""
echo "STAGE 1 done → worker/dist/host (+ risc0-home). Next: build the runtime image:"
echo "  docker build -t <user>/sincerin-prover:n8 worker/   &&   docker push <user>/sincerin-prover:n8"
