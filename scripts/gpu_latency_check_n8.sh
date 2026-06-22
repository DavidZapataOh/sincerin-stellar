#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# gpu_latency_check_n8.sh — PARADA 1 (s3/05 paso 1): cronometrar un prove REAL de
# N=8 (guest desplegado, depth 3, image_id cbeab7aa…) en una caja GPU x86 nativa.
#
# Objetivo: medir el wall-clock real del wrap STARK→Groth16 en x86 NATIVO + CUDA
# (sin emulación ARM, con RAM suficiente) para decidir si N_target=8 se mantiene
# como default de la demo en vivo. Target: pocos minutos (≲ ~10 min).
#
# CERO MOCKS: RISC0_DEV_MODE=0 forzado; se rechaza un seal dev-mode (ffffffff).
#
# Requisitos de la instancia (CONFIRMADOS): x86_64 · NVIDIA con nvcc (CUDA *devel*,
# no solo runtime) · **Docker funcionando** (nuestro pipeline lo usa para el build
# reproducible del guest y para el wrap Groth16) · ≥32 GiB RAM · ≥60 GiB disco.
# → Por el Docker, una VM GPU (Lambda Cloud / AWS g6 Deep Learning AMI) es lo seguro;
#   un pod-contenedor de RunPod NO trae daemon Docker y revienta en el build.
#
# Cómo correrlo:
#   curl -fsSL https://raw.githubusercontent.com/DavidZapataOh/sincerin-stellar/main/scripts/gpu_latency_check_n8.sh | bash
# El script clona el repo público, instala lo que falte, habilita la feature cuda,
# construye y cronometra el prove. La PRIMERA build (kernels CUDA + guest en Docker
# + pull de contenedores) tarda ~15–40 min ANTES del prove; eso NO se cronometra.
# Solo se mide el `host prove`. Pega TODA la salida (sobre todo el bloque RESULT).
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

REPO_URL="https://github.com/DavidZapataOh/sincerin-stellar.git"
WORKDIR="${WORKDIR:-$HOME/sincerin-latency}"
INPUTS="golden/n8_inputs.json"
OUTDIR="/tmp/n8_gpu_receipt"
R0_VERSION="3.0.5"
EXPECTED_CYCLES="122683392"   # executor padded cycles para N=8 (sanity)

log(){ printf '\n\033[1m== %s ==\033[0m\n' "$*"; }
die(){ printf '\n\033[31mERROR: %s\033[0m\n' "$*" >&2; exit 1; }

# 0) Sanity: arch x86 + GPU NVIDIA + Docker + RAM ────────────────────────────
log "0) Sanity del entorno"
ARCH="$(uname -m)"; echo "arch: $ARCH"
[ "$ARCH" = "x86_64" ] || die "No es x86_64 ($ARCH). El Groth16 prover SOLO corre en x86. Usa una caja GPU x86."
command -v nvidia-smi >/dev/null 2>&1 || die "nvidia-smi ausente — sin GPU/driver NVIDIA. Instala el driver o elige una instancia GPU."
nvidia-smi --query-gpu=name,memory.total,driver_version --format=csv,noheader || die "nvidia-smi falló"
command -v docker >/dev/null 2>&1 || die "docker no instalado."
docker info >/dev/null 2>&1 || die "daemon de docker apagado (sudo systemctl start docker; añade tu user al grupo docker)."
RAM_GB="$(free -g | awk '/Mem:/{print $2}')"; echo "RAM de sistema (GiB): $RAM_GB"
[ "${RAM_GB:-0}" -ge 24 ] || echo "AVISO: <24 GiB de RAM; el wrap Groth16 topó a 7.65 GiB en Mac — vigila OOM."
# Disco: risc0 toolchain + imágenes Docker (guest build + groth16 wrap) + build release
# + kernels CUDA pesan. Abortar ANTES de pagar el setup si no hay espacio.
free_gb(){ df -BG "$1" 2>/dev/null | awk 'NR==2{gsub(/G/,"",$4); print $4+0}'; }
HOME_FREE="$(free_gb "$(dirname "$WORKDIR")")"; VAR_FREE="$(free_gb /var/lib 2>/dev/null || free_gb /var)"
echo "disco libre — work($(dirname "$WORKDIR")): ${HOME_FREE:-?}G  /var(docker): ${VAR_FREE:-?}G"
[ "${HOME_FREE:-0}" -ge 50 ] || die "Menos de 50 GiB libres en el disco de trabajo (${HOME_FREE}G). risc0 + Docker + build release no caben. Levanta la instancia con ≥60 GiB y re-corre."
[ "${VAR_FREE:-0}" -ge 20 ] || echo "AVISO: /var con <20 GiB (las imágenes Docker viven ahí — guest build + groth16 prover). Vigila ENOSPC en el build."

