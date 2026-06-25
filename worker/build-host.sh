#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# build-host.sh — STAGE 1 of the production worker bake.
#
# Builds the PRODUCTION `host` binary with image_id **cbeab7aa…0d46** — the
# reproducible `r0.1.88.0` Docker guest build the on-chain `settle_batch` BINDS.
#
# Runs on a CHEAP x86 CPU VM with Docker — **NO GPU needed**. risc0's only build-
# time GPU dependency is `-arch=native` (which needs a GPU to detect the arch);
# worker/build-in-container.sh forces a fixed arch (compute_86, PTX) via an nvcc
# wrapper, so the CUDA kernels compile on a GPU-less box — standard CUDA-CI
# practice. The GPU is touched ONLY at Stop A (the real prove).
#
# Needs a REAL Docker daemon (the r0.1.88.0 guest build) → a VM, NOT a RunPod/Vast
# container (those are containers with no daemon). Any cheap x86 VM works:
# Hetzner CPX41 / DigitalOcean 16 GB / Linode — instant, no GPU quota, you delete
# to stop. ~16 GB RAM, ≥80 GB disk.
#
#   git clone -b sdd/s3-05 … && cd … && bash worker/build-host.sh
#
# Outputs worker/dist/{host,risc0-home}, which worker/Dockerfile (Stage 2) COPYs.
#
# Docker-out-of-Docker: the build runs inside a cuda:12.4.1-devel container (nvcc +
# ABI match with the runtime image) with the host Docker socket AND the checkout
# mounted at the SAME path ($ROOT:$ROOT) — so risc0's nested r0.1.88.0 guest-build
# container can mount the source (the host daemon resolves the path on the host).
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
export EXPECTED_IMAGE_ID="cbeab7aa6ce69944e10cca8c7ed94d15aae297f2580752f07a15c6cab6ba0d46"
mkdir -p "$ROOT/worker/dist"

# Fail-fast: a real Docker daemon is required. A RunPod/Vast container has none.
docker run --rm hello-world >/dev/null 2>&1 \
  || { echo "ERROR: no working Docker daemon. Use a real VM (NOT a RunPod/Vast container). See worker/README.md."; exit 1; }

echo "Stage 1: building production host (no GPU; forced arch) from $(git -C "$ROOT" rev-parse --short HEAD)"
docker run --rm \
  -e EXPECTED_IMAGE_ID="$EXPECTED_IMAGE_ID" \
  -v /var/run/docker.sock:/var/run/docker.sock \
  -v "$ROOT:$ROOT" -w "$ROOT" \
  nvidia/cuda:12.4.1-devel-ubuntu22.04 bash "$ROOT/worker/build-in-container.sh"

echo ""
echo "Stage 1 done → worker/dist/{host,risc0-home}. Next: docker build worker/ && push."
