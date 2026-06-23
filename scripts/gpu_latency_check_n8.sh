#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# gpu_latency_check_n8.sh — PARADA 1 (s3/05 paso 1): cronometrar un prove REAL de
# N=8 (guest depth 3, 122,683,392 cycles) en una caja GPU x86 nativa.
#
# Objetivo: medir el wall-clock real del wrap STARK→Groth16 en x86 NATIVO + CUDA
# (sin emulación ARM, con RAM suficiente) para decidir si N_target=8 se mantiene
# como default de la demo en vivo. Target: pocos minutos (≲ ~10 min).
#
# CERO MOCKS: RISC0_DEV_MODE=0 forzado; se rechaza un seal dev-mode (ffffffff).
#
# Requisitos de la instancia (CONFIRMADOS): x86_64 · NVIDIA con nvcc (CUDA *devel*,
# no solo runtime) · ≥32 GiB RAM · ≥50 GiB disco. **NO se necesita Docker daemon.**
# Verificado en risc0 v3.0.5 (groth16/src/prove/mod.rs): con la feature `cuda` el wrap
# Groth16 corre NATIVO (risc0-groth16/cuda → rapidsnark FFI, cero Docker). El guest se
# construye en LOCAL con ROLLUP_LOCAL_GUEST=1 (image_id distinto, pero cycles y tiempo
# de prove IDÉNTICOS). → Sirve una caja CUDA simple: **RunPod CUDA *devel***, Vast.ai,
# o Lambda. Este ES el camino del worker serverless de producción: host prove nativo.
#
# Cómo correrlo:
#   curl -fsSL https://raw.githubusercontent.com/DavidZapataOh/sincerin-stellar/main/scripts/gpu_latency_check_n8.sh | bash
# El script clona el repo público, instala lo que falte, habilita la feature cuda,
# FIJA sppark=0.1.12 (la versión que risc0 3.0.5 testeó; 0.1.15 da illegal memory
# access en runtime), construye el guest en LOCAL (sin Docker) y cronometra el prove
# NATIVO. La primera
# build (kernels CUDA + guest local + params groth16) tarda ~10–25 min ANTES del prove;
# eso NO se cronometra. Solo se mide el `host prove`. Pega TODA la salida (bloque RESULT).
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

# 0) Sanity: arch x86 + GPU NVIDIA + RAM + disco (SIN Docker) ─────────────────
log "0) Sanity del entorno"
ARCH="$(uname -m)"; echo "arch: $ARCH"
[ "$ARCH" = "x86_64" ] || die "No es x86_64 ($ARCH). El Groth16 prover SOLO corre en x86. Usa una caja GPU x86."
command -v nvidia-smi >/dev/null 2>&1 || die "nvidia-smi ausente — sin GPU/driver NVIDIA. Instala el driver o elige una instancia GPU."
nvidia-smi --query-gpu=name,memory.total,driver_version --format=csv,noheader || die "nvidia-smi falló"
RAM_GB="$(free -g | awk '/Mem:/{print $2}')"; echo "RAM de sistema (GiB): $RAM_GB"
[ "${RAM_GB:-0}" -ge 24 ] || echo "AVISO: <24 GiB de RAM; el wrap Groth16 topó a 7.65 GiB en Mac — vigila OOM."
# Disco: risc0 toolchain + cargo target + kernels CUDA + params groth16 pesan.
# Abortar ANTES de pagar el setup si no hay espacio. (Sin Docker → menos peso.)
free_gb(){ df -BG "$1" 2>/dev/null | awk 'NR==2{gsub(/G/,"",$4); print $4+0}'; }
HOME_FREE="$(free_gb "$(dirname "$WORKDIR")")"
echo "disco libre — work($(dirname "$WORKDIR")): ${HOME_FREE:-?}G"
[ "${HOME_FREE:-0}" -ge 50 ] || die "Menos de 50 GiB libres en el disco de trabajo (${HOME_FREE}G). risc0 toolchain + build release + kernels CUDA no caben. Levanta la instancia con ≥50 GiB y re-corre."

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

