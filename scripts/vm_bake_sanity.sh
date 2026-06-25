#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# vm_bake_sanity.sh — run this FIRST on the bake VM, before cloning/building.
# The bake needs NO GPU (kernels are compiled for a fixed arch), so the only
# requirement is a REAL Docker daemon — which a RunPod/Vast container does NOT
# have. Aborts in seconds if Docker isn't real, so you don't pay build time to
# discover the same "no Docker" wall.
#
#   curl -fsSL https://raw.githubusercontent.com/DavidZapataOh/sincerin-stellar/sdd/s3-05/scripts/vm_bake_sanity.sh | bash
# ─────────────────────────────────────────────────────────────────────────────
set -uo pipefail
ok(){ printf '  \033[32m✓\033[0m %s\n' "$*"; }
die(){ printf '\n  \033[31m✗ %s\033[0m\n  → this machine does NOT qualify. Use a real x86 VM with Docker (Hetzner / DigitalOcean / Linode — instant, no GPU quota).\n' "$*" >&2; exit 1; }

echo "== bake sanity (real Docker daemon — no GPU needed) =="

command -v docker >/dev/null 2>&1 || die "no docker CLI. Install: curl -fsSL https://get.docker.com | sh"
if ! docker run --rm hello-world >/dev/null 2>&1; then
  # distinguish "no daemon" from "permission" (the common VM case → docker group)
  if docker version >/dev/null 2>&1; then
    die "docker needs sudo/group. Fix: sudo usermod -aG docker \$USER && newgrp docker  (then re-run)."
  fi
  die "no working Docker daemon (a RunPod/Vast container? those have none). Use a real VM."
fi
ok "real Docker daemon (hello-world ran)"

ARCH="$(uname -m)"
[ "$ARCH" = "x86_64" ] || die "arch is $ARCH, need x86_64 (the build image is x86)."
ok "x86_64"

FREE_GB="$(df -BG --output=avail / 2>/dev/null | tail -1 | tr -dc '0-9')"
[ "${FREE_GB:-0}" -ge 60 ] && ok "disk: ${FREE_GB}G free (≥60G)" \
  || echo "  (warning: only ${FREE_GB:-?}G free on / — the build wants ≥60G; risc0 + CUDA + guest image are big)"

echo ""
printf '  \033[32mVM QUALIFIES\033[0m — clone the branch and run: bash worker/build-host.sh\n'
