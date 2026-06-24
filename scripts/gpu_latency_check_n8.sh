#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# gpu_latency_check_n8.sh — PARADA 1 (s3/05 paso 1): cronometrar un prove REAL de
# N=8 (guest depth 3, 122,683,392 cycles) en una caja GPU x86 nativa.
#
# Objetivo: medir el wall-clock real del wrap STARK→Groth16 en x86 NATIVO + CUDA
# para decidir si N_target=8 se mantiene como default. Target: ≲ ~10 min.
# CERO MOCKS: RISC0_DEV_MODE=0 forzado; se rechaza un seal dev-mode (ffffffff).
#
# Requisitos (CONFIRMADOS): x86_64 · NVIDIA con nvcc (CUDA *devel* 12.x, no runtime) ·
# compute capability sm_70..sm_90 (Ampere/Ada/Hopper; **Blackwell NO**) · ≥32 GiB RAM ·
# ≥50 GiB disco. **NO se necesita Docker daemon** (guest build local + wrap nativo cuda).
#
# Verificado en risc0 v3.0.5: el wrap Groth16 con feature `cuda` corre NATIVO
# (risc0-groth16/cuda → rapidsnark FFI, cero Docker). → caja CUDA simple: RunPod CUDA
# *devel*, Vast, Lambda. Es el camino del worker serverless de producción.
#
# Cómo correrlo:
#   curl -fsSL https://raw.githubusercontent.com/DavidZapataOh/sincerin-stellar/main/scripts/gpu_latency_check_n8.sh | bash
#
# Hace TODO solo: deps de sistema (protoc/clang/libclang/llvm) + LIBCLANG_PATH +
# toolchain risc0 (rust/cargo-risczero/r0vm/risc0-groth16) + feature cuda + pin
# sppark=0.1.12 (0.1.15 da illegal memory access) + guest build LOCAL (sin Docker) +
# reuso del clone (conserva caché de build). Solo cronometra el `host prove`.
# Pega TODA la salida (sobre todo el bloque RESULT).
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

REPO_URL="https://github.com/DavidZapataOh/sincerin-stellar.git"
WORKDIR="${WORKDIR:-$HOME/sincerin-latency}"
INPUTS="golden/n8_inputs.json"
OUTDIR="/tmp/n8_gpu_receipt"
R0_VERSION="3.0.5"
EXPECTED_CYCLES="122683392"        # executor padded cycles para N=8 (sanity)
export ROLLUP_LOCAL_GUEST=1        # GLOBAL: guest build local (sin Docker) para todo cargo
export RISC0_DEV_MODE=0            # GLOBAL: prueba REAL siempre

log(){ printf '\n\033[1m== %s ==\033[0m\n' "$*"; }
die(){ printf '\n\033[31mERROR: %s\033[0m\n' "$*" >&2; exit 1; }

# 0) GATE TEMPRANO (~2s): arch · GPU · compute-cap vs CUDA · RAM · disco ───────
#    Aborta YA si la GPU/CUDA es incompatible, ANTES del build pesado.
log "0) Gate temprano (arch · GPU · compute-cap vs CUDA · RAM · disco)"
ARCH="$(uname -m)"; echo "arch: $ARCH"
[ "$ARCH" = "x86_64" ] || die "No es x86_64 ($ARCH). El Groth16 prover SOLO corre en x86."
command -v nvidia-smi >/dev/null 2>&1 || die "nvidia-smi ausente — sin GPU/driver NVIDIA."
nvidia-smi --query-gpu=name,memory.total,driver_version --format=csv,noheader || die "nvidia-smi falló"

