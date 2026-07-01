#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# vm_bake_sanity.sh — run FIRST on the bake VM, before cloning/building. Aborts in
# seconds if the machine doesn't qualify, so you don't pay build time to discover
# it. Route B (real GPU VM): the bake builds AND runs a real validation prove, so
# it needs a GPU + a real Docker daemon.
#
# Checks: (a) real Docker daemon (a RunPod/Vast container has NONE), (b) x86_64,
# (c) an sm_86 GPU — CC 8.6 EXACTLY (A4000/A5000/A6000/A40/A10 or RTX 3090); NOT A100
# (sm_80), NOT L4/4090 (sm_89), NOT Blackwell, (d) docker --gpus all reaches it,
# (e) driver CUDA ≥ 12.4. The deployed endpoint is LOCKED to "A6000, A40 (48 GB)" = sm_86,
# and native SASS is per-CC, so the bake GPU MUST be CC 8.6 or production won't run.
#
#   curl -fsSL https://raw.githubusercontent.com/DavidZapataOh/sincerin-stellar/sdd/s3-05/scripts/vm_bake_sanity.sh | bash
# ─────────────────────────────────────────────────────────────────────────────
set -uo pipefail
ok(){ printf '  \033[32m✓\033[0m %s\n' "$*"; }
die(){ printf '\n  \033[31m✗ %s\033[0m\n  → this machine does NOT qualify. Use a real GPU VM with root + Docker (BitLaunch A40 sm_86 via crypto, or a TensorDock KVM VM: real VMs, root, self-serve) — NOT a RunPod/Vast container.\n' "$*" >&2; exit 1; }

echo "== bake sanity (real GPU VM: Docker + Ampere/Ada GPU) =="

command -v docker >/dev/null 2>&1 || die "no docker CLI. Install: curl -fsSL https://get.docker.com | sh"
if ! docker run --rm hello-world >/dev/null 2>&1; then
  docker version >/dev/null 2>&1 \
    && die "docker needs sudo/group: sudo usermod -aG docker \$USER && newgrp docker  (then re-run)." \
    || die "no working Docker daemon (a RunPod/Vast container? those have none). Use a real VM."
fi
ok "real Docker daemon"

[ "$(uname -m)" = "x86_64" ] || die "not x86_64 ($(uname -m)); the build image is x86."
ok "x86_64"

command -v nvidia-smi >/dev/null 2>&1 || die "no nvidia-smi — no NVIDIA GPU on this VM."
NAME="$(nvidia-smi --query-gpu=name --format=csv,noheader 2>/dev/null | head -1)"
CC="$(nvidia-smi --query-gpu=compute_cap --format=csv,noheader 2>/dev/null | head -1 | tr -d ' ')"
[ -n "$CC" ] || die "could not read compute_cap (driver too old?)."
MAJ="${CC%%.*}"
echo "  GPU: ${NAME:-?} (compute cap $CC = sm_${CC/./})"
[ "${MAJ:-0}" -ge 10 ] && die "Blackwell (CC $CC): risc0 3.0.5 + sppark 0.1.12 + CUDA 12.4 do NOT support it."
[ "$CC" != "8.6" ] && die "GPU is CC $CC (sm_${CC/./}), but the endpoint is LOCKED to sm_86 (A6000/A40). SASS is per-CC: an A100 (sm_80) or L4/4090 (sm_89) build would NOT run on the sm_86 endpoint. Bake on a CC 8.6 card: A4000/A5000/A6000/A40/A10 or RTX 3090."
ok "sm_86 GPU (CC 8.6) — matches the A6000/A40 endpoint exactly"

docker run --rm --gpus all nvidia/cuda:12.4.1-base-ubuntu22.04 nvidia-smi >/dev/null 2>&1 \
  || die "docker --gpus all can't reach the GPU — install/enable nvidia-container-toolkit."
ok "docker --gpus all reaches the GPU (nvidia-container-toolkit OK)"

DRV_CUDA="$(nvidia-smi 2>/dev/null | grep -oiE 'CUDA Version: [0-9]+\.[0-9]+' | grep -oE '[0-9]+\.[0-9]+' | head -1)"
[ -n "$DRV_CUDA" ] && { awk -v v="$DRV_CUDA" 'BEGIN{split(v,a,"."); exit !(a[1]>12||(a[1]==12&&a[2]>=4))}' \
  && ok "driver CUDA $DRV_CUDA (≥ 12.4)" || die "driver CUDA $DRV_CUDA < 12.4 — the cuda:12.4.1 image needs a newer driver."; } \
  || echo "  (could not read driver CUDA — must be ≥ 12.4)"

echo ""
printf '  \033[32mVM QUALIFIES\033[0m (sm_86). Endpoint category: "A6000, A40 (48 GB)" — same arch, covered exactly.\n'
printf '  Then: git clone -b sdd/s3-05 … && bash worker/build-host.sh\n'
