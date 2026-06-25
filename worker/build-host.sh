#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# build-host.sh — STAGE 1 of the production worker bake (Route B: real GPU VM).
#
# Builds the PRODUCTION `host` (image_id cbeab7aa, native CUDA kernels) AND runs a
# REAL validation prove on the VM's GPU before shipping — so a broken image can
# never reach GHCR or Stop A. PARADA-1 style: GPU present, -arch=native, no hack.
#
# Runs on a real x86 VM with a GPU + Docker (NOT a RunPod/Vast container — those
# have no Docker daemon). The GPU's compute capability is the arch the binary
# covers, so the serverless endpoint MUST be a SINGLE-ARCH category matching it
# (e.g. bake on an sm_86 GPU → endpoint category "A6000, A40 (48 GB)", both sm_86).
# Then the validation prove covers production EXACTLY — no cross-arch gamble.
#
#   git clone -b sdd/s3-05 … && cd … && bash worker/build-host.sh
#
# Outputs worker/dist/{host,risc0-home}. Same-path mount ($ROOT:$ROOT) lets risc0's
# nested r0.1.88.0 guest-build container resolve the source on the host daemon.
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
export EXPECTED_IMAGE_ID="cbeab7aa6ce69944e10cca8c7ed94d15aae297f2580752f07a15c6cab6ba0d46"
mkdir -p "$ROOT/worker/dist"

# Fail-fast: real Docker daemon + a GPU reachable via --gpus (the build compiles +
# validates CUDA inside a container). Run scripts/vm_bake_sanity.sh for the full check.
docker run --rm hello-world >/dev/null 2>&1 \
  || { echo "ERROR: no working Docker daemon. Use a real VM (NOT a RunPod/Vast container)."; exit 1; }
docker run --rm --gpus all nvidia/cuda:12.4.1-base-ubuntu22.04 nvidia-smi >/dev/null 2>&1 \
  || { echo "ERROR: no GPU via 'docker --gpus all' (install nvidia-container-toolkit). Run scripts/vm_bake_sanity.sh."; exit 1; }

echo "Stage 1: build + VALIDATE production host from $(git -C "$ROOT" rev-parse --short HEAD)"
docker run --rm --gpus all \
  -e EXPECTED_IMAGE_ID="$EXPECTED_IMAGE_ID" \
  -v /var/run/docker.sock:/var/run/docker.sock \
  -v "$ROOT:$ROOT" -w "$ROOT" \
  nvidia/cuda:12.4.1-devel-ubuntu22.04 bash "$ROOT/worker/build-in-container.sh"

echo ""
echo "Stage 1 done (build + validation prove PASSED) → worker/dist/{host,risc0-home}."
echo "Next: docker build worker/ && push. (Endpoint category MUST match the bake GPU arch.)"