# 1) Deps de build (apt, best-effort) ────────────────────────────────────────
log "1) Deps de sistema (build-essential, libssl-dev, …)"
if command -v apt-get >/dev/null 2>&1; then
  sudo apt-get update -y
  sudo apt-get install -y build-essential libssl-dev pkg-config curl git
fi

# 2) CUDA toolkit (nvcc) — la feature cuda compila kernels con nvcc ──────────
log "2) CUDA toolkit (nvcc)"
if [ -d /usr/local/cuda/bin ]; then
  export PATH=/usr/local/cuda/bin:$PATH
  export LD_LIBRARY_PATH=/usr/local/cuda/lib64:${LD_LIBRARY_PATH:-}
fi
command -v nvcc >/dev/null 2>&1 || die "nvcc (CUDA toolkit) ausente. Instálalo (p.ej. 'sudo apt-get install -y cuda-toolkit' en una imagen NVIDIA) y re-corre. La feature 'cuda' de risc0 lo necesita."
nvcc --version | tail -1

# 3) Rust stable ─────────────────────────────────────────────────────────────
log "3) Rust toolchain (stable)"
if ! command -v cargo >/dev/null 2>&1; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
fi
# shellcheck disable=SC1091
. "$HOME/.cargo/env" 2>/dev/null || true
cargo --version || die "cargo no disponible tras instalar rustup."

# 4) RISC Zero toolchain (rzup) — match exacto del crate risc0-zkvm =3.0.5 ────
log "4) RISC Zero toolchain (rzup → cargo-risczero/r0vm $R0_VERSION)"
if ! command -v rzup >/dev/null 2>&1; then
  curl -L https://risczero.com/install | bash || true
fi
export PATH="$HOME/.risc0/bin:$PATH"
rzup install cargo-risczero "$R0_VERSION" || echo "AVISO: rzup cargo-risczero falló (belt-and-suspenders; el guest se construye en Docker)."
rzup install r0vm "$R0_VERSION" || echo "AVISO: rzup r0vm falló (belt-and-suspenders)."

# 5) Clonar el repo público (workspace en la raíz) ───────────────────────────
log "5) Clonar repo"
if [ ! -d "$WORKDIR/.git" ]; then git clone "$REPO_URL" "$WORKDIR"; fi
cd "$WORKDIR"
echo "HEAD: $(git log --oneline -1)"
[ -f "$INPUTS" ] || die "$INPUTS no está en el clone."

# 6) Habilitar la feature 'cuda' de risc0-zkvm (solo en este clone) ──────────
log "6) Habilitar risc0-zkvm features=[\"cuda\"] en host/Cargo.toml"
if ! grep -q 'features = \["cuda"\]' host/Cargo.toml; then
  sed -i 's#risc0-zkvm\( *\)= { workspace = true }#risc0-zkvm\1= { workspace = true, features = ["cuda"] }#' host/Cargo.toml