# compute capability — RECHAZAR Blackwell (sm_100/sm_120, CC≥10) y arch demasiado vieja.
CC="$(nvidia-smi --query-gpu=compute_cap --format=csv,noheader 2>/dev/null | head -1 | tr -d ' ')"
if [ -n "$CC" ]; then
  CC_MAJOR="${CC%%.*}"
  echo "compute capability: sm_${CC/./}  (CC $CC)"
  [ "${CC_MAJOR:-0}" -ge 10 ] && die "GPU Blackwell sm_${CC/./} (CC $CC): risc0 3.0.5 + sppark 0.1.12 + CUDA 12.x NO la soportan. Apaga y elige Ampere/Ada/Hopper: A100(8.0)·A10(8.6)·RTX4090/L4/A40(8.9)·H100(9.0)."
  [ "${CC_MAJOR:-0}" -lt 7 ]  && die "GPU demasiado vieja (CC $CC, < sm_70). Necesita Volta+ (sm_70). Elige Ampere/Ada (sm_80/86/89)."
else
  echo "AVISO: no pude leer compute_cap (driver viejo). NO uses Blackwell (sm_100/sm_120) — incompatible con CUDA 12.x."
fi

# nvcc + versión CUDA (la imagen debe ser CUDA *devel*; nvcc presente desde el arranque)
if [ -d /usr/local/cuda/bin ]; then
  export PATH=/usr/local/cuda/bin:$PATH
  export LD_LIBRARY_PATH=/usr/local/cuda/lib64:${LD_LIBRARY_PATH:-}
fi
command -v nvcc >/dev/null 2>&1 || die "nvcc ausente: usa una imagen CUDA *devel* (no runtime). p.ej. RunPod container image 'nvidia/cuda:12.4.1-devel-ubuntu22.04'."
CUDA_VER="$(nvcc --version 2>/dev/null | grep -oiE 'release [0-9]+\.[0-9]+' | grep -oE '[0-9]+\.[0-9]+' | head -1)"
CUDA_MAJOR="${CUDA_VER%%.*}"
echo "CUDA toolkit: ${CUDA_VER:-?}"
[ "${CUDA_MAJOR:-0}" = "12" ] || echo "AVISO: CUDA ${CUDA_VER:-?} — risc0 3.0.5 se testea con CUDA 12.x. Si falla, usa una imagen CUDA 12.x devel."

RAM_GB="$(free -g | awk '/Mem:/{print $2}')"; echo "RAM de sistema (GiB): $RAM_GB"
[ "${RAM_GB:-0}" -ge 24 ] || echo "AVISO: <24 GiB de RAM; el wrap topó a 7.65 GiB en Mac — vigila OOM."
free_gb(){ df -BG "$1" 2>/dev/null | awk 'NR==2{gsub(/G/,"",$4); print $4+0}'; }
HOME_FREE="$(free_gb "$(dirname "$WORKDIR")")"
echo "disco libre — work($(dirname "$WORKDIR")): ${HOME_FREE:-?}G"
[ "${HOME_FREE:-0}" -ge 50 ] || die "Menos de 50 GiB libres (${HOME_FREE}G). risc0 toolchain + build release + kernels CUDA no caben. Usa ≥50 GiB."

# 1) Deps de sistema — protoc + clang/libclang/llvm (bindgen) + LIBCLANG_PATH ──
log "1) Deps de sistema (build-essential, protobuf-compiler, clang/libclang/llvm)"
if command -v apt-get >/dev/null 2>&1; then
  sudo apt-get update -y
  sudo apt-get install -y build-essential libssl-dev pkg-config curl git \
    protobuf-compiler clang libclang-dev llvm-dev
fi
command -v protoc >/dev/null 2>&1 || die "protoc ausente (apt install protobuf-compiler) — un dep de risc0 lo necesita en build."
# LIBCLANG_PATH — bindgen lo necesita; auto-detectar (NO hardcodear la versión de llvm)
if [ -z "${LIBCLANG_PATH:-}" ]; then
  CLANG_SO="$(find /usr/lib /usr/lib64 /usr/local/lib \( -name 'libclang.so*' -o -name 'libclang-*.so*' \) 2>/dev/null | head -1)"
  [ -n "$CLANG_SO" ] && export LIBCLANG_PATH="$(dirname "$CLANG_SO")"