# 4) RISC Zero toolchain (rzup) — incluye el toolchain 'rust' para el guest LOCAL ─
log "4) RISC Zero toolchain (rzup → cargo-risczero/r0vm/rust $R0_VERSION)"
if ! command -v rzup >/dev/null 2>&1; then
  curl -L https://risczero.com/install | bash || true
fi
export PATH="$HOME/.risc0/bin:$PATH"
rzup install cargo-risczero "$R0_VERSION" || die "rzup cargo-risczero falló — necesario para el guest build local."
rzup install r0vm "$R0_VERSION" || die "rzup r0vm falló — necesario para proving."
rzup install rust || die "rzup rust falló — el toolchain risc0 'rust' construye el guest en LOCAL (ROLLUP_LOCAL_GUEST=1, sin Docker)."

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

# 6b) Fijar la pila CUDA a las versiones que risc0 3.0.5 TESTEÓ (su Cargo.lock). ─
# Nuestro lock no fija los deps cuda (opcionales) → la resolución fresca trae
# sppark 0.1.15, pero risc0 3.0.5 fue compilado/testeado con sppark 0.1.12. El
# drift (45+ commits de cambios internos de sppark que los kernels de risc0 3.0.5
# NO esperan) revienta en runtime con "illegal memory access" en sppark/gpu_t.cuh.
# CONFIRMADO: risc0 v3.0.5 Cargo.lock = sppark 0.1.12, cust 0.3.2, blst 0.3.15.
log "6b) Fijar sppark=0.1.12 (risc0 3.0.5 testeada; 0.1.15 → illegal memory access)"
cargo fetch >/dev/null 2>&1 || true        # resuelve la feature cuda → sppark entra al lock
cargo update -p sppark --precise 0.1.12 2>&1 | tail -3 || die "no pude fijar sppark 0.1.12 — pega el error."
grep -A1 'name = "sppark"' Cargo.lock | grep -q '"0.1.12"' || die "sppark no quedó en 0.1.12 en el lock."
cargo update -p blst --precise 0.3.15 2>&1 | tail -2 || echo "AVISO: no pude fijar blst 0.3.15 (insurance; el crash es sppark, no crítico)."
echo "pila cuda fijada:"; for c in sppark cust blst; do printf '  %-8s' "$c"; grep -A1 "name = \"$c\"" Cargo.lock | grep version | head -1 | tr -d ' '; done

# 7) Build host (kernels CUDA + guest LOCAL sin Docker) — NO cronometrado ─────
# ROLLUP_LOCAL_GUEST=1 → methods/build.rs compila el guest en el toolchain local
# (sin Docker). image_id distinto al cbeab7aa desplegado, pero cycles y tiempo de
# prove IDÉNTICOS (misma lógica). El wrap Groth16 va por risc0-groth16/cuda = nativo.
log "7) cargo build --release -p host (primera build LENTA; kernels CUDA + guest local)"
export ROLLUP_LOCAL_GUEST=1
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
printf 'prove path:           CUDA NATIVO (risc0-groth16/cuda, sin Docker) = camino de producción\n'
echo
if   [ "$ELAPSED" -le 600 ];  then echo "VEREDICTO: ≲10min → N_target=8 SE MANTIENE ✅";
elif [ "$ELAPSED" -le 1800 ]; then echo "VEREDICTO: ~10–30min → AVISAR a David: reconsiderar N_target default ⚠️";
else                              echo "VEREDICTO: >30min → demasiado para demo en vivo con N=8; reconsiderar default ⛔"; fi
echo "PARADA_1_DONE elapsed_seconds=$ELAPSED selector=$SEAL_HEAD image_id=$(cat "$OUTDIR/image_id.hex" 2>/dev/null)"