fi
grep -n 'risc0-zkvm' host/Cargo.toml | head -1
grep -q 'features = \["cuda"\]' host/Cargo.toml || die "no pude habilitar la feature cuda en host/Cargo.toml (revisa el formato de la línea)."

# 7) Build host (kernels CUDA + guest reproducible en Docker) — NO cronometrado
log "7) cargo build --release -p host (primera build LENTA; pulls + kernels + guest Docker)"
RISC0_DEV_MODE=0 cargo build --release -p host

# 8) Sanity del executor (rápido, sin proving): cycles == canónico N=8 ────────
log "8) Sanity executor (cycles esperados: $EXPECTED_CYCLES)"
RISC0_DEV_MODE=0 ./target/release/host execute --inputs "$INPUTS" 2>&1 | tee /tmp/n8_exec.txt || true
grep -q "$EXPECTED_CYCLES" /tmp/n8_exec.txt && echo "cycles OK (coincide con el N=8 canónico)" \
  || echo "AVISO: no vi $EXPECTED_CYCLES en la salida del executor — revisa (no aborta)."

# 9) LA MEDICIÓN — prove REAL Groth16 de N=8, cronometrado ────────────────────
log "9) PROVE REAL N=8 (RISC0_DEV_MODE=0) — esto es lo que se mide"
rm -rf "$OUTDIR"; mkdir -p "$OUTDIR"
TIMELOG=/tmp/n8_prove_time.txt
START=$(date +%s)
if command -v /usr/bin/time >/dev/null 2>&1; then
  RISC0_DEV_MODE=0 /usr/bin/time -v ./target/release/host prove --inputs "$INPUTS" --out "$OUTDIR" 2> "$TIMELOG" || { cat "$TIMELOG" >&2; die "el prove falló (ver log arriba)."; }
  cat "$TIMELOG"
else
  RISC0_DEV_MODE=0 ./target/release/host prove --inputs "$INPUTS" --out "$OUTDIR" || die "el prove falló."
fi
END=$(date +%s); ELAPSED=$((END-START))
MAXRSS_KB="$(awk '/Maximum resident set size/{print $NF}' "$TIMELOG" 2>/dev/null || echo '')"

# 10) RESULTADO ──────────────────────────────────────────────────────────────
log "10) PARADA 1 — RESULT"
SEAL_HEAD="$(cut -c1-8 "$OUTDIR/seal.hex" 2>/dev/null || echo '??')"
[ "$SEAL_HEAD" = "ffffffff" ] && die "SEAL DEV-MODE (ffffffff) — NO es una prueba real. Abortado."
GPU_NAME="$(nvidia-smi --query-gpu=name --format=csv,noheader | head -1)"
printf 'GPU:                  %s\n' "$GPU_NAME"
printf 'N=8 prove wall-clock: %dm %ds   (%d segundos)\n' $((ELAPSED/60)) $((ELAPSED%60)) "$ELAPSED"
[ -n "$MAXRSS_KB" ] && printf 'pico de RAM (RSS):    %d MiB\n' $((MAXRSS_KB/1024))
printf 'seal selector:        %s   (≠ ffffffff ✓ prueba real)\n' "$SEAL_HEAD"
printf 'image_id:             %s\n' "$(cat "$OUTDIR/image_id.hex" 2>/dev/null)"
echo
if   [ "$ELAPSED" -le 600 ];  then echo "VEREDICTO: ≲10min → N_target=8 SE MANTIENE ✅";
elif [ "$ELAPSED" -le 1800 ]; then echo "VEREDICTO: ~10–30min → AVISAR a David: reconsiderar N_target default ⚠️";
else                              echo "VEREDICTO: >30min → demasiado para demo en vivo con N=8; reconsiderar default ⛔"; fi
echo "PARADA_1_DONE elapsed_seconds=$ELAPSED selector=$SEAL_HEAD image_id=$(cat "$OUTDIR/image_id.hex" 2>/dev/null)"
