#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# vm_bake_sanity.sh — run this FIRST, in the first 30 seconds after the VM boots,
# BEFORE the heavy build. Aborts instantly if the machine doesn't qualify, so you
# never pay ~40 min of build to discover it lacks Docker or the GPU is wrong.
#
# Requires, verifiably:
#   (a) a REAL Docker daemon (NOT a RunPod-style pod — that has none),
#   (b) an Ampere/Ada GPU, compute cap 8.x (sm_80–89; NOT Blackwell, which
#       CUDA 12.4 / sppark can't build for; NOT pre-Ampere),
#   (c) Docker can SEE the GPU (`--gpus all` → nvidia-container-toolkit present),
#   (d) the driver supports CUDA ≥ 12.4 (the build image is cuda:12.4.1).
#
#   curl -fsSL https://raw.githubusercontent.com/DavidZapataOh/sincerin-stellar/sdd/s3-05/scripts/vm_bake_sanity.sh | bash
# ─────────────────────────────────────────────────────────────────────────────
set -uo pipefail
ok(){ printf '  \033[32m✓\033[0m %s\n' "$*"; }
die(){ printf '\n  \033[31m✗ %s\033[0m\n  → this VM does NOT qualify. Terminate it and pick a real GPU VM (e.g. AWS g5.xlarge, A10G sm_86, Ubuntu 22.04 DLAMI).\n' "$*" >&2; exit 1; }

echo "== VM bake sanity (Docker + Ampere/Ada GPU) =="

# (a) real Docker daemon
command -v docker >/dev/null 2>&1 || die "no docker CLI installed."
docker run --rm hello-world >/dev/null 2>&1 || die "no working Docker daemon (a pod/container without dockerd? use a real VM)."
ok "real Docker daemon (hello-world ran)"

# (b) GPU + compute capability
command -v nvidia-smi >/dev/null 2>&1 || die "no nvidia-smi — this VM has no NVIDIA GPU."
NAME="$(nvidia-smi --query-gpu=name --format=csv,noheader 2>/dev/null | head -1)"
CC="$(nvidia-smi --query-gpu=compute_cap --format=csv,noheader 2>/dev/null | head -1 | tr -d ' ')"
[ -n "$CC" ] || die "could not read compute_cap (driver too old?)."
MAJ="${CC%%.*}"
echo "  GPU: ${NAME:-?} (compute cap $CC = sm_${CC/./})"
[ "${MAJ:-0}" -ge 10 ] && die "Blackwell (CC $CC): risc0 3.0.5 + sppark 0.1.12 + CUDA 12.4 do NOT support it."
[ "${MAJ:-0}" -ne 8 ] && die "GPU is CC $CC, not Ampere/Ada (sm_80–89). Pick A10G/A5000/3090/L4/4090 (all sm_8x)."
ok "Ampere/Ada GPU (sm_${CC/./}) — matches the serverless 24GB category, builds with -arch=native"

# (c) Docker can see the GPU (the build compiles CUDA inside a container)
docker run --rm --gpus all nvidia/cuda:12.4.1-base-ubuntu22.04 nvidia-smi >/dev/null 2>&1 \
  || die "docker --gpus all can't reach the GPU — install/enable nvidia-container-toolkit."
ok "docker --gpus all sees the GPU (nvidia-container-toolkit OK)"

# (d) driver supports CUDA >= 12.4
DRV_CUDA="$(nvidia-smi 2>/dev/null | grep -oiE 'CUDA Version: [0-9]+\.[0-9]+' | grep -oE '[0-9]+\.[0-9]+' | head -1)"
if [ -n "$DRV_CUDA" ]; then
  awk -v v="$DRV_CUDA" 'BEGIN{split(v,a,"."); exit !(a[1]>12 || (a[1]==12 && a[2]>=4))}' \
    && ok "driver supports CUDA $DRV_CUDA (≥ 12.4)" \
    || die "driver max CUDA is $DRV_CUDA (< 12.4) — the cuda:12.4.1 build image needs a newer driver."
else
  echo "  (could not read driver CUDA version — proceed, but it must be ≥ 12.4)"
fi

echo ""
echo "  \033[32mVM QUALIFIES\033[0m — safe to clone + run: bash worker/build-host.sh"