fi
[ -n "${LIBCLANG_PATH:-}" ] && echo "LIBCLANG_PATH=$LIBCLANG_PATH" \
  || echo "AVISO: no encontré libclang.so — si bindgen falla, apt install libclang-dev y re-corre."

# 2) Rust stable ──────────────────────────────────────────────────────────────
log "2) Rust toolchain (stable)"
command -v cargo >/dev/null 2>&1 || curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
# shellcheck disable=SC1091
. "$HOME/.cargo/env" 2>/dev/null || true
cargo --version || die "cargo no disponible tras instalar rustup."

# 3) RISC Zero toolchain (rzup): rust (guest local) + cargo-risczero + r0vm + ───
#    risc0-groth16 (el componente del wrap; sin él, el Groth16 nativo no encuentra
#    su circuito/proving key).
log "3) RISC Zero toolchain (rzup: rust/cargo-risczero/r0vm/risc0-groth16 $R0_VERSION)"
command -v rzup >/dev/null 2>&1 || { curl -L https://risczero.com/install | bash || true; }
export PATH="$HOME/.risc0/bin:$PATH"
rzup install rust                      || die "rzup rust falló — necesario para el guest build LOCAL (sin Docker)."
rzup install cargo-risczero "$R0_VERSION" || die "rzup cargo-risczero falló."
rzup install r0vm "$R0_VERSION"        || die "rzup r0vm falló."
rzup install risc0-groth16             || die "rzup risc0-groth16 falló — el wrap Groth16 nativo lo necesita."

# 4) Clonar O REUSAR el clone (conserva el caché de build en target/) ──────────
log "4) Repo (reusa el clone si existe → conserva caché de build)"
if [ ! -d "$WORKDIR/.git" ]; then
  echo "clonando fresco en $WORKDIR"
  git clone "$REPO_URL" "$WORKDIR"
  cd "$WORKDIR"
else
  echo "REUSANDO $WORKDIR (target/ se conserva; actualizo la fuente a origin/main)"
  cd "$WORKDIR"
  git fetch origin -q && git reset -q --hard origin/main   # target/ es untracked → sobrevive
fi
echo "HEAD: $(git log --oneline -1)"
[ -f "$INPUTS" ] || die "$INPUTS no está en el clone."

# 5) Habilitar la feature 'cuda' de risc0-zkvm ────────────────────────────────
log "5) Habilitar risc0-zkvm features=[\"cuda\"] en host/Cargo.toml"
grep -q 'features = \["cuda"\]' host/Cargo.toml \
  || sed -i 's#risc0-zkvm\( *\)= { workspace = true }#risc0-zkvm\1= { workspace = true, features = ["cuda"] }#' host/Cargo.toml
grep -q 'features = \["cuda"\]' host/Cargo.toml || die "no pude habilitar la feature cuda en host/Cargo.toml."
grep -n 'risc0-zkvm ' host/Cargo.toml | head -1

# 6) Fijar la pila CUDA a lo que risc0 3.0.5 TESTEÓ (su Cargo.lock) ────────────
#    La resolución fresca trae sppark 0.1.15, pero risc0 3.0.5 se testeó con 0.1.12.
#    El drift → "illegal memory access" en sppark/gpu_t.cuh en runtime.
#    El ÚNICO pin necesario es sppark 0.1.12. NO se pinea blst: sppark 0.1.12 no
#    depende de blst (deps = cc + which), y c-kzg (vía alloy ← risc0-ethereum-
#    contracts, para encode_seal) exige blst ^0.3.16 → forzar 0.3.15 rompería la
#    resolución. Se deja blst en el 0.3.16 que resuelve todo el proyecto.
log "6) Fijar sppark=0.1.12 (risc0 3.0.5 testeada; 0.1.15 → illegal memory access)"
cargo fetch >/dev/null 2>&1 || true        # resuelve la feature cuda → sppark entra al lock
cargo update -p sppark --precise 0.1.12 2>&1 | tail -3 || die "no pude fijar sppark 0.1.12 — pega el error."
grep -A1 'name = "sppark"' Cargo.lock | grep -q '"0.1.12"' || die "sppark no quedó en 0.1.12 en el lock."
echo "pila cuda fijada:"; for c in sppark cust blst; do printf '  %-8s' "$c"; grep -A1 "name = \"$c\"" Cargo.lock | grep version | head -1 | tr -d ' '; done

# 7) Build host (kernels CUDA + guest LOCAL sin Docker) — NO cronometrado ─────
log "7) cargo build --release -p host (incremental si reusaste el clone)"
cargo build --release -p host

# 8) Sanity del executor (rápido, sin proving): cycles == canónico N=8 ─────────
log "8) Sanity executor (cycles esperados: $EXPECTED_CYCLES)"
./target/release/host execute --inputs "$INPUTS" 2>&1 | tee /tmp/n8_exec.txt || true
grep -q "$EXPECTED_CYCLES" /tmp/n8_exec.txt && echo "cycles OK (coincide con el N=8 canónico)" \
  || echo "AVISO: no vi $EXPECTED_CYCLES en la salida del executor — revisa (no aborta)."

# 9) LA MEDICIÓN — prove REAL Groth16 de N=8 (nativo cuda), cronometrado ───────
log "9) PROVE REAL N=8 (RISC0_DEV_MODE=0) — esto es lo que se mide"
rm -rf "$OUTDIR"; mkdir -p "$OUTDIR"
TIMELOG=/tmp/n8_prove_time.txt
START=$(date +%s)
if command -v /usr/bin/time >/dev/null 2>&1; then
  /usr/bin/time -v ./target/release/host prove --inputs "$INPUTS" --out "$OUTDIR" 2> "$TIMELOG" || { cat "$TIMELOG" >&2; die "el prove falló (ver log arriba)."; }
  cat "$TIMELOG"
else
  ./target/release/host prove --inputs "$INPUTS" --out "$OUTDIR" || die "el prove falló."
fi
END=$(date +%s); ELAPSED=$((END-START))
MAXRSS_KB="$(awk '/Maximum resident set size/{print $NF}' "$TIMELOG" 2>/dev/null || echo '')"

# 10) RESULTADO ───────────────────────────────────────────────────────────────
log "10) PARADA 1 — RESULT"
SEAL_HEAD="$(cut -c1-8 "$OUTDIR/seal.hex" 2>/dev/null || echo '??')"
[ "$SEAL_HEAD" = "ffffffff" ] && die "SEAL DEV-MODE (ffffffff) — NO es una prueba real. Abortado."
printf 'GPU:                  %s  (CC %s, CUDA %s)\n' "$(nvidia-smi --query-gpu=name --format=csv,noheader | head -1)" "${CC:-?}" "${CUDA_VER:-?}"
printf 'N=8 prove wall-clock: %dm %ds   (%d segundos)\n' $((ELAPSED/60)) $((ELAPSED%60)) "$ELAPSED"
[ -n "$MAXRSS_KB" ] && printf 'pico de RAM (RSS):    %d MiB\n' $((MAXRSS_KB/1024))
printf 'seal selector:        %s   (≠ ffffffff ✓ prueba real)\n' "$SEAL_HEAD"
printf 'image_id:             %s\n' "$(cat "$OUTDIR/image_id.hex" 2>/dev/null)"
printf 'prove path:           CUDA NATIVO (risc0-groth16/cuda, sin Docker) = camino de producción\n'
echo
if   [ "$ELAPSED" -le 600 ];  then echo "VEREDICTO: ≲10min → N_target=8 SE MANTIENE ✅";
elif [ "$ELAPSED" -le 1800 ]; then echo "VEREDICTO: ~10–30min → AVISAR a David: reconsiderar N_target default ⚠️";
else                              echo "VEREDICTO: >30min → demasiado para demo en vivo con N=8; reconsiderar default ⛔"; fi
echo "PARADA_1_DONE elapsed_seconds=$ELAPSED selector=$SEAL_HEAD cc=${CC:-?} cuda=${CUDA_VER:-?}"
