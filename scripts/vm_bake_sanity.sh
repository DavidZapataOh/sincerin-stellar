#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# vm_bake_sanity.sh — run FIRST on the bake VM, before cloning/building. Aborts in
# seconds if the machine doesn't qualify, so you don't pay build time to discover
# it. Route B (real GPU VM): the bake builds AND runs a real validation prove, so
# it needs a GPU + a real Docker daemon.
#
# Checks: (a) real Docker daemon (a RunPod/Vast container has NONE), (b) x86_64,
# (c) Ampere/Ada GPU (sm_8x; NOT Blackwell), (d) docker --gpus all reaches it,
# (e) driver CUDA ≥ 12.4. Prints the GPU arch — your serverless endpoint MUST be a
# SINGLE-ARCH category matching it (sm_86 → "A6000, A40 (48 GB)"; sm_89 → a 4090/L40
# category), so the validation prove covers production exactly.
#
#   curl -fsSL https://raw.githubusercontent.com/DavidZapataOh/sincerin-stellar/sdd/s3-05/scripts/vm_bake_sanity.sh | bash
# ─────────────────────────────────────────────────────────────────────────────
set -uo pipefail
ok(){ printf '  \033[32m✓\033[0m %s\n' "$*"; }
die(){ printf '\n  \033[31m✗ %s\033[0m\n  → this machine does NOT qualify. Use a real GPU VM with root + Docker (TensorDock or Trooper.AI: KVM VMs, root, self-serve sm_86) — NOT a RunPod/Vast container.\n' "$*" >&2; exit 1; }

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
[ "${MAJ:-0}" -ne 8 ] && die "GPU is CC $CC, not Ampere/Ada (sm_80–89). Pick A5000/A4000/3090 (sm_86) or 4090/L4 (sm_89)."
ok "Ampere/Ada GPU (sm_${CC/./})"

docker run --rm --gpus all nvidia/cuda:12.4.1-base-ubuntu22.04 nvidia-smi >/dev/null 2>&1 \
  || die "docker --gpus all can't reach the GPU — install/enable nvidia-container-toolkit."
ok "docker --gpus all reaches the GPU (nvidia-container-toolkit OK)"

DRV_CUDA="$(nvidia-smi 2>/dev/null | grep -oiE 'CUDA Version: [0-9]+\.[0-9]+' | grep -oE '[0-9]+\.[0-9]+' | head -1)"
[ -n "$DRV_CUDA" ] && { awk -v v="$DRV_CUDA" 'BEGIN{split(v,a,"."); exit !(a[1]>12||(a[1]==12&&a[2]>=4))}' \
  && ok "driver CUDA $DRV_CUDA (≥ 12.4)" || die "driver CUDA $DRV_CUDA < 12.4 — the cuda:12.4.1 image needs a newer driver."; } \
  || echo "  (could not read driver CUDA — must be ≥ 12.4)"

echo ""
printf '  \033[32mVM QUALIFIES\033[0m (sm_%s). Set the serverless endpoint to a SINGLE-ARCH category for sm_%s.\n' "${CC/./}" "${CC/./}"
printf '  Then: git clone -b sdd/s3-05 … && bash worker/build-host.sh\n'
